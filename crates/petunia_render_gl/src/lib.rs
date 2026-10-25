//! OpenGL 3.3 core adapter.
//!
//! Draws a [`RenderFrame`] with one shader (diffuse light plus gentle fog). The renderer owns
//! every GPU object it creates and deletes them on drop; callers only hold the renderer. No
//! compute shaders, so the same feature set can later run on WebGL2.

// glow exposes OpenGL as `unsafe` functions; this adapter is where that is allowed.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use glow::HasContext;
use petunia_render::{
    DrawItem, Mat4, MeshData, OverlayItem, RenderFrame, SculptItem, Shape, axis_lines, build_mesh,
    grid_lines, model_matrix,
};

const VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
uniform mat4 u_view_projection;
uniform mat4 u_model;
out vec3 v_normal;
out vec3 v_world_position;
void main() {
    vec4 world = u_model * vec4(a_position, 1.0);
    v_world_position = world.xyz;
    v_normal = transpose(inverse(mat3(u_model))) * a_normal;
    gl_Position = u_view_projection * world;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec3 v_normal;
in vec3 v_world_position;
uniform vec4 u_color;
uniform vec3 u_camera_position;
uniform vec4 u_tint;
uniform int u_faceted;
out vec4 out_color;
void main() {
    vec3 normal = normalize(v_normal);
    if (u_faceted != 0) {
        // Low poly: one normal per triangle, from the slope of the surface on screen.
        vec3 flat_normal = normalize(cross(dFdx(v_world_position), dFdy(v_world_position)));
        normal = dot(flat_normal, normal) < 0.0 ? -flat_normal : flat_normal;
    }
    vec3 sun = normalize(vec3(0.4, 0.8, 0.5));
    float diffuse = max(dot(normal, sun), 0.0);
    float ambient = 0.35 + 0.15 * normal.y;
    vec3 lit = u_color.rgb * (ambient + 0.65 * diffuse);
    float fog = clamp(length(v_world_position - u_camera_position) / 200.0, 0.0, 1.0);
    lit = mix(lit, vec3(0.53, 0.70, 0.88), fog * 0.6);
    lit = mix(lit, u_tint.rgb, u_tint.a);
    out_color = vec4(lit, u_color.a);
}
"#;

const OVERLAY_VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec4 a_color;
out vec4 v_color;
void main() {
    v_color = a_color;
    gl_Position = vec4(a_position, 0.0, 1.0);
}
"#;

const OVERLAY_FRAGMENT_SHADER: &str = r#"#version 330 core
in vec4 v_color;
out vec4 out_color;
void main() {
    out_color = v_color;
}
"#;

/// Most overlay vertices drawn in one frame (a hostile caller cannot ask for endless geometry).
const MAX_OVERLAY_VERTICES: usize = 60_000;

/// Upper bound on cached meshes (a hostile scene could ask for endless unique sizes).
const MAX_CACHED_MESHES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    ShaderCompile(String),
    ProgramLink(String),
    Gl(String),
    Mesh(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShaderCompile(log) => write!(formatter, "falha ao compilar o shader: {log}"),
            Self::ProgramLink(log) => {
                write!(formatter, "falha ao ligar o programa de shader: {log}")
            }
            Self::Gl(message) => write!(formatter, "erro de OpenGL: {message}"),
            Self::Mesh(message) => write!(formatter, "malha inválida: {message}"),
        }
    }
}

impl std::error::Error for RenderError {}

struct Uniforms {
    view_projection: Option<glow::UniformLocation>,
    model: Option<glow::UniformLocation>,
    color: Option<glow::UniformLocation>,
    camera_position: Option<glow::UniformLocation>,
    tint: Option<glow::UniformLocation>,
    faceted: Option<glow::UniformLocation>,
}

/// Tint of the selected object and of the object under the mouse: color and amount.
const SELECTION_TINT: [f32; 4] = [1.0, 0.62, 0.10, 0.28];
const HOVER_TINT: [f32; 4] = [0.49, 0.86, 1.0, 0.16];
/// Objects with less opacity than this are drawn in the translucent pass.
const OPAQUE_ALPHA: f32 = 0.999;

const GRID_VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec4 a_color;
uniform mat4 u_view_projection;
uniform float u_height;
out vec4 v_color;
out vec3 v_world_position;
void main() {
    v_color = a_color;
    v_world_position = a_position + vec3(0.0, u_height, 0.0);
    gl_Position = u_view_projection * vec4(v_world_position, 1.0);
}
"#;

const GRID_FRAGMENT_SHADER: &str = r#"#version 330 core
in vec4 v_color;
in vec3 v_world_position;
uniform vec3 u_camera_position;
out vec4 out_color;
void main() {
    float distance_to_camera = length(v_world_position - u_camera_position);
    float fade = 1.0 - smoothstep(18.0, 48.0, distance_to_camera);
    out_color = vec4(v_color.rgb, v_color.a * fade);
}
"#;

/// Half size of the grid in meters, and its spacing.
const GRID_EXTENT: f32 = 50.0;
const GRID_SPACING: f32 = 1.0;

/// The ground grid: built once, drawn when the frame asks for it.
struct GridPass {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
    vertex_count: i32,
    view_projection: Option<glow::UniformLocation>,
    camera_position: Option<glow::UniformLocation>,
    height: Option<glow::UniformLocation>,
}

/// The pixel-space pass for gizmos: its own program and a streaming vertex buffer.
struct OverlayPass {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
}

struct GpuMesh {
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
    index_buffer: glow::Buffer,
    index_count: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MeshKey {
    shape: Shape,
    size_bits: [u32; 3],
    detail: u32,
}

impl MeshKey {
    fn of(item: &DrawItem) -> Self {
        Self {
            shape: item.shape,
            size_bits: [
                item.size.x.to_bits(),
                item.size.y.to_bits(),
                item.size.z.to_bits(),
            ],
            detail: item.detail,
        }
    }
}

/// Owns the shader program and the cached meshes.
pub struct GlRenderer {
    gl: Arc<glow::Context>,
    program: glow::Program,
    uniforms: Uniforms,
    meshes: HashMap<MeshKey, GpuMesh>,
    /// One buffer per sculpted chunk, by the chunk's mesh id (new on every re-mesh).
    sculpt_meshes: HashMap<u64, GpuMesh>,
    grid_pass: GridPass,
    overlay_pass: OverlayPass,
}

impl GlRenderer {
    /// Creates the renderer on a current OpenGL 3.3 core context.
    pub fn new(gl: Arc<glow::Context>) -> Result<Self, RenderError> {
        let program = compile_program(&gl)?;
        // SAFETY: the context is current on this thread (the caller's contract).
        let uniforms = unsafe {
            Uniforms {
                view_projection: gl.get_uniform_location(program, "u_view_projection"),
                model: gl.get_uniform_location(program, "u_model"),
                color: gl.get_uniform_location(program, "u_color"),
                camera_position: gl.get_uniform_location(program, "u_camera_position"),
                tint: gl.get_uniform_location(program, "u_tint"),
                faceted: gl.get_uniform_location(program, "u_faceted"),
            }
        };
        let grid_pass = create_grid_pass(&gl)?;
        let overlay_pass = create_overlay_pass(&gl)?;
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.front_face(glow::CCW);
        }
        Ok(Self {
            gl,
            program,
            uniforms,
            meshes: HashMap::new(),
            sculpt_meshes: HashMap::new(),
            grid_pass,
            overlay_pass,
        })
    }

    /// Draws one frame into the currently bound framebuffer.
    pub fn render(&mut self, frame: &RenderFrame, viewport: (u32, u32)) -> Result<(), RenderError> {
        let (width, height) = viewport;
        if width == 0 || height == 0 {
            return Ok(());
        }
        let aspect_ratio = width as f32 / height as f32;
        let view_projection = frame.camera.view_projection(aspect_ratio);
        let camera = frame.camera.position;
        let clear = frame.clear_color;
        unsafe {
            // A host (Qt) may draw with this context between frames: set the state again.
            self.gl.enable(glow::DEPTH_TEST);
            self.gl.depth_mask(true);
            self.gl.enable(glow::CULL_FACE);
            self.gl.cull_face(glow::BACK);
            self.gl.front_face(glow::CCW);
            self.gl.disable(glow::BLEND);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.clear_color(clear.r, clear.g, clear.b, clear.a);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            self.gl.use_program(Some(self.program));
            self.gl.uniform_matrix_4_f32_slice(
                self.uniforms.view_projection.as_ref(),
                false,
                &view_projection,
            );
            self.gl.uniform_3_f32(
                self.uniforms.camera_position.as_ref(),
                camera.x,
                camera.y,
                camera.z,
            );
        }

        // Opaque first; translucent objects (ghosted floors) after, far to near, without
        // writing depth so they never hide what is behind them.
        let (opaque, mut translucent): (Vec<&DrawItem>, Vec<&DrawItem>) = frame
            .items
            .iter()
            .partition(|item| item.color.a >= OPAQUE_ALPHA);
        for item in opaque {
            self.draw_item(item)?;
        }
        for sculpt in &frame.sculpts {
            self.draw_sculpt(sculpt)?;
        }
        // Only the chunks of this frame stay on the GPU: sculpting re-meshes the chunks around the
        // brush on every dab, and the old buffers must not pile up.
        let current: std::collections::HashSet<u64> = frame
            .sculpts
            .iter()
            .flat_map(|sculpt| sculpt.chunks.iter().map(|chunk| chunk.id))
            .collect();
        let stale: Vec<u64> = self
            .sculpt_meshes
            .keys()
            .filter(|id| !current.contains(id))
            .copied()
            .collect();
        for id in stale {
            if let Some(mesh) = self.sculpt_meshes.remove(&id) {
                delete_mesh(&self.gl, &mesh);
            }
        }
        if frame.show_grid {
            self.draw_grid(&view_projection, camera, frame.grid_height);
        }
        if !translucent.is_empty() {
            let distance = |item: &DrawItem| (item.transform.position - camera).length_squared();
            translucent.sort_by(|first, second| distance(second).total_cmp(&distance(first)));
            unsafe {
                self.gl.use_program(Some(self.program));
                self.gl.enable(glow::BLEND);
                self.gl
                    .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                self.gl.depth_mask(false);
            }
            for item in translucent {
                self.draw_item(item)?;
            }
            unsafe {
                self.gl.depth_mask(true);
                self.gl.disable(glow::BLEND);
            }
        }
        self.draw_overlay(&frame.overlay, &view_projection, (width, height));
        unsafe { self.gl.bind_vertex_array(None) };
        Ok(())
    }

    /// Draws one object with the main program (bound by the caller), building its mesh once.
    fn draw_item(&mut self, item: &DrawItem) -> Result<(), RenderError> {
        let key = MeshKey::of(item);
        if !self.meshes.contains_key(&key) {
            if self.meshes.len() >= MAX_CACHED_MESHES {
                self.clear_meshes();
            }
            let data = build_mesh(item.shape, item.size, item.detail)
                .map_err(|error| RenderError::Mesh(error.to_string()))?;
            let uploaded = upload_mesh(&self.gl, &data)?;
            self.meshes.insert(key, uploaded);
        }
        let Some(mesh) = self.meshes.get(&key) else {
            return Ok(());
        };
        self.draw_mesh(
            mesh,
            &item.transform,
            item.color,
            item.selected,
            item.hovered,
            false,
        );
        Ok(())
    }

    fn draw_grid(&self, view_projection: &Mat4, camera: petunia_math::Vec3, height: f32) {
        unsafe {
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl.depth_func(glow::LEQUAL);
            self.gl.depth_mask(false);
            self.gl.use_program(Some(self.grid_pass.program));
            self.gl.uniform_matrix_4_f32_slice(
                self.grid_pass.view_projection.as_ref(),
                false,
                view_projection,
            );
            self.gl
                .uniform_1_f32(self.grid_pass.height.as_ref(), height);
            self.gl.uniform_3_f32(
                self.grid_pass.camera_position.as_ref(),
                camera.x,
                camera.y,
                camera.z,
            );
            self.gl.bind_vertex_array(Some(self.grid_pass.vertex_array));
            self.gl
                .draw_arrays(glow::LINES, 0, self.grid_pass.vertex_count);
            self.gl.bind_vertex_array(None);
            self.gl.depth_mask(true);
            self.gl.depth_func(glow::LESS);
            self.gl.disable(glow::BLEND);
            self.gl.use_program(Some(self.program));
        }
    }

    /// Draws gizmos on top of everything: thick lines and squares measured in pixels.
    fn draw_overlay(&self, items: &[OverlayItem], view_projection: &Mat4, viewport: (u32, u32)) {
        if items.is_empty() {
            return;
        }
        let vertices = overlay_vertices(items, view_projection, viewport);
        if vertices.is_empty() {
            return;
        }
        let mut bytes = Vec::with_capacity(vertices.len() * 4);
        for value in &vertices {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        unsafe {
            self.gl.disable(glow::DEPTH_TEST);
            self.gl.disable(glow::CULL_FACE);
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl.use_program(Some(self.overlay_pass.program));
            self.gl
                .bind_vertex_array(Some(self.overlay_pass.vertex_array));
            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.overlay_pass.vertex_buffer));
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STREAM_DRAW);
            self.gl.draw_arrays(
                glow::TRIANGLES,
                0,
                (vertices.len() / OVERLAY_FLOATS_PER_VERTEX) as i32,
            );
            self.gl.bind_vertex_array(None);
            self.gl.disable(glow::BLEND);
            self.gl.enable(glow::DEPTH_TEST);
            self.gl.enable(glow::CULL_FACE);
            self.gl.use_program(Some(self.program));
        }
    }

    /// Draws the chunks of one sculpted volume, uploading each the first time it is seen.
    fn draw_sculpt(&mut self, sculpt: &SculptItem) -> Result<(), RenderError> {
        for chunk in &sculpt.chunks {
            if !self.sculpt_meshes.contains_key(&chunk.id) {
                let data = MeshData {
                    positions: chunk.mesh.positions.clone(),
                    normals: chunk.mesh.normals.clone(),
                    indices: chunk.mesh.indices.clone(),
                };
                let uploaded = upload_mesh(&self.gl, &data)?;
                self.sculpt_meshes.insert(chunk.id, uploaded);
            }
            if let Some(mesh) = self.sculpt_meshes.get(&chunk.id) {
                self.draw_mesh(
                    mesh,
                    &sculpt.transform,
                    sculpt.color,
                    sculpt.selected,
                    sculpt.hovered,
                    sculpt.faceted,
                );
            }
        }
        Ok(())
    }

    /// Sets the per-object uniforms and draws a mesh with the main program (bound by the caller).
    fn draw_mesh(
        &self,
        mesh: &GpuMesh,
        transform: &petunia_math::Transform,
        color: petunia_math::Color,
        selected: bool,
        hovered: bool,
        faceted: bool,
    ) {
        let model = model_matrix(transform);
        unsafe {
            let [r, g, b, amount] = if selected {
                SELECTION_TINT
            } else if hovered {
                HOVER_TINT
            } else {
                [0.0; 4]
            };
            self.gl
                .uniform_4_f32(self.uniforms.tint.as_ref(), r, g, b, amount);
            self.gl
                .uniform_1_i32(self.uniforms.faceted.as_ref(), i32::from(faceted));
            self.gl
                .uniform_matrix_4_f32_slice(self.uniforms.model.as_ref(), false, &model);
            self.gl.uniform_4_f32(
                self.uniforms.color.as_ref(),
                color.r,
                color.g,
                color.b,
                color.a,
            );
            self.gl.bind_vertex_array(Some(mesh.vertex_array));
            self.gl
                .draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
        }
    }

    fn clear_meshes(&mut self) {
        for (_, mesh) in self.meshes.drain() {
            delete_mesh(&self.gl, &mesh);
        }
        for (_, mesh) in self.sculpt_meshes.drain() {
            delete_mesh(&self.gl, &mesh);
        }
    }
}

impl Drop for GlRenderer {
    fn drop(&mut self) {
        self.clear_meshes();
        unsafe {
            self.gl.delete_vertex_array(self.grid_pass.vertex_array);
            self.gl.delete_buffer(self.grid_pass.vertex_buffer);
            self.gl.delete_program(self.grid_pass.program);
            self.gl.delete_vertex_array(self.overlay_pass.vertex_array);
            self.gl.delete_buffer(self.overlay_pass.vertex_buffer);
            self.gl.delete_program(self.overlay_pass.program);
        }
        unsafe { self.gl.delete_program(self.program) };
    }
}

fn delete_mesh(gl: &glow::Context, mesh: &GpuMesh) {
    unsafe {
        gl.delete_vertex_array(mesh.vertex_array);
        gl.delete_buffer(mesh.vertex_buffer);
        gl.delete_buffer(mesh.index_buffer);
    }
}

/// Floats per overlay vertex: position (2, clip space) and color (4).
const OVERLAY_FLOATS_PER_VERTEX: usize = 6;

/// Projects a world point to pixels (origin at the bottom-left). `None` behind the camera.
fn project_to_pixels(
    point: petunia_math::Vec3,
    matrix: &Mat4,
    viewport: (u32, u32),
) -> Option<[f32; 2]> {
    let entry = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
    let clip = |row: usize| {
        entry(row) * point.x + entry(4 + row) * point.y + entry(8 + row) * point.z + entry(12 + row)
    };
    let w = clip(3);
    if w <= 1.0e-4 {
        return None;
    }
    let ndc_x = clip(0) / w;
    let ndc_y = clip(1) / w;
    Some([
        (ndc_x * 0.5 + 0.5) * viewport.0 as f32,
        (ndc_y * 0.5 + 0.5) * viewport.1 as f32,
    ])
}

/// Turns the overlay into triangles in clip space (two per line or square).
fn overlay_vertices(items: &[OverlayItem], matrix: &Mat4, viewport: (u32, u32)) -> Vec<f32> {
    let (width, height) = (viewport.0 as f32, viewport.1 as f32);
    let mut vertices: Vec<f32> = Vec::new();
    let mut quad = |corners: [[f32; 2]; 4], color: petunia_math::Color| {
        if vertices.len() / OVERLAY_FLOATS_PER_VERTEX + 6 > MAX_OVERLAY_VERTICES {
            return;
        }
        for index in [0usize, 1, 2, 1, 3, 2] {
            let [x, y] = corners.get(index).copied().unwrap_or([0.0, 0.0]);
            vertices.extend_from_slice(&[
                x / width * 2.0 - 1.0,
                y / height * 2.0 - 1.0,
                color.r,
                color.g,
                color.b,
                color.a,
            ]);
        }
    };
    for item in items {
        match *item {
            OverlayItem::Line {
                from,
                to,
                color,
                width_pixels,
            } => {
                let (Some(a), Some(b)) = (
                    project_to_pixels(from, matrix, viewport),
                    project_to_pixels(to, matrix, viewport),
                ) else {
                    continue;
                };
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let length = (dx * dx + dy * dy).sqrt();
                if length < 1.0e-3 {
                    continue;
                }
                let half = width_pixels * 0.5;
                let (nx, ny) = (-dy / length * half, dx / length * half);
                quad(
                    [
                        [a[0] + nx, a[1] + ny],
                        [a[0] - nx, a[1] - ny],
                        [b[0] + nx, b[1] + ny],
                        [b[0] - nx, b[1] - ny],
                    ],
                    color,
                );
            }
            OverlayItem::Square {
                center,
                color,
                size_pixels,
            } => {
                let Some(c) = project_to_pixels(center, matrix, viewport) else {
                    continue;
                };
                let half = size_pixels * 0.5;
                quad(
                    [
                        [c[0] - half, c[1] - half],
                        [c[0] + half, c[1] - half],
                        [c[0] - half, c[1] + half],
                        [c[0] + half, c[1] + half],
                    ],
                    color,
                );
            }
        }
    }
    vertices
}

fn create_grid_pass(gl: &glow::Context) -> Result<GridPass, RenderError> {
    let mut lines = grid_lines(GRID_EXTENT, GRID_SPACING);
    lines.extend(axis_lines(GRID_EXTENT));
    let mut bytes = Vec::with_capacity(lines.len() * 2 * 7 * 4);
    for (from, to, color) in &lines {
        for point in [from, to] {
            for value in [
                point.x, point.y, point.z, color.r, color.g, color.b, color.a,
            ] {
                bytes.extend_from_slice(&value.to_ne_bytes());
            }
        }
    }
    unsafe {
        let program = compile_source_program(gl, GRID_VERTEX_SHADER, GRID_FRAGMENT_SHADER)?;
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        gl.bind_vertex_array(Some(vertex_array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
        let stride = 7 * 4;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, stride, 12);
        gl.bind_vertex_array(None);
        Ok(GridPass {
            view_projection: gl.get_uniform_location(program, "u_view_projection"),
            camera_position: gl.get_uniform_location(program, "u_camera_position"),
            height: gl.get_uniform_location(program, "u_height"),
            program,
            vertex_array,
            vertex_buffer,
            vertex_count: (lines.len() * 2) as i32,
        })
    }
}

fn create_overlay_pass(gl: &glow::Context) -> Result<OverlayPass, RenderError> {
    unsafe {
        let program = compile_source_program(gl, OVERLAY_VERTEX_SHADER, OVERLAY_FRAGMENT_SHADER)?;
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        gl.bind_vertex_array(Some(vertex_array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        let stride = (OVERLAY_FLOATS_PER_VERTEX * 4) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, stride, 8);
        gl.bind_vertex_array(None);
        Ok(OverlayPass {
            program,
            vertex_array,
            vertex_buffer,
        })
    }
}

fn compile_program(gl: &glow::Context) -> Result<glow::Program, RenderError> {
    compile_source_program(gl, VERTEX_SHADER, FRAGMENT_SHADER)
}

fn compile_source_program(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<glow::Program, RenderError> {
    unsafe {
        let program = gl.create_program().map_err(RenderError::Gl)?;
        let mut shaders = Vec::new();
        for (kind, source) in [
            (glow::VERTEX_SHADER, vertex_source),
            (glow::FRAGMENT_SHADER, fragment_source),
        ] {
            let shader = gl.create_shader(kind).map_err(RenderError::Gl)?;
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                for earlier in shaders {
                    gl.delete_shader(earlier);
                }
                gl.delete_program(program);
                return Err(RenderError::ShaderCompile(log));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        let linked = gl.get_program_link_status(program);
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        if !linked {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(RenderError::ProgramLink(log));
        }
        Ok(program)
    }
}

fn upload_mesh(gl: &glow::Context, mesh: &MeshData) -> Result<GpuMesh, RenderError> {
    let mut vertex_bytes = Vec::with_capacity(mesh.positions.len() * 24);
    for value in mesh.interleaved() {
        vertex_bytes.extend_from_slice(&value.to_ne_bytes());
    }
    let mut index_bytes = Vec::with_capacity(mesh.indices.len() * 4);
    for index in &mesh.indices {
        index_bytes.extend_from_slice(&index.to_ne_bytes());
    }
    unsafe {
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        let index_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        gl.bind_vertex_array(Some(vertex_array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &vertex_bytes, glow::STATIC_DRAW);
        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, &index_bytes, glow::STATIC_DRAW);
        let stride = 6 * 4;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.bind_vertex_array(None);
        Ok(GpuMesh {
            vertex_array,
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as i32,
        })
    }
}
