//! The editor session: the part of the editor that is **not** a window.
//!
//! It owns the open project, the undo history, the selection and the camera, and it is driven by
//! JSON operations (`{ "op": "...", ... }`), so the same session serves the Qt editor, automated
//! tests and, later, MCP agents. World changes always go through validated commands; selection
//! and camera are editor state, not world state.
//!
//! This crate knows nothing about windows, Qt or the GPU.

mod assets;
mod brush_ops;
mod colliders;
mod failure;
mod floors;
mod gizmo;
mod graph_ops;
mod lights;
mod mesh_tool;
mod play;
mod prefabs;
mod protocol;
mod scripts;
mod sculpt_tool;
mod session;
mod terrain_tool;
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
    fn saving_is_refused_while_a_drag_is_live_and_the_file_stays_untouched() {
        let root = scratch_dir("save_while_dragging");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Teste" }),
        );
        let scene = root.join("scenes").join("room.scene.json");
        let before = std::fs::read(&scene).expect("scene file");

        let preview = call(
            &mut session,
            json!({ "op": "gesture.preview", "entity": 9, "table": "Transform",
                    "column": "position", "value": { "x": 6.5, "y": 0.5, "z": 1.0 } }),
        );
        assert_eq!(preview["ok"], true, "{preview}");
        let refused = call(&mut session, json!({ "op": "project.save" }));
        assert_eq!(refused["ok"], false);
        assert_eq!(refused["error"]["code"], "EDITOR_BUSY");
        assert_eq!(
            std::fs::read(&scene).expect("scene file"),
            before,
            "a refused save must not write the transient preview"
        );

        assert_eq!(
            call(&mut session, json!({ "op": "gesture.cancel" }))["ok"],
            true
        );
        assert_eq!(
            call(&mut session, json!({ "op": "project.save" }))["ok"],
            true
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn opening_a_project_drops_the_drag_that_was_in_progress() {
        let root = scratch_dir("open_while_dragging");
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
        let preview = call(
            &mut session,
            json!({ "op": "gesture.preview", "entity": 9, "table": "Transform",
                    "column": "position", "value": { "x": before + 1.5, "y": 0.5, "z": 1.0 } }),
        );
        assert_eq!(preview["ok"], true, "{preview}");

        let opened = call(
            &mut session,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        assert_eq!(opened["ok"], true, "{opened}");
        assert_eq!(
            read(&mut session),
            before,
            "the reopened scene is the saved one"
        );

        let stale = call(&mut session, json!({ "op": "gesture.commit" }));
        assert_eq!(stale["ok"], true, "{stale}");
        assert_eq!(
            read(&mut session),
            before,
            "the stale drag must not change the new project"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn undo_is_refused_while_a_drag_is_live() {
        let root = scratch_dir("undo_while_dragging");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Teste" }),
        );
        let preview = call(
            &mut session,
            json!({ "op": "gesture.preview", "entity": 9, "table": "Transform",
                    "column": "position", "value": { "x": 4.0, "y": 0.5, "z": 1.0 } }),
        );
        assert_eq!(preview["ok"], true, "{preview}");
        let refused = call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(refused["ok"], false);
        assert_eq!(refused["error"]["code"], "EDITOR_BUSY");
        assert_eq!(
            call(&mut session, json!({ "op": "gesture.cancel" }))["ok"],
            true
        );
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

    #[test]
    fn camera_orbit_projection_and_isometric_views_work_through_protocol() {
        let mut session = EditorSession::new().expect("session");
        let root = scratch_dir("camera_orbit_test");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "CameraTest" }),
        );

        // Initial camera get
        let initial = call(&mut session, json!({ "op": "camera.get" }));
        assert_eq!(initial["ok"], true);
        assert_eq!(initial["result"]["projection_mode"], "perspective");

        // camera.orbit
        let orbit = call(
            &mut session,
            json!({ "op": "camera.orbit", "dx": 10.0, "dy": 5.0 }),
        );
        assert_eq!(orbit["ok"], true);
        let after_orbit = call(&mut session, json!({ "op": "camera.get" }));
        let yaw = after_orbit["result"]["yaw_degrees"].as_f64().unwrap();
        let pitch = after_orbit["result"]["pitch_degrees"].as_f64().unwrap();
        assert!((yaw - 4.0).abs() < 1.0e-3);
        assert!((pitch - (-2.0)).abs() < 1.0e-3);

        // camera.projection toggle
        let toggle = call(&mut session, json!({ "op": "camera.projection" }));
        assert_eq!(toggle["ok"], true);
        assert_eq!(toggle["result"]["projection_mode"], "orthographic");
        let after_toggle = call(&mut session, json!({ "op": "camera.get" }));
        assert_eq!(after_toggle["result"]["projection_mode"], "orthographic");

        // camera.projection explicit
        let set_persp = call(
            &mut session,
            json!({ "op": "camera.projection", "mode": "perspective" }),
        );
        assert_eq!(set_persp["ok"], true);
        assert_eq!(set_persp["result"]["projection_mode"], "perspective");

        // camera.projection validation: zero, negative, NaN height must fail
        let bad_height_zero = call(
            &mut session,
            json!({ "op": "camera.projection", "height": 0.0 }),
        );
        assert_eq!(bad_height_zero["ok"], false);
        let bad_height_neg = call(
            &mut session,
            json!({ "op": "camera.projection", "height": -5.0 }),
        );
        assert_eq!(bad_height_neg["ok"], false);

        // camera.projection with explicit valid height
        let set_ortho_custom = call(
            &mut session,
            json!({ "op": "camera.projection", "mode": "orthographic", "height": 12.0 }),
        );
        assert_eq!(set_ortho_custom["ok"], true);
        assert_eq!(
            set_ortho_custom["result"]["projection_mode"],
            "orthographic"
        );
        assert_eq!(set_ortho_custom["result"]["orthographic_height"], 12.0);

        // camera.view game_2_1
        let game_view = call(
            &mut session,
            json!({ "op": "camera.view", "name": "game_2_1" }),
        );
        assert_eq!(game_view["ok"], true);
        let game_cam = call(&mut session, json!({ "op": "camera.get" }));
        assert_eq!(game_cam["result"]["projection_mode"], "orthographic");
        assert!((game_cam["result"]["yaw_degrees"].as_f64().unwrap() - 45.0).abs() < 1.0e-3);
        assert!((game_cam["result"]["pitch_degrees"].as_f64().unwrap() - (-30.0)).abs() < 1.0e-3);

        // camera.view isometric
        let iso_view = call(
            &mut session,
            json!({ "op": "camera.view", "name": "isometric" }),
        );
        assert_eq!(iso_view["ok"], true);
        let iso_cam = call(&mut session, json!({ "op": "camera.get" }));
        assert_eq!(iso_cam["result"]["projection_mode"], "orthographic");
        assert!((iso_cam["result"]["yaw_degrees"].as_f64().unwrap() - 45.0).abs() < 1.0e-3);
        let true_iso_pitch = -(1.0f64 / 3.0f64.sqrt()).asin().to_degrees();
        assert!(
            (iso_cam["result"]["pitch_degrees"].as_f64().unwrap() - true_iso_pitch).abs() < 1.0e-3
        );

        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn terrain_is_created_generated_sculpted_saved_and_reopened() {
        let root = scratch_dir("terrain");
        let mut session = EditorSession::new().expect("session");
        let opened = call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Terreno" }),
        );
        assert_eq!(opened["ok"], true, "{opened}");

        let made = call(
            &mut session,
            json!({ "op": "create.terrain", "cell_size": 1.0, "chunk_cells": 8 }),
        );
        assert_eq!(made["ok"], true, "{made}");
        let entity = made["result"]["entity"].as_u64().expect("entity");

        let generated = call(
            &mut session,
            json!({
                "op": "terrain.generate",
                "entity": entity,
                "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0,
                "seed": 11, "amplitude": 4.0
            }),
        );
        assert_eq!(generated["ok"], true, "{generated}");

        let height_before = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": 4.0, "z": 4.0 } }),
        );
        let height = height_before["result"]["height"].as_f64().expect("height");

        let brushed = call(
            &mut session,
            json!({
                "op": "terrain.brush",
                "entity": entity,
                "shape": "sphere",
                "center": { "x": 4.0, "y": height, "z": 4.0 },
                "radius": 2.0,
                "falloff": "constant",
                "strength": 2.0,
                "operation": "raise"
            }),
        );
        assert_eq!(brushed["ok"], true, "{brushed}");

        let raised = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": 4.0, "z": 4.0 } }),
        );
        let raised_height = raised["result"]["height"].as_f64().expect("height");
        assert!((raised_height - height - 2.0).abs() < 0.01, "{raised}");

        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let undone = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": 4.0, "z": 4.0 } }),
        );
        let undone_height = undone["result"]["height"].as_f64().expect("height");
        assert!((undone_height - height).abs() < 0.01, "{undone}");

        assert_eq!(call(&mut session, json!({ "op": "edit.redo" }))["ok"], true);
        let redone = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": 4.0, "z": 4.0 } }),
        );
        let redone_height = redone["result"]["height"].as_f64().expect("height");
        assert!((redone_height - raised_height).abs() < 0.01, "{redone}");

        assert_eq!(
            call(&mut session, json!({ "op": "project.save" }))["ok"],
            true
        );
        call(
            &mut session,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        let reopened = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": 4.0, "z": 4.0 } }),
        );
        let reopened_height = reopened["result"]["height"].as_f64().expect("height");
        assert!((reopened_height - raised_height).abs() < 0.01, "{reopened}");

        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn a_terrain_drag_is_one_undo_step_and_cancel_leaves_no_trace() {
        let root = scratch_dir("terrain_tool");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Pincel" }),
        );
        let made = call(
            &mut session,
            json!({ "op": "create.terrain", "cell_size": 1.0, "chunk_cells": 8 }),
        );
        let entity = made["result"]["entity"].as_u64().expect("terrain");
        call(
            &mut session,
            json!({
                "op": "terrain.generate", "entity": entity,
                "min_x": -1, "min_z": -1, "max_x": 0, "max_z": 0,
                "seed": 1, "amplitude": 0.0
            }),
        );
        call(
            &mut session,
            json!({ "op": "camera.view_direction", "x": 0.0, "y": 0.6, "z": 1.0 }),
        );
        call(
            &mut session,
            json!({
                "op": "terrain.set", "radius": 2.0, "strength": 1.0,
                "operation": "raise", "falloff": "constant", "paint": false
            }),
        );
        let begun = call(
            &mut session,
            json!({
                "op": "terrain.begin",
                "x": 400.0, "y": 300.0, "width": 800.0, "height": 600.0
            }),
        );
        assert!(
            begun["result"]["changed"].as_u64().unwrap_or(0) > 0,
            "{begun}"
        );
        let preview = call(&mut session, json!({ "op": "terrain.preview" }));
        assert_eq!(preview["result"]["active"], true, "{preview}");
        let point = &preview["result"]["cursor"]["point"];
        let x = point["x"].as_f64().expect("x");
        let z = point["z"].as_f64().expect("z");
        let height_after = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": x, "z": z } }),
        )["result"]["height"]
            .as_f64()
            .expect("height");
        assert!(
            height_after > 0.1,
            "the first dab raised the surface: {height_after}"
        );

        let dragged = call(
            &mut session,
            json!({ "op": "terrain.drag", "x": 460.0, "y": 320.0, "width": 800.0, "height": 600.0 }),
        );
        assert_eq!(dragged["ok"], true, "{dragged}");
        let ended = call(&mut session, json!({ "op": "terrain.end" }));
        assert_eq!(ended["result"]["changed"], true, "{ended}");

        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let restored = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": x, "z": z } }),
        )["result"]["height"]
            .as_f64()
            .expect("height");
        assert!(
            restored.abs() < 0.01,
            "undo restored the terrain: {restored}"
        );

        // Begin again and cancel: the world goes back and no history entry is created.
        call(
            &mut session,
            json!({
                "op": "terrain.begin",
                "x": 400.0, "y": 300.0, "width": 800.0, "height": 600.0
            }),
        );
        let cancelled = call(&mut session, json!({ "op": "terrain.cancel" }));
        assert_eq!(cancelled["result"]["changed"], false);
        let after_cancel = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.height",
                    "payload": { "entity": entity, "x": x, "z": z } }),
        )["result"]["height"]
            .as_f64()
            .expect("height");
        assert!(
            after_cancel.abs() < 0.01,
            "cancel kept the surface: {after_cancel}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn surface_follow_moves_objects_and_the_preference_adds_the_row() {
        let root = scratch_dir("surface_follow");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Superfície" }),
        );
        let made = call(
            &mut session,
            json!({ "op": "create.terrain", "cell_size": 1.0, "chunk_cells": 8 }),
        );
        let terrain = made["result"]["entity"].as_u64().expect("terrain");
        call(
            &mut session,
            json!({
                "op": "terrain.generate", "entity": terrain,
                "min_x": 0, "min_z": 0, "max_x": 0, "max_z": 0,
                "seed": 1, "amplitude": 0.0
            }),
        );

        let created = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "world.create",
                "payload": {
                    "rows": {
                        "Transform": { "position": { "x": 2.0, "y": 5.0, "z": 2.0 } },
                        "SurfaceFollow": { "offset": 1.0 }
                    }
                }
            }),
        );
        let object = created["result"]["entity"].as_u64().expect("object");
        call(
            &mut session,
            json!({ "op": "surface.follow", "terrain": terrain }),
        );
        let position = call(
            &mut session,
            json!({ "op": "query.run", "query": "world.get",
                    "payload": { "entity": object, "table": "Transform", "column": "position" } }),
        );
        let y = position["result"]["y"].as_f64().expect("y");
        assert!((y - 1.0).abs() < 0.01, "{position}");
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let restored = call(
            &mut session,
            json!({ "op": "query.run", "query": "world.get",
                    "payload": { "entity": object, "table": "Transform", "column": "position" } }),
        );
        assert!((restored["result"]["y"].as_f64().unwrap_or(0.0) - 5.0).abs() < 0.01);

        call(
            &mut session,
            json!({ "op": "preferences.surface_follow", "enabled": true }),
        );
        let brush = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let brush_id = brush["result"]["entity"].as_u64().expect("brush");
        let row = call(
            &mut session,
            json!({ "op": "query.run", "query": "world.get",
                    "payload": { "entity": brush_id, "table": "SurfaceFollow" } }),
        );
        assert_eq!(row["ok"], true, "{row}");

        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn terrain_hover_updates_the_cursor_and_xray_is_a_preference() {
        let root = scratch_dir("terrain_hover");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Hover" }),
        );
        let made = call(
            &mut session,
            json!({ "op": "create.terrain", "cell_size": 1.0, "chunk_cells": 8 }),
        );
        let terrain = made["result"]["entity"].as_u64().expect("terrain");
        call(
            &mut session,
            json!({
                "op": "terrain.generate", "entity": terrain,
                "min_x": -1, "min_z": -1, "max_x": 0, "max_z": 0,
                "seed": 1, "amplitude": 0.0
            }),
        );
        call(
            &mut session,
            json!({ "op": "camera.view_direction", "x": 0.0, "y": 0.6, "z": 1.0 }),
        );
        call(&mut session, json!({ "op": "tool.set", "tool": "terrain" }));
        let hovered = call(
            &mut session,
            json!({ "op": "terrain.hover", "x": 400.0, "y": 300.0, "width": 800.0, "height": 600.0 }),
        );
        assert!(hovered["result"]["cursor"].is_object(), "{hovered}");
        call(
            &mut session,
            json!({ "op": "terrain.xray", "enabled": true }),
        );
        let preview = call(&mut session, json!({ "op": "terrain.preview" }));
        assert_eq!(preview["result"]["xray"], true);
        let snapshot = call(&mut session, json!({ "op": "editor.snapshot" }));
        assert_eq!(snapshot["result"]["terrain_xray"], true);

        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn terrain_section_sets_the_render_cut_plane() {
        let root = scratch_dir("terrain_section");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Corte" }),
        );
        let section = call(
            &mut session,
            json!({ "op": "terrain.section", "enabled": true, "height": 1.5 }),
        );
        assert_eq!(section["result"]["enabled"], true);
        assert!((section["result"]["height"].as_f64().unwrap_or(0.0) - 1.5).abs() < 1.0e-6);
        assert_eq!(
            session.frame().and_then(|frame| frame.section_height),
            Some(1.5)
        );
        call(
            &mut session,
            json!({ "op": "terrain.section", "enabled": false }),
        );
        assert_eq!(session.frame().and_then(|frame| frame.section_height), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
