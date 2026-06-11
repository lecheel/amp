//--+ src/models/application/buffer_metadata.rs

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BufferType {
    /// Normal file-backed buffer. User can edit and save.
    #[default]
    Normal,
    /// Virtual/scratch buffer. Programmatically updated, never saved,
    /// never shows as modified. No file backing.
    Virtual,
    /// Readonly buffer. User cannot edit or save. Never shows as modified.
    /// May or may not have file backing.
    Readonly,
    SedDiff,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BufferMetadata {
    pub buffer_type: BufferType,
}

/// Tracks per-buffer metadata keyed by buffer ID.
#[derive(Default)]
pub struct BufferRegistry {
    entries: HashMap<usize, BufferMetadata>,
}

impl BufferRegistry {
    pub fn register(&mut self, id: Option<usize>, meta: BufferMetadata) {
        if let Some(id) = id {
            self.entries.insert(id, meta);
        }
    }

    pub fn is_sed_diff(&self, id: Option<usize>) -> bool {
        self.get(id)
            .map_or(false, |m| m.buffer_type == BufferType::SedDiff)
    }

    pub fn unregister(&mut self, id: Option<usize>) {
        if let Some(id) = id {
            self.entries.remove(&id);
        }
    }

    pub fn get(&self, id: Option<usize>) -> Option<&BufferMetadata> {
        id.and_then(|id| self.entries.get(&id))
    }

    pub fn is_virtual(&self, id: Option<usize>) -> bool {
        self.get(id)
            .map_or(false, |m| m.buffer_type == BufferType::Virtual)
    }

    pub fn is_readonly(&self, id: Option<usize>) -> bool {
        self.get(id)
            .map_or(false, |m| m.buffer_type == BufferType::Readonly)
    }

    /// Returns true if the buffer can be edited by the user.
    /// Unregistered buffers default to editable (Normal).
    pub fn is_editable(&self, id: Option<usize>) -> bool {
        self.get(id)
            .map_or(true, |m| m.buffer_type == BufferType::Normal)
    }
}
