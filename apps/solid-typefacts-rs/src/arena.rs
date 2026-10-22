//! The Rust-owned transition arena (`-transition-arena`). S1 only opens the
//! file, as Go does at startup, so a bad path fails the same way. Writing rows
//! into it arrives with the transition encoder (S2).
//!
//! Go also sets `F_NOCACHE` on macOS here; that needs `unsafe`, which this
//! crate forbids. S2 should obtain it from the client crate's
//! `unsafe_boundary` (expose it behind a `producer` feature).

use crate::gocompat::io_message;
use std::fs::{File, OpenOptions};

#[derive(Debug)]
pub struct TransitionArena {
    pub path: String,
    file: File,
}

impl TransitionArena {
    #[must_use]
    pub fn file(&self) -> &File {
        &self.file
    }
}

/// Opens the arena read-write; an empty path means no arena.
pub fn open(path: &str) -> Result<Option<TransitionArena>, String> {
    if path.is_empty() {
        return Ok(None);
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("open transition arena: open {path}: {}", io_message(&error)))?;
    Ok(Some(TransitionArena {
        path: path.to_owned(),
        file,
    }))
}
