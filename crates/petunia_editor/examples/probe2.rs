use serde_json::json;
fn main() {
    let mut s = petunia_editor::EditorSession::new().expect("s");
    let root = std::env::temp_dir().join("petunia_rain_probe2");
    let _ = std::fs::remove_dir_all(&root);
    s.handle(&json!({"op":"project.new_sample","path":root.display().to_string(),"name":"r"}));
    s.handle(&json!({"op":"weather.set","hours":17.0,"weather":"rain"}));
    let f = s.frame().expect("f");
    let mesh = f.precipitation.as_ref().expect("chuva");
    let camera = f.camera.position;
    println!("camera: {camera:?}");
    println!("gotas: {}", mesh.positions.len() / 4);
    // How many drops are near the camera?
    let near = mesh
        .positions
        .iter()
        .filter(|p| {
            let dx = p[0] - camera.x;
            let dy = p[1] - camera.y;
            let dz = p[2] - camera.z;
            dx * dx + dy * dy + dz * dz < 900.0
        })
        .count();
    println!("perto da camera (<30m): {near}");
    println!("primeira gota: {:?}", mesh.positions.first());
    println!("alpha da chuva e fixo no adaptador: 0.55");
}
