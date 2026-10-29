use std::fmt;

/// Everything this crate rejects. Messages are in Portuguese, like the rest of the engine.
#[derive(Clone, Debug, PartialEq)]
pub enum TerrainError {
    InvalidCellSize(f32),
    InvalidChunkCells(usize),
    InvalidBrush(&'static str),
    NonFinite(&'static str),
    CoordinateOutOfRange(i64),
    TooManyChunks(usize),
    HeightOutsideLimit(f32),
    UnknownChunk,
}

impl fmt::Display for TerrainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCellSize(cell_size) => write!(
                formatter,
                "o tamanho da célula é {cell_size} m; use um valor entre 0,05 e 64 m"
            ),
            Self::InvalidChunkCells(cells) => write!(
                formatter,
                "o bloco tem {cells} células por lado; use um valor entre 1 e 256"
            ),
            Self::InvalidBrush(reason) => write!(formatter, "pincel inválido: {reason}"),
            Self::NonFinite(what) => write!(formatter, "{what} precisa ser um número finito"),
            Self::CoordinateOutOfRange(value) => {
                write!(
                    formatter,
                    "a coordenada {value} está fora do intervalo aceito"
                )
            }
            Self::TooManyChunks(count) => {
                write!(
                    formatter,
                    "o terreno chegou a {count} blocos; o máximo é 16384"
                )
            }
            Self::HeightOutsideLimit(height) => write!(
                formatter,
                "a altura {height} m passou do limite de ±10000 m"
            ),
            Self::UnknownChunk => formatter.write_str("o bloco não existe neste terreno"),
        }
    }
}

impl std::error::Error for TerrainError {}
