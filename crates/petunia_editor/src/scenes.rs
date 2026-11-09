//! Several scenes per project, the automatic save and its recovery (DR-38).
//!
//! A project keeps one scene file per scene (`scenes/<nome>.scene.json`) and the manifest names the
//! one that opens first. Switching scenes does not reopen the project: the manifest, the scripts and
//! the `assets/` folder stay; only the world, the selection and the undo history change. Everything
//! goes through the same JSON operations as the rest of the editor.
//!
//! The automatic save writes `scenes/<nome>.scene.autosave.json` next to the scene (a different
//! file, atomic like a normal save), so a crash or a power cut never leaves a half-written scene.
//! Opening a project does not restore it by itself: the window asks, and the person chooses. Saving
//! the scene removes the automatic copy, because the file on disk is now the newest one.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use petunia_project::{
    SCENE_SUFFIX, SCENES_DIRECTORY, list_scenes, load_scene, register_standard_tables, save_scene,
    scene_name,
};
use petunia_world::World;
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::{EditorSession, live_edit_busy, new_dispatcher};

/// A scene name becomes a file name: letters, digits, `-`, `_` and single spaces kept as `-`.
const MAX_NAME_CHARACTERS: usize = 60;

fn failure(code: &str, message: &str, action: &str) -> Failure {
    Failure::new(code, message, action)
}

/// `Pedra do rio` → `Pedra-do-rio`; anything that could leave the folder or confuse a path is
/// dropped. The result is never empty.
fn clean_scene_name(name: &str) -> String {
    let mut cleaned = String::new();
    let mut last_was_dash = false;
    for character in name.trim().chars() {
        let keep = if character.is_alphanumeric() || matches!(character, '-' | '_') {
            character
        } else if character.is_whitespace() || character == '.' {
            '-'
        } else {
            continue;
        };
        if keep == '-' {
            if last_was_dash || cleaned.is_empty() {
                continue;
            }
            last_was_dash = true;
        } else {
            last_was_dash = false;
        }
        cleaned.push(keep);
        if cleaned.chars().count() >= MAX_NAME_CHARACTERS {
            break;
        }
    }
    let trimmed = cleaned.trim_end_matches('-');
    if trimmed.is_empty() {
        "cena".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The automatic copy that sits next to a scene: `sala.scene.json` → `sala.scene.autosave.json`.
fn autosave_path(scene_path: &Path) -> PathBuf {
    let file = scene_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cena.scene.json");
    let base = file.strip_suffix(".json").unwrap_or(file);
    scene_path.with_file_name(format!("{base}.autosave.json"))
}

fn seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn modified_seconds(path: &Path) -> Option<u64> {
    let metadata = std::fs::metadata(path).ok()?;
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs())
}

impl EditorSession {
    /// The current scene path relative to the project root (`scenes/sala.scene.json`).
    fn current_scene(&self) -> Result<(PathBuf, String), Failure> {
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let relative = project
            .scene_path
            .strip_prefix(&project.root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| project.scene_path.display().to_string());
        Ok((project.root.clone(), relative))
    }

    /// `project.scenes`: the scenes of the project and which one is open.
    pub fn scene_list(&self) -> Result<Json, Failure> {
        let (root, current) = self.current_scene()?;
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let mut scenes = Vec::new();
        for scene in list_scenes(&root) {
            scenes.push(json!({
                "path": scene.path,
                "name": scene.name,
                "bytes": scene.bytes,
                "modified_seconds": scene.modified_seconds,
                "current": scene.path == current,
            }));
        }
        Ok(json!({
            "scenes": scenes,
            "current": current,
            "current_name": scene_name(&current),
            "default": project.manifest.default_scene,
            "folder": SCENES_DIRECTORY,
        }))
    }

    /// `project.open_scene`: replaces the open scene with another one of the same project. The
    /// world, the selection and the undo history are replaced; the camera, the open graph and the
    /// project stay. A scene with errors is refused and the open one is untouched. Unsaved work in
    /// the open scene refuses the switch unless `force` says the person chose to discard it: an
    /// agent or a script must not throw work away by accident.
    pub fn open_scene(&mut self, relative: &str, force: bool) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        if self.has_live_edit() {
            return Err(live_edit_busy("trocar de cena"));
        }
        if self.dirty && !force {
            return Err(Failure::new(
                "SCENE_UNSAVED",
                "a cena aberta tem alterações não salvas",
                "Salve (Ctrl+S) ou confirme que quer descartar antes de trocar de cena.",
            ));
        }
        let (root, _current) = self.current_scene()?;
        if !relative.ends_with(SCENE_SUFFIX) {
            return Err(failure(
                "SCENE_INVALID_PATH",
                "só arquivos de cena podem ser abertos",
                "Escolha uma cena da lista (termina em .scene.json).",
            ));
        }
        let loaded = load_scene(&root, relative);
        self.report = loaded.report;
        let Some(world) = loaded.world else {
            return Err(self.first_report_failure(
                "SCENE_NOT_LOADED",
                "não foi possível abrir a cena",
                "Veja os diagnósticos.",
            ));
        };
        if self.report.has_errors() {
            return Err(self.first_report_failure(
                "SCENE_HAS_ERRORS",
                "a cena tem problemas que impedem abri-la",
                "Corrija o que os diagnósticos apontam e tente de novo.",
            ));
        }
        self.install_world(world, loaded.scene_path, false)?;
        let name = scene_name(relative);
        self.record("project", &format!("abrir cena {name}"), true, relative);
        Ok(json!({ "name": name, "path": relative }))
    }

    /// `project.new_scene`: writes an empty scene next to the others and opens it. An existing
    /// file is never overwritten.
    pub fn new_scene(&mut self, name: &str, force: bool) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        if self.has_live_edit() {
            return Err(live_edit_busy("criar uma cena"));
        }
        if self.dirty && !force {
            return Err(Failure::new(
                "SCENE_UNSAVED",
                "a cena aberta tem alterações não salvas",
                "Salve (Ctrl+S) antes de criar outra cena.",
            ));
        }
        let (root, _current) = self.current_scene()?;
        let cleaned = clean_scene_name(name);
        let relative = format!("{SCENES_DIRECTORY}/{cleaned}.scene.json");
        let path = root.join(&relative);
        if path.exists() {
            return Err(failure(
                "SCENE_ALREADY_EXISTS",
                &format!("já existe uma cena chamada `{cleaned}`"),
                "Escolha outro nome ou abra a cena existente.",
            ));
        }
        let mut world = World::new(1);
        register_standard_tables(&mut world)?;
        save_scene(&path, &world)?;
        self.open_scene(&relative, true)?;
        self.record("project", &format!("criar cena {cleaned}"), true, &relative);
        Ok(json!({ "name": cleaned, "path": relative }))
    }

    /// The automatic copy of the open scene, written next to it. It only exists while there is
    /// something newer than the scene file.
    pub fn autosave(&mut self) -> Result<Json, Failure> {
        if self.has_live_edit() {
            return Err(live_edit_busy("salvar automaticamente"));
        }
        let (_, relative) = self.current_scene()?;
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let path = autosave_path(&project.scene_path);
        if !self.dirty {
            return Ok(json!({ "saved": false, "path": relative, "reason": "nada mudou" }));
        }
        save_scene(&path, &project.world)?;
        let saved_at = seconds_now();
        let auto_relative = path
            .strip_prefix(&project.root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.display().to_string());
        Ok(json!({
            "saved": true,
            "path": auto_relative,
            "saved_at": saved_at,
            "scene": relative,
        }))
    }

    /// `project.recovery`: is there an automatic copy, when was it written and is it newer than
    /// the scene on disk?
    pub fn recovery_info(&self) -> Result<Json, Failure> {
        let (_, relative) = self.current_scene()?;
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let path = autosave_path(&project.scene_path);
        let exists = path.is_file();
        let saved_at = if exists {
            modified_seconds(&path)
        } else {
            None
        };
        let scene_saved_at = modified_seconds(&project.scene_path);
        // File systems report whole seconds (some report less), so a copy written in the same
        // second as the scene counts as newer: offering recovery costs nothing, hiding it can
        // lose work. A real save deletes the copy, so an old copy never points at newer work.
        let newer = match (saved_at, scene_saved_at) {
            (Some(auto), Some(scene)) => auto >= scene,
            (Some(_), None) => true,
            _ => false,
        };
        let relative_path = path
            .strip_prefix(&project.root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.display().to_string());
        Ok(json!({
            "exists": exists,
            "path": relative_path,
            "saved_at": saved_at,
            "scene_saved_at": scene_saved_at,
            "newer": newer,
            "scene": relative,
        }))
    }

    /// `project.recover`: loads the automatic copy over the open scene. The scene file is not
    /// touched; the session stays dirty until the person saves, so recovery is always undoable by
    /// not saving.
    pub fn recover(&mut self) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        if self.has_live_edit() {
            return Err(live_edit_busy("recuperar o trabalho"));
        }
        if self.dirty {
            return Err(Failure::new(
                "SCENE_UNSAVED",
                "a cena aberta tem alterações não salvas",
                "Salve (Ctrl+S) antes de recuperar a cópia automática.",
            ));
        }
        let (root, relative) = self.current_scene()?;
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let path = autosave_path(&project.scene_path);
        if !path.is_file() {
            return Err(failure(
                "SCENE_NO_RECOVERY",
                "não há trabalho automático para recuperar",
                "A cópia automática já foi usada ou descartada.",
            ));
        }
        let autosave_relative = path
            .strip_prefix(&root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.display().to_string());
        let loaded = load_scene(&root, &autosave_relative);
        self.report = loaded.report;
        let Some(world) = loaded.world else {
            return Err(self.first_report_failure(
                "SCENE_NOT_LOADED",
                "a cópia automática não pôde ser lida",
                "Descarte a cópia e abra a cena salva.",
            ));
        };
        if self.report.has_errors() {
            return Err(self.first_report_failure(
                "SCENE_HAS_ERRORS",
                "a cópia automática tem problemas",
                "Descarte a cópia e abra a cena salva.",
            ));
        }
        self.install_world(world, root.join(&relative), true)?;
        self.record("project", "recuperar trabalho automático", true, &relative);
        Ok(json!({ "path": relative, "recovered_from": autosave_relative }))
    }

    /// `project.recovery_discard`: throws the automatic copy away.
    pub fn recovery_discard(&mut self) -> Result<Json, Failure> {
        let (_, relative) = self.current_scene()?;
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let path = autosave_path(&project.scene_path);
        let removed = match std::fs::remove_file(&path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(Failure::new(
                    "SCENE_RECOVERY_IO",
                    &format!("não foi possível descartar a cópia automática: {error}"),
                    "Confira se a pasta do projeto pode ser escrita.",
                ));
            }
        };
        if removed {
            self.record("project", "descartar trabalho automático", true, &relative);
        }
        Ok(json!({ "removed": removed }))
    }

    /// Puts a freshly loaded world in place and forgets everything bound to the old one.
    fn install_world(
        &mut self,
        world: World,
        scene_path: PathBuf,
        keep_dirty: bool,
    ) -> Result<(), Failure> {
        self.reset_live_edit();
        self.dispatcher = new_dispatcher()?;
        self.selection = None;
        self.floor = None;
        self.components = None;
        self.mesh_edit_mode = false;
        if let Some(project) = self.project.as_mut() {
            project.world = world;
            project.scene_path = scene_path;
        }
        self.dirty = keep_dirty;
        self.refresh_script_diagnostics();
        Ok(())
    }

    /// Deletes the automatic copy after a real save: the scene on disk is the newest file now.
    pub(crate) fn forget_autosave(&self) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let _ = std::fs::remove_file(autosave_path(&project.scene_path));
    }

    /// The first error of the report as a failure the window can show.
    fn first_report_failure(&self, code: &str, message: &str, action: &str) -> Failure {
        match self
            .report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == petunia_project::Severity::Error)
        {
            Some(first) => Failure::new(&first.code, &first.message, &first.action),
            None => Failure::new(code, message, action),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_safe_file_names() {
        assert_eq!(clean_scene_name("Sala de estar"), "Sala-de-estar");
        // Accented letters are letters: they become part of the file name, not a refusal.
        assert_eq!(clean_scene_name("Ação e coração"), "Ação-e-coração");
        assert_eq!(clean_scene_name("  a/b\\c  "), "abc");
        assert_eq!(clean_scene_name(".."), "cena");
        assert_eq!(clean_scene_name(""), "cena");
        assert_eq!(clean_scene_name("a---b"), "a-b");
        assert_eq!(clean_scene_name(&"x".repeat(200)).chars().count(), 60);
    }

    #[test]
    fn the_automatic_copy_lives_next_to_the_scene_with_its_own_name() {
        let scene = Path::new("/proj/scenes/sala.scene.json");
        assert_eq!(
            autosave_path(scene),
            Path::new("/proj/scenes/sala.scene.autosave.json")
        );
        // The automatic copy is not a scene, so the scene list never shows it.
        assert!(
            !autosave_path(scene)
                .to_string_lossy()
                .ends_with(SCENE_SUFFIX)
        );
    }
}
