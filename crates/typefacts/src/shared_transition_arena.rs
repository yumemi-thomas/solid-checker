use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{MAX_MESSAGE_BYTES, session::SessionError, v3::Response};

const HEADER_SIZE: usize = 32;
const MAGIC: &[u8; 8] = b"STFARENA";
static NEXT_ARENA: AtomicU64 = AtomicU64::new(1);

#[cfg(target_os = "macos")]
fn disable_cache(file: &File) -> Result<(), SessionError> {
    use std::{ffi::c_int, os::fd::AsRawFd};

    const F_NOCACHE: c_int = 48;
    unsafe extern "C" {
        fn fcntl(file_descriptor: c_int, command: c_int, ...) -> c_int;
    }
    // SAFETY: F_NOCACHE takes one integer argument and does not retain it.
    if unsafe { fcntl(file.as_raw_fd(), F_NOCACHE, 1 as c_int) } == -1 {
        return Err(SessionError::Process(format!(
            "disable shared transition cache: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn disable_cache(_file: &File) -> Result<(), SessionError> {
    Ok(())
}

/// Rust owns the lifecycle and validation of one producer-side transition
/// arena. Go may write the file but cannot choose its path, length, request
/// identity, or lifetime.
pub(crate) struct SharedTransitionArena {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl SharedTransitionArena {
    pub(crate) fn create() -> Result<Self, SessionError> {
        for _ in 0..32 {
            let sequence = NEXT_ARENA.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "solid-typefacts-transition-{}-{sequence}",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => {
                    if let Err(error) = disable_cache(&file) {
                        drop(file);
                        let _ = std::fs::remove_file(&path);
                        return Err(error);
                    }
                    return Ok(Self {
                        path,
                        file: Mutex::new(Some(file)),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(SessionError::Process(format!(
                        "create shared transition arena: {error}"
                    )));
                }
            }
        }
        Err(SessionError::Process(
            "could not allocate a unique shared transition arena".into(),
        ))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn attach(&self, response: &mut Response) -> Result<bool, SessionError> {
        if response.source_arena != self.path.to_string_lossy() {
            return Ok(false);
        }
        if !response.table_transition.is_empty() {
            return Err(SessionError::InvalidResponse(
                "shared arena response also carries an inline transition".into(),
            ));
        }
        let [request_id, offset, length] = response.source_lengths.as_slice() else {
            return Err(SessionError::InvalidResponse(
                "shared transition arena descriptor must contain request, offset, and length"
                    .into(),
            ));
        };
        if *request_id != response.request_id {
            return Err(SessionError::InvalidResponse(
                "shared transition arena request identity mismatch".into(),
            ));
        }
        let length = usize::try_from(*length).map_err(|_| {
            SessionError::InvalidResponse("shared transition length overflows usize".into())
        })?;
        if length == 0 || length > MAX_MESSAGE_BYTES {
            return Err(SessionError::InvalidResponse(format!(
                "shared transition length {length} is out of range"
            )));
        }
        let mut file_guard = self.file.lock().map_err(|_| {
            SessionError::Process("shared transition arena lock is poisoned".into())
        })?;
        let file = file_guard.as_mut().ok_or_else(|| {
            SessionError::Process("shared transition arena is already closed".into())
        })?;
        let mut header = [0_u8; HEADER_SIZE];
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.read_exact(&mut header))
            .map_err(|error| {
                SessionError::Process(format!("read transition arena header: {error}"))
            })?;
        if &header[..8] != MAGIC {
            return Err(SessionError::InvalidResponse(
                "shared transition arena magic mismatch".into(),
            ));
        }
        let header_request = u64::from_le_bytes(header[8..16].try_into().expect("fixed range"));
        let header_offset = u64::from_le_bytes(header[16..24].try_into().expect("fixed range"));
        let header_length = u64::from_le_bytes(header[24..32].try_into().expect("fixed range"));
        if header_request != *request_id
            || header_offset != *offset
            || header_length != length as u64
        {
            return Err(SessionError::InvalidResponse(
                "shared transition descriptor does not match its committed header".into(),
            ));
        }
        let file_length = file
            .metadata()
            .map_err(|error| SessionError::Process(format!("stat transition arena: {error}")))?
            .len();
        let end = offset.checked_add(length as u64).ok_or_else(|| {
            SessionError::InvalidResponse("shared transition range overflow".into())
        })?;
        if end > file_length {
            return Err(SessionError::InvalidResponse(
                "shared transition range exceeds its arena".into(),
            ));
        }
        let mut transition = vec![0_u8; length];
        file.seek(SeekFrom::Start(*offset))
            .and_then(|_| file.read_exact(&mut transition))
            .map_err(|error| SessionError::Process(format!("read shared transition: {error}")))?;
        response.table_transition = transition;
        response.source_arena.clear();
        response.source_lengths.clear();
        response.client_response_bytes = response
            .client_response_bytes
            .saturating_add(u64::try_from(length).unwrap_or(u64::MAX));
        Ok(true)
    }
}

impl Drop for SharedTransitionArena {
    fn drop(&mut self) {
        if let Ok(file) = self.file.get_mut() {
            file.take();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom, Write};

    use super::*;
    use crate::v3::{self, Response};

    fn response(
        arena: &SharedTransitionArena,
        request_id: u64,
        offset: u64,
        length: u64,
    ) -> Response {
        Response {
            schema: v3::TYPE_FACTS_SCHEMA_V9,
            request_id,
            project_id: "/p/tsconfig.json".into(),
            generation: 1,
            ok: true,
            table_transition: Vec::new(),
            symbol_evidence: Vec::new(),
            reference_evidence: Vec::new(),
            changed_reference_symbols: Vec::new(),
            reference_changes_exact: false,
            state_token: "1".into(),
            affected: Vec::new(),
            sources: Vec::new(),
            source_arena: arena.path.to_string_lossy().into_owned(),
            source_lengths: vec![request_id, offset, length],
            timings: None,
            error: None,
            client_decode_ns: 0,
            client_response_bytes: 37,
            client_request_send_ns: 0,
            client_request_bytes: 0,
            client_roundtrip_ns: 0,
        }
    }

    fn commit(arena: &SharedTransitionArena, header_request: u64, offset: u64, payload: &[u8]) {
        let mut header = [0_u8; HEADER_SIZE];
        header[..8].copy_from_slice(MAGIC);
        header[8..16].copy_from_slice(&header_request.to_le_bytes());
        header[16..24].copy_from_slice(&offset.to_le_bytes());
        header[24..32].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        let mut file_guard = arena.file.lock().unwrap();
        let file = file_guard.as_mut().unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(payload).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&header).unwrap();
    }

    #[test]
    fn attaches_only_the_committed_request_and_removes_the_arena() {
        let arena = SharedTransitionArena::create().unwrap();
        let path = arena.path.clone();
        commit(&arena, 7, 128, b"packed");
        let mut actual = response(&arena, 7, 128, 6);
        assert!(arena.attach(&mut actual).unwrap());
        assert_eq!(actual.table_transition, b"packed");
        assert!(actual.source_arena.is_empty());
        assert_eq!(actual.client_response_bytes, 43);
        drop(arena);
        assert!(!path.exists());
    }

    #[test]
    fn rejects_a_stale_committed_header() {
        let arena = SharedTransitionArena::create().unwrap();
        commit(&arena, 6, 128, b"old");
        let mut stale = response(&arena, 7, 128, 3);
        assert!(matches!(
            arena.attach(&mut stale),
            Err(SessionError::InvalidResponse(_))
        ));
    }
}
