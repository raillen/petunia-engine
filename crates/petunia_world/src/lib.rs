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
mod error;
mod heightfield;
mod ids;
mod limits;
mod path_network;
mod rng;
mod stamp;
mod table;
mod value;
mod world;

pub use buffer::{CommandBuffer, DeferredOp};
pub use error::WorldError;
pub use heightfield::{HEIGHT_CHUNK_SIZE, HeightChunk, Heightfield};
pub use ids::{EntityHandle, PersistentId};
pub use limits::Limits;
pub use path_network::{PathEdge, PathNetwork, PathNode, PathNodeType, WaterVolume};
pub use rng::Rng;
pub use stamp::StampShape;
pub use table::{ColumnSpec, TableSpec, is_valid_name};
pub use value::{MAX_SAFE_INT, MAX_TEXT_BYTES, Value, ValueKind};
pub use world::{EntitySnapshot, World};
