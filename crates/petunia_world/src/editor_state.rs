//! Per-object visibility and locking in the outliner (`QT-12`).
//!
//! Two flags and a colour live in a table of their own instead of on every object's rows: an object
//! with no row is visible, unlocked and keeps its own colour, which is the "defaults → overrides"
//! shape the AGENTS file asks for. Nothing here changes geometry, so hiding an object never touches
//! the undo history unless the person asked for a command.
//!
//! A **hidden** object is not drawn and cannot be clicked in the viewport. A **locked** object is
//! drawn but cannot be selected or moved, which is the "background reference" case.

use crate::value::Value;
use crate::world::World;

/// Table that carries the per-object overrides.
pub const EDITOR_STATE_TABLE: &str = "EditorState";

/// What the outliner knows about one object.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObjectState {
    pub hidden: bool,
    pub locked: bool,
}

/// Reads the state of one object. Missing row or missing column means "visible and unlocked".
#[must_use]
pub fn state_of(world: &World, object: u64) -> ObjectState {
    let Ok(handles) = world.entities_with_table(EDITOR_STATE_TABLE) else {
        return ObjectState::default();
    };
    for handle in handles {
        let Ok(Value::Int(owner)) = world.get(handle, EDITOR_STATE_TABLE, "owner") else {
            continue;
        };
        if *owner < 0 || *owner as u64 != object {
            continue;
        }
        let hidden = matches!(
            world.get(handle, EDITOR_STATE_TABLE, "hidden"),
            Ok(Value::Bool(true))
        );
        let locked = matches!(
            world.get(handle, EDITOR_STATE_TABLE, "locked"),
            Ok(Value::Bool(true))
        );
        return ObjectState { hidden, locked };
    }
    ObjectState::default()
}

/// Every object that is hidden, as a set the renderer and the picking use.
#[must_use]
pub fn hidden_objects(world: &World) -> std::collections::BTreeSet<u64> {
    collect(world, |state| state.hidden)
}

/// Every object that is locked.
#[must_use]
pub fn locked_objects(world: &World) -> std::collections::BTreeSet<u64> {
    collect(world, |state| state.locked)
}

fn collect(world: &World, wanted: impl Fn(ObjectState) -> bool) -> std::collections::BTreeSet<u64> {
    let mut found = std::collections::BTreeSet::new();
    let Ok(handles) = world.entities_with_table(EDITOR_STATE_TABLE) else {
        return found;
    };
    for handle in handles {
        let Ok(Value::Int(owner)) = world.get(handle, EDITOR_STATE_TABLE, "owner") else {
            continue;
        };
        if *owner < 0 {
            continue;
        }
        let id = *owner as u64;
        if wanted(state_of(world, id)) {
            found.insert(id);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::TableSpec;
    use crate::value::ValueKind;

    fn world() -> World {
        let mut world = World::new(7);
        // Only what this module needs: the real table set lives in `petunia_project`, which depends
        // on this crate and so cannot be used here.
        world
            .define_table(
                TableSpec::new(EDITOR_STATE_TABLE)
                    .column("owner", ValueKind::Int, Value::Int(0))
                    .column("hidden", ValueKind::Bool, Value::Bool(false))
                    .column("locked", ValueKind::Bool, Value::Bool(false)),
            )
            .expect("table");
        world
    }

    fn hide(world: &mut World, owner: u64, hidden: bool, locked: bool) {
        let handle = world.spawn().expect("room");
        world
            .insert_row_json(
                handle,
                EDITOR_STATE_TABLE,
                &serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&format!(
                    r#"{{"owner": {owner}, "hidden": {hidden}, "locked": {locked}}}"#
                ))
                .expect("row"),
            )
            .expect("row fits");
        let _ = world.persistent_id(handle).expect("alive");
    }

    #[test]
    fn an_object_without_a_row_is_visible_and_unlocked() {
        let world = world();
        assert_eq!(state_of(&world, 3), ObjectState::default());
        assert!(hidden_objects(&world).is_empty());
    }

    #[test]
    fn hiding_and_locking_are_read_back_per_object() {
        let mut world = world();
        hide(&mut world, 3, true, false);
        hide(&mut world, 4, false, true);
        assert_eq!(
            state_of(&world, 3),
            ObjectState {
                hidden: true,
                locked: false
            }
        );
        assert_eq!(
            state_of(&world, 4),
            ObjectState {
                hidden: false,
                locked: true
            }
        );
        assert_eq!(hidden_objects(&world), [3].into_iter().collect());
        assert_eq!(locked_objects(&world), [4].into_iter().collect());
    }
}
