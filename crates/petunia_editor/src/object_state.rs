//! The outliner's visibility and locking, as editor operations (`QT-12`).
//!
//! Three operations, each one command so it undoes in one step: `object.hide`, `object.lock` and
//! `object.solo`. Solo is a **view** of the scene, not a change to it: it hides everything except
//! the chosen object without writing a hidden flag to anyone, so leaving solo puts the scene back
//! exactly as it was.
//!
//! Hiding is an editor decision, not a scene edit, so it travels with the project file (the world
//! table is saved like any other). Locking does the same. That is on purpose: a person who hid the
//! ceiling of a house expects it to stay hidden after reopening.

use petunia_world::{EDITOR_STATE_TABLE, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

impl EditorSession {
    /// `object.hide`: shows or hides one object (or every object with `all`).
    pub fn set_hidden(
        &mut self,
        entity: Option<u64>,
        hidden: bool,
        all: bool,
    ) -> Result<Json, Failure> {
        if all {
            let ids = self.all_object_ids()?;
            let mut records = Vec::new();
            for id in ids {
                records.push(self.state_record(id, Some(hidden), None));
            }
            self.execute(&petunia_commands::CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            ))?;
            return Ok(json!({ "hidden": hidden, "all": true }));
        }
        let id = entity.ok_or_else(crate::colliders::no_collider)?;
        self.require_object(id)?;
        self.execute(&self.state_record(id, Some(hidden), None))?;
        Ok(json!({ "entity": id, "hidden": hidden }))
    }

    /// `object.lock`: locks or unlocks one object.
    pub fn set_locked(&mut self, entity: u64, locked: bool) -> Result<Json, Failure> {
        self.require_object(entity)?;
        self.execute(&self.state_record(entity, None, Some(locked)))?;
        Ok(json!({ "entity": entity, "locked": locked }))
    }

    /// `object.solo`: shows only `entity`. Solo of the object already alone turns solo off.
    pub fn solo(&mut self, entity: u64) -> Result<Json, Failure> {
        self.require_object(entity)?;
        let leaving = self.solo_object == Some(entity);
        self.solo_object = if leaving { None } else { Some(entity) };
        Ok(json!({ "solo": self.solo_object, "entity": entity }))
    }

    /// Removes the hidden objects from a frame, and keeps only the solo one when solo is on.
    ///
    /// Runs on the frame the renderer gets, not on the world: hiding is a view, so the file keeps
    /// every object and a scene reloaded shows exactly what the person left.
    pub(crate) fn apply_object_visibility(&self, items: &mut Vec<petunia_render::DrawItem>) {
        let Some(visible) = self.visible_ids() else {
            return;
        };
        items.retain(|item| {
            // Items without an entity (the grid helpers) always stay.
            item.entity == 0 || visible.contains(&item.entity)
        });
    }

    /// The objects the viewport should draw: everything except the hidden ones, or only the solo one.
    pub(crate) fn visible_ids(&self) -> Option<std::collections::BTreeSet<u64>> {
        if let Some(solo) = self.solo_object {
            return Some([solo].into_iter().collect());
        }
        let world = self.world().ok()?;
        let hidden = petunia_world::hidden_objects(world);
        (!hidden.is_empty()).then_some(
            self.all_object_ids()
                .ok()?
                .into_iter()
                .filter(|id| !hidden.contains(id))
                .collect(),
        )
    }

    /// Every object id, colliders and script attachments left out (they follow their owner).
    fn all_object_ids(&self) -> Result<Vec<u64>, Failure> {
        let world = self.world()?;
        let mut ids = Vec::new();
        for handle in world.all_entities() {
            let Ok(identity) = world.persistent_id(handle) else {
                continue;
            };
            let id = identity.0;
            if crate::colliders::owner_of(world, id).is_some()
                || crate::scripts::script_owner_of(world, id).is_some()
            {
                continue;
            }
            ids.push(id);
        }
        Ok(ids)
    }

    fn require_object(&self, entity: u64) -> Result<(), Failure> {
        let world = self.world()?;
        world
            .handle_of(petunia_world::PersistentId(entity))
            .map(|_| ())
            .map_err(|_| {
                Failure::new(
                    "EDITOR_NO_ENTITY",
                    &format!("o objeto {entity} não existe"),
                    "Escolha um objeto na Hierarquia.",
                )
            })
    }

    /// The record that writes one object's state. Only the columns named are touched, so hiding and
    /// locking never fight over the same row.
    fn state_record(
        &self,
        entity: u64,
        hidden: Option<bool>,
        locked: Option<bool>,
    ) -> petunia_commands::CommandRecord {
        // One row per object, created on first use and then updated in place.
        let existing = self.existing_state_row(entity);
        match existing {
            Some(row) => {
                let mut values = serde_json::Map::new();
                if let Some(hidden) = hidden {
                    values.insert("hidden".to_string(), json!(hidden));
                }
                if let Some(locked) = locked {
                    values.insert("locked".to_string(), json!(locked));
                }
                petunia_commands::CommandRecord::new(
                    "row.set",
                    json!({
                        "entity": row,
                        "table": EDITOR_STATE_TABLE,
                        "values": values,
                    }),
                )
            }
            None => {
                let mut row = serde_json::Map::new();
                row.insert("owner".to_string(), json!(entity));
                if let Some(hidden) = hidden {
                    row.insert("hidden".to_string(), json!(hidden));
                }
                if let Some(locked) = locked {
                    row.insert("locked".to_string(), json!(locked));
                }
                petunia_commands::CommandRecord::new(
                    "world.create",
                    json!({ "rows": { EDITOR_STATE_TABLE: row } }),
                )
            }
        }
    }

    fn existing_state_row(&self, entity: u64) -> Option<u64> {
        let world = self.world().ok()?;
        let handles = world.entities_with_table(EDITOR_STATE_TABLE).ok()?;
        for handle in handles {
            let Ok(Value::Int(owner)) = world.get(handle, EDITOR_STATE_TABLE, "owner") else {
                continue;
            };
            if *owner >= 0 && *owner as u64 == entity {
                return world.persistent_id(handle).ok().map(|id| id.0);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn opened(label: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": label }),
        );
        (session, root)
    }

    fn drawn(session: &EditorSession) -> Vec<u64> {
        session
            .frame()
            .expect("frame")
            .items
            .iter()
            .map(|item| item.entity)
            .filter(|id| *id != 0)
            .collect()
    }

    #[test]
    fn hiding_an_object_takes_it_out_of_the_frame_and_one_undo_puts_it_back() {
        let (mut session, root) = opened("hide");
        let made = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let entity = made["entity"].as_u64().expect("entity");
        assert!(drawn(&session).contains(&entity));

        call(
            &mut session,
            json!({ "op": "object.hide", "entity": entity, "hidden": true }),
        );
        assert!(
            !drawn(&session).contains(&entity),
            "a hidden object is not drawn"
        );

        // One undo brings it back: hiding is one step, not one per frame.
        call(&mut session, json!({ "op": "edit.undo" }));
        assert!(drawn(&session).contains(&entity));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_locked_object_is_drawn_but_cannot_be_chosen() {
        let (mut session, root) = opened("lock");
        let made = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let entity = made["entity"].as_u64().expect("entity");
        call(
            &mut session,
            json!({ "op": "object.lock", "entity": entity, "locked": true }),
        );
        assert!(drawn(&session).contains(&entity), "still drawn");

        let refused = session.handle(&json!({ "op": "selection.set", "entity": entity }));
        assert_eq!(refused["ok"], false, "{refused}");
        assert_eq!(refused["error"]["code"], "EDITOR_OBJECT_LOCKED");

        call(
            &mut session,
            json!({ "op": "object.lock", "entity": entity, "locked": false }),
        );
        call(
            &mut session,
            json!({ "op": "selection.set", "entity": entity }),
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn solo_shows_one_object_and_leaves_the_scene_as_it_was() {
        let (mut session, root) = opened("solo");
        let first = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        )["entity"]
            .as_u64()
            .expect("entity");
        let second = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "sphere" }),
        )["entity"]
            .as_u64()
            .expect("entity");
        let before = drawn(&session).len();

        call(
            &mut session,
            json!({ "op": "object.solo", "entity": first }),
        );
        let during = drawn(&session);
        assert!(during.contains(&first));
        assert!(
            !during.contains(&second),
            "solo shows one object: {during:?}"
        );

        // Solo is a view: leaving it restores everything without undoing anything.
        call(
            &mut session,
            json!({ "op": "object.solo", "entity": first }),
        );
        assert_eq!(drawn(&session).len(), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_state_survives_save_and_reopen() {
        let (mut session, root) = opened("state_saved");
        let entity = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        )["entity"]
            .as_u64()
            .expect("entity");
        call(
            &mut session,
            json!({ "op": "object.hide", "entity": entity, "hidden": true }),
        );
        call(&mut session, json!({ "op": "project.save" }));

        let mut reopened = EditorSession::new().expect("session");
        call(
            &mut reopened,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        assert!(
            !drawn(&reopened).contains(&entity),
            "a hidden object stays hidden after reopening"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
