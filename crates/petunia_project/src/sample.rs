use std::path::Path;

use petunia_world::{World, WorldError};
use serde_json::{Map, Value as Json, json};

use crate::error::ProjectError;
use crate::io::atomic_write;
use crate::manifest::{MANIFEST_FILE, ProjectManifest};
use crate::save::save_scene;
use crate::standard::register_standard_tables;

const SCENE_PATH: &str = "scenes/room.scene.json";

fn object(value: Json) -> Map<String, Json> {
    match value {
        Json::Object(map) => map,
        _ => Map::new(),
    }
}

fn vector(values: [f32; 3]) -> Json {
    json!({ "x": values[0], "y": values[1], "z": values[2] })
}

struct BrushSpec<'a> {
    name: &'a str,
    shape: &'a str,
    position: [f32; 3],
    size: [f32; 3],
    color: [f32; 3],
    rotation_degrees: [f32; 3],
}

fn spawn_brush(world: &mut World, brush: &BrushSpec<'_>) -> Result<(), WorldError> {
    let handle = world.spawn()?;
    world.insert_row_json(handle, "Name", &object(json!({ "text": brush.name })))?;
    world.insert_row_json(
        handle,
        "Transform",
        &object(json!({
            "position": vector(brush.position),
            "rotation_degrees": vector(brush.rotation_degrees),
        })),
    )?;
    world.insert_row_json(
        handle,
        "Brush",
        &object(json!({
            "shape": brush.shape,
            "size": vector(brush.size),
            "color": { "r": brush.color[0], "g": brush.color[1], "b": brush.color[2], "a": 1.0 },
        })),
    )?;
    Ok(())
}

fn build_sample_world() -> Result<World, WorldError> {
    let mut world = World::new(1);
    register_standard_tables(&mut world)?;

    let wall = [0.80, 0.78, 0.72];
    let bricks = [
        BrushSpec {
            name: "Piso",
            shape: "box",
            position: [0.0, -0.25, 0.0],
            size: [24.0, 0.5, 24.0],
            color: [0.55, 0.58, 0.62],
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Parede norte",
            shape: "box",
            position: [0.0, 2.0, -12.0],
            size: [24.0, 4.0, 0.5],
            color: wall,
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Parede sul",
            shape: "box",
            position: [0.0, 2.0, 12.0],
            size: [24.0, 4.0, 0.5],
            color: wall,
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Parede leste",
            shape: "box",
            position: [12.0, 2.0, 0.0],
            size: [0.5, 4.0, 24.0],
            color: wall,
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Parede oeste",
            shape: "box",
            position: [-12.0, 2.0, 0.0],
            size: [0.5, 4.0, 24.0],
            color: wall,
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Rampa",
            shape: "ramp",
            position: [-5.0, 0.75, -6.0],
            size: [4.0, 1.5, 6.0],
            color: [0.90, 0.60, 0.30],
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Plataforma",
            shape: "box",
            position: [-5.0, 0.75, -10.5],
            size: [4.0, 1.5, 3.0],
            color: [0.40, 0.60, 0.85],
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Pilar",
            shape: "cylinder",
            position: [5.0, 2.0, -4.0],
            size: [1.2, 4.0, 1.2],
            color: [0.70, 0.30, 0.35],
            rotation_degrees: [0.0; 3],
        },
        BrushSpec {
            name: "Caixa grande",
            shape: "box",
            position: [3.0, 0.5, 3.0],
            size: [1.0, 1.0, 1.0],
            color: [0.60, 0.45, 0.30],
            rotation_degrees: [0.0, 25.0, 0.0],
        },
        BrushSpec {
            name: "Caixa pequena",
            shape: "box",
            position: [6.0, 0.35, 5.0],
            size: [0.7, 0.7, 0.7],
            color: [0.55, 0.40, 0.28],
            rotation_degrees: [0.0, -15.0, 0.0],
        },
    ];
    for brush in &bricks {
        spawn_brush(&mut world, brush)?;
    }

    let start = world.spawn()?;
    world.insert_row_json(
        start,
        "Name",
        &object(json!({ "text": "Início do jogador" })),
    )?;
    world.insert_row_json(
        start,
        "Transform",
        &object(json!({ "position": vector([0.0, 1.7, 9.0]) })),
    )?;
    world.insert_row_json(start, "PlayerStart", &object(json!({ "yaw_degrees": 0.0 })))?;
    Ok(world)
}

/// Creates a new project folder with a small sample room (floor, walls, ramp, pillar, crates).
/// Refuses to overwrite an existing project.
pub fn create_sample_project(root: &Path, name: &str) -> Result<(), ProjectError> {
    create_sample_project_with(root, name, "aipo")
}

/// Creates the sample project with the programming method the person picked (DR-40).
pub fn create_sample_project_with(
    root: &Path,
    name: &str,
    programming_method: &str,
) -> Result<(), ProjectError> {
    let mut manifest = ProjectManifest::new(name, SCENE_PATH);
    manifest.programming_method = programming_method.to_string();
    if let Some(problem) = manifest.validate().first() {
        return Err(ProjectError::InvalidName(problem.message.clone()));
    }
    let manifest_path = root.join(MANIFEST_FILE);
    if manifest_path.exists() {
        return Err(ProjectError::AlreadyExists(root.display().to_string()));
    }

    let world = build_sample_world()?;
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| ProjectError::Serialize(error.to_string()))?;

    save_scene(&root.join(SCENE_PATH), &world)?;
    atomic_write(&manifest_path, format!("{manifest_text}\n").as_bytes())?;
    Ok(())
}
