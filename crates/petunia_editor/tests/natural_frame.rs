//! What the editor hands to the renderer for the natural-world entities.
//!
//! The unit tests in `petunia_render` prove the meshes; this one proves the frame uses them, so a
//! bridge reaches the viewport as a deck with an underside instead of a floating strip.

use serde_json::{Value as Json, json};

use petunia_editor::EditorSession;

fn call(session: &mut EditorSession, request: Json) -> Json {
    session.handle(&request)
}

fn scratch_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "petunia_editor_test_{label}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn straight_spline(height: f32) -> petunia_math::CubicBezierSpline {
    let mut spline = petunia_math::CubicBezierSpline::new();
    for x in [0.0, 24.0] {
        spline.points.push(petunia_math::SplinePoint {
            position: petunia_math::Vec3::new(x, height, 0.0),
            ..petunia_math::SplinePoint::default()
        });
    }
    spline
}

fn entity_with_table(session: &mut EditorSession, table: &str) -> Option<u64> {
    let snapshot = call(session, json!({ "op": "editor.snapshot" }));
    let entities = snapshot.get("result")?.get("entities")?.as_array()?;
    for entity in entities {
        let has_table = entity["tables"]
            .as_array()
            .is_some_and(|tables| tables.iter().any(|name| name == table));
        if has_table {
            return entity["id"].as_u64();
        }
    }
    None
}

#[test]
fn a_detected_bridge_reaches_the_frame_as_a_deck_with_an_underside() {
    let root = scratch_dir("natural_frame_bridge");
    let mut session = EditorSession::new().expect("session");
    let opened = call(
        &mut session,
        json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Mundo" }),
    );
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(
        call(&mut session, json!({ "op": "create.terrain" }))["ok"],
        true
    );

    // The road flies six meters above the flat terrain, so the whole span needs a bridge.
    let road = call(
        &mut session,
        json!({
            "op": "command.execute",
            "command": "path.create",
            "payload": {
                "kind": "road",
                "spline": petunia_world::Value::Spline(straight_spline(6.0)).to_json(),
            },
        }),
    );
    assert_eq!(road["ok"], true, "{road}");
    let road_id = road["result"]["entity"].as_u64().expect("road id");

    let structures = call(
        &mut session,
        json!({
            "op": "command.execute",
            "command": "road.detect_structures",
            "payload": { "road": road_id, "minimum_height_difference": 2.0 },
        }),
    );
    assert_eq!(structures["ok"], true, "{structures}");
    assert_eq!(
        structures["result"]["bridges"].as_u64(),
        Some(1),
        "{structures}"
    );
    let bridge_id = entity_with_table(&mut session, "Bridge").expect("bridge entity");

    let frame = session.frame().expect("frame");
    let item = frame
        .items
        .iter()
        .find(|item| item.entity == bridge_id)
        .expect("the bridge is drawn");
    let mesh = item.mesh.clone().expect("the bridge draws its own mesh");
    assert!(!mesh.indices.is_empty(), "the bridge mesh has triangles");
    assert!(
        mesh.normals.iter().any(|normal| normal[1] > 0.5),
        "the deck is lit from above"
    );
    assert!(
        mesh.normals.iter().any(|normal| normal[1] < -0.5),
        "the underside faces down instead of the sky"
    );
    let lowest = mesh
        .positions
        .iter()
        .map(|position| position[1])
        .fold(f32::INFINITY, f32::min);
    assert!(
        lowest < 5.5,
        "the deck hangs below the road surface: lowest {lowest}"
    );
}

#[test]
fn a_road_inside_a_cutting_stays_data_only() {
    let root = scratch_dir("natural_frame_culvert");
    let mut session = EditorSession::new().expect("session");
    assert_eq!(
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "Mundo" }),
        )["ok"],
        true
    );
    assert_eq!(
        call(&mut session, json!({ "op": "create.terrain" }))["ok"],
        true
    );
    // A hill next to the road: raising the terrain above the road turns the span into a cutting.
    let terrain = entity_with_table(&mut session, "Terrain").expect("terrain entity");
    let painted = call(
        &mut session,
        json!({
            "op": "command.execute",
            "command": "terrain.paint",
            "payload": {
                "terrain": terrain,
                "stamps": [{
                    "center": { "x": 12.0, "y": 0.0, "z": 0.0 },
                    "radius": 14.0,
                    "strength": 8.0,
                    "mode": "raise",
                }],
            },
        }),
    );
    assert_eq!(painted["ok"], true, "{painted}");

    let road = call(
        &mut session,
        json!({
            "op": "command.execute",
            "command": "path.create",
            "payload": {
                "kind": "road",
                "spline": petunia_world::Value::Spline(straight_spline(0.5)).to_json(),
            },
        }),
    );
    assert_eq!(road["ok"], true, "{road}");
    let road_id = road["result"]["entity"].as_u64().expect("road id");
    let structures = call(
        &mut session,
        json!({
            "op": "command.execute",
            "command": "road.detect_structures",
            "payload": { "road": road_id, "minimum_height_difference": 2.0 },
        }),
    );
    assert_eq!(structures["ok"], true, "{structures}");
    assert_eq!(
        structures["result"]["bridges"].as_u64(),
        Some(0),
        "a cutting is not a bridge: {structures}"
    );
    assert_eq!(
        structures["result"]["culverts"].as_u64(),
        Some(1),
        "the span under the hill is a culvert: {structures}"
    );

    let culvert = entity_with_table(&mut session, "Bridge").expect("culvert row");
    let frame = session.frame().expect("frame");
    assert!(
        frame.items.iter().all(|item| item.entity != culvert),
        "a culvert stays data only: the terrain is the structure"
    );
}
