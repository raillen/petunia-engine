//! Renderer boundary.
//!
//! This crate knows how to turn the world into a [`RenderFrame`] (what to draw, from where)
//! and how to build meshes and matrices. It contains **no GPU code**: the OpenGL adapter lives
//! in `petunia_render_gl`, so a different backend can replace it without touching the world.

mod camera;
mod fly;
mod frame;
mod grid;
mod mesh;
mod natural;
mod pick;
mod sculpt;

pub mod particles;
pub mod vegetation;
pub mod weather;

pub use camera::{Camera, Mat4, ProjectionMode, model_matrix, multiply};
pub use fly::FlyCamera;
pub use frame::{
    DrawItem, Lighting, MAX_POINT_LIGHTS, OverlayItem, PointLight, RenderFrame, Shape,
    extract_frame, extract_frame_cached, solid_shapes, start_camera, start_camera_at,
};
pub use grid::{GRID_LIFT, GridLine, axis_lines, grid_lines};
pub use mesh::{MeshCache, MeshData, MeshError, build_mesh, item_mesh, polygon_mesh_data};
pub use natural::{
    bridge_mesh_data, hash_spline, rail_pair_mesh_data, road_mesh_data, terrain_mesh_data,
    water_mesh_data,
};
pub use petunia_sculpt::{ChunkMesh, SurfaceMesh};
pub use pick::{Ray, SurfaceHit, mesh_points, pick_entity, pick_surface, ray_through};
pub use sculpt::{SculptCache, SculptItem, SculptVolume, extract_sculpt_volumes, hit_sculpt};
