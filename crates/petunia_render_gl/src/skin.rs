//! Skinned meshes in the GL adapter (phase 4).
//!
//! A skinned mesh changes every frame, so it cannot live in the cache the static meshes use (that
//! cache is keyed by fingerprint and assumes the data never changes). Instead this module keeps a
//! **short-lived** buffer per draw item, rewritten each frame and freed when the item disappears.
//! The vertex layout is the same one the static path uses, so the shader is shared.
//!
//! Why the CPU and not a bone palette in the shader: the vertex has no joint index or weight
//! attribute yet, and adding two attributes to every mesh for the sake of the few that are skinned
//! would cost memory on all of them. Skinning on the CPU keeps the change local to this file; a GPU
//! path becomes worth it when a scene carries many animated characters at once.

use std::collections::HashMap;

use glow::HasContext;

use petunia_render::{DrawItem, MeshData, skin_mesh};

use crate::{GpuMesh, RenderError};

/// Cap on cached skinned meshes, so a hostile scene cannot ask for endless buffers.
pub const MAX_SKINNED: usize = 256;

/// Buffers of the skinned meshes of the last frame.
pub(crate) struct SkinCache {
    meshes: HashMap<u64, GpuMesh>,
}

impl SkinCache {
    pub(crate) fn new() -> Self {
        Self {
            meshes: HashMap::new(),
        }
    }

    /// The GPU mesh for this item's **current** pose, uploading only when the pose changed.
    ///
    /// `pose` is a cheap fingerprint the caller builds from the clip time and blend: two frames at
    /// the same time reuse the buffer instead of re-uploading the same vertices.
    pub(crate) fn mesh_for(
        &mut self,
        context: &glow::Context,
        item: &DrawItem,
        pose: u64,
    ) -> Result<Option<GpuMesh>, RenderError> {
        if item.skin.is_empty() || item.skin_matrices.is_empty() {
            return Ok(None);
        }
        let Some(skinned) = skin_mesh(item) else {
            return Ok(None);
        };
        if skinned.indices.is_empty() {
            return Ok(None);
        }
        // Key by both the item and the pose: the same object at a new time is a new buffer.
        let key = item
            .mesh_fingerprint
            .wrapping_mul(31)
            .wrapping_add(pose)
            .wrapping_add(item.entity);
        if !self.meshes.contains_key(&key) {
            if self.meshes.len() >= MAX_SKINNED {
                // Drop the buffers of the previous frame: a skinned mesh is regenerated anyway.
                self.meshes.clear();
            }
            let uploaded = upload_skinned(context, &skinned)?;
            self.meshes.insert(key, uploaded);
        }
        Ok(self.meshes.get(&key).copied())
    }

    /// Frees every skinned buffer (project closed, or context lost).
    pub(crate) fn clear(&mut self, context: &glow::Context) {
        for (_, mesh) in self.meshes.drain() {
            crate::delete_mesh(context, &mesh);
        }
    }
}

/// Uploads a skinned mesh with the vertex layout the static path uses.
///
/// `unsafe` because every `glow` call is: this crate is the OpenGL adapter, the one place the
/// engine allows it (`AGENTS.md` §2).
fn upload_skinned(context: &glow::Context, mesh: &MeshData) -> Result<GpuMesh, RenderError> {
    let mut vertex_bytes = Vec::with_capacity(mesh.positions.len() * 48);
    for value in mesh.interleaved() {
        vertex_bytes.extend_from_slice(&value.to_ne_bytes());
    }
    let mut index_bytes = Vec::with_capacity(mesh.indices.len() * 4);
    for index in &mesh.indices {
        index_bytes.extend_from_slice(&index.to_ne_bytes());
    }
    unsafe {
        let vertex_array = context.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = context.create_buffer().map_err(RenderError::Gl)?;
        let index_buffer = context.create_buffer().map_err(RenderError::Gl)?;
        context.bind_vertex_array(Some(vertex_array));
        context.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        context.buffer_data_u8_slice(glow::ARRAY_BUFFER, &vertex_bytes, glow::DYNAMIC_DRAW);
        context.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        context.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, &index_bytes, glow::DYNAMIC_DRAW);
        // Same interleaving as `upload_mesh`: position(3) normal(3) uv(2) flow(3) margin(1).
        let stride = 12 * 4;
        let layout = [
            (0u32, 3u32, 0i32),
            (1, 3, 12),
            (2, 2, 24),
            (3, 3, 32),
            (4, 1, 44),
        ];
        for (location, size, offset) in layout {
            context.enable_vertex_attrib_array(location);
            context.vertex_attrib_pointer_f32(
                location,
                size as i32,
                glow::FLOAT,
                false,
                stride,
                offset,
            );
        }
        context.bind_vertex_array(None);
        Ok(GpuMesh {
            vertex_array,
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as i32,
        })
    }
}
