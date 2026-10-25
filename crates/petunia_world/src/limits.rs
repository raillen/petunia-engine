/// Hard limits. Files, scripts and network messages are hostile input, so every collection
/// the world grows from outside data has a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_entities: usize,
    pub max_tables: usize,
    pub max_columns_per_table: usize,
    pub max_tags_per_entity: usize,
    pub max_text_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entities: 100_000,
            max_tables: 128,
            max_columns_per_table: 32,
            max_tags_per_entity: 16,
            max_text_bytes: crate::value::MAX_TEXT_BYTES,
        }
    }
}
