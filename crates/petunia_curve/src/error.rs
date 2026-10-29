use std::fmt;

/// Everything this crate rejects. Messages are in Portuguese, like the rest of the engine.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveError {
    /// How many points (or profile vertices) came, and what the limit is.
    TooManyPoints {
        count: usize,
        maximum: usize,
    },
    /// A curve needs at least two points; a profile needs at least two vertices.
    TooFewPoints(usize),
    NonFinite(&'static str),
    /// A frame could not be built because the curve has no direction there.
    Degenerate(&'static str),
    /// The requested step count is outside `1 ..= MAX_SWEEP_STEPS`.
    InvalidSteps(usize),
    /// A profile vertex was written with no width.
    DegenerateProfile(&'static str),
    /// The swept mesh failed the mesh rules (should not happen with the caps above).
    MeshInvalid(String),
}

impl fmt::Display for CurveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyPoints { count, maximum } => {
                write!(formatter, "{count} pontos; o máximo é {maximum}")
            }
            Self::TooFewPoints(count) => write!(
                formatter,
                "são precisos pelo menos 2 pontos, e vieram {count}"
            ),
            Self::NonFinite(what) => write!(formatter, "{what} precisa ser um número finito"),
            Self::Degenerate(what) => write!(formatter, "{what} não tem direção"),
            Self::InvalidSteps(steps) => {
                write!(formatter, "{steps} passos; use um valor entre 1 e 512")
            }
            Self::DegenerateProfile(what) => write!(formatter, "perfil inválido: {what}"),
            Self::MeshInvalid(reason) => write!(formatter, "malha gerada inválida: {reason}"),
        }
    }
}

impl std::error::Error for CurveError {}
