//! OpenGL 3.3 core adapter.
//!
//! Draws a [`RenderFrame`] with two programs: the lit meshes (diffuse light plus gentle fog) and
//! the flat sprites of a 2D scene (an unlit textured quad, DR-38). The renderer owns every GPU
//! object it creates and deletes them on drop; callers only hold the renderer. No compute
//! shaders, so the same feature set can later run on WebGL2.

// glow exposes OpenGL as `unsafe` functions; this adapter is where that is allowed.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use glow::HasContext;
mod skin;
mod world_mesh;

use petunia_render::{
    DrawItem, Mat4, MeshData, OverlayItem, RenderFrame, SculptItem, Shape, SpriteItem, TextureId,
    TexturePixels, axis_lines, build_mesh, grid_lines, model_matrix,
};

const VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec3 a_flow;
layout(location = 4) in float a_margin;
uniform mat4 u_view_projection;
uniform mat4 u_model;
out vec3 v_normal;
out vec3 v_world_position;
out vec2 v_uv;
out vec3 v_flow;
out float v_margin;
void main() {
    vec4 world = u_model * vec4(a_position, 1.0);
    v_world_position = world.xyz;
    v_normal = transpose(inverse(mat3(u_model))) * a_normal;
    v_uv = a_uv;
    v_flow = a_flow;
    v_margin = a_margin;
    gl_Position = u_view_projection * world;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec3 v_normal;
in vec3 v_world_position;
in vec2 v_uv;
in vec3 v_flow;
in float v_margin;
uniform float u_time;
uniform vec4 u_color;
uniform vec3 u_camera_position;
uniform vec4 u_tint;
uniform int u_faceted;
uniform int u_custom_light;
uniform vec3 u_ambient;
uniform vec3 u_sun_direction;
uniform vec3 u_sun_color;
uniform int u_point_count;
uniform vec4 u_point_position[8];
uniform vec3 u_point_color[8];

uniform sampler2D u_base_texture;
uniform int u_has_base_texture;
uniform int u_material_kind;
uniform float u_cutoff;
out vec4 out_color;
void main() {
    vec4 base_color = u_color;
    if (u_has_base_texture != 0) {
        vec4 tex_color = texture(u_base_texture, v_uv);
        base_color *= tex_color;
    }
    if (u_material_kind == 2 && base_color.a < u_cutoff) {
        discard;
    }

    vec3 normal = normalize(v_normal);
    if (u_faceted != 0) {
        // Low poly: one normal per triangle, from the slope of the surface on screen.
        vec3 flat_normal = normalize(cross(dFdx(v_world_position), dFdy(v_world_position)));
        normal = dot(flat_normal, normal) < 0.0 ? -flat_normal : flat_normal;
    }
    vec3 light;
    if (u_custom_light == 0) {
        // No Light in the scene: the built-in sun and ambient light.
        vec3 sun = normalize(vec3(0.4, 0.8, 0.5));
        float diffuse = max(dot(normal, sun), 0.0);
        float ambient = 0.35 + 0.15 * normal.y;
        light = vec3(ambient + 0.65 * diffuse);
    } else {
        // Scene lights: a soft ambient (a bit brighter facing up), the sun and up to 8 bulbs.
        light = u_ambient * (0.8 + 0.2 * normal.y);
        light += u_sun_color * max(dot(normal, u_sun_direction), 0.0);
        for (int index = 0; index < u_point_count; ++index) {
            vec3 to_light = u_point_position[index].xyz - v_world_position;
            float distance_to_light = length(to_light);
            float reach = u_point_position[index].w;
            float falloff = clamp(1.0 - distance_to_light / reach, 0.0, 1.0);
            float facing = max(dot(normal, to_light / max(distance_to_light, 0.0001)), 0.0);
            light += u_point_color[index] * falloff * falloff * facing;
        }
    }
    vec3 lit;
    if (u_material_kind == 1) {
        lit = base_color.rgb;
    } else {
        lit = base_color.rgb * light;
    }
    // Water: flow ripples and foam on the margins. Other meshes have zero flow and margin.
    float flow_speed = length(v_flow);
    float ripple = 0.5 + 0.5 * sin(v_uv.x * 6.0 - u_time * (1.0 + flow_speed * 2.0) + v_uv.y * 5.0);
    float foam = smoothstep(0.55, 1.0, v_margin) * (0.55 + 0.45 * ripple);
    lit = mix(lit, vec3(0.92, 0.97, 1.0), foam * 0.55);
    lit += vec3(0.03, 0.05, 0.07) * ripple * step(0.01, flow_speed);
    // Water only: a glancing view picks up the sky, so a river reads as a surface.
    if (flow_speed > 0.01) {
        vec3 view_direction = normalize(u_camera_position - v_world_position);
        float fresnel = pow(1.0 - max(dot(normal, view_direction), 0.0), 3.0);
        lit = mix(lit, vec3(0.55, 0.72, 0.82), fresnel * 0.7);
    }
    float fog = clamp(length(v_world_position - u_camera_position) / 200.0, 0.0, 1.0);
    lit = mix(lit, vec3(0.53, 0.70, 0.88), fog * 0.6);
    lit = mix(lit, u_tint.rgb, u_tint.a);
    out_color = vec4(lit, base_color.a);
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

const SPRITE_VERTEX_SHADER: &str = r#"#version 330 core
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_uv;
uniform mat4 u_view_projection;
uniform mat4 u_model;
uniform vec2 u_size;
out vec2 v_uv;
void main() {
    vec3 local = vec3(a_position * u_size, 0.0);
    v_uv = a_uv;
    gl_Position = u_view_projection * u_model * vec4(local, 1.0);
}
"#;

const SPRITE_FRAGMENT_SHADER: &str = r#"#version 330 core
in vec2 v_uv;
uniform sampler2D u_texture;
uniform vec4 u_color;
uniform vec4 u_tint;
// The frame of the sheet this sprite shows: left, top, right, bottom, from 0 to 1. Mirroring is
// already inside these four numbers (DR-39).
uniform vec4 u_rect;
out vec4 out_color;
void main() {
    // u_rect.xy lands on (0, 0) of the quad and u_rect.zw on (1, 1). A mirrored frame has its ends
    // swapped, so the interpolation mirrors the picture by itself.
    vec2 uv = mix(u_rect.xy, u_rect.zw, v_uv);
    vec4 color = texture(u_texture, uv) * u_color;
    color.rgb = mix(color.rgb, u_tint.rgb, u_tint.a);
    if (color.a < 0.004) {
        discard;
    }
    out_color = color;
}
"#;

/// Most overlay vertices drawn in one frame (a hostile caller cannot ask for endless geometry).
const MAX_OVERLAY_VERTICES: usize = 60_000;

/// Upper bound on cached meshes (a hostile scene could ask for endless unique sizes).
const MAX_CACHED_MESHES: usize = 4096;

/// Upper bound on textures kept on the GPU at once.
const MAX_CACHED_TEXTURES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    ShaderCompile(String),
    ProgramLink(String),
    Gl(String),
    Mesh(String),
    Texture(String),
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
            Self::Texture(message) => write!(formatter, "textura inválida: {message}"),
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
    custom_light: Option<glow::UniformLocation>,
    ambient: Option<glow::UniformLocation>,
    sun_direction: Option<glow::UniformLocation>,
    sun_color: Option<glow::UniformLocation>,
    point_count: Option<glow::UniformLocation>,
    point_position: Option<glow::UniformLocation>,
    point_color: Option<glow::UniformLocation>,
    time: Option<glow::UniformLocation>,
    base_texture: Option<glow::UniformLocation>,
    has_base_texture: Option<glow::UniformLocation>,
    material_kind: Option<glow::UniformLocation>,
    cutoff: Option<glow::UniformLocation>,
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

/// The scene image and the one fullscreen grade drawn on top of it.
struct GradePass {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    framebuffer: glow::Framebuffer,
    color: glow::Texture,
    depth: glow::Renderbuffer,
    width: i32,
    height: i32,
    /// How bright the image is allowed to be this frame. Comfort moves it slowly.
    ceiling: f32,
    contrast: Option<glow::UniformLocation>,
    vignette: Option<glow::UniformLocation>,
    grain: Option<glow::UniformLocation>,
    luma_ceiling: Option<glow::UniformLocation>,
    time: Option<glow::UniformLocation>,
}

#[derive(Clone, Copy)]
/// The flat sprites of a 2D scene: an unlit textured quad, one draw call per sprite.
struct SpritePass {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
    view_projection: Option<glow::UniformLocation>,
    model: Option<glow::UniformLocation>,
    size: Option<glow::UniformLocation>,
    color: Option<glow::UniformLocation>,
    tint: Option<glow::UniformLocation>,
    rect: Option<glow::UniformLocation>,
    texture: Option<glow::UniformLocation>,
}

/// A texture on the GPU beside the pixels it came from: the same handle with new pixels (the
/// host reloaded the file) has to be uploaded again.
struct CachedTexture {
    texture: glow::Texture,
    pixels: Arc<TexturePixels>,
}

#[derive(Clone, Copy)]
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
    /// One buffer per editable mesh, by the polygon mesh's fingerprint (new on every edit).
    edit_meshes: HashMap<u64, GpuMesh>,
    /// Buffers of the skinned meshes, rewritten whenever the pose changes (phase 4).
    skinned: skin::SkinCache,
    /// The vegetation impostors of the frame, in world coordinates (phase 5).
    vegetation: world_mesh::WorldMesh,
    /// The precipitation of the frame (phase 4 and 5).
    precipitation: world_mesh::WorldMesh,
    /// Clock for water ripples (rendering only; the simulation is deterministic).
    started: std::time::Instant,
    grid_pass: GridPass,
    overlay_pass: OverlayPass,
    /// Scene target and the fused grade (DR-13, first cut).
    grade: GradePass,
    sprite_pass: SpritePass,
    /// Sprites left without a loaded asset are drawn with this 1x1 white texture.
    white_texture: glow::Texture,
    sprite_textures: HashMap<TextureId, CachedTexture>,
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
                custom_light: gl.get_uniform_location(program, "u_custom_light"),
                ambient: gl.get_uniform_location(program, "u_ambient"),
                sun_direction: gl.get_uniform_location(program, "u_sun_direction"),
                sun_color: gl.get_uniform_location(program, "u_sun_color"),
                point_count: gl.get_uniform_location(program, "u_point_count"),
                point_position: gl.get_uniform_location(program, "u_point_position[0]"),
                point_color: gl.get_uniform_location(program, "u_point_color[0]"),
                time: gl.get_uniform_location(program, "u_time"),
                base_texture: gl.get_uniform_location(program, "u_base_texture"),
                has_base_texture: gl.get_uniform_location(program, "u_has_base_texture"),
                material_kind: gl.get_uniform_location(program, "u_material_kind"),
                cutoff: gl.get_uniform_location(program, "u_cutoff"),
            }
        };
        let grid_pass = create_grid_pass(&gl)?;
        let overlay_pass = create_overlay_pass(&gl)?;
        let grade = create_grade_pass(&gl)?;
        let sprite_pass = create_sprite_pass(&gl)?;
        let white_texture = create_texture(&gl, 1, 1, &[255, 255, 255, 255])?;
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
            edit_meshes: HashMap::new(),
            skinned: skin::SkinCache::new(),
            vegetation: world_mesh::WorldMesh::new(),
            precipitation: world_mesh::WorldMesh::new(),
            grid_pass,
            overlay_pass,
            grade,
            sprite_pass,
            white_texture,
            sprite_textures: HashMap::new(),
            started: std::time::Instant::now(),
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
        let host_framebuffer = unsafe { self.gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING) };
        self.resize_grade_target(width, height)?;
        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.grade.framebuffer));
        }
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
            self.gl.uniform_1_f32(
                self.uniforms.time.as_ref(),
                self.started.elapsed().as_secs_f32(),
            );
            self.upload_lighting(&frame.lighting);
        }

        // Opaque first; translucent objects (ghosted floors, blended materials) after, far to near,
        // without writing depth so they never hide what is behind them.
        let (opaque, mut translucent): (Vec<&DrawItem>, Vec<&DrawItem>) =
            frame.items.iter().partition(|item| {
                item.color.a >= OPAQUE_ALPHA
                    && item.material_kind != petunia_render::MaterialKind::Blended
            });
        for item in opaque {
            self.draw_item(item, &frame.textures)?;
        }
        for sculpt in &frame.sculpts {
            self.draw_sculpt(sculpt)?;
        }
        // Vegetation is opaque: it is drawn with the solid pass, before the translucent objects
        // (phase 5). The mesh is in world coordinates, so the model matrix is the identity.
        self.draw_vegetation(frame)?;
        // Precipitation is translucent and drawn after everything else, with blending on and the
        // depth test still on, so rain behind a wall stays behind it.
        self.draw_precipitation(frame)?;
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
        // The same for editable meshes: every edit has a new fingerprint.
        let drawn: std::collections::HashSet<u64> = frame
            .items
            .iter()
            .filter(|item| item.shape == Shape::Mesh)
            .map(|item| item.mesh_fingerprint)
            .collect();
        let stale: Vec<u64> = self
            .edit_meshes
            .keys()
            .filter(|fingerprint| !drawn.contains(fingerprint))
            .copied()
            .collect();
        for fingerprint in stale {
            if let Some(mesh) = self.edit_meshes.remove(&fingerprint) {
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
                self.draw_item(item, &frame.textures)?;
            }
            unsafe {
                self.gl.depth_mask(true);
                self.gl.disable(glow::BLEND);
            }
        }
        self.present_grade(frame, host_framebuffer);
        self.draw_sprites(frame, &view_projection)?;
        self.draw_overlay(&frame.overlay, &view_projection, (width, height));
        unsafe { self.gl.bind_vertex_array(None) };
        Ok(())
    }

    /// Grows the scene target when the window size changes. The first frame allocates it.
    fn resize_grade_target(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        let width = width as i32;
        let height = height as i32;
        if self.grade.width == width && self.grade.height == height {
            return Ok(());
        }
        unsafe {
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.grade.color));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA8 as i32,
                width,
                height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            self.gl
                .bind_renderbuffer(glow::RENDERBUFFER, Some(self.grade.depth));
            self.gl.renderbuffer_storage(
                glow::RENDERBUFFER,
                glow::DEPTH_COMPONENT24,
                width,
                height,
            );
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.grade.framebuffer));
            let status = self.gl.check_framebuffer_status(glow::FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(RenderError::Gl(
                    "o alvo da cena não ficou completo".to_string(),
                ));
            }
        }
        self.grade.width = width;
        self.grade.height = height;
        Ok(())
    }

    /// Draws the graded scene into the framebuffer Qt (or the test) had bound.
    fn present_grade(&mut self, frame: &RenderFrame, host_framebuffer: i32) {
        let goal = frame.grade.luma_ceiling;
        let step = if frame.grade.comfort { 0.04 } else { 1.0 };
        let delta = (goal - self.grade.ceiling).clamp(-step, step);
        self.grade.ceiling += delta;
        unsafe {
            bind_framebuffer_id(&self.gl, host_framebuffer);
            self.gl.disable(glow::DEPTH_TEST);
            self.gl.disable(glow::BLEND);
            self.gl.use_program(Some(self.grade.program));
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.grade.color));
            self.gl
                .uniform_1_f32(self.grade.contrast.as_ref(), frame.grade.contrast);
            self.gl
                .uniform_1_f32(self.grade.vignette.as_ref(), frame.grade.vignette);
            self.gl
                .uniform_1_f32(self.grade.grain.as_ref(), frame.grade.grain);
            self.gl
                .uniform_1_f32(self.grade.luma_ceiling.as_ref(), self.grade.ceiling);
            self.gl.uniform_1_f32(
                self.grade.time.as_ref(),
                self.started.elapsed().as_secs_f32(),
            );
            self.gl.bind_vertex_array(Some(self.grade.vertex_array));
            self.gl.draw_arrays(glow::TRIANGLES, 0, 3);
            self.gl.enable(glow::DEPTH_TEST);
        }
    }

    /// Draws one object with the main program (bound by the caller), building its mesh once.
    fn draw_item(
        &mut self,
        item: &DrawItem,
        textures: &[Arc<TexturePixels>],
    ) -> Result<(), RenderError> {
        let texture = match item.base_texture {
            Some(id) => self.texture_for(id, textures)?,
            None => None,
        };
        if item.shape == Shape::Mesh {
            if !item.skin.is_empty() && !item.skin_matrices.is_empty() {
                return self.draw_skinned_mesh(item, texture);
            }
            return self.draw_edit_mesh(item, texture);
        }
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
            MeshLook::of(item),
            DrawMode::of(item),
            item.material_kind,
            item.cutoff,
            texture,
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

    /// Draws the flat sprites of the frame in the order the extractor sorted them (painter's
    /// order: a higher layer and a later entity are drawn on top), after the scene and before the
    /// gizmos. A sprite lands on its own XY plane, so the same quad works under a perspective or an
    /// orthographic camera.
    fn draw_sprites(
        &mut self,
        frame: &RenderFrame,
        view_projection: &Mat4,
    ) -> Result<(), RenderError> {
        // Only the textures of this frame stay on the GPU.
        let current: std::collections::HashSet<TextureId> =
            frame.textures.iter().map(|pixels| pixels.id).collect();
        let stale: Vec<TextureId> = self
            .sprite_textures
            .keys()
            .filter(|id| !current.contains(id))
            .copied()
            .collect();
        for id in stale {
            if let Some(cached) = self.sprite_textures.remove(&id) {
                unsafe { self.gl.delete_texture(cached.texture) };
            }
        }
        if frame.sprites.is_empty() {
            return Ok(());
        }
        unsafe {
            // Two-sided and unlit: the picture already has its own colours, and the sorted order
            // decides who is on top, so a sprite writes no depth.
            self.gl.disable(glow::CULL_FACE);
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl.depth_mask(false);
            self.gl.use_program(Some(self.sprite_pass.program));
            self.gl.uniform_matrix_4_f32_slice(
                self.sprite_pass.view_projection.as_ref(),
                false,
                view_projection,
            );
            self.gl.uniform_1_i32(self.sprite_pass.texture.as_ref(), 0);
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_vertex_array(Some(self.sprite_pass.vertex_array));
        }
        for sprite in &frame.sprites {
            let texture = match sprite.texture {
                Some(id) => self
                    .texture_for(id, &frame.textures)?
                    .unwrap_or(self.white_texture),
                None => self.white_texture,
            };
            self.draw_sprite(sprite, texture);
        }
        unsafe {
            self.gl.bind_vertex_array(None);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            self.gl.depth_mask(true);
            self.gl.disable(glow::BLEND);
            self.gl.enable(glow::CULL_FACE);
            self.gl.use_program(Some(self.program));
        }
        Ok(())
    }

    /// Sets the uniforms of one sprite and draws the shared unit quad with its texture.
    fn draw_sprite(&self, sprite: &SpriteItem, texture: glow::Texture) {
        let model = model_matrix(&sprite.transform);
        let [r, g, b, amount] = if sprite.selected {
            SELECTION_TINT
        } else if sprite.hovered {
            HOVER_TINT
        } else {
            [0.0; 4]
        };
        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl
                .uniform_matrix_4_f32_slice(self.sprite_pass.model.as_ref(), false, &model);
            self.gl
                .uniform_2_f32(self.sprite_pass.size.as_ref(), sprite.width, sprite.height);
            self.gl.uniform_4_f32(
                self.sprite_pass.color.as_ref(),
                sprite.color.r,
                sprite.color.g,
                sprite.color.b,
                sprite.color.a,
            );
            self.gl
                .uniform_4_f32(self.sprite_pass.tint.as_ref(), r, g, b, amount);
            // The frame of the sheet the sprite shows right now, mirroring included: the pass
            // never looks at frames or time, it just samples this rectangle (DR-39).
            let [left, top, right, bottom] = sprite.uv();
            self.gl
                .uniform_4_f32(self.sprite_pass.rect.as_ref(), left, top, right, bottom);
            self.gl.draw_arrays(glow::TRIANGLES, 0, SPRITE_VERTEX_COUNT);
        }
    }

    /// The GPU texture of a handle, uploaded on first sight and again when its pixels changed.
    fn texture_for(
        &mut self,
        id: TextureId,
        textures: &[Arc<TexturePixels>],
    ) -> Result<Option<glow::Texture>, RenderError> {
        let Some(pixels) = textures.iter().find(|pixels| pixels.id == id) else {
            return Ok(None);
        };
        if let Some(cached) = self.sprite_textures.get(&id) {
            if Arc::ptr_eq(&cached.pixels, pixels) {
                return Ok(Some(cached.texture));
            }
            // The host loaded the file again: the old pixels are the wrong ones.
            if let Some(cached) = self.sprite_textures.remove(&id) {
                unsafe { self.gl.delete_texture(cached.texture) };
            }
        }
        if self.sprite_textures.len() >= MAX_CACHED_TEXTURES {
            self.clear_textures();
        }
        let texture = create_texture(&self.gl, pixels.width, pixels.height, &pixels.rgba)?;
        self.sprite_textures.insert(
            id,
            CachedTexture {
                texture,
                pixels: Arc::clone(pixels),
            },
        );
        Ok(Some(texture))
    }

    fn clear_textures(&mut self) {
        for (_, cached) in self.sprite_textures.drain() {
            unsafe { self.gl.delete_texture(cached.texture) };
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
    /// Draws the vegetation impostors of the frame (phase 5).
    ///
    /// The mesh already carries world positions, so the model matrix is the identity. A frame with no
    /// vegetation clears the buffer once and costs nothing after that.
    fn draw_vegetation(&mut self, frame: &RenderFrame) -> Result<(), RenderError> {
        let Some(data) = frame.vegetation.as_ref() else {
            self.vegetation.clear(&self.gl);
            return Ok(());
        };
        let fingerprint = world_mesh::fingerprint(data);
        let gl = self.gl.clone();
        let Some(mesh) = self.vegetation.update(&gl, data, fingerprint)? else {
            return Ok(());
        };
        if mesh.index_count == 0 {
            return Ok(());
        }
        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl
                .uniform_matrix_4_f32_slice(self.uniforms.model.as_ref(), false, &IDENTITY);
            self.gl.uniform_4_f32(
                self.uniforms.color.as_ref(),
                FOLIAGE_TINT[0],
                FOLIAGE_TINT[1],
                FOLIAGE_TINT[2],
                FOLIAGE_TINT[3],
            );
            self.gl
                .uniform_4_f32(self.uniforms.tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
            self.gl.uniform_1_i32(self.uniforms.faceted.as_ref(), 0);
            self.gl.bind_vertex_array(Some(mesh.vertex_array));
            // Two triangles per plant, wound both ways so a quad is visible from either side.
            self.gl.disable(glow::CULL_FACE);
            self.gl
                .draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
            // The plant quads are visible from both sides. Everything after them (ghosted floors)
            // keeps the back-face cull the rest of the frame uses.
            self.gl.enable(glow::CULL_FACE);
        }
        Ok(())
    }

    /// Draws the precipitation of the frame: rain streaks or snow flakes, blended (phase 4 and 5).
    fn draw_precipitation(&mut self, frame: &RenderFrame) -> Result<(), RenderError> {
        let Some(data) = frame.precipitation.as_ref() else {
            self.precipitation.clear(&self.gl);
            return Ok(());
        };
        let fingerprint = world_mesh::fingerprint(data);
        let gl = self.gl.clone();
        let Some(mesh) = self.precipitation.update(&gl, data, fingerprint)? else {
            return Ok(());
        };
        if mesh.index_count == 0 {
            return Ok(());
        }
        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl
                .uniform_matrix_4_f32_slice(self.uniforms.model.as_ref(), false, &IDENTITY);
            self.gl.uniform_4_f32(
                self.uniforms.color.as_ref(),
                RAIN_TINT[0],
                RAIN_TINT[1],
                RAIN_TINT[2],
                RAIN_TINT[3],
            );
            self.gl
                .uniform_4_f32(self.uniforms.tint.as_ref(), 0.0, 0.0, 0.0, 0.0);
            self.gl.uniform_1_i32(self.uniforms.faceted.as_ref(), 0);
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            self.gl.depth_mask(false);
            self.gl.bind_vertex_array(Some(mesh.vertex_array));
            self.gl.disable(glow::CULL_FACE);
            self.gl
                .draw_elements(glow::TRIANGLES, mesh.index_count, glow::UNSIGNED_INT, 0);
            self.gl.enable(glow::CULL_FACE);
            self.gl.depth_mask(true);
            self.gl.disable(glow::BLEND);
        }
        Ok(())
    }

    fn draw_sculpt(&mut self, sculpt: &SculptItem) -> Result<(), RenderError> {
        for chunk in &sculpt.chunks {
            if !self.sculpt_meshes.contains_key(&chunk.id) {
                let data = MeshData {
                    positions: chunk.mesh.positions.clone(),
                    normals: chunk.mesh.normals.clone(),
                    indices: chunk.mesh.indices.clone(),
                    ..MeshData::default()
                };
                let uploaded = upload_mesh(&self.gl, &data)?;
                self.sculpt_meshes.insert(chunk.id, uploaded);
            }
            if let Some(mesh) = self.sculpt_meshes.get(&chunk.id) {
                self.draw_mesh(
                    mesh,
                    &sculpt.transform,
                    sculpt.color,
                    MeshLook {
                        selected: sculpt.selected,
                        hovered: sculpt.hovered,
                        faceted: sculpt.faceted,
                    },
                    DrawMode::SOLID,
                    petunia_render::MaterialKind::Standard,
                    0.5,
                    None,
                );
            }
        }
        Ok(())
    }

    /// Sends the scene's lights to the main program (bound by the caller).
    ///
    /// # Safety
    /// The context must be current and the main program in use.
    unsafe fn upload_lighting(&self, lighting: &petunia_render::Lighting) {
        let uniforms = &self.uniforms;
        // SAFETY: forwarded contract; every slice has the size the shader declares.
        unsafe {
            self.gl
                .uniform_1_i32(uniforms.custom_light.as_ref(), i32::from(lighting.custom));
            self.gl.uniform_3_f32(
                uniforms.ambient.as_ref(),
                lighting.ambient.x,
                lighting.ambient.y,
                lighting.ambient.z,
            );
            let (direction, color) = lighting
                .sun
                .unwrap_or((petunia_math::Vec3::ZERO, petunia_math::Vec3::ZERO));
            self.gl.uniform_3_f32(
                uniforms.sun_direction.as_ref(),
                direction.x,
                direction.y,
                direction.z,
            );
            self.gl
                .uniform_3_f32(uniforms.sun_color.as_ref(), color.x, color.y, color.z);
            let count = lighting.points.len().min(petunia_render::MAX_POINT_LIGHTS);
            self.gl
                .uniform_1_i32(uniforms.point_count.as_ref(), count as i32);
            let mut positions = [0.0f32; 4 * petunia_render::MAX_POINT_LIGHTS];
            let mut colors = [0.0f32; 3 * petunia_render::MAX_POINT_LIGHTS];
            let slots = positions
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(colors.as_chunks_mut::<3>().0.iter_mut())
                .zip(lighting.points.iter().take(count));
            for ((position_slot, color_slot), light) in slots {
                position_slot.copy_from_slice(&[
                    light.position.x,
                    light.position.y,
                    light.position.z,
                    light.range,
                ]);
                color_slot.copy_from_slice(&[light.color.x, light.color.y, light.color.z]);
            }
            self.gl
                .uniform_4_f32_slice(uniforms.point_position.as_ref(), &positions);
            self.gl
                .uniform_3_f32_slice(uniforms.point_color.as_ref(), &colors);
        }
    }

    /// Sets the per-object uniforms and draws a mesh with the main program (bound by the caller).
    /// Draws one mesh. `mode` carries the two view options (aramado and raio-X, `QT-09`).
    #[allow(clippy::too_many_arguments)]
    fn draw_mesh(
        &self,
        mesh: &GpuMesh,
        transform: &petunia_math::Transform,
        color: petunia_math::Color,
        look: MeshLook,
        mode: DrawMode,
        material_kind: petunia_render::MaterialKind,
        cutoff: f32,
        texture: Option<glow::Texture>,
    ) {
        let model = model_matrix(transform);
        unsafe {
            let [r, g, b, amount] = if look.selected {
                SELECTION_TINT
            } else if look.hovered {
                HOVER_TINT
            } else {
                [0.0; 4]
            };
            self.gl
                .uniform_4_f32(self.uniforms.tint.as_ref(), r, g, b, amount);
            self.gl
                .uniform_1_i32(self.uniforms.faceted.as_ref(), i32::from(look.faceted));
            self.gl
                .uniform_matrix_4_f32_slice(self.uniforms.model.as_ref(), false, &model);
            self.gl.uniform_4_f32(
                self.uniforms.color.as_ref(),
                color.r,
                color.g,
                color.b,
                color.a,
            );
            let (has_texture, tex_handle) = match texture {
                Some(handle) => (1, handle),
                None => (0, self.white_texture),
            };
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(tex_handle));
            self.gl
                .uniform_1_i32(self.uniforms.base_texture.as_ref(), 0);
            self.gl
                .uniform_1_i32(self.uniforms.has_base_texture.as_ref(), has_texture);
            let kind_int = match material_kind {
                petunia_render::MaterialKind::Standard => 0,
                petunia_render::MaterialKind::Flat => 1,
                petunia_render::MaterialKind::Masked => 2,
                petunia_render::MaterialKind::Blended => 3,
                petunia_render::MaterialKind::Post => 4,
            };
            self.gl
                .uniform_1_i32(self.uniforms.material_kind.as_ref(), kind_int);
            self.gl.uniform_1_f32(self.uniforms.cutoff.as_ref(), cutoff);

            if mode.xray {
                self.gl.disable(glow::DEPTH_TEST);
            }
            self.gl.bind_vertex_array(Some(mesh.vertex_array));
            // The mode travels in the draw call, not in the shader: drawing triangles as lines costs
            // one enum change, and a wireframe needs no second mesh.
            let primitive = if mode.wireframe {
                glow::LINES
            } else {
                glow::TRIANGLES
            };
            self.gl
                .draw_elements(primitive, mesh.index_count, glow::UNSIGNED_INT, 0);
            if mode.xray {
                self.gl.enable(glow::DEPTH_TEST);
            }
        }
    }

    /// Draws an editable mesh, uploading its triangles the first time its fingerprint is seen.
    /// A mesh moved by its skeleton this frame (`DR-23`, phase 4).
    ///
    /// The pose fingerprint combines the item's mesh and the matrices that moved it, so a still
    /// character reuses the buffer and an animated one is re-uploaded only when it actually moved.
    fn draw_skinned_mesh(
        &mut self,
        item: &DrawItem,
        texture: Option<glow::Texture>,
    ) -> Result<(), RenderError> {
        let pose = pose_fingerprint(item);
        let gl = self.gl.clone();
        let Some(mesh) = self.skinned.mesh_for(&gl, item, pose)? else {
            // Nothing to draw (no source mesh or no triangles): fall back to the plain path.
            return self.draw_edit_mesh(item, texture);
        };
        self.draw_mesh(
            &mesh,
            &item.transform,
            item.color,
            MeshLook::of(item),
            DrawMode::of(item),
            item.material_kind,
            item.cutoff,
            texture,
        );
        Ok(())
    }

    fn draw_edit_mesh(
        &mut self,
        item: &DrawItem,
        texture: Option<glow::Texture>,
    ) -> Result<(), RenderError> {
        let Some(data) = &item.mesh else {
            return Ok(());
        };
        if data.indices.is_empty() {
            return Ok(());
        }
        if !self.edit_meshes.contains_key(&item.mesh_fingerprint) {
            let uploaded = upload_mesh(&self.gl, data)?;
            self.edit_meshes.insert(item.mesh_fingerprint, uploaded);
        }
        let Some(mesh) = self.edit_meshes.get(&item.mesh_fingerprint) else {
            return Ok(());
        };
        self.draw_mesh(
            mesh,
            &item.transform,
            item.color,
            MeshLook::of(item),
            DrawMode::of(item),
            item.material_kind,
            item.cutoff,
            texture,
        );
        Ok(())
    }

    fn clear_meshes(&mut self) {
        for (_, mesh) in self.meshes.drain() {
            delete_mesh(&self.gl, &mesh);
        }
        for (_, mesh) in self.sculpt_meshes.drain() {
            delete_mesh(&self.gl, &mesh);
        }
        for (_, mesh) in self.edit_meshes.drain() {
            delete_mesh(&self.gl, &mesh);
        }
        self.skinned.clear(&self.gl);
    }
}

impl Drop for GlRenderer {
    fn drop(&mut self) {
        self.clear_meshes();
        self.clear_textures();
        unsafe {
            self.gl.delete_vertex_array(self.grid_pass.vertex_array);
            self.gl.delete_buffer(self.grid_pass.vertex_buffer);
            self.gl.delete_program(self.grid_pass.program);
            self.gl.delete_vertex_array(self.overlay_pass.vertex_array);
            self.gl.delete_buffer(self.overlay_pass.vertex_buffer);
            self.gl.delete_program(self.overlay_pass.program);
            self.gl.delete_program(self.grade.program);
            self.gl.delete_vertex_array(self.grade.vertex_array);
            self.gl.delete_framebuffer(self.grade.framebuffer);
            self.gl.delete_texture(self.grade.color);
            self.gl.delete_renderbuffer(self.grade.depth);
            self.gl.delete_vertex_array(self.sprite_pass.vertex_array);
            self.gl.delete_buffer(self.sprite_pass.vertex_buffer);
            self.gl.delete_program(self.sprite_pass.program);
            self.gl.delete_texture(self.white_texture);
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

/// Vertices of the shared sprite quad (two triangles).
const SPRITE_VERTEX_COUNT: i32 = 6;

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

/// Six vertices of a unit square on the XY plane: position (2) and texture coordinate (2).
fn sprite_quad() -> [f32; 24] {
    // Texture coordinate (0, 0) is the top-left pixel of the image: the decoder's first row.
    [
        -0.5, 0.5, 0.0, 0.0, // top left
        0.5, 0.5, 1.0, 0.0, // top right
        -0.5, -0.5, 0.0, 1.0, // bottom left
        0.5, 0.5, 1.0, 0.0, // top right again
        0.5, -0.5, 1.0, 1.0, // bottom right
        -0.5, -0.5, 0.0, 1.0, // bottom left again
    ]
}

fn create_sprite_pass(gl: &glow::Context) -> Result<SpritePass, RenderError> {
    let mut bytes = Vec::with_capacity(sprite_quad().len() * 4);
    for value in sprite_quad() {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    unsafe {
        let program = compile_source_program(gl, SPRITE_VERTEX_SHADER, SPRITE_FRAGMENT_SHADER)?;
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let vertex_buffer = gl.create_buffer().map_err(RenderError::Gl)?;
        gl.bind_vertex_array(Some(vertex_array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
        let stride = 4 * 4;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 8);
        gl.bind_vertex_array(None);
        Ok(SpritePass {
            view_projection: gl.get_uniform_location(program, "u_view_projection"),
            model: gl.get_uniform_location(program, "u_model"),
            size: gl.get_uniform_location(program, "u_size"),
            color: gl.get_uniform_location(program, "u_color"),
            tint: gl.get_uniform_location(program, "u_tint"),
            rect: gl.get_uniform_location(program, "u_rect"),
            texture: gl.get_uniform_location(program, "u_texture"),
            program,
            vertex_array,
            vertex_buffer,
        })
    }
}

/// Uploads 8-bit RGBA pixels as a texture, rows tightly packed (whatever the width).
fn create_texture(
    gl: &glow::Context,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<glow::Texture, RenderError> {
    let expected = width as usize * height as usize * 4;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(RenderError::Texture(format!(
            "{width} por {height} pixels não cabem em {} bytes",
            rgba.len()
        )));
    }
    unsafe {
        let texture = gl.create_texture().map_err(RenderError::Gl)?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        // Rows are tight: without this a texture whose width is not a multiple of four would be
        // read with padding.
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(rgba)),
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(texture)
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

/// What marks a mesh carries: the selection tint, the hover tint and the low-poly look.
#[derive(Clone, Copy, Debug, Default)]
struct MeshLook {
    selected: bool,
    hovered: bool,
    faceted: bool,
}

impl MeshLook {
    fn of(item: &DrawItem) -> Self {
        Self {
            selected: item.selected,
            hovered: item.hovered,
            faceted: false,
        }
    }
}

/// Identity model matrix: world-space meshes carry their own positions.
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Foliage colour, multiplied by the vertex colour the mesh carries.
const FOLIAGE_TINT: [f32; 4] = [0.42, 0.62, 0.30, 1.0];

/// Rain and snow read as pale water against any background.
const RAIN_TINT: [f32; 4] = [0.93, 0.96, 1.0, 0.75];

/// How one mesh is drawn: the two view options of `QT-09`.
#[derive(Clone, Copy, Debug, Default)]
struct DrawMode {
    wireframe: bool,
    xray: bool,
}

impl DrawMode {
    const SOLID: Self = Self {
        wireframe: false,
        xray: false,
    };

    fn of(item: &DrawItem) -> Self {
        Self {
            wireframe: item.wireframe,
            xray: item.xray,
        }
    }
}

fn upload_mesh(gl: &glow::Context, mesh: &MeshData) -> Result<GpuMesh, RenderError> {
    let mut vertex_bytes = Vec::with_capacity(mesh.positions.len() * 48);
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
        let stride = 12 * 4;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 2, glow::FLOAT, false, stride, 24);
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_f32(3, 3, glow::FLOAT, false, stride, 32);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_f32(4, 1, glow::FLOAT, false, stride, 44);
        gl.bind_vertex_array(None);
        Ok(GpuMesh {
            vertex_array,
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as i32,
        })
    }
}

/// A cheap fingerprint of the pose: the matrices that moved the mesh this frame.
///
/// Two frames at the same clip time give the same fingerprint, so the buffer is reused. It is not a
/// hash of the pixels: one comparison against the previous value is enough here.
fn pose_fingerprint(item: &DrawItem) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64; // FNV-1a offset basis
    for matrix in &item.skin_matrices {
        for value in matrix.0 {
            // `to_bits` keeps the comparison exact (no epsilon on a cache key).
            hash ^= u64::from(value.to_bits());
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

const GRADE_VERTEX: &str = r#"#version 330 core
void main() {
    vec2 corners[3] = vec2[](vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    gl_Position = vec4(corners[gl_VertexID], 0.0, 1.0);
}
"#;

const GRADE_FRAGMENT: &str = r#"#version 330 core
out vec4 out_color;
uniform sampler2D u_scene;
uniform float u_contrast;
uniform float u_vignette;
uniform float u_grain;
uniform float u_ceiling;
uniform float u_time;
void main() {
    vec2 size = vec2(textureSize(u_scene, 0));
    vec2 uv = gl_FragCoord.xy / size;
    vec3 color = texture(u_scene, uv).rgb;
    float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    float shaped = pow(max(luma, 0.0001), 1.0 / max(u_contrast, 0.05));
    color *= shaped / max(luma, 0.0001);
    float vignette = smoothstep(0.35, 1.15, length((uv - 0.5) * vec2(1.15, 1.0)));
    color *= 1.0 - u_vignette * vignette;
    float noise = fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233)) + u_time) * 43758.5453);
    color += (noise - 0.5) * u_grain;
    float out_luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    if (out_luma > u_ceiling) {
        color *= u_ceiling / out_luma;
    }
    out_color = vec4(max(color, vec3(0.0)), 1.0);
}
"#;

fn create_grade_pass(gl: &glow::Context) -> Result<GradePass, RenderError> {
    let program = compile_source_program(gl, GRADE_VERTEX, GRADE_FRAGMENT)?;
    unsafe {
        let vertex_array = gl.create_vertex_array().map_err(RenderError::Gl)?;
        let framebuffer = gl.create_framebuffer().map_err(RenderError::Gl)?;
        let color = gl.create_texture().map_err(RenderError::Gl)?;
        let depth = gl.create_renderbuffer().map_err(RenderError::Gl)?;
        gl.bind_texture(glow::TEXTURE_2D, Some(color));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(color),
            0,
        );
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(depth),
        );
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        let contrast = gl.get_uniform_location(program, "u_contrast");
        let vignette = gl.get_uniform_location(program, "u_vignette");
        let grain = gl.get_uniform_location(program, "u_grain");
        let luma_ceiling = gl.get_uniform_location(program, "u_ceiling");
        let time = gl.get_uniform_location(program, "u_time");
        if let Some(scene) = gl.get_uniform_location(program, "u_scene") {
            gl.use_program(Some(program));
            gl.uniform_1_i32(Some(&scene), 0);
        }
        Ok(GradePass {
            program,
            vertex_array,
            framebuffer,
            color,
            depth,
            width: 0,
            height: 0,
            ceiling: 0.45,
            contrast,
            vignette,
            grain,
            luma_ceiling,
            time,
        })
    }
}

fn bind_framebuffer_id(gl: &glow::Context, id: i32) {
    unsafe {
        if let Some(raw) = std::num::NonZeroU32::new(id as u32) {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(glow::NativeFramebuffer(raw)));
        } else {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
    }
}
