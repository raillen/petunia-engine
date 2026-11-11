//! The world as tables of data.
//!
//! An entity is a row. A table is a set of named, typed columns. Behavior lives in functions
//! and rules elsewhere, never inside the data. Iteration order is always deterministic
//! (ascending entity handle), which keeps simulation, saves and replays repeatable.
//!
//! Two kinds of identity, as in the architecture notes:
//! - [`PersistentId`]: goes into files, never reused, small and diff-friendly.
//! - [`EntityHandle`]: index plus generation, valid only in this run; a stale handle is an error,
//!   never another entity.

mod buffer;
mod editor_state;
mod error;
mod face_color;
mod heightfield;
mod hierarchy;
mod ids;
mod limits;
mod material;
mod path_network;
mod rng;
pub mod sky_layer;
mod stamp;
mod table;
mod value;
pub mod weather;
mod world;

pub use buffer::{CommandBuffer, DeferredOp};
pub use editor_state::{EDITOR_STATE_TABLE, ObjectState, hidden_objects, locked_objects, state_of};
pub use error::WorldError;
pub use face_color::{FACE_COLOR_TABLE, face_colors, face_material_of};
pub use heightfield::{HEIGHT_CHUNK_SIZE, HeightChunk, Heightfield};
pub use hierarchy::{ShotPose, active_shot, composed_transform, parent_id, shot_of};
pub use ids::{EntityHandle, PersistentId};
pub use limits::Limits;
pub use material::{MATERIAL_TABLE, MaterialKind, MaterialSpec, material_of};
pub use path_network::{PathEdge, PathNetwork, PathNode, PathNodeType, WaterVolume};
pub use rng::Rng;
pub use sky_layer::{SKY_LAYER_TABLE, SkyLayerSpec, extract_sky_layers, sky_layer_of};
pub use stamp::StampShape;
pub use table::{ColumnSpec, TableSpec, is_valid_name};
pub use value::{MAX_SAFE_INT, MAX_TEXT_BYTES, Value, ValueKind};
pub use weather::{
    EvaluatedWeather, WEATHER_TABLE, WeatherSpec, active_weathers_at, extract_all_weathers,
    weather_of,
};
pub use world::{EntitySnapshot, World};
