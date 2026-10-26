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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_inside_rejects_escapes() {
        let root = Path::new("/projects/game");
        assert!(resolve_inside(root, "scenes/room.scene.json").is_ok());
        assert!(resolve_inside(root, "../secret").is_err());
        assert!(resolve_inside(root, "/etc/passwd").is_err());
        assert!(resolve_inside(root, "").is_err());
    }
}
