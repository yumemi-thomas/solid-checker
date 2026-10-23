//! The Rust-owned transition arena (`-transition-arena`), Go's
//! `transition_arena.go`. The client creates the file and owns its lifetime; the
//! producer opens it at startup, so a bad path fails the same way, and writes
//! one committed transition into it per `analyze` or `symbols` request.
//!
//! Layout: a 32-byte header at offset 0 (`STFARENA`, request id, offset, length;
//! little-endian u64s), then, at a fixed sparse-file offset of 64 MiB, the row
//! run. The header and dictionary prefix are written last, immediately before
//! the rows, so the completed row run never shifts or copies. The header is the
//! commit: a reader that sees another request's id, or none, ignores the file.
//!
//! macOS gets `F_NOCACHE` through `typefacts::producer`, the client crate's one
//! `unsafe` boundary; this crate's own `unsafe` lives in `unsafe_boundary` alone.

use crate::gocompat::io_message;
use std::fs::{File, OpenOptions};
use std::io;
use typefacts::producer::TransitionSink;

/// Where the row run starts.
pub const ROWS_OFFSET: u64 = 64 << 20;
pub const HEADER_SIZE: usize = 32;
pub const MAGIC: [u8; 8] = *b"STFARENA";

#[derive(Debug)]
pub struct TransitionArena {
    path: String,
    file: File,
    request_id: u64,
    row_bytes: u64,
    offset: u64,
    length: u64,
    committed: bool,
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
    typefacts::producer::disable_file_cache(&file)
        .map_err(|error| format!("disable transition arena cache: {}", io_message(&error)))?;
    Ok(Some(TransitionArena {
        path: path.to_owned(),
        file,
        request_id: 0,
        row_bytes: 0,
        offset: 0,
        length: 0,
        committed: false,
    }))
}

impl TransitionArena {
    #[must_use]
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Starts a new transition for `request_id`: shrinks the file to the rows
    /// offset (dropping the previous transition), and clears the header so a
    /// half-written transition is never mistaken for a committed one.
    pub fn begin(&mut self, request_id: u64) -> Result<(), String> {
        self.request_id = request_id;
        self.row_bytes = 0;
        self.offset = 0;
        self.length = 0;
        self.committed = false;
        self.file
            .set_len(ROWS_OFFSET)
            .map_err(|error| format!("reset transition arena: {}", io_message(&error)))?;
        write_at(&self.file, &[0; HEADER_SIZE], 0)
            .map_err(|error| format!("clear transition arena header: {}", io_message(&error)))
    }

    /// The `(sourceArena, sourceLengths)` pair that tells the client where this
    /// request's committed transition lives; `None` until `finish` committed.
    #[must_use]
    pub fn descriptor(&self) -> Option<(String, Vec<u64>)> {
        self.committed.then(|| {
            (
                self.path.clone(),
                vec![self.request_id, self.offset, self.length],
            )
        })
    }
}

impl TransitionSink for TransitionArena {
    fn append_rows(&mut self, chunk: &[u8]) -> io::Result<()> {
        write_at(&self.file, chunk, ROWS_OFFSET + self.row_bytes)?;
        self.row_bytes += chunk.len() as u64;
        Ok(())
    }

    fn finish(&mut self, prefix: &[u8]) -> io::Result<()> {
        if prefix.is_empty() || prefix.len() as u64 > ROWS_OFFSET - HEADER_SIZE as u64 {
            return Err(io::Error::other(format!(
                "transition prefix length {} is out of range",
                prefix.len()
            )));
        }
        let offset = ROWS_OFFSET - prefix.len() as u64;
        write_at(&self.file, prefix, offset)
            .map_err(|error| io::Error::other(format!("write transition arena prefix: {error}")))?;
        let length = prefix.len() as u64 + self.row_bytes;
        let mut header = [0_u8; HEADER_SIZE];
        header[..8].copy_from_slice(&MAGIC);
        header[8..16].copy_from_slice(&self.request_id.to_le_bytes());
        header[16..24].copy_from_slice(&offset.to_le_bytes());
        header[24..32].copy_from_slice(&length.to_le_bytes());
        write_at(&self.file, &header, 0)
            .map_err(|error| io::Error::other(format!("commit transition arena: {error}")))?;
        self.offset = offset;
        self.length = length;
        self.committed = true;
        Ok(())
    }
}

#[cfg(unix)]
fn write_at(file: &File, bytes: &[u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, bytes, offset)
}

/// WASI has no stable positional write; the reactor never opens an arena, so
/// this seek-and-write only keeps the module compiling.
#[cfg(target_os = "wasi")]
fn write_at(mut file: &File, bytes: &[u8], offset: u64) -> io::Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(bytes)
}

#[cfg(windows)]
fn write_at(file: &File, mut bytes: &[u8], mut offset: u64) -> io::Result<()> {
    while !bytes.is_empty() {
        let written = std::os::windows::fs::FileExt::seek_write(file, bytes, offset)?;
        if written == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        bytes = &bytes[written..];
        offset += written as u64;
    }
    Ok(())
}
