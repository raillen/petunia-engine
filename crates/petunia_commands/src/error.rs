use petunia_schema::SchemaError;
use petunia_world::WorldError;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Error)]
pub enum CommandError {
    #[error("comando desconhecido `{name}`")]
    UnknownCommand {
        name: String,
        suggestions: Vec<String>,
    },
    #[error("payload inválido para `{command}`: {message}")]
    InvalidPayload { command: String, message: String },
    #[error(transparent)]
    World(#[from] WorldError),
    #[error("nada para desfazer")]
    NothingToUndo,
    #[error("nada para refazer")]
    NothingToRedo,
    #[error("não foi possível registrar o comando: {0}")]
    Registration(String),
}

impl From<SchemaError> for CommandError {
    fn from(error: SchemaError) -> Self {
        Self::Registration(error.to_string())
    }
}

impl CommandError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownCommand { .. } => "COMMAND_UNKNOWN",
            Self::InvalidPayload { .. } => "COMMAND_INVALID_PAYLOAD",
            Self::World(error) => error.code(),
            Self::NothingToUndo => "COMMAND_NOTHING_TO_UNDO",
            Self::NothingToRedo => "COMMAND_NOTHING_TO_REDO",
            Self::Registration(_) => "COMMAND_REGISTRATION",
        }
    }

    /// What to do about it, in plain language.
    pub fn suggestion(&self) -> String {
        match self {
            Self::UnknownCommand { suggestions, .. } if !suggestions.is_empty() => {
                format!("Você quis dizer: {}?", suggestions.join(", "))
            }
            Self::UnknownCommand { .. } => {
                "Use `petunia schema` para listar os comandos disponíveis.".to_string()
            }
            Self::InvalidPayload { message, .. } => format!("Corrija o payload: {message}."),
            Self::World(error) => error.suggestion(),
            Self::NothingToUndo => "Ainda não há nada para desfazer.".to_string(),
            Self::NothingToRedo => "Ainda não há nada para refazer.".to_string(),
            Self::Registration(reason) => format!("Corrija o registro do comando: {reason}."),
        }
    }
}
