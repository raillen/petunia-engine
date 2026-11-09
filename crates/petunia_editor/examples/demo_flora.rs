use serde_json::json;

fn main() {
    let mut session = petunia_editor::EditorSession::new().expect("session");
    let root = std::env::temp_dir().join("petunia_flora_demo");
    let _ = std::fs::remove_dir_all(&root);
    session.handle(&json!({
        "op": "project.new_sample",
        "path": root.display().to_string(),
        "name": "flora"
    }));
    // A terrain to plant on, then a rule that matches it.
    session.handle(&json!({ "op": "create.terrain" }));
    session.handle(&json!({
        "op": "flora.add", "species": "grama", "spacing": 3.0, "seed": 7
    }));
    // Rain at dusk: fog plus falling water, so the capture shows both.
    session.handle(&json!({ "op": "weather.set", "hours": 17.0, "weather": "rain" }));
    let saved = session.handle(&json!({ "op": "project.save" }));
    println!("salvo: {saved}");
    let frame = session.frame().expect("frame");
    println!(
        "vegetacao: {:?} indices, chuva: {:?} indices, nevoa: {}",
        frame.vegetation.as_ref().map(|mesh| mesh.indices.len()),
        frame.precipitation.as_ref().map(|mesh| mesh.indices.len()),
        frame.fog.is_some()
    );
}
