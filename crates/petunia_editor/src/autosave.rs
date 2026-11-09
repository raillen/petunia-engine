//! Autosave (fase 2, pendência declarada): never lose work.
//!
//! The editor cannot save while a live edit (a drag, a sculpt stroke, a path) is changing the world
//! without a command — the file would keep a transient preview. So autosave is **asked**, and the
//! session decides: it saves when there is something unsaved and nothing live, and otherwise says
//! why it waited. The clock lives in the window (a `QTimer`), not here, so the engine stays free of
//! wall-clock reads and the tests drive it explicitly.

use serde_json::{Value as Json, json};

use crate::session::EditorSession;

impl EditorSession {
    /// `editor.autosave`: saves when there is unsaved work and no live edit. Returns what it did,
    /// so the window can tell the person instead of guessing.
    ///
    /// A refused save is never an error: nothing was lost, the file simply stayed as it was.
    pub fn autosave(&mut self) -> Json {
        let Some(project) = self.project.as_ref() else {
            return json!({ "saved": false, "reason": "no_project" });
        };
        if !self.dirty {
            return json!({ "saved": false, "reason": "clean" });
        }
        let path = project.scene_path.display().to_string();
        if self.has_live_edit() {
            return json!({ "saved": false, "reason": "busy", "path": path });
        }
        match self.save() {
            Ok(result) => json!({
                "saved": true,
                "reason": "saved",
                "path": result.get("path").cloned().unwrap_or(Json::Null),
            }),
            Err(failure) => json!({
                "saved": false,
                "reason": "refused",
                "problem": failure.message,
                "path": path,
            }),
        }
    }
}
