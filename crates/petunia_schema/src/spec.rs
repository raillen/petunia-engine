use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::SCHEMA_VERSION;
use crate::naming::{IdKind, validate_id};

/// How far a feature can be trusted (AGENTS.md: distinguish verified, provisional,
/// approximation and planned).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Verified,
    Provisional,
    Approximation,
    Planned,
}

/// Whether a call is cheap or counts against a budget (physics, ropes, particles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    Cheap,
    Budgeted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reversibility {
    Undoable,
    NotUndoable,
}

/// Type of a parameter or result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Bool,
    Int,
    Float,
    Text,
    Vec3,
    Color,
    /// Persistent entity id.
    Entity,
    TextList,
    /// Object of table name to object of column values.
    Rows,
    Any,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    pub kind: ParamKind,
    pub required: bool,
    pub unit: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub summary: String,
}

impl ParamSpec {
    pub fn required(name: &str, kind: ParamKind, summary: &str) -> Self {
        Self::new(name, kind, true, summary)
    }

    pub fn optional(name: &str, kind: ParamKind, summary: &str) -> Self {
        Self::new(name, kind, false, summary)
    }

    fn new(name: &str, kind: ParamKind, required: bool, summary: &str) -> Self {
        Self {
            name: name.to_string(),
            kind,
            required,
            unit: None,
            minimum: None,
            maximum: None,
            summary: summary.to_string(),
        }
    }

    pub fn with_unit(mut self, unit: &str) -> Self {
        self.unit = Some(unit.to_string());
        self
    }

    pub fn with_range(mut self, minimum: f64, maximum: f64) -> Self {
        self.minimum = Some(minimum);
        self.maximum = Some(maximum);
        self
    }
}

/// A way a call can fail, with a stable code and what to do about it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FailureSpec {
    pub code: String,
    pub suggestion: String,
}

impl FailureSpec {
    pub fn new(code: &str, suggestion: &str) -> Self {
        Self {
            code: code.to_string(),
            suggestion: suggestion.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub id: String,
    pub summary: String,
    pub params: Vec<ParamSpec>,
    pub returns: Option<ParamKind>,
    pub failures: Vec<FailureSpec>,
    pub permission: String,
    pub cost: Cost,
    pub reversibility: Reversibility,
    pub status: Status,
    pub example: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuerySpec {
    pub id: String,
    pub summary: String,
    pub params: Vec<ParamSpec>,
    pub returns: ParamKind,
    pub failures: Vec<FailureSpec>,
    pub permission: String,
    pub status: Status,
    pub example: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventSpec {
    pub id: String,
    pub summary: String,
    pub status: Status,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum SchemaError {
    #[error("invalid id `{id}`: {reason}")]
    InvalidId { id: String, reason: String },
    #[error("duplicate id `{0}`")]
    DuplicateId(String),
    #[error("`{id}` has an empty or multi-line `{field}`")]
    BadText { id: String, field: &'static str },
    #[error("`{id}` declares a failure code `{code}` that is not UPPER_SNAKE_CASE")]
    BadFailureCode { id: String, code: String },
    #[error("`{id}` declares a failure `{code}` without a suggestion")]
    MissingSuggestion { id: String, code: String },
}

/// The whole engine API.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub version: u32,
    pub commands: Vec<CommandSpec>,
    pub queries: Vec<QuerySpec>,
    pub events: Vec<EventSpec>,
}

impl Schema {
    pub fn new() -> Self {
        Self {
            version: SCHEMA_VERSION,
            commands: Vec::new(),
            queries: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn command(&self, id: &str) -> Option<&CommandSpec> {
        self.commands.iter().find(|spec| spec.id == id)
    }

    pub fn query(&self, id: &str) -> Option<&QuerySpec> {
        self.queries.iter().find(|spec| spec.id == id)
    }

    /// Checks naming, uniqueness and documentation of every entry.
    pub fn validate(&self) -> Result<(), SchemaError> {
        let mut seen = std::collections::BTreeSet::new();
        for spec in &self.commands {
            validate_entry(
                &spec.id,
                IdKind::Command,
                &spec.summary,
                &spec.example,
                &spec.failures,
            )?;
            unique(&mut seen, &spec.id)?;
        }
        for spec in &self.queries {
            validate_entry(
                &spec.id,
                IdKind::Query,
                &spec.summary,
                &spec.example,
                &spec.failures,
            )?;
            unique(&mut seen, &spec.id)?;
        }
        for spec in &self.events {
            validate_id(&spec.id, IdKind::Event)?;
            if !is_one_line(&spec.summary) {
                return Err(SchemaError::BadText {
                    id: spec.id.clone(),
                    field: "summary",
                });
            }
            unique(&mut seen, &spec.id)?;
        }
        Ok(())
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

fn unique(seen: &mut std::collections::BTreeSet<String>, id: &str) -> Result<(), SchemaError> {
    if seen.insert(id.to_string()) {
        Ok(())
    } else {
        Err(SchemaError::DuplicateId(id.to_string()))
    }
}

fn is_one_line(text: &str) -> bool {
    !text.trim().is_empty() && !text.contains('\n')
}

fn validate_entry(
    id: &str,
    kind: IdKind,
    summary: &str,
    example: &str,
    failures: &[FailureSpec],
) -> Result<(), SchemaError> {
    validate_id(id, kind)?;
    if !is_one_line(summary) {
        return Err(SchemaError::BadText {
            id: id.to_string(),
            field: "summary",
        });
    }
    if example.trim().is_empty() {
        return Err(SchemaError::BadText {
            id: id.to_string(),
            field: "example",
        });
    }
    for failure in failures {
        let upper_snake = !failure.code.is_empty()
            && failure
                .code
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !upper_snake {
            return Err(SchemaError::BadFailureCode {
                id: id.to_string(),
                code: failure.code.clone(),
            });
        }
        if failure.suggestion.trim().is_empty() {
            return Err(SchemaError::MissingSuggestion {
                id: id.to_string(),
                code: failure.code.clone(),
            });
        }
    }
    Ok(())
}
