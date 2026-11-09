//! Drawing the vegetation and the precipitation of a frame (phase 4 and 5).
//!
//! Both arrive as one `MeshData` in **world coordinates**: the flora crate decides where plants go
//! and the particle system decides where the drops are, so this module only puts them on the GPU.
//!
//! They change with the camera (impostors turn) and with time (drops fall), so they cannot use the
//! static cache keyed by fingerprint. The mesh is rebuilt by the frame builder and uploaded here on
//! a **pose fingerprint**: the same mesh twice in a row reuses the buffer, which keeps a still
//! forest free.

use std::sync::Arc;

use glow::HasContext;
use petunia_render::MeshData;

use crate::GpuMesh;
use crate::RenderError;

/// A buffer of one changing world mesh (vegetation or precipitation), rebuilt when its content
/// fingerprint changes.
#[derive(Default)]
pub(crate) struct WorldMesh {
    mesh: Option<GpuMesh>,
    fingerprint: u64,
}

impl WorldMesh {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The GPU mesh for `data`, uploading only when the fingerprint differs.
    pub(crate) fn update(
        &mut self,
        gl: &glow::Context,
        data: &Arc<MeshData>,
        fingerprint: u64,
    ) -> Result<Option<GpuMesh>, RenderError> {
        if self.mesh.is_none() || self.fingerprint != fingerprint {
            if let Some(old) = self.mesh.take() {
                delete(gl, &old);
            }
            self.mesh = Some(upload(gl, data)?);
            self.fingerprint = fingerprint;
        }
        Ok(self.mesh)
    }

    /// Frees the buffer (project closed, or the feature turned off).
    pub(crate) fn clear(&mut self, gl: &glow::Context) {
        if let Some(mesh) = self.mesh.take() {
            delete(gl, &mesh);
        }
        self.fingerprint = 0;
    }
}

fn delete(gl: &glow::Context, mesh: &GpuMesh) {
    crate::delete_mesh(gl, mesh);
}

/// Uploads a world mesh with the layout the static path uses.
fn upload(gl: &glow::Context, data: &MeshData) -> Result<GpuMesh, RenderError> {
    if data.indices.is_empty() {
        // An empty mesh still gets a buffer, so the caller never has to special-case it.
        return upload_empty(gl);
    }
    let mut vertex_bytes = Vec::with_capacity(data.positions.len() * 48);
    for value in data.interleaved() {
        vertex_bytes.extend_from_slice(&value.to_ne_bytes());
    }
    let mut index_bytes = Vec::with_capacity(data.indices.len() * 4);
    for index in &data.indices {
        index_bytes.extend_from_slice(&index.to_ne_bytes());
    }
    unsafe {
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        let index_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        gl.bind_vertex_array(Some(vertex_array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &vertex_bytes, glow::DYNAMIC_DRAW);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, &index_bytes, glow::DYNAMIC_DRAW);
        let stride = 12 * 4;
        let layout = [
            (0u32, 3u32, 0i32),
            (1, 3, 12),
            (2, 2, 24),
            (3, 3, 32),
            (4, 1, 44),
        ];
        for (location, size, offset) in layout {
            gl.enable_vertex_attrib_array(location);
            gl.vertex_attrib_pointer_f32(location, size as i32, glow::FLOAT, false, stride, offset);
        }
        gl.bind_vertex_array(None);
        Ok(GpuMesh {
            vertex_array,
            vertex_buffer,
            index_buffer,
            index_count: data.indices.len() as i32,
        })
    }
}

fn upload_empty(gl: &glow::Context) -> Result<GpuMesh, RenderError> {
    unsafe {
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        let index_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        gl.bind_vertex_array(Some(vertex_array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &[], glow::DYNAMIC_DRAW);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, &[], glow::DYNAMIC_DRAW);
        gl.bind_vertex_array(None);
        Ok(GpuMesh {
            vertex_array,
            vertex_buffer,
            index_buffer,
            index_count: 0,
        })
    }
}

/// A cheap, stable fingerprint of a mesh's content: FNV-1a over the floats as bits.
///
/// Two meshes with the same triangles give the same number, so a still forest reuses its buffer
/// instead of re-uploading every frame.
#[must_use]
pub(crate) fn fingerprint(mesh: &MeshData) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(mesh.positions.len() as u64);
    mix(mesh.indices.len() as u64);
    for position in &mesh.positions {
        for value in position {
            mix(u64::from(value.to_bits()));
        }
    }
    for index in &mesh.indices {
        mix(u64::from(*index));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fingerprint must follow the data, not the pointer: two meshes with the same triangles
    /// share a buffer, and one vertex more is a new upload.
    #[test]
    fn the_fingerprint_follows_the_content() {
        let first = MeshData {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            indices: vec![0, 1],
            ..MeshData::default()
        };
        let same = first.clone();
        let bigger = MeshData {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            ..MeshData::default()
        };
        assert_eq!(fingerprint(&first), fingerprint(&same));
        assert_ne!(fingerprint(&first), fingerprint(&bigger));
    }

    /// A camera turn moves every impostor, so the fingerprint must change with the vertices.
    #[test]
    fn a_moved_mesh_gets_another_fingerprint() {
        let mut mesh = MeshData {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            indices: vec![0, 1],
            ..MeshData::default()
        };
        let before = fingerprint(&mesh);
        if let Some(position) = mesh.positions.get_mut(0) {
            position[0] = 0.5;
        }
        assert_ne!(before, fingerprint(&mesh));
    }
}
