use petunia_commands::CommandError;
use petunia_project::ProjectError;
use petunia_world::WorldError;
use serde_json::{Value as Json, json};

/// Why an editor operation did not happen: a stable code, what went wrong and what to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: String,
    pub message: String,
    pub action: String,
}

impl Failure {
    pub fn new(code: &str, message: &str, action: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            action: action.to_string(),
        }
    }

    pub fn no_project() -> Self {
        Self::new(
            "EDITOR_NO_PROJECT",
            "nenhum projeto está aberto",
            "Use Arquivo > Novo projeto de exemplo, ou Arquivo > Abrir projeto.",
        )
    }

    pub fn bad_request(message: &str) -> Self {
        Self::new(
            "EDITOR_BAD_REQUEST",
            message,
            "Confira o campo indicado e tente de novo.",
        )
    }

    pub fn to_json(&self) -> Json {
        json!({ "code": self.code, "message": self.message, "action": self.action })
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for Failure {}

impl From<CommandError> for Failure {
    fn from(error: CommandError) -> Self {
        Self::new(error.code(), &error.to_string(), &error.suggestion())
    }
}

impl From<WorldError> for Failure {
    fn from(error: WorldError) -> Self {
        Self::new(error.code(), &error.to_string(), &error.suggestion())
    }
}

impl From<ProjectError> for Failure {
    fn from(error: ProjectError) -> Self {
        let (code, action) = match &error {
            ProjectError::Io { .. } => (
                "PROJECT_IO",
                "Confira se a pasta existe e se você tem permissão.",
            ),
            ProjectError::TooLarge { .. } => {
                ("PROJECT_TOO_LARGE", "Divida a cena em arquivos menores.")
            }
            ProjectError::InvalidPath(_) => (
                "PROJECT_INVALID_PATH",
                "Use um caminho relativo que fique dentro do projeto.",
            ),
            ProjectError::AlreadyExists(_) => (
                "PROJECT_ALREADY_EXISTS",
                "Escolha uma pasta vazia ou abra o projeto existente.",
            ),
            ProjectError::InvalidName(_) => (
                "PROJECT_INVALID_NAME",
                "Use de 1 a 80 caracteres, sem caracteres de controle.",
            ),
            ProjectError::Serialize(_) => (
                "PROJECT_SERIALIZE",
                "Isso é um defeito da engine; registre o problema.",
            ),
            ProjectError::World(_) => ("PROJECT_WORLD", "Confira os valores da cena."),
        };
        Self::new(code, &error.to_string(), action)
    }
}
