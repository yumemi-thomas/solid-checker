//! Per-project daemon holding the retained `NativeIncrementalSession` behind
//! a Unix socket, so repeat CLI checks reuse the warm session instead of
//! rebuilding the TypeScript program and demand closure from scratch.
//!
//! Opt-in: clients use it only when `SOLID_CHECKER_DAEMON=1`. The socket path is
//! derived from the canonical project id. Before every answer the daemon
//! resynchronizes with the filesystem: a changed tsconfig, a changed source
//! directory (file created, deleted, or renamed), or an unreadable known file
//! rebuilds the whole session; changed file contents become incremental
//! overlay updates. The response body is byte-identical to one-shot output.
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::fs::MetadataExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solid_facts_backend::{
    NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession, analyze_project,
    discovered_contract_paths, imported_package_roots,
};

use super::{Request, snapshot_emission};
use crate::daemon_cache::{CachedAnswer, CachedSnapshot, ContractFile};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckRequest {
    project_id: String,
    #[serde(default)]
    contract_paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckHeader {
    ok: bool,
    #[serde(default)]
    status: String,
    #[serde(default)]
    error: String,
}

pub fn enabled() -> bool {
    std::env::var("SOLID_CHECKER_DAEMON").is_ok_and(|value| value == "1" || value == "true")
}

pub fn eligible(request: &Request) -> bool {
    request.sources.is_empty()
        && request.emit_contract.is_empty()
        && !request.check_contracts
        && matches!(request.format.as_str(), "json" | "text")
}

fn socket_path(project_id: &str) -> PathBuf {
    let digest = Sha256::digest(project_id.as_bytes());
    let mut name = String::from("solid-checker-");
    for byte in &digest[..8] {
        name.push_str(&format!("{byte:02x}"));
    }
    std::env::temp_dir().join(format!("{name}.sock"))
}

fn idle_limit() -> Duration {
    let seconds = std::env::var("SOLID_CHECKER_DAEMON_IDLE_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(600);
    Duration::from_secs(seconds.max(1))
}

struct State {
    project: PathBuf,
    session: NativeIncrementalSession,
    sources: Vec<SourceFile>,
    fingerprints: BTreeMap<String, FileFingerprint>,
    dirs: BTreeMap<PathBuf, Option<FileStamp>>,
    tsconfig: FileFingerprint,
    last: Option<CachedAnswer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileStamp {
    device: u64,
    inode: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileStamp {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileFingerprint {
    stamp: FileStamp,
    hash: [u8; 32],
}

enum FileRefresh {
    Unchanged,
    MetadataOnly(FileFingerprint),
    Content {
        fingerprint: FileFingerprint,
        bytes: Vec<u8>,
    },
}

enum Sync {
    Ready(Vec<SourceChange>),
    Rebuild,
}

impl State {
    fn open(request: &Request) -> Result<Self, Box<dyn Error>> {
        let typescript =
            TypeFactsSession::open(&request.typefacts_executable, &request.project_id, &[])?;
        let (session, configured) =
            NativeIncrementalSession::open_pipelined(request.project_id.clone(), typescript)?;
        let project = PathBuf::from(&request.project_id);
        let tsconfig = fingerprint_file(&project)?;
        let mut sources = configured;
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        let mut fingerprints = BTreeMap::new();
        let mut dirs = BTreeMap::new();
        if let Some(parent) = project.parent() {
            dirs.insert(parent.to_path_buf(), directory_stamp(parent));
        }
        for source in &sources {
            fingerprints.insert(
                source.path.clone(),
                fingerprint_bytes(Path::new(&source.path), source.source.as_bytes())?,
            );
            if let Some(parent) = PathBuf::from(&source.path).parent() {
                dirs.entry(parent.to_path_buf())
                    .or_insert_with(|| directory_stamp(parent));
            }
        }
        Ok(Self {
            project,
            session,
            sources,
            fingerprints,
            dirs,
            tsconfig,
            last: None,
        })
    }

    /// Reconcile the retained session with the filesystem. Content edits
    /// to known files become overlay updates; anything that can change
    /// the project's file set demands a full rebuild.
    fn resync(&mut self) -> Result<Sync, Box<dyn Error>> {
        match refresh_file_with(&self.project, &self.tsconfig, |path| fs::read(path))? {
            FileRefresh::Unchanged => {}
            FileRefresh::MetadataOnly(fingerprint) => self.tsconfig = fingerprint,
            FileRefresh::Content { .. } => return Ok(Sync::Rebuild),
        }
        for (dir, recorded) in &self.dirs {
            if directory_stamp(dir) != *recorded {
                return Ok(Sync::Rebuild);
            }
        }
        let mut changes = Vec::new();
        let paths = self.fingerprints.keys().cloned().collect::<Vec<_>>();
        for path in paths {
            let recorded = self.fingerprints[&path];
            let refresh =
                match refresh_file_with(Path::new(&path), &recorded, |path| fs::read(path)) {
                    Ok(refresh) => refresh,
                    Err(_) => return Ok(Sync::Rebuild),
                };
            match refresh {
                FileRefresh::Unchanged => {}
                FileRefresh::MetadataOnly(fingerprint) => {
                    self.fingerprints.insert(path, fingerprint);
                }
                FileRefresh::Content { fingerprint, bytes } => {
                    let text = String::from_utf8(bytes)?;
                    changes.push(SourceChange {
                        path: path.clone(),
                        version: self.session.generation() + 1,
                        source: Some(text.clone()),
                        compiler_options: Default::default(),
                    });
                    self.fingerprints.insert(path.clone(), fingerprint);
                    let index = self
                        .sources
                        .binary_search_by(|source| source.path.as_str().cmp(path.as_str()))
                        .map_err(|_| format!("configured source disappeared from state: {path}"))?;
                    self.sources[index] = SourceFile {
                        path,
                        source: text,
                        compiler_options: Default::default(),
                    };
                }
            }
        }
        Ok(Sync::Ready(changes))
    }
}

fn content_hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hash_file(path: &std::path::Path) -> Result<[u8; 32], Box<dyn Error>> {
    Ok(content_hash(&fs::read(path)?))
}

fn file_stamp(path: &Path) -> io::Result<FileStamp> {
    fs::metadata(path).map(|metadata| FileStamp::from_metadata(&metadata))
}

fn fingerprint_bytes(path: &Path, bytes: &[u8]) -> Result<FileFingerprint, Box<dyn Error>> {
    Ok(FileFingerprint {
        stamp: file_stamp(path)?,
        hash: content_hash(bytes),
    })
}

fn fingerprint_file(path: &Path) -> Result<FileFingerprint, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    fingerprint_bytes(path, &bytes)
}

fn refresh_file_with(
    path: &Path,
    recorded: &FileFingerprint,
    read: impl FnOnce(&Path) -> io::Result<Vec<u8>>,
) -> Result<FileRefresh, Box<dyn Error>> {
    let stamp = file_stamp(path)?;
    if stamp == recorded.stamp {
        return Ok(FileRefresh::Unchanged);
    }
    let bytes = read(path)?;
    let fingerprint = FileFingerprint {
        stamp,
        hash: content_hash(&bytes),
    };
    if fingerprint.hash == recorded.hash {
        Ok(FileRefresh::MetadataOnly(fingerprint))
    } else {
        Ok(FileRefresh::Content { fingerprint, bytes })
    }
}

fn directory_stamp(path: &Path) -> Option<FileStamp> {
    file_stamp(path).ok()
}

pub fn serve(request: &Request) -> Result<i32, Box<dyn Error>> {
    let socket = socket_path(&request.project_id);
    if UnixStream::connect(&socket).is_ok() {
        return Ok(0); // a live daemon already serves this project
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    let mut state = State::open(request)?;
    // Blocking accept keeps request latency free of poll sleeps; a
    // watchdog thread ends the whole process after the idle limit.
    let idle = idle_limit();
    let last_activity = std::sync::Arc::new(std::sync::Mutex::new(Instant::now()));
    let watchdog_activity = std::sync::Arc::clone(&last_activity);
    let watchdog_socket = socket.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5).min(idle));
            let idle_for = watchdog_activity
                .lock()
                .map(|instant| instant.elapsed())
                .unwrap_or(idle);
            if idle_for >= idle {
                let _ = fs::remove_file(&watchdog_socket);
                std::process::exit(0);
            }
        }
    });
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Ok(mut instant) = last_activity.lock() {
                    *instant = Instant::now();
                }
                if let Err(error) = handle(&mut state, request, stream) {
                    eprintln!("solid-checker daemon: {error}");
                }
                if let Ok(mut instant) = last_activity.lock() {
                    *instant = Instant::now();
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&socket);
                return Err(error.into());
            }
        }
    }
}

fn handle(state: &mut State, request: &Request, stream: UnixStream) -> Result<(), Box<dyn Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let check: CheckRequest = serde_json::from_str(&line)?;
    let mut stream = reader.into_inner();
    if check.project_id != request.project_id {
        return respond_error(&mut stream, "daemon serves a different project");
    }
    let outcome = answer(state, request, &check);
    match outcome {
        Ok((status, body)) => {
            let header = serde_json::to_vec(&CheckHeader {
                ok: true,
                status,
                error: String::new(),
            })?;
            stream.write_all(&header)?;
            stream.write_all(b"\n")?;
            stream.write_all(&body)?;
            stream.flush()?;
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            let _ = respond_error(&mut stream, &message);
            Err(message.into())
        }
    }
}

fn respond_error(stream: &mut UnixStream, message: &str) -> Result<(), Box<dyn Error>> {
    let header = serde_json::to_vec(&CheckHeader {
        ok: false,
        status: String::new(),
        error: message.into(),
    })?;
    stream.write_all(&header)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn answer(
    state: &mut State,
    request: &Request,
    check: &CheckRequest,
) -> Result<(String, Vec<u8>), Box<dyn Error>> {
    let changes = match state.resync()? {
        Sync::Rebuild => {
            *state = State::open(request)?;
            Vec::new()
        }
        Sync::Ready(changes) => changes,
    };
    if changes.is_empty()
        && let Some(cached) = cached_answer(state, check)?
    {
        return Ok(cached);
    }
    let facts = if changes.is_empty() {
        state.session.analyze()?
    } else {
        state.session.edit(changes, None)?
    };
    let analysis = analyze_project(
        &state.project,
        &state.sources,
        &facts,
        &check.contract_paths,
    )?;
    let body = snapshot_emission::emit(
        "json",
        &request.project_id,
        &analysis.snapshot,
        false,
        Duration::ZERO,
    )?
    .output;
    let status = analysis.snapshot.status.to_string();
    let modules = imported_package_roots(&facts);
    state.last = Some(CachedAnswer {
        generation: state.session.generation(),
        explicit: check.contract_paths.clone(),
        contract_files: contract_files(state, &modules, &check.contract_paths)?,
        modules,
        status: status.clone(),
        body: body.clone(),
    });
    Ok((status, body))
}

/// Return the cached snapshot only when its inputs still hold: same
/// generation, same explicit contracts, the discovery walk resolves the
/// same contract files, and their contents are unchanged.
fn cached_answer(
    state: &State,
    check: &CheckRequest,
) -> Result<Option<CachedSnapshot>, Box<dyn Error>> {
    let Some(cached) = &state.last else {
        return Ok(None);
    };
    let current = contract_files(state, &cached.modules, &check.contract_paths)?;
    Ok(cached.snapshot_if_current(state.session.generation(), &check.contract_paths, &current))
}

/// The current on-disk contract inputs: package manifests and discovered
/// contracts for the module set plus explicit overrides, each with its
/// content hash, sorted.
fn contract_files(
    state: &State,
    modules: &[String],
    explicit: &[String],
) -> Result<Vec<ContractFile>, Box<dyn Error>> {
    let directory = state
        .project
        .parent()
        .ok_or("tsconfig has no parent directory")?;
    let mut paths = discovered_contract_paths(directory, modules)?;
    paths.extend(explicit.iter().map(PathBuf::from));
    paths.sort();
    paths.dedup();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        files.push((path.clone(), hash_file(&path)?));
    }
    Ok(files)
}

pub fn check(request: &Request) -> Result<i32, Box<dyn Error>> {
    let started = Instant::now();
    let socket = socket_path(&request.project_id);
    let stream = match UnixStream::connect(&socket) {
        Ok(stream) => stream,
        Err(_) => spawn_and_connect(request, &socket)?,
    };
    let payload = serde_json::to_vec(&CheckRequest {
        project_id: request.project_id.clone(),
        contract_paths: request.contract_paths.clone(),
    })?;
    let mut stream = stream;
    stream.write_all(&payload)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut header_line = String::new();
    reader.read_line(&mut header_line)?;
    let header: CheckHeader = serde_json::from_str(&header_line)?;
    if !header.ok {
        return Err(header.error.into());
    }
    let mut body = Vec::new();
    reader.read_to_end(&mut body)?;
    let snapshot: solid_facts_backend::Snapshot = serde_json::from_slice(&body)?;
    if snapshot.status != header.status {
        return Err("daemon response status does not match snapshot".into());
    }
    let emission = snapshot_emission::emit(
        &request.format,
        &request.project_id,
        &snapshot,
        request.certify,
        started.elapsed(),
    )?;
    io::stdout().write_all(&emission.output)?;
    Ok(emission.exit_code)
}

fn spawn_and_connect(
    request: &Request,
    socket: &std::path::Path,
) -> Result<UnixStream, Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    Command::new(executable)
        .arg("--serve")
        .arg("--project")
        .arg(&request.project_id)
        .arg("--typefacts")
        .arg(&request.typefacts_executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                if Instant::now() >= deadline {
                    return Err(format!("daemon did not start: {error}").into());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{FileRefresh, fingerprint_file, refresh_file_with};

    #[test]
    fn unchanged_fingerprint_does_not_read_or_hash_source_content() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "solid-checker-daemon-fingerprint-{}-{nonce}.tsx",
            std::process::id()
        ));
        fs::write(&path, "export const value = 1;\n").expect("write fixture");
        let recorded = fingerprint_file(&path).expect("fingerprint fixture");
        let reads = Cell::new(0);

        let refresh = refresh_file_with(&path, &recorded, |path| {
            reads.set(reads.get() + 1);
            fs::read(path)
        })
        .expect("refresh fixture");

        assert!(matches!(refresh, FileRefresh::Unchanged));
        assert_eq!(reads.get(), 0, "unchanged source content was read");
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn metadata_change_with_same_content_reads_once_without_reporting_an_edit() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "solid-checker-daemon-metadata-{}-{nonce}.tsx",
            std::process::id()
        ));
        fs::write(&path, "export const value = 1;\n").expect("write fixture");
        let mut recorded = fingerprint_file(&path).expect("fingerprint fixture");
        recorded.stamp.changed_nanoseconds = recorded.stamp.changed_nanoseconds.wrapping_add(1);
        let reads = Cell::new(0);

        let refresh = refresh_file_with(&path, &recorded, |path| {
            reads.set(reads.get() + 1);
            fs::read(path)
        })
        .expect("refresh fixture");

        assert!(matches!(refresh, FileRefresh::MetadataOnly(_)));
        assert_eq!(reads.get(), 1);
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn changed_content_reads_once_and_reports_the_new_bytes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "solid-checker-daemon-content-{}-{nonce}.tsx",
            std::process::id()
        ));
        fs::write(&path, "export const value = 1;\n").expect("write fixture");
        let recorded = fingerprint_file(&path).expect("fingerprint fixture");
        fs::write(&path, "export const value = 2;\n").expect("edit fixture");
        let reads = Cell::new(0);

        let refresh = refresh_file_with(&path, &recorded, |path| {
            reads.set(reads.get() + 1);
            fs::read(path)
        })
        .expect("refresh fixture");

        match refresh {
            FileRefresh::Content { bytes, .. } => {
                assert_eq!(bytes, b"export const value = 2;\n");
            }
            _ => panic!("changed content was not reported"),
        }
        assert_eq!(reads.get(), 1);
        fs::remove_file(path).expect("remove fixture");
    }
}
