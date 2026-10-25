use serde::{Deserialize, Serialize};

/// Session handle: slot index plus generation. A handle whose generation no longer matches
/// its slot is stale and resolves to an error, never to another entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityHandle {
    index: u32,
    generation: u32,
}

impl EntityHandle {
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Identity that survives saving and loading. Assigned by a per-world counter, so files are
/// deterministic and diff-friendly. `0` is never a valid id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PersistentId(pub u64);
