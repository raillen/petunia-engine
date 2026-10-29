//! `spline.sweep`: drag a profile along a path and write the mesh into an object.
//!
//! This is the command that turns a curve into geometry, which is what a road, a river bed or a
//! cable is. It replaces the geometry of the object's `Mesh` row, so the object keeps its name,
//! transform and color; the previous geometry comes back with one undo step.

use petunia_curve::{ProfileKind, Spline, SweepSettings, sweep};
use petunia_math::{PolygonMesh, Vec3};
use petunia_schema::{CommandSpec, Cost, FailureSpec, ParamKind, ParamSpec, Reversibility, Status};
use petunia_world::{EntityHandle, Value, World};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

use crate::builtin::resolve;
use crate::dispatcher::{ApplyFn, CommandRecord, Outcome};
use crate::error::CommandError;

const MESH_TABLE: &str = "Mesh";
const GEOMETRY_COLUMN: &str = "geometry";

fn invalid(message: String) -> CommandError {
    CommandError::InvalidPayload {
        command: "spline.sweep".to_string(),
        message,
    }
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SweepPayload {
    entity: u64,
    /// Path of the curve, in world meters. Handles are derived (a smooth curve through the points).
    points: Vec<Vec3>,
    #[serde(default)]
    closed: bool,
    /// `ribbon`, `trough` or `tube`. Default: `ribbon`.
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    width: Option<f32>,
    /// How deep a trough goes, in meters.
    #[serde(default)]
    depth: Option<f32>,
    /// How thick a ribbon is; `0` makes a single surface.
    #[serde(default)]
    thickness: Option<f32>,
    /// Vertices of a tube.
    #[serde(default)]
    sides: Option<usize>,
    #[serde(default)]
    steps: Option<usize>,
    #[serde(default)]
    cap_ends: bool,
}

pub(crate) fn commands() -> Vec<(CommandSpec, ApplyFn)> {
    vec![(spec(), apply as ApplyFn)]
}

fn spec() -> CommandSpec {
    CommandSpec {
        id: "spline.sweep".to_string(),
        summary: "Varre um perfil ao longo de uma spline e grava a malha no objeto.".to_string(),
        params: vec![
            ParamSpec::required(
                "entity",
                ParamKind::Entity,
                "Objeto com malha (`Mesh`) que recebe a geometria varrida.",
            ),
            ParamSpec::required(
                "points",
                ParamKind::Any,
                "Pontos da curva, em metros: [{x, y, z}, …] (2 a 256).",
            ),
            ParamSpec::optional("closed", ParamKind::Bool, "Liga o último ponto ao primeiro."),
            ParamSpec::optional(
                "profile",
                ParamKind::Text,
                "Perfil: ribbon (faixa), trough (calha) ou tube (tubo). Padrão ribbon.",
            ),
            ParamSpec::optional("width", ParamKind::Float, "Largura do perfil em metros (padrão 4)."),
            ParamSpec::optional("depth", ParamKind::Float, "Profundidade da calha em metros (padrão 1)."),
            ParamSpec::optional("thickness", ParamKind::Float, "Espessura da faixa em metros (padrão 0)."),
            ParamSpec::optional("sides", ParamKind::Int, "Vértices do tubo (padrão 8, de 3 a 64)."),
            ParamSpec::optional("steps", ParamKind::Int, "Anéis ao longo da curva (padrão 24, até 512)."),
            ParamSpec::optional("cap_ends", ParamKind::Bool, "Fecha as pontas de um perfil fechado."),
        ],
        returns: Some(ParamKind::Any),
        failures: vec![
            FailureSpec::new(
                "SPLINE_INVALID",
                "Corrija os pontos, o perfil ou os passos indicados na mensagem.",
            ),
            FailureSpec::new(
                "WORLD_ROW_MISSING",
                "O objeto precisa de uma tabela `Mesh`; crie uma malha antes.",
            ),
        ],
        permission: "world.write".to_string(),
        cost: Cost::Cheap,
        reversibility: Reversibility::Undoable,
        status: Status::Provisional,
        example: r#"{"entity": 7, "points": [{"x": 0, "y": 0, "z": 0}, {"x": 0, "y": 0, "z": 20}], "profile": "ribbon", "width": 4, "steps": 12}"#
            .to_string(),
    }
}

fn apply(world: &mut World, payload: &Json) -> Result<Outcome, CommandError> {
    let request: SweepPayload = parse("spline.sweep", payload)?;
    let spline = Spline::through(&request.points, request.closed)
        .map_err(|error| invalid(format!("a curva não foi aceita: {error}")))?;
    let kind = match request.profile.as_deref() {
        Some(name) => ProfileKind::parse(name).ok_or_else(|| {
            invalid(format!(
                "perfil desconhecido `{name}`; use ribbon, trough ou tube"
            ))
        })?,
        None => ProfileKind::Ribbon,
    };
    let width = request.width.unwrap_or(4.0);
    let depth = request.depth.unwrap_or(1.0);
    let thickness = request.thickness.unwrap_or(0.0);
    let sides = request.sides.unwrap_or(8);
    let profile = kind
        .build(width, depth, thickness, sides)
        .map_err(|error| invalid(format!("o perfil não foi aceito: {error}")))?;
    let settings = SweepSettings {
        steps: request.steps.unwrap_or(24),
        cap_ends: request.cap_ends,
    };
    let mesh = sweep(&spline, &profile, &settings)
        .map_err(|error| invalid(format!("a varredura não foi aceita: {error}")))?;

    let handle = resolve(world, request.entity)?;
    let previous = match world.get(handle, MESH_TABLE, GEOMETRY_COLUMN) {
        Ok(Value::Mesh(previous)) => previous.clone(),
        Ok(other) => {
            return Err(invalid(format!(
                "a coluna `{GEOMETRY_COLUMN}` guarda {:?}, não uma malha",
                other.kind()
            )));
        }
        // The world error already names the missing table (`WORLD_ROW_MISSING`).
        Err(error) => return Err(error.into()),
    };
    let vertices = mesh.positions.len();
    let faces = mesh.faces.len();
    let steps = settings.steps;
    write_geometry(world, handle, &mesh)?;
    Ok(Outcome {
        result: json!({ "vertices": vertices, "faces": faces, "steps": steps }),
        inverse: Some(restore_geometry(request.entity, &previous)),
    })
}

fn write_geometry(
    world: &mut World,
    handle: EntityHandle,
    mesh: &PolygonMesh,
) -> Result<(), CommandError> {
    world
        .set(
            handle,
            MESH_TABLE,
            GEOMETRY_COLUMN,
            Value::Mesh(mesh.clone()),
        )
        .map(|_| ())
        .map_err(CommandError::from)
}

/// Undo: put the geometry that was there before back, in one step.
fn restore_geometry(entity: u64, previous: &PolygonMesh) -> CommandRecord {
    CommandRecord::new(
        "row.set",
        json!({
            "entity": entity,
            "table": MESH_TABLE,
            "values": { GEOMETRY_COLUMN: Value::Mesh(previous.clone()).to_json() },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dispatcher;
    use petunia_math::Color;
    use petunia_world::{TableSpec, ValueKind};

    fn world() -> (World, u64) {
        let mut world = World::new(1);
        world
            .define_table(TableSpec::new("Name").column(
                "text",
                ValueKind::Text,
                Value::Text(String::new()),
            ))
            .expect("Name");
        world
            .define_table(
                TableSpec::new("Mesh")
                    .column(
                        "geometry",
                        ValueKind::Mesh,
                        Value::Mesh(PolygonMesh::default()),
                    )
                    .column("color", ValueKind::Color, Value::Color(Color::WHITE)),
            )
            .expect("Mesh");
        let handle = world.spawn().expect("entidade");
        world
            .insert_row_values(
                handle,
                "Mesh",
                vec![
                    Value::Mesh(PolygonMesh::default()),
                    Value::Color(Color::WHITE),
                ],
            )
            .expect("linha de malha");
        let entity = world.persistent_id(handle).expect("id").0;
        (world, entity)
    }

    fn dispatcher() -> Dispatcher {
        Dispatcher::with_builtin().expect("dispatcher")
    }

    #[test]
    fn the_command_is_registered() {
        let ids: Vec<String> = crate::builtin_schema()
            .commands
            .into_iter()
            .map(|spec| spec.id)
            .collect();
        assert!(ids.contains(&"spline.sweep".to_string()));
    }

    #[test]
    fn sweeping_writes_the_mesh_and_undo_puts_the_old_one_back() {
        let (mut world, entity) = world();
        let mut dispatcher = dispatcher();
        let result = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(
                    "spline.sweep",
                    json!({
                        "entity": entity,
                        "points": [
                            {"x": 0.0, "y": 0.0, "z": 0.0},
                            {"x": 0.0, "y": 0.0, "z": 20.0}
                        ],
                        "profile": "ribbon",
                        "width": 4.0,
                        "steps": 5
                    }),
                ),
            )
            .expect("comando");
        assert_eq!(result["vertices"], json!(10));
        assert_eq!(result["faces"], json!(4));
        let handle = world
            .handle_of(petunia_world::PersistentId(entity))
            .expect("handle");
        let geometry = match world.get(handle, "Mesh", "geometry").expect("geometria") {
            Value::Mesh(mesh) => mesh,
            other => panic!("esperava malha, veio {:?}", other.kind()),
        };
        let (minimum, maximum) = geometry.bounds().expect("limites");
        assert!((maximum.x - minimum.x - 4.0).abs() < 0.01);
        assert!((maximum.z - minimum.z - 20.0).abs() < 0.01);

        dispatcher.undo(&mut world).expect("desfazer");
        let handle = world
            .handle_of(petunia_world::PersistentId(entity))
            .expect("handle");
        match world.get(handle, "Mesh", "geometry").expect("geometria") {
            Value::Mesh(mesh) => assert!(mesh.positions.is_empty()),
            other => panic!("esperava malha vazia, veio {:?}", other.kind()),
        }
    }

    #[test]
    fn a_bad_path_or_profile_is_rejected_without_touching_the_object() {
        let (mut world, entity) = world();
        let mut dispatcher = dispatcher();
        for payload in [
            json!({ "entity": entity, "points": [{"x": 0.0, "y": 0.0, "z": 0.0}] }),
            json!({
                "entity": entity,
                "points": [{"x": 0.0, "y": 0.0, "z": 0.0}, {"x": 1.0, "y": 0.0, "z": 1.0}],
                "profile": "fita"
            }),
            json!({
                "entity": entity,
                "points": [{"x": 0.0, "y": 0.0, "z": 0.0}, {"x": 1.0, "y": 0.0, "z": 1.0}],
                "profile": "tube", "width": 0.0
            }),
            json!({
                "entity": entity,
                "points": [{"x": 0.0, "y": 0.0, "z": 0.0}, {"x": 1.0, "y": 0.0, "z": 1.0}],
                "steps": 0
            }),
        ] {
            assert!(
                dispatcher
                    .execute(
                        &mut world,
                        &CommandRecord::new("spline.sweep", payload.clone())
                    )
                    .is_err(),
                "aceitou {payload}"
            );
        }
        let handle = world
            .handle_of(petunia_world::PersistentId(entity))
            .expect("handle");
        match world.get(handle, "Mesh", "geometry").expect("geometria") {
            Value::Mesh(mesh) => assert!(mesh.positions.is_empty()),
            other => panic!("esperava malha vazia, veio {:?}", other.kind()),
        }
    }

    #[test]
    fn an_object_without_a_mesh_row_is_refused() {
        let (mut world, _) = world();
        let bare = world.spawn().expect("entidade");
        let bare = world.persistent_id(bare).expect("id").0;
        let mut dispatcher = dispatcher();
        let error = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(
                    "spline.sweep",
                    json!({
                        "entity": bare,
                        "points": [
                            {"x": 0.0, "y": 0.0, "z": 0.0},
                            {"x": 0.0, "y": 0.0, "z": 5.0}
                        ]
                    }),
                ),
            )
            .expect_err("sem malha");
        assert!(format!("{error:?}").contains("Mesh"), "{error:?}");
    }
}
