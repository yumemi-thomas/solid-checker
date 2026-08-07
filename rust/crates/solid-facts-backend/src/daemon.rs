//! Per-project daemon holding the retained `NativeIncrementalSession` behind
//! a Unix socket, so repeat CLI checks reuse the warm session instead of
//! rebuilding the TypeScript program and demand closure from scratch.
//!
//! Optimized release binaries use it by default; `SOLID_CHECKER_DAEMON=0`
//! selects a one-shot check. Debug binaries remain one-shot by default so test
//! processes do not leak retained actors. The socket path is derived from the
//! canonical project id. Before every answer the daemon resynchronizes with
//! the filesystem: a changed tsconfig, a changed source directory (file
//! created, deleted, or renamed), or an unreadable known file rebuilds the
//! whole session; changed file contents become incremental overlay updates.
//! The response body is byte-identical to one-shot output.
use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    ffi::OsStr,
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::fs::MetadataExt,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solid_facts_backend::{
    DiagnosticSession, NativeIncrementalSession, SourceChange, SourceFile, TypeFactsSession,
    discovered_contract_paths, imported_package_roots,
};
use solid_reactive_ir::CacheRetention;

use super::{Request, snapshot_emission};
use crate::daemon_cache::{CachedAnswer, CachedSnapshot, ContractFile};
use crate::idle_memory;

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
    #[serde(default)]
    cache_hit: bool,
    #[serde(default)]
    generation: u64,
    #[serde(default)]
    analysis_ns: u64,
    #[serde(default)]
    response_bytes: u64,
}

struct Answer {
    status: Arc<str>,
    body: Arc<[u8]>,
    cache_hit: bool,
    generation: u64,
    analysis_ns: u64,
}

pub fn enabled() -> bool {
    enabled_from(
        std::env::var_os("SOLID_CHECKER_DAEMON").as_deref(),
        !cfg!(debug_assertions),
    )
}

fn enabled_from(setting: Option<&OsStr>, production_default: bool) -> bool {
    let Some(setting) = setting else {
        return production_default;
    };
    matches!(setting.to_str(), Some("1" | "true"))
}

pub fn eligible(request: &Request) -> bool {
    request.sources.is_empty()
        && request.emit_contract.is_empty()
        && !request.check_contracts
        && retained_format(&request.format)
}

fn retained_format(format: &str) -> bool {
    matches!(format, "default" | "json" | "text")
}

fn resolve_dialect(
    request: &Request,
) -> Result<&'static solid_facts_backend::dialect::Dialect, Box<dyn Error>> {
    match request.dialect.as_deref() {
        Some(id) => solid_facts_backend::dialect::by_id(id)
            .ok_or_else(|| format!("unknown dialect {id:?}").into()),
        None => Ok(solid_facts_backend::dialect::detect(Path::new(
            &request.project_id,
        ))),
    }
}

fn socket_path(project_id: &str, typefacts_executable: &str, dialect_id: &str) -> PathBuf {
    let mut identity = Sha256::new();
    identity.update(project_id.as_bytes());
    identity.update([0]);
    identity.update(typefacts_executable.as_bytes());
    identity.update([0]);
    // A daemon retains one dialect's caches and findings, so a different
    // dialect for the same project must land on a different socket.
    identity.update(dialect_id.as_bytes());
    identity.update([0]);
    identity.update(option_env!("SOLID_CHECKER_BUILD_ID").unwrap_or("dev"));
    if let Ok(executable) = std::env::current_exe() {
        identity.update([0]);
        identity.update(executable.as_os_str().as_encoded_bytes());
    }
    let digest = identity.finalize();
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
        .unwrap_or(120);
    Duration::from_secs(seconds.max(1))
}

fn memory_limit_bytes() -> Option<u64> {
    let mebibytes = std::env::var("SOLID_CHECKER_DAEMON_MAX_RSS_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(2048);
    (mebibytes != 0).then(|| mebibytes.saturating_mul(1024 * 1024))
}

const LARGE_PROJECT_SOURCE_THRESHOLD: usize = 1_000;

fn cache_retention_from(setting: Option<&OsStr>, source_count: usize) -> CacheRetention {
    match setting.and_then(OsStr::to_str) {
        Some("performance" | "full") => CacheRetention::Performance,
        Some("balanced") => CacheRetention::Balanced,
        Some("compact") => CacheRetention::Compact,
        _ if source_count >= LARGE_PROJECT_SOURCE_THRESHOLD => CacheRetention::Balanced,
        _ => CacheRetention::Performance,
    }
}

fn cache_retention(source_count: usize) -> CacheRetention {
    cache_retention_from(
        std::env::var_os("SOLID_CHECKER_CACHE_RETENTION").as_deref(),
        source_count,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessMemory {
    pid: u32,
    parent: u32,
    resident_kib: u64,
}

fn parse_process_memory(output: &str) -> Vec<ProcessMemory> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some(ProcessMemory {
                pid: fields.next()?.parse().ok()?,
                parent: fields.next()?.parse().ok()?,
                resident_kib: fields.next()?.parse().ok()?,
            })
        })
        .collect()
}

fn process_tree_resident_bytes_from(samples: &[ProcessMemory], root: u32) -> u64 {
    let mut included = HashSet::from([root]);
    let mut resident_kib = 0_u64;
    loop {
        let mut changed = false;
        for sample in samples {
            if included.contains(&sample.pid) {
                continue;
            }
            if included.contains(&sample.parent) {
                included.insert(sample.pid);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for sample in samples {
        if included.contains(&sample.pid) {
            resident_kib = resident_kib.saturating_add(sample.resident_kib);
        }
    }
    resident_kib.saturating_mul(1024)
}

fn process_tree_resident_bytes(root: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss="])
        .output()
        .ok()?;
    output.status.success().then(|| {
        process_tree_resident_bytes_from(
            &parse_process_memory(&String::from_utf8_lossy(&output.stdout)),
            root,
        )
    })
}

struct State {
    project: PathBuf,
    session: NativeIncrementalSession,
    diagnostics: DiagnosticSession,
    sources: Vec<SourceFile>,
    fingerprints: BTreeMap<String, FileFingerprint>,
    dirs: BTreeMap<PathBuf, Option<FileStamp>>,
    tsconfig: FileFingerprint,
    last: Option<CachedAnswer>,
    cache_retention: CacheRetention,
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
        let dialect = resolve_dialect(request)?;
        let typescript =
            TypeFactsSession::open(&request.typefacts_executable, &request.project_id, &[])?;
        let (session, configured) = NativeIncrementalSession::open_pipelined(
            dialect,
            request.project_id.clone(),
            typescript,
        )?;
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
        let cache_retention = cache_retention(sources.len());
        Ok(Self {
            project,
            session,
            diagnostics: DiagnosticSession::new(dialect),
            sources,
            fingerprints,
            dirs,
            tsconfig,
            last: None,
            cache_retention,
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
                        source: text.into(),
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
    let socket = socket_path(
        &request.project_id,
        &request.typefacts_executable,
        resolve_dialect(request)?.id,
    );
    if UnixStream::connect(&socket).is_ok() {
        return Ok(0); // a live daemon already serves this project
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    let mut state = State::open(request)?;
    // Blocking accept keeps request latency free of poll sleeps; a
    // watchdog thread ends the whole process after the idle limit.
    let idle = idle_limit();
    let memory_limit = memory_limit_bytes();
    let process_id = std::process::id();
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
            if memory_limit.is_some_and(|limit| {
                process_tree_resident_bytes(process_id).is_some_and(|resident| resident > limit)
            }) {
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
        Ok(answer) => {
            let materialized = !answer.cache_hit;
            let header = serde_json::to_vec(&CheckHeader {
                ok: true,
                status: answer.status.to_string(),
                error: String::new(),
                cache_hit: answer.cache_hit,
                generation: answer.generation,
                analysis_ns: answer.analysis_ns,
                response_bytes: u64::try_from(answer.body.len()).unwrap_or(u64::MAX),
            })?;
            stream.write_all(&header)?;
            stream.write_all(b"\n")?;
            stream.write_all(&answer.body)?;
            stream.flush()?;
            if materialized {
                idle_memory::reclaim_idle_pages();
            }
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
        cache_hit: false,
        generation: 0,
        analysis_ns: 0,
        response_bytes: 0,
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
) -> Result<Answer, Box<dyn Error>> {
    let started = Instant::now();
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
        return Ok(Answer {
            status: cached.0,
            body: cached.1,
            cache_hit: true,
            generation: state.session.generation(),
            analysis_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        });
    }
    let facts = if changes.is_empty() {
        state.session.analyze()?
    } else {
        state.session.edit(changes, None)?
    };
    let analysis = state.diagnostics.analyze(
        &state.project,
        &state.sources,
        &facts,
        &check.contract_paths,
    )?;
    let body: Arc<[u8]> = snapshot_emission::emit(
        resolve_dialect(request)?,
        "json",
        &request.project_id,
        &analysis.snapshot,
        false,
        Duration::ZERO,
    )?
    .output
    .into();
    let status: Arc<str> = analysis.snapshot.status.as_str().into();
    let modules = imported_package_roots(&facts);
    state.last = Some(CachedAnswer {
        generation: state.session.generation(),
        explicit: check.contract_paths.clone(),
        contract_files: contract_files(state, &modules, &check.contract_paths)?,
        modules,
        status: Arc::clone(&status),
        body: Arc::clone(&body),
    });
    state.diagnostics.retain_for_idle(state.cache_retention);
    Ok(Answer {
        status,
        body,
        cache_hit: false,
        generation: state.session.generation(),
        analysis_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
    })
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
    // Rule options carry the same weight as a contract: they are part of
    // every diagnostic identity, and `DiagnosticSession::analyze` re-reads
    // them from disk on every run. The cached-answer short-circuit returns
    // before analyze does, so the file must be in this input set or an
    // options edit keeps serving the old snapshot for a whole generation.
    if let Some(path) = solid_facts_backend::discovered_rule_options_path(directory) {
        paths.push(path);
    }
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
    let socket = socket_path(
        &request.project_id,
        &request.typefacts_executable,
        resolve_dialect(request)?.id,
    );
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
    if request.format == "json" {
        // The daemon caches the canonical JSON emission. Stream it directly:
        // parsing and serializing the multi-megabyte snapshot again made the
        // payload itself dominate otherwise constant-time cache hits.
        let response_bytes = io::copy(&mut reader, &mut io::stdout())?;
        report_timings(&header, started.elapsed(), response_bytes);
        return Ok(i32::from(request.certify && header.status != "certified"));
    }
    let mut body = Vec::new();
    reader.read_to_end(&mut body)?;
    let snapshot: solid_facts_backend::Snapshot = serde_json::from_slice(&body)?;
    if snapshot.status != header.status {
        return Err("daemon response status does not match snapshot".into());
    }
    let emission = snapshot_emission::emit(
        resolve_dialect(request)?,
        &request.format,
        &request.project_id,
        &snapshot,
        request.certify,
        started.elapsed(),
    )?;
    io::stdout().write_all(&emission.output)?;
    report_timings(
        &header,
        started.elapsed(),
        u64::try_from(body.len()).unwrap_or(u64::MAX),
    );
    Ok(emission.exit_code)
}

fn report_timings(header: &CheckHeader, elapsed: Duration, received_bytes: u64) {
    if std::env::var_os("SOLID_CHECKER_TIMINGS").is_none() {
        return;
    }
    eprintln!("{}", timing_value(header, elapsed, received_bytes));
}

fn timing_value(header: &CheckHeader, elapsed: Duration, received_bytes: u64) -> serde_json::Value {
    serde_json::json!({
        "mode": "retained-daemon",
        "cacheHit": header.cache_hit,
        "generation": header.generation,
        "analysisNs": header.analysis_ns,
        "roundtripNs": u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
        "responseBytes": header.response_bytes,
        "receivedBytes": received_bytes,
    })
}

fn spawn_and_connect(
    request: &Request,
    socket: &std::path::Path,
) -> Result<UnixStream, Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--serve")
        .arg("--project")
        .arg(&request.project_id)
        .arg("--typefacts")
        .arg(&request.typefacts_executable);
    // The client hashed the resolved dialect into the socket path; the
    // spawned daemon must resolve identically. Detection can flip between
    // the client's hash and daemon startup (an `npm install` finishing, a
    // package.json edit), and a daemon that re-detects then binds a socket
    // the client never connects to — so the resolved id is always
    // forwarded, never re-derived.
    command.arg("--dialect").arg(resolve_dialect(request)?.id);
    command
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
        ffi::OsStr,
        fs,
        time::Duration,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        CheckHeader, FileRefresh, ProcessMemory, cache_retention_from, enabled_from,
        fingerprint_file, parse_process_memory, process_tree_resident_bytes_from,
        refresh_file_with, retained_format, socket_path, timing_value,
    };
    use solid_reactive_ir::CacheRetention;

    #[test]
    fn release_default_and_explicit_daemon_policy_are_unambiguous() {
        assert!(enabled_from(None, true));
        assert!(!enabled_from(None, false));
        assert!(enabled_from(Some(OsStr::new("1")), false));
        assert!(enabled_from(Some(OsStr::new("true")), false));
        assert!(!enabled_from(Some(OsStr::new("0")), true));
        assert!(!enabled_from(Some(OsStr::new("false")), true));
        assert!(!enabled_from(Some(OsStr::new("unexpected")), true));
    }

    #[test]
    fn large_projects_release_expensive_idle_indexes_by_default() {
        assert_eq!(cache_retention_from(None, 999), CacheRetention::Performance);
        assert_eq!(cache_retention_from(None, 1_000), CacheRetention::Balanced);
        assert_eq!(
            cache_retention_from(Some(OsStr::new("performance")), 5_000),
            CacheRetention::Performance
        );
        assert_eq!(
            cache_retention_from(Some(OsStr::new("compact")), 1),
            CacheRetention::Compact
        );
    }

    #[test]
    fn retained_actor_supports_every_diagnostic_format() {
        assert!(retained_format("default"));
        assert!(retained_format("json"));
        assert!(retained_format("text"));
        assert!(!retained_format("sarif"));
    }

    #[test]
    fn retained_actor_identity_includes_project_and_typefacts_build() {
        let baseline = socket_path("/project/a/tsconfig.json", "/bin/typefacts-a", "solid-v2");
        assert_ne!(
            baseline,
            socket_path("/project/b/tsconfig.json", "/bin/typefacts-a", "solid-v2")
        );
        assert_ne!(
            baseline,
            socket_path("/project/a/tsconfig.json", "/bin/typefacts-b", "solid-v2")
        );
    }

    #[test]
    fn process_tree_memory_includes_descendants_and_excludes_neighbors() {
        let parsed = parse_process_memory("10 1 100\n11 10 200\n12 11 300\n13 1 400\nmalformed\n");
        assert_eq!(
            parsed,
            vec![
                ProcessMemory {
                    pid: 10,
                    parent: 1,
                    resident_kib: 100,
                },
                ProcessMemory {
                    pid: 11,
                    parent: 10,
                    resident_kib: 200,
                },
                ProcessMemory {
                    pid: 12,
                    parent: 11,
                    resident_kib: 300,
                },
                ProcessMemory {
                    pid: 13,
                    parent: 1,
                    resident_kib: 400,
                },
            ]
        );
        assert_eq!(process_tree_resident_bytes_from(&parsed, 10), 600 * 1024);
    }

    #[test]
    fn retained_timing_reports_cache_generation_and_payload() {
        let value = timing_value(
            &CheckHeader {
                ok: true,
                status: "certified".into(),
                error: String::new(),
                cache_hit: true,
                generation: 7,
                analysis_ns: 11,
                response_bytes: 13,
            },
            Duration::from_nanos(17),
            13,
        );
        assert_eq!(value["mode"], "retained-daemon");
        assert_eq!(value["cacheHit"], true);
        assert_eq!(value["generation"], 7);
        assert_eq!(value["analysisNs"], 11);
        assert_eq!(value["roundtripNs"], 17);
        assert_eq!(value["responseBytes"], 13);
        assert_eq!(value["receivedBytes"], 13);
    }

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
