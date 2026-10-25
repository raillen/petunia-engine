use thiserror::Error;

use crate::value::ValueKind;

/// Everything that can go wrong when touching the world. Each variant has a stable code and a
/// suggestion that says what to do next.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum WorldError {
    #[error("referência de entidade obsoleta ou desconhecida")]
    StaleHandle,
    #[error("não existe entidade com id {0}")]
    UnknownPersistentId(u64),
    #[error("o id {0} já está em uso")]
    DuplicatePersistentId(u64),
    #[error("a tabela `{0}` não existe")]
    UnknownTable(String),
    #[error("a tabela `{0}` já existe")]
    DuplicateTable(String),
    #[error("a tabela `{table}` não tem a coluna `{column}`")]
    UnknownColumn { table: String, column: String },
    #[error("a coluna `{column}` espera {expected:?}, mas recebeu {got:?}")]
    KindMismatch {
        column: String,
        expected: ValueKind,
        got: ValueKind,
    },
    #[error("valor inválido em `{column}`: {reason}")]
    InvalidValue { column: String, reason: String },
    #[error("nome inválido `{0}`")]
    InvalidName(String),
    #[error("a entidade já tem uma linha em `{0}`")]
    RowExists(String),
    #[error("a entidade não tem linha em `{0}`")]
    RowMissing(String),
    #[error("limite excedido: {0}")]
    LimitExceeded(&'static str),
}

impl WorldError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::StaleHandle => "WORLD_STALE_HANDLE",
            Self::UnknownPersistentId(_) => "WORLD_UNKNOWN_ENTITY",
            Self::DuplicatePersistentId(_) => "WORLD_DUPLICATE_ENTITY_ID",
            Self::UnknownTable(_) => "WORLD_UNKNOWN_TABLE",
            Self::DuplicateTable(_) => "WORLD_DUPLICATE_TABLE",
            Self::UnknownColumn { .. } => "WORLD_UNKNOWN_COLUMN",
            Self::KindMismatch { .. } => "WORLD_KIND_MISMATCH",
            Self::InvalidValue { .. } => "WORLD_INVALID_VALUE",
            Self::InvalidName(_) => "WORLD_INVALID_NAME",
            Self::RowExists(_) => "WORLD_ROW_EXISTS",
            Self::RowMissing(_) => "WORLD_ROW_MISSING",
            Self::LimitExceeded(_) => "WORLD_LIMIT_EXCEEDED",
        }
    }

    /// What the person (or script) can do about it.
    pub fn suggestion(&self) -> String {
        match self {
            Self::StaleHandle => {
                "A entidade foi removida. Crie a entidade antes de usá-la ou confira se ela ainda existe."
                    .to_string()
            }
            Self::UnknownPersistentId(id) => {
                format!("Não existe entidade com id {id}. Use `world.find` para listar as entidades.")
            }
            Self::DuplicatePersistentId(id) => {
                format!("O id {id} já pertence a outra entidade. Deixe o mundo escolher o id.")
            }
            Self::UnknownTable(name) => {
                format!("A tabela `{name}` não existe. Confira o nome (as tabelas começam com maiúscula).")
            }
            Self::DuplicateTable(name) => format!("A tabela `{name}` já foi definida."),
            Self::UnknownColumn { table, column } => {
                format!("A tabela `{table}` não tem a coluna `{column}`. Confira o nome da coluna.")
            }
            Self::KindMismatch { column, expected, .. } => {
                format!("A coluna `{column}` espera um valor do tipo {expected:?}.")
            }
            Self::InvalidValue { column, reason } => {
                format!("Corrija o valor de `{column}`: {reason}.")
            }
            Self::InvalidName(name) => format!(
                "O nome `{name}` é inválido. Use letras, números e `_`, começando por letra, com até 64 caracteres."
            ),
            Self::RowExists(table) => {
                format!("A entidade já tem `{table}`. Use `world.set` para mudar um valor.")
            }
            Self::RowMissing(table) => {
                format!("A entidade não tem `{table}`. Use `row.create` para adicioná-la.")
            }
            Self::LimitExceeded(what) => {
                format!("O limite de {what} foi atingido. Remova itens ou reduza o tamanho.")
            }
        }
    }
}
