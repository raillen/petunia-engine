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
mod natural_ops;
mod natural_tool;
mod play;
mod prefabs;
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
    fn terrain_and_paths_are_commands_that_save_and_undo() {
        let root = scratch_dir("natural");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Mundo" }),
        );

        let created = call(&mut session, json!({ "op": "create.terrain" }));
        assert_eq!(created["ok"], true, "{created}");
        let terrain = created["result"]["entity"].as_u64().expect("terrain id");

        let painted = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "terrain.paint",
                "payload": {
                    "terrain": terrain,
                    "stamps": [{
                        "center": {"x": 10.0, "y": 0.0, "z": 10.0},
                        "radius": 4.0,
                        "strength": 2.0,
                        "mode": "raise"
                    }]
                }
            }),
        );
        assert_eq!(painted["ok"], true, "{painted}");
        let sample = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": 10.0, "z": 10.0 } }),
        );
        let raised = sample["result"]["height"].as_f64().unwrap_or(0.0);
        assert!(raised > 0.5, "height {raised}");

        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let after_undo = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": 10.0, "z": 10.0 } }),
        );
        assert!(
            after_undo["result"]["height"].as_f64().unwrap_or(1.0).abs() < 1.0e-4,
            "{after_undo}"
        );
        assert_eq!(call(&mut session, json!({ "op": "edit.redo" }))["ok"], true);

        // The editor accepts the erosion mode and the command wears the bump down.
        let mode = call(
            &mut session,
            json!({ "op": "terrain.mode", "mode": "erode" }),
        );
        assert_eq!(mode["ok"], true, "{mode}");
        assert_eq!(mode["result"]["mode"], "erode", "{mode}");
        let eroded = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "terrain.paint",
                "payload": {
                    "terrain": terrain,
                    "stamps": [{
                        "center": {"x": 10.0, "y": 0.0, "z": 10.0},
                        "radius": 4.0,
                        "strength": 1.0,
                        "mode": "erode"
                    }]
                }
            }),
        );
        assert_eq!(eroded["ok"], true, "{eroded}");
        let sample_after_erosion = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": 10.0, "z": 10.0 } }),
        );
        let eroded_height = sample_after_erosion["result"]["height"]
            .as_f64()
            .unwrap_or(0.0);
        assert!(
            eroded_height < raised,
            "erosion lowered the peak: {eroded_height} >= {raised}"
        );

        // The noise brush adds detail on the spot without lifting the patch as a whole.
        let noise_mode = call(
            &mut session,
            json!({ "op": "terrain.mode", "mode": "noise" }),
        );
        assert_eq!(noise_mode["ok"], true, "{noise_mode}");
        assert_eq!(noise_mode["result"]["mode"], "noise", "{noise_mode}");
        let roughened = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "terrain.paint",
                "payload": {
                    "terrain": terrain,
                    "stamps": [{
                        "center": {"x": 40.0, "y": 0.0, "z": 40.0},
                        "radius": 4.0,
                        "strength": 1.0,
                        "mode": "noise"
                    }]
                }
            }),
        );
        assert_eq!(roughened["ok"], true, "{roughened}");
        let height_at = |session: &mut EditorSession, x: f64, z: f64| {
            call(
                session,
                json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": x, "z": z } }),
            )["result"]["height"]
                .as_f64()
                .unwrap_or(0.0)
        };
        let samples = [
            (38.5, 40.0),
            (39.25, 41.5),
            (40.75, 39.5),
            (41.5, 41.0),
            (40.0, 38.5),
        ];
        let heights: Vec<f64> = samples
            .iter()
            .map(|(x, z)| height_at(&mut session, *x, *z))
            .collect();
        let detailed = heights.iter().filter(|height| height.abs() > 0.05).count();
        assert!(detailed >= 3, "the noise left the ground flat: {heights:?}");
        assert!(
            heights.iter().all(|height| height.abs() <= 2.0),
            "the noise went past twice the strength: {heights:?}"
        );
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let restored: Vec<f64> = samples
            .iter()
            .map(|(x, z)| height_at(&mut session, *x, *z))
            .collect();
        assert!(
            restored.iter().all(|height| height.abs() < 1.0e-4),
            "one undo did not bring the ground back: {restored:?}"
        );

        let mut spline = petunia_math::CubicBezierSpline::new();
        spline.points.push(petunia_math::SplinePoint {
            position: petunia_math::Vec3::new(0.0, 2.0, 0.0),
            ..petunia_math::SplinePoint::default()
        });
        spline.points.push(petunia_math::SplinePoint {
            position: petunia_math::Vec3::new(20.0, 0.0, 0.0),
            ..petunia_math::SplinePoint::default()
        });
        let river = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "path.create",
                "payload": {
                    "kind": "river",
                    "spline": petunia_world::Value::Spline(spline).to_json()
                }
            }),
        );
        assert_eq!(river["ok"], true, "{river}");
        let river_id = river["result"]["entity"].as_u64().expect("river id");
        let report = call(
            &mut session,
            json!({ "op": "query.run", "query": "path.report", "payload": { "entity": river_id } }),
        );
        assert_eq!(report["result"]["kind"], "river");
        assert_eq!(report["result"]["points"], 2);
        assert!(report["result"]["length_meters"].as_f64().unwrap_or(0.0) > 15.0);

        assert_eq!(
            call(&mut session, json!({ "op": "project.save" }))["ok"],
            true
        );
        let mut reopened = EditorSession::new().expect("session");
        call(
            &mut reopened,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        let kept = call(
            &mut reopened,
            json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": 10.0, "z": 10.0 } }),
        );
        assert!(
            kept["result"]["height"].as_f64().unwrap_or(0.0) > 0.5,
            "{kept}"
        );
        let kept_river = call(
            &mut reopened,
            json!({ "op": "query.run", "query": "path.report", "payload": { "entity": river_id } }),
        );
        assert_eq!(kept_river["result"]["kind"], "river");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_terrain_and_path_tools_work_from_pointer_events() {
        let root = scratch_dir("natural_tools");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Ferramentas" }),
        );
        let pointer = json!({ "x": 720.0, "y": 560.0, "width": 1440.0, "height": 900.0 });
        call(&mut session, json!({ "op": "tool.set", "tool": "terrain" }));
        let mut begin = pointer.clone();
        begin["op"] = json!("terrain.begin");
        assert_eq!(call(&mut session, begin)["ok"], true);
        let mut drag = pointer.clone();
        drag["op"] = json!("terrain.drag");
        assert_eq!(call(&mut session, drag)["ok"], true);
        let ended = call(&mut session, json!({ "op": "terrain.end" }));
        assert_eq!(ended["ok"], true, "{ended}");
        let snapshot = call(&mut session, json!({ "op": "editor.snapshot" }));
        assert!(
            snapshot["result"]["natural"]["terrain"].as_u64().is_some(),
            "{snapshot}"
        );

        call(
            &mut session,
            json!({ "op": "tool.set", "tool": "river_path" }),
        );
        let mut point = pointer.clone();
        point["op"] = json!("natural.point");
        assert_eq!(call(&mut session, point.clone())["ok"], true);
        point["x"] = json!(760.0);
        assert_eq!(call(&mut session, point.clone())["ok"], true);
        point["x"] = json!(820.0);
        assert_eq!(call(&mut session, point)["ok"], true);
        let committed = call(&mut session, json!({ "op": "natural.commit" }));
        assert_eq!(committed["ok"], true, "{committed}");
        let river_id = committed["result"]["entity"].as_u64().expect("river");
        let report = call(
            &mut session,
            json!({ "op": "query.run", "query": "path.report", "payload": { "entity": river_id } }),
        );
        assert_eq!(report["result"]["kind"], "river", "{report}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_step_brush_raises_the_terrain_without_a_drag_and_one_undo_takes_it_back() {
        let root = scratch_dir("terrain_step");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Passo" }),
        );
        call(&mut session, json!({ "op": "tool.set", "tool": "terrain" }));
        // Exactly what the window sends when the terrain tool is active and Space is pressed: a
        // pointer position and how many times to stamp. No `terrain.begin`, no drag, no release.
        let stepped = call(
            &mut session,
            json!({
                "op": "terrain.step",
                "x": 720.0, "y": 560.0, "width": 1440.0, "height": 900.0,
                "times": 3
            }),
        );
        assert_eq!(stepped["ok"], true, "{stepped}");
        let result = &stepped["result"];
        assert_eq!(result["steps"].as_u64(), Some(3), "{stepped}");
        assert_eq!(result["mode"], "raise", "{stepped}");
        let terrain = result["terrain"].as_u64().expect("terrain");
        let height = result["height"].as_f64().expect("height");
        assert!(
            height > 0.2,
            "três passos de 0,5 m deixaram {height} m: {stepped}"
        );
        let x = result["point"]["x"].as_f64().expect("x");
        let z = result["point"]["z"].as_f64().expect("z");

        // The step is ONE undo step: the ground goes back to flat, not to two thirds of it.
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let after = call(
            &mut session,
            json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": x, "z": z } }),
        );
        assert!(
            after["result"]["height"].as_f64().unwrap_or(1.0).abs() < 1.0e-4,
            "{after}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_crater_stamp_carves_a_bowl_with_a_bank_in_one_undo_step() {
        let root = scratch_dir("terrain_stamp");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Carimbo" }),
        );
        call(&mut session, json!({ "op": "tool.set", "tool": "terrain" }));
        // The same op the options bar sends: a named shape instead of a plain raise.
        let options = call(
            &mut session,
            json!({ "op": "terrain.mode", "mode": "stamp", "stamp": "crater", "radius": 8.0, "strength": 2.0 }),
        );
        assert_eq!(options["ok"], true, "{options}");
        assert_eq!(options["result"]["mode"], "stamp", "{options}");
        assert_eq!(options["result"]["stamp"], "crater", "{options}");
        let stamped = call(
            &mut session,
            json!({ "op": "terrain.step", "x": 720.0, "y": 560.0, "width": 1440.0, "height": 900.0 }),
        );
        assert_eq!(stamped["ok"], true, "{stamped}");
        assert_eq!(stamped["result"]["stamp"], "crater", "{stamped}");
        let terrain = stamped["result"]["terrain"].as_u64().expect("terrain");
        let x = stamped["result"]["point"]["x"].as_f64().expect("x");
        let z = stamped["result"]["point"]["z"].as_f64().expect("z");

        let height_at = |session: &mut EditorSession, x: f64, z: f64| -> f64 {
            let sampled = call(
                session,
                json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": x, "z": z } }),
            );
            sampled["result"]["height"].as_f64().unwrap_or(f64::NAN)
        };
        let middle = height_at(&mut session, x, z);
        // The bank sits at 0.72 of the radius, the strongest point of the crater rim.
        let bank = height_at(&mut session, x + 8.0 * 0.72, z);
        assert!(middle < -1.5, "the middle is a bowl: {middle} ({stamped})");
        assert!(bank > 0.2, "the rim is a bank: {bank} ({stamped})");

        // One stroke, one undo: the whole shape goes back at once.
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let after = height_at(&mut session, x, z);
        assert!(after.abs() < 1.0e-4, "undo left {after} m behind");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_stamp_turned_by_ninety_degrees_runs_the_other_way() {
        let root = scratch_dir("terrain_stamp_turn");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Giro" }),
        );
        call(&mut session, json!({ "op": "tool.set", "tool": "terrain" }));
        // A crest along x, the way the shape comes out of the box.
        let options = call(
            &mut session,
            json!({ "op": "terrain.mode", "mode": "stamp", "stamp": "ridge", "rotation": 0.0, "radius": 8.0, "strength": 2.0 }),
        );
        assert_eq!(options["result"]["rotation"], 0.0, "{options}");
        let straight = call(
            &mut session,
            json!({ "op": "terrain.step", "x": 720.0, "y": 560.0, "width": 1440.0, "height": 900.0 }),
        );
        assert_eq!(straight["ok"], true, "{straight}");
        let terrain = straight["result"]["terrain"].as_u64().expect("terrain");
        let x = straight["result"]["point"]["x"].as_f64().expect("x");
        let z = straight["result"]["point"]["z"].as_f64().expect("z");
        let height_at = |session: &mut EditorSession, x: f64, z: f64| -> f64 {
            let sampled = call(
                session,
                json!({ "op": "query.run", "query": "terrain.sample", "payload": { "terrain": terrain, "x": x, "z": z } }),
            );
            sampled["result"]["height"].as_f64().unwrap_or(f64::NAN)
        };
        assert!(
            height_at(&mut session, x + 5.0, z) > 0.5,
            "the straight crest runs along x"
        );
        assert!(
            height_at(&mut session, x, z + 5.0) < 0.05,
            "the straight crest is narrow along z"
        );

        // Same shape, a quarter turn: the crest now runs along z.
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let turned = call(
            &mut session,
            json!({ "op": "terrain.mode", "rotation": 90.0 }),
        );
        assert_eq!(turned["result"]["rotation"], 90.0, "{turned}");
        let stamped = call(
            &mut session,
            json!({ "op": "terrain.step", "x": 720.0, "y": 560.0, "width": 1440.0, "height": 900.0 }),
        );
        assert_eq!(stamped["result"]["rotation"], 90.0, "{stamped}");
        assert!(
            height_at(&mut session, x, z + 5.0) > 0.5,
            "the turned crest runs along z: {stamped}"
        );
        assert!(
            height_at(&mut session, x + 5.0, z) < 0.05,
            "the turned crest is narrow along x: {stamped}"
        );
        // The turn does not resize the shape: the peak is the same two meters.
        let peak = height_at(&mut session, x, z);
        assert!((peak - 2.0).abs() < 0.1, "the peak is {peak} m, not 2 m");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_road_above_the_terrain_gets_a_bridge_span_that_undo_removes() {
        let root = scratch_dir("bridge");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Ponte" }),
        );
        let terrain = call(&mut session, json!({ "op": "create.terrain" }))["result"]["entity"]
            .as_u64()
            .expect("terrain");
        assert!(terrain > 0);
        let mut spline = petunia_math::CubicBezierSpline::new();
        spline.points.push(petunia_math::SplinePoint {
            position: petunia_math::Vec3::new(-10.0, 6.0, 10.0),
            ..petunia_math::SplinePoint::default()
        });
        spline.points.push(petunia_math::SplinePoint {
            position: petunia_math::Vec3::new(10.0, 6.0, 10.0),
            ..petunia_math::SplinePoint::default()
        });
        let road = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "path.create",
                "payload": { "kind": "road", "spline": petunia_world::Value::Spline(spline).to_json(), "width": 6.0 }
            }),
        );
        assert_eq!(road["ok"], true, "{road}");
        let road_id = road["result"]["entity"].as_u64().expect("road");
        let found = call(
            &mut session,
            json!({
                "op": "command.execute",
                "command": "road.detect_structures",
                "payload": { "road": road_id, "minimum_height_difference": 2.0 }
            }),
        );
        assert_eq!(found["ok"], true, "{found}");
        assert!(
            found["result"]["bridges"].as_u64().unwrap_or(0) > 0,
            "{found}"
        );
        let snapshot = call(&mut session, json!({ "op": "editor.snapshot" }));
        assert!(
            snapshot["result"]["entities"]
                .to_string()
                .contains("\"Bridge\""),
            "{snapshot}"
        );
        assert_eq!(call(&mut session, json!({ "op": "edit.undo" }))["ok"], true);
        let after = call(&mut session, json!({ "op": "editor.snapshot" }));
        assert!(
            !after["result"]["entities"]
                .to_string()
                .contains("\"Bridge\""),
            "{after}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
