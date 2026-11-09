//! The several scenes of a project: listing them, loading one into a fresh world and naming it.
//!
//! A project is a folder; `scenes/` holds one JSON file per scene (`*.scene.json`). The manifest
//! names the one that opens first (`default_scene`). The editor switches scenes without reopening
//! the project: the manifest, the scripts and the assets stay, only the world changes.
//!
//! Listing never follows symbolic links and never leaves `scenes/`; loading refuses a path outside
//! the project, a file too large and a format version this engine does not know.

use std::path::{Path, PathBuf};

use petunia_world::World;

use crate::check::{check_world, read_scene};
use crate::diagnostics::{Diagnostic, Report};
use crate::io::resolve_inside;
use crate::scene::load_entities;
use crate::standard::register_standard_tables;

pub const SCENES_DIRECTORY: &str = "scenes";
pub const SCENE_SUFFIX: &str = ".scene.json";
/// A project with more scenes than this is refused by the dialog, not by the engine; the list is
/// cut here so a hostile folder cannot exhaust memory.
const MAX_SCENES: usize = 200;
/// Seed used for the world of a scene until projects carry one (same rule as `load_project`).
const SCENE_SEED: i64 = 1;

/// One scene file on disk, as the editor's scene list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneSummary {
    /// Path relative to the project root, with `/` separators (`scenes/sala.scene.json`).
    pub path: String,
    /// The name without folder and suffix (`sala`).
    pub name: String,
    pub bytes: u64,
    /// Seconds since the Unix epoch; `None` when the file system does not say.
    pub modified_seconds: Option<u64>,
}

/// Result of loading one scene: the world (unless a problem stopped it) and what was found.
#[derive(Debug)]
pub struct SceneLoadOutcome {
    pub scene_path: PathBuf,
    pub world: Option<World>,
    pub report: Report,
}

fn modified_seconds(metadata: &std::fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs())
}

/// `scenes/sala.scene.json` → `sala`; anything else is returned unchanged.
pub fn scene_name(path: &str) -> String {
    let file = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    file.strip_suffix(SCENE_SUFFIX).unwrap_or(file).to_string()
}

/// The scenes of the project, sorted by name. A missing folder is an empty list, not an error:
/// a project may have no scene yet and still open (the Project Doctor explains it).
pub fn list_scenes(root: &Path) -> Vec<SceneSummary> {
    let folder = root.join(SCENES_DIRECTORY);
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut scenes = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        if scenes.len() >= MAX_SCENES {
            break;
        }
        let path = entry.path();
        // Symbolic links are not followed: a link could point outside the project.
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(SCENE_SUFFIX) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| format!("{SCENES_DIRECTORY}/{file_name}"));
        scenes.push(SceneSummary {
            path: relative,
            name: scene_name(file_name),
            bytes: metadata.len(),
            modified_seconds: modified_seconds(&metadata),
        });
    }
    scenes.sort_by(|left, right| left.name.cmp(&right.name));
    scenes
}

/// Loads one scene of the project into a fresh world. The path is relative to the project root
/// (`scenes/sala.scene.json`) and must stay inside it. Problems come back as diagnostics; the
/// world is `None` when one of them is an error, and the caller must not replace the open scene.
pub fn load_scene(root: &Path, relative: &str) -> SceneLoadOutcome {
    let mut report = Report::default();
    let path = match resolve_inside(root, relative) {
        Ok(path) => path,
        Err(error) => {
            report.push(Diagnostic::error(
                "scene.path.invalid",
                relative,
                &error.to_string(),
                "Use um caminho relativo dentro do projeto, como `scenes/sala.scene.json`.",
            ));
            return SceneLoadOutcome {
                scene_path: PathBuf::new(),
                world: None,
                report,
            };
        }
    };
    if !path.is_file() {
        report.push(Diagnostic::error(
            "scene.file.missing",
            relative,
            "essa cena não existe",
            "Escolha uma cena da lista ou crie uma nova.",
        ));
        return SceneLoadOutcome {
            scene_path: path,
            world: None,
            report,
        };
    }
    let scene = match read_scene(&path, relative) {
        Ok(scene) => scene,
        Err(diagnostic) => {
            report.push(diagnostic);
            return SceneLoadOutcome {
                scene_path: path,
                world: None,
                report,
            };
        }
    };

    let mut world = World::new(SCENE_SEED);
    if let Err(error) = register_standard_tables(&mut world) {
        report.push(Diagnostic::error(
            "engine.standard_tables",
            "engine",
            &format!("não foi possível registrar as tabelas padrão: {error}"),
            "Isso é um defeito da engine; registre o problema.",
        ));
        return SceneLoadOutcome {
            scene_path: path,
            world: None,
            report,
        };
    }
    load_entities(&mut world, &scene, &mut report);
    check_world(&world, &mut report);
    SceneLoadOutcome {
        scene_path: path,
        world: Some(world),
        report,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("petunia_scenes_{label}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join(SCENES_DIRECTORY)).expect("folder");
        path
    }

    fn write_scene(root: &Path, name: &str, entities: &str) {
        let text = format!(
            "{{\n  \"format_version\": 1,\n  \"next_persistent_id\": 2,\n  \"entities\": [{entities}]\n}}\n"
        );
        std::fs::write(root.join(SCENES_DIRECTORY).join(name), text).expect("scene");
    }

    #[test]
    fn the_list_has_only_scene_files_sorted_by_name() {
        let root = scratch("list");
        write_scene(&root, "sala.scene.json", "");
        write_scene(&root, "porao.scene.json", "");
        std::fs::write(root.join(SCENES_DIRECTORY).join("notas.json"), "{}").expect("notes");
        std::fs::create_dir_all(root.join(SCENES_DIRECTORY).join("pasta.scene.json")).expect("dir");

        let scenes = list_scenes(&root);
        let names: Vec<&str> = scenes.iter().map(|scene| scene.name.as_str()).collect();
        assert_eq!(names, vec!["porao", "sala"]);
        assert_eq!(scenes[0].path, "scenes/porao.scene.json");
        assert!(scenes[0].bytes > 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn opening_a_scene_loads_its_entities_and_a_missing_one_reports_instead_of_panicking() {
        let root = scratch("open");
        write_scene(
            &root,
            "sala.scene.json",
            "{\"id\": 1, \"rows\": {\"Name\": {\"text\": \"Caixa\"}}}",
        );
        let loaded = load_scene(&root, "scenes/sala.scene.json");
        // A scene without a PlayerStart is a warning, not a reason to refuse it.
        assert!(!loaded.report.has_errors(), "{:?}", loaded.report);
        let world = loaded.world.expect("world");
        assert_eq!(world.entity_count(), 1);

        let missing = load_scene(&root, "scenes/nao_existe.scene.json");
        assert!(missing.world.is_none());
        assert_eq!(missing.report.diagnostics[0].code, "scene.file.missing");

        let outside = load_scene(&root, "../fora.scene.json");
        assert!(outside.world.is_none());
        assert_eq!(outside.report.diagnostics[0].code, "scene.path.invalid");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_drop_the_folder_and_the_suffix() {
        assert_eq!(scene_name("scenes/sala.scene.json"), "sala");
        assert_eq!(scene_name("a/b/c.other.json"), "c.other.json");
    }
}
