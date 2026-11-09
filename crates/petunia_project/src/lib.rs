//! The text project format.
//!
//! A project is a folder of plain JSON files (diff-friendly, Git-friendly):
//!
//! ```text
//! my_game/
//!   petunia.project.json     manifest (name, default scene, scripting language)
//!   scenes/room.scene.json   entities as rows of tables, with persistent ids
//! ```
//!
//! Files are hostile input: size, count and value limits apply, floats must be finite, paths
//! must stay inside the project. Problems are reported as stable-coded diagnostics that say
//! what to do, in the style of the Project Doctor.

mod check;
mod diagnostics;
mod error;
mod io;
mod manifest;
mod play;
mod sample;
mod save;
mod scene;
mod scenes;
mod standard;

pub use check::{LoadOutcome, LoadedProject, check_project, load_project};
pub use diagnostics::{Diagnostic, Report, Severity};
pub use error::ProjectError;
pub use io::{
    MAX_MANIFEST_BYTES, MAX_SCENE_BYTES, atomic_write, load_png_asset, read_limited, resolve_inside,
};
pub use manifest::{FORMAT_VERSION, MANIFEST_FILE, PROGRAMMING_METHODS, ProjectManifest};
pub use play::{EYE_HEIGHT, PlayerStart, ensure_player, has_player, player_start};
pub use sample::{create_sample_project, create_sample_project_with};
pub use save::save_scene;
pub use scene::{SceneEntity, SceneFile, scene_from_world};
pub use scenes::{
    SCENE_SUFFIX, SCENES_DIRECTORY, SceneLoadOutcome, SceneSummary, list_scenes, load_scene,
    scene_name,
};
pub use standard::{
    BODY_KINDS, BRUSH_SHAPES, COLLIDER_SHAPES, LIGHT_KINDS, register_standard_tables,
};
