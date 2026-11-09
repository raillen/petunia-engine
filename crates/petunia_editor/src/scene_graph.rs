//! Parent links and fixed cameras (DR-41, DR-42).
//!
//! Parenting writes `Transform.parent`. The stored position stays local. A fixed camera is a
//! `Shot` row on an object: while playing, the player inside its box sees from that object.

use petunia_commands::CommandRecord;
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

impl EditorSession {
    /// `transform.set_parent`: `child` keeps a local position under `parent`.
    pub fn set_parent(&mut self, child: u64, parent: u64) -> Result<Json, Failure> {
        if child == 0 || parent == 0 {
            return Err(Failure::bad_request(
                "diga o objeto e o pai, os dois com id maior que zero",
            ));
        }
        if child == parent {
            return Err(Failure::new(
                "PARENT_SELF",
                "um objeto não pode ser pai de si mesmo",
                "Escolha outro objeto como pai.",
            ));
        }
        self.ensure_transform(child)?;
        self.ensure_transform(parent)?;
        if self.parent_cycle(child, parent) {
            return Err(Failure::new(
                "PARENT_CYCLE",
                "isso fecharia um ciclo na hierarquia",
                "Solte um dos dois antes de aninhar de novo.",
            ));
        }
        self.execute(&CommandRecord::new(
            "world.set",
            json!({
                "entity": child,
                "table": "Transform",
                "column": "parent",
                "value": parent as i64,
            }),
        ))?;
        Ok(json!({ "child": child, "parent": parent }))
    }

    /// `transform.clear_parent`: the object returns to the root. Its position stays where it was
    /// stored, which is the local offset, so the caller that wants it to stay visually still
    /// writes the world position first.
    pub fn clear_parent(&mut self, child: u64) -> Result<Json, Failure> {
        self.ensure_transform(child)?;
        self.execute(&CommandRecord::new(
            "world.set",
            json!({
                "entity": child,
                "table": "Transform",
                "column": "parent",
                "value": 0,
            }),
        ))?;
        Ok(json!({ "child": child, "parent": 0 }))
    }

    /// `shot.add`: the selected object becomes a fixed camera with a room-sized box.
    pub fn add_shot(&mut self, entity: u64, priority: i64) -> Result<Json, Failure> {
        self.ensure_transform(entity)?;
        let world = self.world()?;
        let handle = world
            .handle_of(PersistentId(entity))
            .map_err(|_| Failure::bad_request("esse objeto não existe"))?;
        if world.has_row(handle, "Shot") {
            self.execute(&CommandRecord::new(
                "world.set",
                json!({
                    "entity": entity,
                    "table": "Shot",
                    "column": "priority",
                    "value": priority,
                }),
            ))?;
            self.execute(&CommandRecord::new(
                "world.set",
                json!({
                    "entity": entity,
                    "table": "Shot",
                    "column": "enabled",
                    "value": true,
                }),
            ))?;
        } else {
            self.execute(&CommandRecord::new(
                "row.create",
                json!({
                    "entity": entity,
                    "table": "Shot",
                    "values": { "priority": priority, "fov_degrees": 50.0 },
                }),
            ))?;
        }
        Ok(json!({ "entity": entity, "priority": priority }))
    }

    /// `shot.list`: every fixed camera, for the window and for tests.
    pub fn list_shots(&self) -> Result<Json, Failure> {
        let world = self.world()?;
        let mut shots = Vec::new();
        for handle in world.entities_with_table("Shot").unwrap_or_default() {
            let entity = world.persistent_id(handle).map_or(0, |id| id.0);
            let priority = match world.get(handle, "Shot", "priority") {
                Ok(Value::Int(value)) => *value,
                _ => 0,
            };
            let enabled = !matches!(world.get(handle, "Shot", "enabled"), Ok(Value::Bool(false)));
            shots.push(json!({ "entity": entity, "priority": priority, "enabled": enabled }));
        }
        Ok(json!({ "shots": shots }))
    }

    /// `view.comfort`: the sensory-comfort grade (no vignette, no grain, a slow luminance ceiling).
    pub fn set_comfort(&mut self, enabled: bool) -> Json {
        self.high_contrast = enabled;
        json!({ "comfort": enabled })
    }

    fn ensure_transform(&self, entity: u64) -> Result<(), Failure> {
        let world = self.world()?;
        let handle = world
            .handle_of(PersistentId(entity))
            .map_err(|_| Failure::bad_request("esse objeto não existe"))?;
        if !world.has_row(handle, "Transform") {
            return Err(Failure::new(
                "PARENT_NO_TRANSFORM",
                "esse objeto não tem posição",
                "Parentesco e câmera fixa usam a tabela Transform.",
            ));
        }
        Ok(())
    }

    fn parent_cycle(&self, child: u64, parent: u64) -> bool {
        let Ok(world) = self.world() else {
            return true;
        };
        let mut cursor = parent;
        for _ in 0..32 {
            if cursor == child {
                return true;
            }
            let Ok(handle) = world.handle_of(PersistentId(cursor)) else {
                return false;
            };
            cursor = petunia_world::parent_id(world, handle);
            if cursor == 0 {
                return false;
            }
        }
        true
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

    fn sample(name: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": name }),
        );
        (session, root)
    }

    #[test]
    fn parenting_is_one_undo_and_a_cycle_is_refused() {
        let (mut session, root) = sample("parent");
        let parent = call(&mut session, json!({ "op": "create.brush", "shape": "box" }));
        let child = call(&mut session, json!({ "op": "create.brush", "shape": "sphere" }));
        let parent_id = parent["entity"].as_u64().expect("parent id");
        let child_id = child["entity"].as_u64().expect("child id");
        call(
            &mut session,
            json!({ "op": "transform.set_parent", "entity": child_id, "parent": parent_id }),
        );
        let refused = session.handle(&json!({
            "op": "transform.set_parent", "entity": parent_id, "parent": child_id
        }));
        assert_eq!(refused["ok"], false, "{refused}");
        assert_eq!(refused["error"]["code"], "PARENT_CYCLE");
        call(&mut session, json!({ "op": "edit.undo" }));
        let world = session.world().expect("world");
        let handle = world
            .handle_of(petunia_world::PersistentId(child_id))
            .expect("child");
        assert_eq!(
            world.get(handle, "Transform", "parent").ok(),
            Some(&petunia_world::Value::Int(0))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_shot_on_an_object_is_listed_and_removed_by_undo() {
        let (mut session, root) = sample("shot");
        let created = call(&mut session, json!({ "op": "create.brush", "shape": "box" }));
        let entity = created["entity"].as_u64().expect("id");
        call(
            &mut session,
            json!({ "op": "shot.add", "entity": entity, "priority": 3 }),
        );
        let listed = call(&mut session, json!({ "op": "shot.list" }));
        assert_eq!(listed["shots"][0]["entity"], entity);
        assert_eq!(listed["shots"][0]["priority"], 3);
        call(&mut session, json!({ "op": "edit.undo" }));
        let listed = call(&mut session, json!({ "op": "shot.list" }));
        assert!(listed["shots"].as_array().expect("shots").is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
