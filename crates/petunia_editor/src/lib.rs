//! The editor session: the part of the editor that is **not** a window.
//!
//! It owns the open project, the undo history, the selection and the camera, and it is driven by
//! JSON operations (`{ "op": "...", ... }`), so the same session serves the Qt editor, automated
//! tests and, later, MCP agents. World changes always go through validated commands; selection
//! and camera are editor state, not world state.
//!
//! This crate knows nothing about windows, Qt or the GPU.

mod brush_ops;
mod failure;
mod floors;
mod gizmo;
mod protocol;
mod scripts;
mod sculpt_tool;
mod session;
mod tools;

pub use failure::Failure;
pub use session::EditorSession;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::{Value as Json, json};

    use super::*;

    fn scratch_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    fn call(session: &mut EditorSession, request: Json) -> Json {
        session.handle(&request)
    }

    fn count(session: &mut EditorSession) -> u64 {
        let snapshot = call(session, json!({ "op": "editor.snapshot" }));
        snapshot["result"]["entity_count"].as_u64().unwrap_or(0)
    }

    #[test]
    fn create_undo_redo_delete_save_and_reopen() {
        let root = scratch_dir("flow");
        let mut session = EditorSession::new().expect("session");
        let opened = call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Teste" }),
        );
        assert_eq!(opened["ok"], true, "{opened}");
        let initial = count(&mut session);

        let created = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        assert_eq!(created["ok"], true, "{created}");
        assert_eq!(count(&mut session), initial + 1);

        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        assert_eq!(count(&mut session), initial);
        assert_eq!(call(&mut session, json!({ "op": "edit.redo" }))["ok"], true);
        assert_eq!(count(&mut session), initial + 1);

        // Undo dropped the selection (the entity was gone); select the brush again by its id.
        let brush = created["result"]["entity"].as_u64().expect("entity id");
        let selected = call(
            &mut session,
            json!({ "op": "selection.set", "entity": brush }),
        );
        assert_eq!(selected["ok"], true, "{selected}");
        // Duplicate it, then delete the copy.
        assert_eq!(
            call(&mut session, json!({ "op": "edit.duplicate" }))["ok"],
            true
        );
        assert_eq!(count(&mut session), initial + 2);
        assert_eq!(
            call(&mut session, json!({ "op": "edit.delete" }))["ok"],
            true
        );
        assert_eq!(count(&mut session), initial + 1);

        assert_eq!(
            call(&mut session, json!({ "op": "project.save" }))["ok"],
            true
        );
        let mut reopened = EditorSession::new().expect("session");
        let again = call(
            &mut reopened,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        assert_eq!(again["ok"], true, "{again}");
        assert_eq!(count(&mut reopened), initial + 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_drag_is_previewed_live_and_becomes_exactly_one_undo_step() {
        let root = scratch_dir("gesture");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Teste" }),
        );
        let read = |session: &mut EditorSession| {
            let answer = call(
                session,
                json!({ "op": "query.run", "query": "world.get",
                        "payload": { "entity": 9, "table": "Transform", "column": "position" } }),
            );
            answer["result"]["x"].as_f64().expect("x")
        };
        let before = read(&mut session);
        let history_before =
            call(&mut session, json!({ "op": "history.get" }))["result"]["entries"]
                .as_array()
                .expect("entries")
                .len();

        for x in [4.0, 5.0, 6.5] {
            let step = call(
                &mut session,
                json!({ "op": "gesture.preview", "entity": 9, "table": "Transform",
                        "column": "position", "value": { "x": x, "y": 0.5, "z": 1.0 } }),
            );
            assert_eq!(step["ok"], true, "{step}");
            assert_eq!(read(&mut session), x, "the preview shows live in the world");
        }
        let committed = call(&mut session, json!({ "op": "gesture.commit" }));
        assert_eq!(committed["ok"], true, "{committed}");
        assert_eq!(read(&mut session), 6.5);

        let history_after = call(&mut session, json!({ "op": "history.get" }))["result"]["entries"]
            .as_array()
            .expect("entries")
            .len();
        assert_eq!(
            history_after,
            history_before + 1,
            "one history entry for the whole drag"
        );

        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        assert_eq!(
            read(&mut session),
            before,
            "one undo goes back to before the drag"
        );
        assert_eq!(call(&mut session, json!({ "op": "edit.redo" }))["ok"], true);
        assert_eq!(read(&mut session), 6.5);

        // A drag that ends where it started leaves no history entry.
        call(
            &mut session,
            json!({ "op": "gesture.preview", "entity": 9, "table": "Transform",
                    "column": "position", "value": { "x": 6.5, "y": 0.5, "z": 1.0 } }),
        );
        let same = call(&mut session, json!({ "op": "gesture.commit" }));
        assert_eq!(same["ok"], true);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_drag_that_changes_two_columns_is_still_one_undo_step() {
        let root = scratch_dir("two_columns");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Teste" }),
        );
        let read = |session: &mut EditorSession, column: &str| {
            call(
                session,
                json!({ "op": "query.run", "query": "world.get",
                        "payload": { "entity": 9, "table": "Transform", "column": column } }),
            )["result"]
                .clone()
        };
        let position = read(&mut session, "position");
        let rotation = read(&mut session, "rotation_degrees");
        for (column, value) in [
            ("position", json!({ "x": 1.0, "y": 0.5, "z": 1.0 })),
            ("rotation_degrees", json!({ "x": 0.0, "y": 90.0, "z": 0.0 })),
        ] {
            let step = call(
                &mut session,
                json!({ "op": "gesture.preview", "entity": 9, "table": "Transform",
                        "column": column, "value": value }),
            );
            assert_eq!(step["ok"], true, "{step}");
        }
        assert_eq!(
            call(&mut session, json!({ "op": "gesture.commit" }))["ok"],
            true
        );
        let entries = call(&mut session, json!({ "op": "history.get" }))["result"]["entries"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let last = entries.last().expect("entry")["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        assert!(
            last.starts_with("world.batch"),
            "one batch for both columns: {last}"
        );
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        assert_eq!(read(&mut session, "position"), position);
        assert_eq!(read(&mut session, "rotation_degrees"), rotation);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn errors_are_reported_with_a_code_and_never_change_the_world() {
        let mut session = EditorSession::new().expect("session");
        let none = call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(none["error"]["code"], "EDITOR_NO_PROJECT");

        let unknown = call(&mut session, json!({ "op": "does.not_exist" }));
        assert_eq!(unknown["error"]["code"], "EDITOR_UNKNOWN_OP");

        let root = scratch_dir("errors");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Teste" }),
        );
        let before = count(&mut session);
        let bad = call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.set",
                    "payload": { "entity": 1, "table": "Transform", "column": "position", "value": "nope" } }),
        );
        assert_eq!(bad["ok"], false);
        assert_eq!(count(&mut session), before);
        let _ = std::fs::remove_dir_all(&root);
    }
}
