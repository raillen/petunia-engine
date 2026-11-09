use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::error::ProjectError;

pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// Editable meshes are saved inside the scene (DR-22), so a scene can be a few megabytes.
pub const MAX_SCENE_BYTES: u64 = 64 * 1024 * 1024;

fn io_error(path: &Path, error: &std::io::Error) -> ProjectError {
    ProjectError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    }
}

/// Reads a file, refusing anything larger than `max_bytes` (hostile input).
pub fn read_limited(path: &Path, max_bytes: u64) -> Result<Vec<u8>, ProjectError> {
    let too_large = || ProjectError::TooLarge {
        path: path.display().to_string(),
        max_bytes,
    };
    let metadata = fs::metadata(path).map_err(|error| io_error(path, &error))?;
    if metadata.len() > max_bytes {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|error| io_error(path, &error))?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, &error))?;
    if bytes.len() as u64 > max_bytes {
        Err(too_large())
    } else {
        Ok(bytes)
    }
}

/// Writes through a temporary file in the same folder and renames it into place, so a crash
/// never leaves a half-written project file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = parent.join(format!(".{file_name}.tmp"));
    let write = || -> std::io::Result<()> {
        let mut file = File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    };
    write().map_err(|error| {
        let _ = fs::remove_file(&temporary);
        io_error(path, &error)
    })
}

/// Joins a relative path to the project root, refusing absolute paths and `..`.
pub fn resolve_inside(root: &Path, relative: &str) -> Result<PathBuf, ProjectError> {
    let candidate = Path::new(relative);
    let only_normal = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    if relative.is_empty() || candidate.is_absolute() || !only_normal {
        return Err(ProjectError::InvalidPath(relative.to_string()));
    }
    Ok(root.join(candidate))
}

/// Reads one asset of the project as a PNG: `assets/hero.png` becomes its size and RGBA pixels.
///
/// `None` when the path escapes the project, the file is missing, empty, over the importer's
/// limit, or not a valid PNG. The rule lives here because both hosts (the editor and the player)
/// must read an asset the same way; the decoding limits are the importer's.
pub fn load_png_asset(root: &Path, relative: &str) -> Option<(u32, u32, Vec<u8>)> {
    let path = resolve_inside(root, relative).ok()?;
    let image = petunia_import::read_png_file(&path)?;
    Some((image.width, image.height, image.rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "petunia_project_io_test_{label}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("assets")).expect("a project folder");
        path
    }

    fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("a header");
            writer.write_image_data(rgba).expect("the pixels");
        }
        out
    }

    #[test]
    fn a_png_asset_of_the_project_is_read_and_an_outside_one_is_not() {
        let root = scratch_dir("png");
        let pixels = [1u8, 2, 3, 255, 4, 5, 6, 128];
        fs::write(
            root.join("assets").join("hero.png"),
            encode_png(2, 1, &pixels),
        )
        .expect("the image file");
        let loaded = load_png_asset(&root, "assets/hero.png").expect("a valid image");
        assert_eq!((loaded.0, loaded.1), (2, 1));
        assert_eq!(loaded.2, pixels);

        // Missing, empty and escaping paths are answers, not errors.
        assert!(load_png_asset(&root, "assets/none.png").is_none());
        fs::write(root.join("assets").join("empty.png"), b"").expect("the empty file");
        assert!(load_png_asset(&root, "assets/empty.png").is_none());
        assert!(load_png_asset(&root, "../secret.png").is_none());
        assert!(load_png_asset(&root, "/etc/passwd").is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_inside_rejects_escapes() {
        let root = Path::new("/projects/game");
        assert!(resolve_inside(root, "scenes/room.scene.json").is_ok());
        assert!(resolve_inside(root, "../secret").is_err());
        assert!(resolve_inside(root, "/etc/passwd").is_err());
        assert!(resolve_inside(root, "").is_err());
    }
}
