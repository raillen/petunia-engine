use petunia_schema::check_params;
use petunia_world::{PersistentId, World};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::builtin::query_specs;
use crate::error::CommandError;

/// A query as data. Queries only read and return copies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryRecord {
    pub query: String,
    #[serde(default)]
    pub payload: Json,
}

impl QueryRecord {
    pub fn new(query: &str, payload: Json) -> Self {
        Self {
            query: query.to_string(),
            payload,
        }
    }
}

/// Runs a read-only query against the world.
pub fn run_query(world: &World, record: &QueryRecord) -> Result<Json, CommandError> {
    let specs = query_specs();
    let spec = specs
        .iter()
        .find(|spec| spec.id == record.query)
        .ok_or_else(|| CommandError::UnknownCommand {
            name: record.query.clone(),
            suggestions: specs.iter().map(|spec| spec.id.clone()).take(3).collect(),
        })?;
    check_params(&spec.params, &record.payload).map_err(|message| {
        CommandError::InvalidPayload {
            command: record.query.clone(),
            message,
        }
    })?;
    match record.query.as_str() {
        "world.get" => query_get(world, &record.payload),
        "world.find" => query_find(world, &record.payload),
        query if query.starts_with("terrain.") => {
            crate::terrain::run_query(world, query, &record.payload)
        }
        other => Err(CommandError::UnknownCommand {
            name: other.to_string(),
            suggestions: Vec::new(),
        }),
    }
}

fn text<'a>(payload: &'a Json, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Json::as_str)
}

fn query_get(world: &World, payload: &Json) -> Result<Json, CommandError> {
    let entity = payload.get("entity").and_then(Json::as_u64).unwrap_or(0);
    let handle = world.handle_of(PersistentId(entity))?;
    let table = text(payload, "table").unwrap_or_default();
    match text(payload, "column") {
        Some(column) => Ok(world.get(handle, table, column)?.to_json()),
        None => {
            let row = world.row(handle, table)?;
            Ok(Json::Object(world.row_to_json(table, row)?))
        }
    }
}

fn query_find(world: &World, payload: &Json) -> Result<Json, CommandError> {
    let table = text(payload, "table");
    let tag = text(payload, "tag");
    if table.is_none() && tag.is_none() {
        return Err(CommandError::InvalidPayload {
            command: "world.find".to_string(),
            message: "informe `table`, `tag` ou os dois".to_string(),
        });
    }
    let mut found = match table {
        Some(table) => world.entities_with_table(table)?,
        None => world.all_entities(),
    };
    if let Some(tag) = tag {
        let tagged = world.entities_with_tag(tag);
        found.retain(|handle| tagged.contains(handle));
    }
    let ids: Vec<u64> = found
        .into_iter()
        .filter_map(|handle| world.persistent_id(handle).ok())
        .map(|id| id.0)
        .collect();
    Ok(json!(ids))
}
