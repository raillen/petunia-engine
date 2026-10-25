use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProjectError {
    #[error("não foi possível acessar `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("`{path}` passa do limite de {max_bytes} bytes")]
    TooLarge { path: String, max_bytes: u64 },
    #[error("caminho inválido `{0}`: precisa ser relativo e ficar dentro do projeto")]
    InvalidPath(String),
    #[error("já existe um projeto em `{0}`; escolha outra pasta")]
    AlreadyExists(String),
    #[error("nome de projeto inválido: {0}")]
    InvalidName(String),
    #[error("não foi possível gravar o projeto: {0}")]
    Serialize(String),
    #[error("erro no mundo: {0}")]
    World(String),
}

impl From<petunia_world::WorldError> for ProjectError {
    fn from(error: petunia_world::WorldError) -> Self {
        Self::World(format!("{error} ({})", error.code()))
    }
}
