//! Validated commands and queries.
//!
//! A command is a **data record** (`{ "command": "...", "payload": { ... } }`) plus a function
//! registered in a table. Because the record is plain data it can travel over the network,
//! through MCP, into macros and into the undo history. The inverse of a command is itself a
//! command record, so undo and redo are just "apply the inverse".
//!
//! Every user-, script- or agent-facing mutation goes through the [`Dispatcher`]. Internal
//! systems and per-frame updates are not commands.

mod builtin;
mod dispatcher;
mod error;
mod query;

pub use builtin::{builtin_query_specs, builtin_schema};
pub use dispatcher::{BATCH_COMMAND, CommandRecord, Dispatcher, MAX_BATCH, MAX_HISTORY, Outcome};
pub use error::CommandError;
pub use query::{QueryRecord, run_query};
