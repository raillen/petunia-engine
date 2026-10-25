use std::collections::BTreeMap;

use crate::error::WorldError;
use crate::ids::EntityHandle;
use crate::limits::Limits;
use crate::value::{Value, ValueKind};

/// Names of tables, columns and tags: ASCII letters, digits and `_`, starting with a letter,
/// at most 64 characters.
pub fn is_valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    name.len() <= 64
        && first.is_ascii_alphabetic()
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// One typed column with its default value.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnSpec {
    pub name: String,
    pub kind: ValueKind,
    pub default: Value,
}

/// Definition of a table: a name and its ordered columns.
#[derive(Clone, Debug, PartialEq)]
pub struct TableSpec {
    pub name: String,
    pub columns: Vec<ColumnSpec>,
}

impl TableSpec {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            columns: Vec::new(),
        }
    }

    /// Builder: appends a column. The default's kind is checked later by [`TableSpec::validate`].
    pub fn column(mut self, name: &str, kind: ValueKind, default: Value) -> Self {
        self.columns.push(ColumnSpec {
            name: name.to_string(),
            kind,
            default,
        });
        self
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|column| column.name == name)
    }

    pub fn validate(&self, limits: &Limits) -> Result<(), WorldError> {
        if !is_valid_name(&self.name) {
            return Err(WorldError::InvalidName(self.name.clone()));
        }
        if self.columns.len() > limits.max_columns_per_table {
            return Err(WorldError::LimitExceeded("columns per table"));
        }
        for (position, column) in self.columns.iter().enumerate() {
            if !is_valid_name(&column.name) {
                return Err(WorldError::InvalidName(column.name.clone()));
            }
            if self
                .columns
                .iter()
                .take(position)
                .any(|earlier| earlier.name == column.name)
            {
                return Err(WorldError::InvalidName(format!(
                    "{} (duplicate column)",
                    column.name
                )));
            }
            if column.default.kind() != column.kind {
                return Err(WorldError::KindMismatch {
                    column: column.name.clone(),
                    expected: column.kind,
                    got: column.default.kind(),
                });
            }
            column
                .default
                .validate(limits.max_text_bytes)
                .map_err(|reason| WorldError::InvalidValue {
                    column: column.name.clone(),
                    reason,
                })?;
        }
        Ok(())
    }
}

/// Storage of one table: rows keyed by entity handle, so iteration is deterministic.
#[derive(Clone, Debug)]
pub(crate) struct Table {
    pub spec: TableSpec,
    pub rows: BTreeMap<EntityHandle, Vec<Value>>,
}
