//! Renderer boundary.
//!
//! This crate knows how to turn the world into a [`RenderFrame`] (what to draw, from where)
//! and how to build meshes and matrices. It contains **no GPU code**: the OpenGL adapter lives
//! in `petunia_render_gl`, so a different backend can replace it without touching the world.

mod camera;
mod fly;
mod frame;
mod frustum;
mod grid;
mod mesh;
mod pick;
mod sculpt;
mod terrain;

pub use camera::{Camera, Mat4, ProjectionMode, model_matrix, multiply, transform_point};
pub use fly::FlyCamera;
pub use frame::{
    DEFAULT_VIEWPORT, DrawItem, Lighting, MAX_POINT_LIGHTS, OverlayItem, PointLight, RenderFrame,
    Shape, extract_frame, extract_frame_cached, extract_frame_in, solid_shapes, start_camera,
};
pub use frustum::Frustum;
pub use grid::{GRID_LIFT, GridLine, axis_lines, grid_lines};
pub use mesh::{MeshCache, MeshData, MeshError, build_mesh, item_mesh, polygon_mesh_data};
pub use petunia_sculpt::{ChunkMesh, SurfaceMesh};
pub use pick::{Ray, SurfaceHit, mesh_points, pick_entity, pick_surface, ray_through};
pub use sculpt::{
    SculptCache, SculptItem, SculptVolume, extract_sculpt_volumes, hit_sculpt, volume_bounds,
    volume_center_height,
};
pub use terrain::{
    LOD_MID_DISTANCE, LOD_NEAR_DISTANCE, TERRAIN_LAYERS, chunk_bounds, chunk_fingerprint,
    chunk_mesh, chunk_sphere, layer_color, lod_step,
};
