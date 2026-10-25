use std::collections::BTreeMap;

use serde_json::{Map, Value as Json};

use crate::error::WorldError;
use crate::ids::EntityHandle;
use crate::world::World;

/// One structural change waiting for the safe point of the frame.
#[derive(Clone, Debug, PartialEq)]
pub enum DeferredOp {
    Spawn {
        handle: EntityHandle,
        tags: Vec<String>,
        rows: BTreeMap<String, Map<String, Json>>,
    },
    Despawn(EntityHandle),
}

/// Structural changes (spawn, despawn) requested during a frame are queued here and applied
/// together at a safe point, so loops over the world never see entities appear or vanish.
///
/// A spawn hands out its handle immediately; the entity becomes alive when the buffer applies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandBuffer {
    ops: Vec<DeferredOp>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn spawn(
        &mut self,
        world: &mut World,
        tags: Vec<String>,
        rows: BTreeMap<String, Map<String, Json>>,
    ) -> Result<EntityHandle, WorldError> {
        let handle = world.reserve()?;
        self.ops.push(DeferredOp::Spawn { handle, tags, rows });
        Ok(handle)
    }

    pub fn despawn(&mut self, handle: EntityHandle) {
        self.ops.push(DeferredOp::Despawn(handle));
    }

    /// Applies every queued change in order. Failed changes do not stop the others; their
    /// errors are returned together.
    pub fn apply(&mut self, world: &mut World) -> Vec<WorldError> {
        let mut errors = Vec::new();
        for op in std::mem::take(&mut self.ops) {
            let outcome = match op {
                DeferredOp::Spawn { handle, tags, rows } => {
                    apply_spawn(world, handle, &tags, &rows)
                }
                DeferredOp::Despawn(handle) => apply_despawn(world, handle),
            };
            if let Err(error) = outcome {
                errors.push(error);
            }
        }
        errors
    }
}

fn apply_spawn(
    world: &mut World,
    handle: EntityHandle,
    tags: &[String],
    rows: &BTreeMap<String, Map<String, Json>>,
) -> Result<(), WorldError> {
    // Validate everything first so a bad row cancels the spawn instead of leaving half an entity.
    let mut parsed = Vec::with_capacity(rows.len());
    for (table, values) in rows {
        parsed.push((table, world.parse_row(table, values)));
    }
    for tag in tags {
        if let Err(error) = world.validate_tag(tag) {
            world.cancel_reservation(handle)?;
            return Err(error);
        }
    }
    if let Some((_, Err(error))) = parsed.iter().find(|(_, result)| result.is_err()) {
        let error = error.clone();
        world.cancel_reservation(handle)?;
        return Err(error);
    }
    world.activate(handle)?;
    for tag in tags {
        world.add_tag(handle, tag)?;
    }
    for (table, result) in parsed {
        world.insert_row_values(handle, table, result?)?;
    }
    Ok(())
}

fn apply_despawn(world: &mut World, handle: EntityHandle) -> Result<(), WorldError> {
    if world.is_alive(handle) {
        world.despawn(handle).map(|_| ())
    } else {
        // Reserved but never activated: just free the reservation. Anything else is stale.
        world.cancel_reservation(handle)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::table::TableSpec;
    use crate::value::{Value, ValueKind};

    #[test]
    fn spawn_is_deferred_until_apply() {
        let mut world = World::new(1);
        world
            .define_table(TableSpec::new("Door").column(
                "open",
                ValueKind::Bool,
                Value::Bool(false),
            ))
            .unwrap();
        let mut buffer = CommandBuffer::new();
        let rows = BTreeMap::from([(
            "Door".to_string(),
            json!({"open": true})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        )]);
        let handle = buffer
            .spawn(&mut world, vec!["Interactive".to_string()], rows)
            .unwrap();
        assert!(!world.is_alive(handle), "not alive before the safe point");
        assert!(buffer.apply(&mut world).is_empty());
        assert!(world.is_alive(handle));
        assert_eq!(
            world.get(handle, "Door", "open").unwrap(),
            &Value::Bool(true)
        );
    }
}
