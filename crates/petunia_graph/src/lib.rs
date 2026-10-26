//! Node graphs for game logic (DR-30).
//!
//! A graph is a typed document ([`GraphDocument`]). The set of nodes it may use is a [`Catalog`]
//! generated from the script API, so a node is exactly one API function. [`compile`] checks a graph
//! and writes it as Aipo text with a map from lines back to nodes; [`validate`] reports every
//! problem in plain Portuguese, aimed at a node and pin.
//!
//! Domain crate: no scripting, rendering, windowing or file access.

mod catalog;
mod compile;
mod model;

pub use catalog::{
    ApiSpec, Catalog, DataType, Direction, EventSpec, Lowering, NodeClass, NodeSpec, PinDefault,
    PinKind, PinSpec, SettingChoices, SettingSpec,
};
pub use compile::{Compiled, Diagnostic, Report, Severity, check, compile, literal, validate};
pub use model::{
    GraphDocument, GraphEdge, GraphError, GraphNode, MAX_EDGES, MAX_FILE_BYTES, MAX_NAME_BYTES,
    MAX_NODES, MAX_TEXT_BYTES, PinRef, SCHEMA_VERSION,
};
