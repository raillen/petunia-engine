use std::path::Path;

use petunia_world::World;

use crate::error::ProjectError;
use crate::io::atomic_write;
use crate::scene::scene_from_world;

/// Writes the world as a scene file. Entities are sorted and keys are ordered, so the same world
/// always produces the same bytes (clean diffs). The write is atomic: a crash never leaves a
/// half-written scene.
pub fn save_scene(path: &Path, world: &World) -> Result<(), ProjectError> {
    let scene = scene_from_world(world)?;
    let text = serde_json::to_string_pretty(&scene)
        .map_err(|error| ProjectError::Serialize(error.to_string()))?;
    atomic_write(path, format!("{text}\n").as_bytes())
}
