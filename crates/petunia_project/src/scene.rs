use std::collections::{BTreeMap, BTreeSet};

use petunia_world::{MAX_SAFE_INT, PersistentId, Value, World, WorldError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::diagnostics::{Diagnostic, Report};

pub const SCENE_FORMAT_VERSION: u32 = 1;

/// A scene file: entities as rows of tables, with persistent ids. Keys are sorted, so saving
/// the same world always produces the same bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneFile {
    pub format_version: u32,
    pub next_persistent_id: u64,
    pub entities: Vec<SceneEntity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneEntity {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub rows: BTreeMap<String, Map<String, Json>>,
}

/// Captures the whole world as a scene file (entities sorted by persistent id).
pub fn scene_from_world(world: &World) -> Result<SceneFile, WorldError> {
    let mut entities = Vec::new();
    for handle in world.all_entities() {
        let snapshot = world.snapshot(handle)?;
        let mut rows = BTreeMap::new();
        for (table, values) in &snapshot.rows {
            rows.insert(table.clone(), world.row_to_json(table, values)?);
        }
        entities.push(SceneEntity {
            id: snapshot.persistent_id.0,
            tags: snapshot.tags,
            rows,
        });
    }
    entities.sort_by_key(|entity| entity.id);
    Ok(SceneFile {
        format_version: SCENE_FORMAT_VERSION,
        next_persistent_id: world.next_persistent_id(),
        entities,
    })
}

fn world_diagnostic(target: &str, error: &WorldError) -> Diagnostic {
    Diagnostic::error(
        "scene.row.invalid",
        target,
        &format!("{error} ({})", error.code()),
        &error.suggestion(),
    )
}

/// Loads every entity it can into the world, reporting every problem instead of stopping at
/// the first. An entity with any invalid part is skipped as a whole.
pub(crate) fn load_entities(world: &mut World, scene: &SceneFile, report: &mut Report) {
    let mut seen = BTreeSet::new();
    let mut highest = 0u64;
    for entity in &scene.entities {
        let target = format!("entidade #{}", entity.id);
        if entity.id == 0 || entity.id > MAX_SAFE_INT as u64 {
            report.push(Diagnostic::error(
                "scene.entity.invalid_id",
                &target,
                "o id da entidade precisa estar entre 1 e 2^53-1",
                "Corrija o campo `id` ou remova a entidade.",
            ));
            continue;
        }
        if !seen.insert(entity.id) {
            report.push(Diagnostic::error(
                "scene.entity.duplicate_id",
                &target,
                "dois itens da cena usam o mesmo id",
                "Dê um id diferente a uma das entidades.",
            ));
            continue;
        }

        let mut parsed: Vec<(&str, Vec<Value>)> = Vec::new();
        let mut failed = false;
        for (table, values) in &entity.rows {
            match world.parse_row(table, values) {
                Ok(row) => parsed.push((table.as_str(), row)),
                Err(error) => {
                    report.push(world_diagnostic(&format!("{target} / {table}"), &error));
                    failed = true;
                }
            }
        }
        for tag in &entity.tags {
            if let Err(error) = world.validate_tag(tag) {
                report.push(world_diagnostic(&format!("{target} / tag `{tag}`"), &error));
                failed = true;
            }
        }
        if failed {
            continue;
        }

        match world.spawn_with_persistent_id(PersistentId(entity.id)) {
            Ok(handle) => {
                for tag in &entity.tags {
                    if let Err(error) = world.add_tag(handle, tag) {
                        report.push(world_diagnostic(&target, &error));
                    }
                }
                for (table, row) in parsed {
                    if let Err(error) = world.insert_row_values(handle, table, row) {
                        report.push(world_diagnostic(&format!("{target} / {table}"), &error));
                    }
                }
                highest = highest.max(entity.id);
            }
            Err(error @ WorldError::LimitExceeded(_)) => {
                report.push(Diagnostic::error(
                    "scene.entity.limit",
                    "cena",
                    &format!("{error}"),
                    "Divida a cena em partes menores.",
                ));
                break;
            }
            Err(error) => report.push(world_diagnostic(&target, &error)),
        }
    }

    if scene.next_persistent_id <= highest {
        report.push(Diagnostic::warning(
            "scene.next_id.stale",
            "cena",
            "`next_persistent_id` não é maior que o maior id usado",
            "Salve a cena de novo para corrigir; o valor foi recalculado.",
        ));
    }
    if scene.next_persistent_id <= MAX_SAFE_INT as u64 {
        world.raise_next_persistent_id(scene.next_persistent_id);
    }
}
