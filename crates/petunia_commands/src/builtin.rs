use std::collections::BTreeMap;

use petunia_schema::{
    CommandSpec, Cost, FailureSpec, ParamKind, ParamSpec, QuerySpec, Reversibility, Schema, Status,
};
use petunia_world::{EntityHandle, PersistentId, Value, World};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value as Json, json};

use crate::dispatcher::{ApplyFn, CommandRecord, Outcome};
use crate::error::CommandError;

/// Schema with the built-in commands and queries (no dispatcher needed).
pub fn builtin_schema() -> Schema {
    let mut schema = Schema::new();
    schema.commands = commands().into_iter().map(|(spec, _)| spec).collect();
    schema.queries = query_specs();
    schema
}

pub fn builtin_query_specs() -> Vec<QuerySpec> {
    query_specs()
}

pub(crate) fn commands() -> Vec<(CommandSpec, ApplyFn)> {
    vec![
        (create_spec(), create as ApplyFn),
        (remove_spec(), remove as ApplyFn),
        (set_spec(), set as ApplyFn),
        (row_create_spec(), row_create as ApplyFn),
        (row_set_spec(), row_set as ApplyFn),
        (row_remove_spec(), row_remove as ApplyFn),
    ]
    .into_iter()
    .chain(crate::terrain::commands())
    .chain(crate::spline::commands())
    .collect()
}

pub(crate) fn query_specs() -> Vec<QuerySpec> {
    let mut specs = vec![
        QuerySpec {
            id: "world.get".to_string(),
            summary: "Lê um valor, ou a linha inteira, de uma entidade.".to_string(),
            params: vec![
                ParamSpec::required("entity", ParamKind::Entity, "Id persistente da entidade."),
                ParamSpec::required("table", ParamKind::Text, "Nome da tabela."),
                ParamSpec::optional(
                    "column",
                    ParamKind::Text,
                    "Coluna; sem ela devolve a linha.",
                ),
            ],
            returns: ParamKind::Any,
            failures: vec![
                FailureSpec::new(
                    "WORLD_UNKNOWN_ENTITY",
                    "Use `world.find` para listar as entidades.",
                ),
                FailureSpec::new(
                    "WORLD_ROW_MISSING",
                    "A entidade não tem essa tabela; use `row.create`.",
                ),
            ],
            permission: "world.read".to_string(),
            status: Status::Provisional,
            example: r#"{"entity": 3, "table": "Transform", "column": "position"}"#.to_string(),
        },
        QuerySpec {
            id: "world.find".to_string(),
            summary: "Lista os ids das entidades com uma tabela e/ou uma tag.".to_string(),
            params: vec![
                ParamSpec::optional("table", ParamKind::Text, "Só entidades com esta tabela."),
                ParamSpec::optional("tag", ParamKind::Text, "Só entidades com esta tag."),
            ],
            returns: ParamKind::Any,
            failures: vec![FailureSpec::new(
                "WORLD_UNKNOWN_TABLE",
                "Confira o nome da tabela (as tabelas começam com maiúscula).",
            )],
            permission: "world.read".to_string(),
            status: Status::Provisional,
            example: r#"{"table": "Brush"}"#.to_string(),
        },
    ];
    specs.extend(crate::terrain::queries());
    specs
}

// ---- payloads -------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePayload {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    rows: BTreeMap<String, Map<String, Json>>,
    persistent_id: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntityPayload {
    entity: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetPayload {
    entity: u64,
    table: String,
    column: String,
    value: Json,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RowCreatePayload {
    entity: u64,
    table: String,
    #[serde(default)]
    values: Map<String, Json>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RowRemovePayload {
    entity: u64,
    table: String,
}

fn parse<T: DeserializeOwned>(command: &str, payload: &Json) -> Result<T, CommandError> {
    let source = if payload.is_null() {
        json!({})
    } else {
        payload.clone()
    };
    serde_json::from_value(source).map_err(|error| CommandError::InvalidPayload {
        command: command.to_string(),
        message: error.to_string(),
    })
}

pub(crate) fn resolve(world: &World, entity: u64) -> Result<EntityHandle, CommandError> {
    Ok(world.handle_of(PersistentId(entity))?)
}

// ---- world.create ---------------------------------------------------------------------------

fn create_spec() -> CommandSpec {
    CommandSpec {
        id: "world.create".to_string(),
        summary: "Cria uma entidade com tags e linhas de tabelas.".to_string(),
        params: vec![
            ParamSpec::optional("tags", ParamKind::TextList, "Tags da entidade."),
            ParamSpec::optional(
                "rows",
                ParamKind::Rows,
                "Linhas iniciais: {Tabela: {coluna: valor}}.",
            ),
            ParamSpec::optional(
                "persistent_id",
                ParamKind::Entity,
                "Id a reaproveitar (desfazer).",
            ),
        ],
        returns: Some(ParamKind::Entity),
        failures: vec![
            FailureSpec::new("WORLD_UNKNOWN_TABLE", "Confira o nome da tabela."),
            FailureSpec::new(
                "WORLD_INVALID_VALUE",
                "Corrija o valor indicado na mensagem.",
            ),
            FailureSpec::new(
                "WORLD_LIMIT_EXCEEDED",
                "Remova entidades antes de criar outras.",
            ),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example:
            r#"{"tags": ["Crate"], "rows": {"Transform": {"position": {"x": 1, "y": 0, "z": 2}}}}"#
                .to_string(),
    }
}

fn create(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: CreatePayload = parse("world.create", payload)?;

    // Validate everything before touching the world.
    let mut rows = Vec::with_capacity(request.rows.len());
    for (table, values) in &request.rows {
        rows.push((table.clone(), world.parse_row(table, values)?));
    }
    for tag in &request.tags {
        world.validate_tag(tag)?;
    }
    if request.tags.len() > world.limits().max_tags_per_entity {
        return Err(petunia_world::WorldError::LimitExceeded("tags per entity").into());
    }

    let handle = match request.persistent_id {
        Some(id) => world.spawn_with_persistent_id(PersistentId(id))?,
        None => world.spawn()?,
    };
    let filled = fill_new_entity(world, handle, &request.tags, rows);
    if let Err(error) = filled {
        // Roll back so a failed command leaves no half-built entity behind.
        let _ = world.despawn(handle);
        return Err(error);
    }
    let id = world.persistent_id(handle)?.0;
    Ok(Outcome {
        result: json!({ "entity": id }),
        inverse: Some(CommandRecord::new("world.remove", json!({ "entity": id }))),
    })
}

fn fill_new_entity(
    world: &mut World,
    handle: EntityHandle,
    tags: &[String],
    rows: Vec<(String, Vec<Value>)>,
) -> Result<(), CommandError> {
    for tag in tags {
        world.add_tag(handle, tag)?;
    }
    for (table, values) in rows {
        world.insert_row_values(handle, &table, values)?;
    }
    Ok(())
}

// ---- world.remove ---------------------------------------------------------------------------

fn remove_spec() -> CommandSpec {
    CommandSpec {
        id: "world.remove".to_string(),
        summary: "Remove uma entidade e todas as suas linhas.".to_string(),
        params: vec![ParamSpec::required(
            "entity",
            ParamKind::Entity,
            "Id persistente da entidade.",
        )],
        returns: None,
        failures: vec![FailureSpec::new(
            "WORLD_UNKNOWN_ENTITY",
            "Use `world.find` para listar as entidades.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 3}"#.to_string(),
    }
}

fn remove(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: EntityPayload = parse("world.remove", payload)?;
    let handle = resolve(world, request.entity)?;
    let snapshot = world.despawn(handle)?;

    let mut rows = Map::new();
    for (table, values) in &snapshot.rows {
        rows.insert(
            table.clone(),
            Json::Object(world.row_to_json(table, values)?),
        );
    }
    let inverse = CommandRecord::new(
        "world.create",
        json!({ "persistent_id": snapshot.persistent_id.0, "tags": snapshot.tags, "rows": rows }),
    );
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(inverse),
    })
}

// ---- world.set ------------------------------------------------------------------------------

fn set_spec() -> CommandSpec {
    CommandSpec {
        id: "world.set".to_string(),
        summary: "Muda um valor de uma entidade.".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Id persistente da entidade."),
            ParamSpec::required("table", ParamKind::Text, "Nome da tabela."),
            ParamSpec::required("column", ParamKind::Text, "Nome da coluna."),
            ParamSpec::required("value", ParamKind::Any, "Novo valor, no formato da coluna."),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("WORLD_ROW_MISSING", "A entidade não tem essa tabela; use `row.create`."),
            FailureSpec::new("WORLD_INVALID_VALUE", "Corrija o valor indicado na mensagem."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 3, "table": "Transform", "column": "position", "value": {"x": 0, "y": 1, "z": 0}}"#
            .to_string(),
    }
}

fn set(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: SetPayload = parse("world.set", payload)?;
    let handle = resolve(world, request.entity)?;
    let kind = world.column_kind(&request.table, &request.column)?;
    let value = Value::from_json(kind, &request.value, world.limits().max_text_bytes).map_err(
        |reason| petunia_world::WorldError::InvalidValue {
            column: request.column.clone(),
            reason,
        },
    )?;
    let old = world.set(handle, &request.table, &request.column, value)?;
    let inverse = CommandRecord::new(
        "world.set",
        json!({
            "entity": request.entity,
            "table": request.table,
            "column": request.column,
            "value": old.to_json(),
        }),
    );
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(inverse),
    })
}

// ---- world.batch (run by the dispatcher) ------------------------------------------------------

pub(crate) fn batch_spec() -> CommandSpec {
    CommandSpec {
        id: "world.batch".to_string(),
        summary: "Aplica vários comandos como um só: tudo ou nada, e um único passo de desfazer."
            .to_string(),
        params: vec![ParamSpec::required(
            "records",
            ParamKind::Any,
            "Lista de {command, payload}. Dentro do lote, `{\"$entity_of\": n}` vale a entidade criada pelo comando `n` (contando de zero).",
        )],
        returns: None,
        failures: vec![FailureSpec::new(
            "COMMAND_INVALID_PAYLOAD",
            "Confira a lista `records`; nenhum comando do lote foi aplicado.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"records": [{"command": "world.remove", "payload": {"entity": 3}}, {"command": "world.create", "payload": {}}]}"#
            .to_string(),
    }
}

// ---- row.set ---------------------------------------------------------------------------------

fn row_set_spec() -> CommandSpec {
    CommandSpec {
        id: "row.set".to_string(),
        summary: "Muda várias colunas de uma linha de uma vez (um passo de desfazer).".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Id persistente da entidade."),
            ParamSpec::required("table", ParamKind::Text, "Nome da tabela."),
            ParamSpec::required("values", ParamKind::Any, "Objeto {coluna: valor}."),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new("WORLD_ROW_MISSING", "A entidade não tem essa tabela; use `row.create`."),
            FailureSpec::new("WORLD_INVALID_VALUE", "Corrija o valor indicado na mensagem."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 3, "table": "Transform", "values": {"position": {"x": 1, "y": 0, "z": 0}, "rotation_degrees": {"x": 0, "y": 90, "z": 0}}}"#
            .to_string(),
    }
}

#[derive(Deserialize)]
struct RowSetPayload {
    entity: u64,
    table: String,
    values: Map<String, Json>,
}

fn row_set(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: RowSetPayload = parse("row.set", payload)?;
    let handle = resolve(world, request.entity)?;
    // Validate every value before changing anything: a failure leaves the row untouched.
    let mut parsed = Vec::with_capacity(request.values.len());
    for (column, json) in &request.values {
        let kind = world.column_kind(&request.table, column)?;
        let value =
            Value::from_json(kind, json, world.limits().max_text_bytes).map_err(|reason| {
                petunia_world::WorldError::InvalidValue {
                    column: column.clone(),
                    reason,
                }
            })?;
        parsed.push((column.clone(), value));
    }
    if !world.has_row(handle, &request.table) {
        return Err(petunia_world::WorldError::RowMissing(request.table.clone()).into());
    }
    let mut old = Map::new();
    for (column, value) in parsed {
        let previous = world.set(handle, &request.table, &column, value)?;
        old.insert(column, previous.to_json());
    }
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(CommandRecord::new(
            "row.set",
            json!({ "entity": request.entity, "table": request.table, "values": old }),
        )),
    })
}

// ---- row.create / row.remove ----------------------------------------------------------------

fn row_create_spec() -> CommandSpec {
    CommandSpec {
        id: "row.create".to_string(),
        summary: "Adiciona uma linha de tabela a uma entidade.".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Id persistente da entidade."),
            ParamSpec::required("table", ParamKind::Text, "Nome da tabela."),
            ParamSpec::optional(
                "values",
                ParamKind::Any,
                "Valores das colunas; o resto usa o padrão.",
            ),
        ],
        returns: None,
        failures: vec![
            FailureSpec::new(
                "WORLD_ROW_EXISTS",
                "A entidade já tem essa tabela; use `world.set`.",
            ),
            FailureSpec::new("WORLD_UNKNOWN_TABLE", "Confira o nome da tabela."),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 3, "table": "Name", "values": {"text": "Porta"}}"#.to_string(),
    }
}

fn row_create(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: RowCreatePayload = parse("row.create", payload)?;
    let handle = resolve(world, request.entity)?;
    world.insert_row_json(handle, &request.table, &request.values)?;
    let inverse = CommandRecord::new(
        "row.remove",
        json!({ "entity": request.entity, "table": request.table }),
    );
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(inverse),
    })
}

fn row_remove_spec() -> CommandSpec {
    CommandSpec {
        id: "row.remove".to_string(),
        summary: "Remove uma linha de tabela de uma entidade.".to_string(),
        params: vec![
            ParamSpec::required("entity", ParamKind::Entity, "Id persistente da entidade."),
            ParamSpec::required("table", ParamKind::Text, "Nome da tabela."),
        ],
        returns: None,
        failures: vec![FailureSpec::new(
            "WORLD_ROW_MISSING",
            "A entidade não tem essa tabela.",
        )],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 3, "table": "Name"}"#.to_string(),
    }
}

fn row_remove(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: RowRemovePayload = parse("row.remove", payload)?;
    let handle = resolve(world, request.entity)?;
    let old = world.remove_row(handle, &request.table)?;
    let values = world.row_to_json(&request.table, &old)?;
    let inverse = CommandRecord::new(
        "row.create",
        json!({ "entity": request.entity, "table": request.table, "values": values }),
    );
    Ok(Outcome {
        result: Json::Null,
        inverse: Some(inverse),
    })
}

#[cfg(test)]
mod tests {
    use petunia_math::Vec3;
    use petunia_world::{TableSpec, ValueKind};

    use super::*;
    use crate::dispatcher::Dispatcher;

    fn world() -> World {
        let mut world = World::new(1);
        world
            .define_table(TableSpec::new("Transform").column(
                "position",
                ValueKind::Vec3,
                Value::Vec3(Vec3::ZERO),
            ))
            .unwrap();
        world
    }

    #[test]
    fn builtin_schema_is_valid() {
        builtin_schema().validate().unwrap();
    }

    #[test]
    fn create_undo_redo_round_trip() {
        let mut world = world();
        let mut dispatcher = Dispatcher::with_builtin().unwrap();
        let created = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(
                    "world.create",
                    json!({"rows": {"Transform": {"position": {"x": 1.0, "y": 2.0, "z": 3.0}}}}),
                ),
            )
            .unwrap();
        assert_eq!(world.entity_count(), 1);
        dispatcher.undo(&mut world).unwrap();
        assert_eq!(world.entity_count(), 0);
        dispatcher.redo(&mut world).unwrap();
        assert_eq!(world.entity_count(), 1);
        assert_eq!(created, json!({"entity": 1}));
    }

    #[test]
    fn a_batch_is_all_or_nothing_and_one_undo_step() {
        let mut world = World::new(1);
        world
            .define_table(petunia_world::TableSpec::new("Name").column(
                "text",
                petunia_world::ValueKind::Text,
                Value::Text(String::new()),
            ))
            .expect("table");
        let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
        let batch = |records: serde_json::Value| {
            CommandRecord::new("world.batch", json!({ "records": records }))
        };
        let good = batch(json!([
            { "command": "world.create", "payload": { "rows": { "Name": { "text": "a" } } } },
            { "command": "world.create", "payload": { "rows": { "Name": { "text": "b" } } } }
        ]));
        dispatcher.execute(&mut world, &good).expect("batch");
        assert_eq!(world.entity_count(), 2);
        dispatcher.undo(&mut world).expect("undo");
        assert_eq!(world.entity_count(), 0, "one undo removes both");
        dispatcher.redo(&mut world).expect("redo");
        assert_eq!(world.entity_count(), 2);
        // The second record fails: the first one must be rolled back.
        let bad = batch(json!([
            { "command": "world.create", "payload": {} },
            { "command": "world.create", "payload": { "rows": { "Missing": {} } } }
        ]));
        assert!(dispatcher.execute(&mut world, &bad).is_err());
        assert_eq!(
            world.entity_count(),
            2,
            "nothing from the failed batch stays"
        );
    }

    #[test]
    fn a_batch_can_point_at_what_it_created() {
        let mut world = World::new(1);
        world
            .define_table(petunia_world::TableSpec::new("Name").column(
                "text",
                petunia_world::ValueKind::Text,
                Value::Text(String::new()),
            ))
            .expect("table");
        let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
        let batch = |records: serde_json::Value| {
            CommandRecord::new("world.batch", json!({ "records": records }))
        };
        let good = batch(json!([
            { "command": "world.create", "payload": { "rows": { "Name": { "text": "a" } } } },
            { "command": "world.set", "payload": { "entity": { "$entity_of": 0 },
              "table": "Name", "column": "text", "value": "renomeada" } }
        ]));
        dispatcher.execute(&mut world, &good).expect("batch");
        let handle = world.all_entities()[0];
        assert_eq!(
            world.get(handle, "Name", "text").expect("name"),
            &Value::Text("renomeada".to_string())
        );
        dispatcher.undo(&mut world).expect("undo");
        assert_eq!(world.entity_count(), 0);
        dispatcher.redo(&mut world).expect("redo");
        assert_eq!(world.entity_count(), 1);
        // A reference to a later (or missing) command is refused and nothing stays.
        let bad = batch(json!([
            { "command": "world.create", "payload": {} },
            { "command": "world.remove", "payload": { "entity": { "$entity_of": 5 } } }
        ]));
        assert!(dispatcher.execute(&mut world, &bad).is_err());
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn failed_create_leaves_no_entity() {
        let mut world = world();
        let mut dispatcher = Dispatcher::with_builtin().unwrap();
        let result = dispatcher.execute(
            &mut world,
            &CommandRecord::new("world.create", json!({"rows": {"Missing": {}}})),
        );
        assert!(result.is_err());
        assert_eq!(world.entity_count(), 0);
        assert!(!dispatcher.can_undo());
    }
}
