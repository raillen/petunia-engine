//! Dragging a collider's handles in the viewport (phase 2 pendency).
//!
//! The collider already draws its lines and can be chosen; what was missing is resizing it with the
//! mouse. The shape of the gesture is the same as every other one in this editor: pick, move with a
//! **live preview** (not a command), and one command when the mouse is released — so `Ctrl+Z` takes
//! the whole resize back in one step.
//!
//! The preview writes to the collider row directly (like `gesture.preview` does for transforms), and
//! `collider.resize_cancel` puts the original values back. Nothing reaches the undo history until
//! the release.

use petunia_math::Vec3;
use petunia_world::World;
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

/// A resize in progress: the collider, its original row values and the handle being pulled.
pub(crate) struct ColliderResize {
    pub(crate) collider: u64,
    pub(crate) handle: ResizeHandle,
    /// `size` and `center` as they were before the drag, so a cancel restores them exactly.
    pub(crate) size: Vec3,
    pub(crate) center: Vec3,
}

/// Which part of the collider is being pulled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeHandle {
    /// A corner of the box: grows or shrinks along all axes.
    Size,
    /// The centre: moves the collider without changing its size.
    Center,
}

impl EditorSession {
    /// `collider.resize_begin`: starts pulling `handle` on the chosen collider. Returns false when
    /// there is nothing to pull, so the window can fall back to the usual selection.
    pub(crate) fn collider_resize_begin(&mut self, handle: ResizeHandle) -> Result<bool, Failure> {
        let selection = self.selection;
        let Some(collider) = selection else {
            return Ok(false);
        };
        let (size, center) = {
            let world = self.world()?;
            if crate::colliders::owner_of(world, collider).is_none() {
                return Ok(false);
            }
            let row = crate::colliders::row_of(world, collider).ok_or_else(|| {
                Failure::new(
                    "EDITOR_NOT_A_COLLIDER",
                    "o que está selecionado não é uma colisão",
                    "Clique numa colisão na Hierarquia.",
                )
            })?;
            (row.size, row.center)
        };
        self.collider_resize = Some(ColliderResize {
            collider,
            handle,
            size,
            center,
        });
        Ok(true)
    }

    /// `collider.resize_drag`: moves the handle to `pointer` and **previews** the result.
    ///
    /// The preview is a direct write to the world, not a command: dragging must not fill the undo
    /// history with one step per mouse move.
    pub(crate) fn collider_resize_drag(&mut self, delta: Vec3) -> Result<Json, Failure> {
        let Some(resize) = self.collider_resize.as_ref() else {
            return Ok(Json::Null);
        };
        if !delta.x.is_finite() || !delta.y.is_finite() || !delta.z.is_finite() {
            return Err(Failure::new(
                "EDITOR_BAD_NUMBER",
                "o movimento não é um número finito",
                "Tente de novo.",
            ));
        }
        // A collider never gets a zero or negative size: that would make the shape vanish.
        let minimum = 0.01;
        let (size, center) = match resize.handle {
            ResizeHandle::Size => (
                Vec3::new(
                    (resize.size.x + delta.x).max(minimum),
                    (resize.size.y + delta.y).max(minimum),
                    (resize.size.z + delta.z).max(minimum),
                ),
                resize.center,
            ),
            ResizeHandle::Center => (resize.size, resize.center + delta),
        };
        let collider = resize.collider;
        let world = self.world_mut()?;
        let handle = world.handle_of(petunia_world::PersistentId(collider))?;
        write_column(world, handle, "size", petunia_world::Value::Vec3(size))?;
        write_column(world, handle, "center", petunia_world::Value::Vec3(center))?;
        Ok(json!({
            "collider": collider,
            "size": { "x": size.x, "y": size.y, "z": size.z },
            "center": { "x": center.x, "y": center.y, "z": center.z },
        }))
    }

    /// `collider.resize_end`: commits the preview as **one** command, or leaves nothing behind when
    /// the handle did not move.
    pub(crate) fn collider_resize_end(&mut self) -> Result<Json, Failure> {
        let Some(resize) = self.collider_resize.take() else {
            return Ok(json!({ "changed": false }));
        };
        let (size, center) = {
            let world = self.world()?;
            let row = crate::colliders::row_of(world, resize.collider)
                .ok_or_else(crate::colliders::no_collider)?;
            (row.size, row.center)
        };
        self.collider_resize_restore(resize.collider, resize.size, resize.center)?;
        let unchanged = (size.x - resize.size.x).abs() < 1e-6
            && (size.y - resize.size.y).abs() < 1e-6
            && (size.z - resize.size.z).abs() < 1e-6
            && (center.x - resize.center.x).abs() < 1e-6
            && (center.y - resize.center.y).abs() < 1e-6
            && (center.z - resize.center.z).abs() < 1e-6;
        if unchanged {
            return Ok(json!({ "changed": false }));
        }
        self.execute(&petunia_commands::CommandRecord::new(
            "row.set",
            json!({
                "entity": resize.collider,
                "table": "Collider",
                "values": {
                    "size": { "x": size.x, "y": size.y, "z": size.z },
                    "center": { "x": center.x, "y": center.y, "z": center.z },
                }
            }),
        ))?;
        Ok(json!({ "changed": true, "collider": resize.collider }))
    }

    /// `collider.resize_cancel`: puts the row back exactly as the drag found it.
    pub(crate) fn collider_resize_cancel(&mut self) -> Result<Json, Failure> {
        let Some(resize) = self.collider_resize.take() else {
            return Ok(json!({ "cancelled": false }));
        };
        self.collider_resize_restore(resize.collider, resize.size, resize.center)?;
        Ok(json!({ "cancelled": true }))
    }

    /// Writes `size`/`center` back without touching the history.
    fn collider_resize_restore(
        &mut self,
        collider: u64,
        size: Vec3,
        center: Vec3,
    ) -> Result<(), Failure> {
        let world = self.world_mut()?;
        let handle = world.handle_of(petunia_world::PersistentId(collider))?;
        write_column(world, handle, "size", petunia_world::Value::Vec3(size))?;
        write_column(world, handle, "center", petunia_world::Value::Vec3(center))
    }
}

fn write_column(
    world: &mut World,
    handle: petunia_world::EntityHandle,
    column: &str,
    value: petunia_world::Value,
) -> Result<(), Failure> {
    world
        .set(handle, "Collider", column, value)
        .map(|_| ())
        .map_err(|error| {
            Failure::new(
                "EDITOR_COLLIDER_WRITE",
                &format!("não consegui mudar a colisão: {error}"),
                "Tente de novo ou ajuste no inspetor.",
            )
        })
}
