use std::path::Path;

use petunia_world::World;

use crate::error::ProjectError;
use crate::io::{MAX_SCENE_BYTES, atomic_write};
use crate::scene::scene_from_world;

/// Writes the world as a scene file. Entities are sorted and keys are ordered, so the same world
/// always produces the same bytes (clean diffs). The write is atomic: a crash never leaves a
/// half-written scene. A scene too large to be opened again is refused before writing, so the
/// file on disk stays the last one that opens.
pub fn save_scene(path: &Path, world: &World) -> Result<(), ProjectError> {
    save_scene_within(path, world, MAX_SCENE_BYTES)
}

fn save_scene_within(path: &Path, world: &World, max_bytes: u64) -> Result<(), ProjectError> {
    let scene = scene_from_world(world)?;
    let text = serde_json::to_string_pretty(&scene)
        .map_err(|error| ProjectError::Serialize(error.to_string()))?;
    let bytes = format!("{text}\n");
    if bytes.len() as u64 > max_bytes {
        return Err(ProjectError::TooLarge {
            path: path.display().to_string(),
            max_bytes,
        });
    }
    atomic_write(path, bytes.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standard::register_standard_tables;
    use petunia_world::Value;

    #[test]
    fn a_scene_too_large_to_open_again_is_refused_and_the_old_file_stays() {
        let folder =
            std::env::temp_dir().join(format!("petunia_save_limit_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("folder");
        let path = folder.join("main.scene.json");
        let mut world = World::new(1);
        register_standard_tables(&mut world).expect("tables");
        save_scene_within(&path, &world, 4096).expect("small scene");
        let before = std::fs::read(&path).expect("read");
        for _ in 0..200 {
            let handle = world.spawn().expect("spawn");
            world
                .insert_row_values(handle, "Name", vec![Value::Text("x".repeat(100))])
                .expect("row");
        }
        let error = save_scene_within(&path, &world, 4096).expect_err("too large");
        assert!(
            matches!(
                error,
                ProjectError::TooLarge {
                    max_bytes: 4096,
                    ..
                }
            ),
            "{error}"
        );
        assert_eq!(std::fs::read(&path).expect("read"), before);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
