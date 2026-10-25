//! The engine API described once, as data.
//!
//! Every command, query and event is an entry in a [`Schema`]. From this single source the
//! engine generates script bindings, visual-logic nodes, documentation, autocomplete, MCP tools
//! and network validation. Library types never appear here.

mod naming;
mod params;
mod spec;

pub use naming::{IdKind, validate_id};
pub use params::check_params;
pub use spec::{
    CommandSpec, Cost, EventSpec, FailureSpec, ParamKind, ParamSpec, QuerySpec, Reversibility,
    Schema, SchemaError, Status,
};

/// Version of the schema document format.
pub const SCHEMA_VERSION: u32 = 1;
