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
//!
//! A check whose effective `--dependency-source` selection is nonempty is not
//! served from here (ADR 0311): the executable runs the one-shot path, which
//! opens a new original program for preparation and a new analyzed program, so
//! no program, inventory or fact table of an earlier invocation answers it.
//! `--serve` refuses such a selection before it touches a socket, a program or
//! a producer. ADR 0296 retained those programs and trusted what the base
//! daemon trusts; the dependency-source worker below is no longer reached, and
//! removing it is a separate change.
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
use solid_facts::core::SourceHash;
use solid_facts_backend::{
    BackendError, DiagnosticSession, NativeIncrementalSession, RequestedRuleEnablement,
    SourceChange, SourceFile, TypeFactsSession, discovered_contract_paths, imported_package_roots,
    semantic_demand_options_for_enablement,
};
use solid_reactive_ir::CacheRetention;
use solid_reactive_ir::RuntimeEnvironment;

use super::{Request, snapshot_emission};
use crate::daemon_cache::{CachedAnswer, CachedSnapshot, InputFile};
use crate::daemon_resolution::ResolutionWitness;
use crate::idle_memory;
use solid_facts_backend::dependency_program::{self, PreparedDependencies};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckRequest {
    #[serde(default)]
    implementation: String,
    project_id: String,
    #[serde(default)]
    presets: Vec<String>,
    #[serde(default)]
    enable_rules: Vec<String>,
    #[serde(default)]
    runtime: RuntimeEnvironment,
    /// ADR 0296: the `--dependency-source` selection, normalized. A daemon
    /// serves exactly one, with exactly one runtime host.
    #[serde(default)]
    dependency_sources: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckHeader {
    #[serde(default)]
    implementation: String,
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
    /// The notes the analysis would print, empty when it has none. The
    /// daemon's stderr is not the user's terminal, so it answers them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notice: Option<String>,
}

struct Answer {
    status: Arc<str>,
    body: Arc<[u8]>,
    cache_hit: bool,
    generation: u64,
    analysis_ns: u64,
    notice: Arc<str>,
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
        // Runtime resolution runs project code per check and is not part of
        // the retained identity (ADR 0220).
        && !request.runtime_resolution
        // ADR 0311: an effective dependency-source selection is answered by a
        // fresh one-shot check, whatever preparation later selects. The
        // selection is the named list, `*`, or the implicit `*` that `main`
        // applies before this call. The decision reads the request, never an
        // earlier answer or a preparation output.
        && request.dependency_sources.is_empty()
        && retained_format(&request.format)
        && carried(request)
}

/// A check is retained only where the daemon's program is the one-shot
/// program (ADR 0296). Anything a retained session does not carry stays
/// one-shot, where it is honored or refused; review round 1 found these
/// dropped in base mode too.
fn carried(request: &Request) -> bool {
    // A retained session numbers its own generations from 1; the one-shot
    // path is what validates any other.
    request.generation == 1
        // The project id is the daemon's identity and the answer's spelling;
        // a relative one would be the first caller's.
        && Path::new(&request.project_id).is_absolute()
        // ADR 0295's Window realm is not part of the retained identity.
        && !request.window_realm
        && !request.feedback_facts
        && super::producer_arguments(&request.typefacts_args).is_empty()
}

fn retained_format(format: &str) -> bool {
    matches!(format, "default" | "full" | "json" | "text")
}

fn resolve_dialect(
    request: &Request,
) -> Result<&'static solid_facts_backend::dialect::Dialect, Box<dyn Error>> {
    match request.dialect.as_deref() {
        Some(id) => solid_facts_backend::dialect::by_id(id)
            .ok_or_else(|| format!("unknown dialect {id:?}").into()),
        // A direct `--serve` for a project whose installed runtime this build
        // has no dialect for refuses to start, rather than retaining a session
        // that would answer every request under the wrong language. The
        // ordinary CLI path never gets here: `run` refuses at the selection
        // site, above the branch that consults this daemon at all. This is the
        // backstop for the case that skips it.
        None => match solid_facts_backend::dialect::detect_detailed(Path::new(&request.project_id))
        {
            solid_facts_backend::dialect::Detection::Installed { dialect, .. } => Ok(dialect),
            solid_facts_backend::dialect::Detection::Unsupported {
                installed,
                manifest,
                refusal,
                ..
            } => Err(match refusal {
                None => format!(
                    "solid-js {installed} at {} is a runtime this build carries no dialect for [{}]",
                    manifest.display(),
                    solid_facts_backend::dialect::UNSUPPORTED_RUNTIME_CODE
                ),
                Some(refusal) => format!(
                    "solid-js {installed} at {} is a runtime this build refuses: {} [{}]",
                    manifest.display(),
                    refusal.reason,
                    solid_facts_backend::dialect::UNSUPPORTED_RUNTIME_CODE
                ),
            }
            .into()),
            solid_facts_backend::dialect::Detection::Defaulted { .. } => {
                Ok(solid_facts_backend::dialect::default_dialect())
            }
        },
    }
}

// Captured from the running image, not its replaceable pathname. This also
// covers replacement between exec and the first socket selection/binding.
fn implementation_identity() -> Result<&'static str, Box<dyn Error>> {
    static IDENTITY: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    match IDENTITY.get_or_init(loaded_implementation_identity) {
        Ok(identity) => Ok(identity),
        Err(error) => Err(error.clone().into()),
    }
}

#[cfg(target_os = "linux")]
fn loaded_implementation_identity() -> Result<String, String> {
    // /proc/self/exe opens the executing inode even after unlink/replacement.
    let digest = hash_file(Path::new("/proc/self/exe")).map_err(|error| error.to_string())?;
    Ok(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

#[cfg(target_os = "macos")]
fn loaded_implementation_identity() -> Result<String, String> {
    // SDK mach-o/dyld.h and loader.h: image zero is the executable; LC_UUID
    // is its linker-issued build identity, carried in the loaded image.
    #[repr(C)]
    struct Header {
        magic: u32,
        cpu_type: i32,
        cpu_subtype: i32,
        file_type: u32,
        commands: u32,
        command_bytes: u32,
        flags: u32,
        reserved: u32,
    }
    unsafe extern "C" {
        fn _dyld_get_image_header(index: u32) -> *const Header;
    }
    // SAFETY: dyld owns the executable header and its mapped load commands for
    // the process lifetime. No caller-supplied pointer or file bytes are read.
    let header =
        unsafe { _dyld_get_image_header(0).as_ref() }.ok_or("missing loaded executable header")?;
    if header.magic != 0xfeed_facf || header.command_bytes > 1024 * 1024 || header.commands > 4096 {
        return Err("unsupported loaded executable header".into());
    }
    // SAFETY: a validated native 64-bit Mach-O header's load commands follow
    // the header in the same dyld mapping; their extent is provided by dyld.
    let commands = unsafe {
        std::slice::from_raw_parts(
            (std::ptr::from_ref(header).cast::<u8>()).add(std::mem::size_of::<Header>()),
            header.command_bytes as usize,
        )
    };
    macho_build_identity(commands, header.commands)
        .ok_or_else(|| "missing or ambiguous loaded executable build UUID".into())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn loaded_implementation_identity() -> Result<String, String> {
    // A fresh one-shot check remains available; never reuse an unidentified actor.
    Err("immutable loaded checker identity is unavailable on this platform".into())
}

#[cfg(any(target_os = "macos", test))]
fn macho_build_identity(mut commands: &[u8], count: u32) -> Option<String> {
    let mut uuid = None;
    for _ in 0..count {
        let kind = u32::from_le_bytes(commands.get(..4)?.try_into().ok()?);
        let size = u32::from_le_bytes(commands.get(4..8)?.try_into().ok()?) as usize;
        if size < 8 {
            return None;
        }
        let command = commands.get(..size)?;
        if kind == 0x1b {
            if size != 24 || uuid.is_some() {
                return None;
            }
            uuid = Some(format!(
                "macho-uuid:{}",
                command[8..24]
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ));
        }
        commands = &commands[size..];
    }
    if !commands.is_empty() {
        return None;
    }
    uuid
}

fn verify_implementation(expected: &str, actual: &str) -> Result<(), Box<dyn Error>> {
    if expected.is_empty() || actual != expected {
        return Err("daemon checker implementation identity mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
fn socket_path(
    project_id: &str,
    typefacts_executable: &str,
    dialect_id: &str,
    implementation: &str,
) -> PathBuf {
    socket_path_in_domain(
        project_id,
        typefacts_executable,
        dialect_id,
        implementation,
        None,
    )
}

/// [`socket_path`] for a daemon that also serves one [`dependency_domain`]:
/// a different selection or host is a different daemon.
fn socket_path_in_domain(
    project_id: &str,
    typefacts_executable: &str,
    dialect_id: &str,
    implementation: &str,
    domain: Option<&str>,
) -> PathBuf {
    let mut identity = Sha256::new();
    identity.update(implementation.as_bytes());
    identity.update([0]);
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
    if let Some(domain) = domain {
        identity.update([0]);
        identity.update(b"dependency-sources");
        identity.update([0]);
        identity.update(domain.as_bytes());
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
    dialect: &'static solid_facts_backend::dialect::Dialect,
    /// The tsconfig the Type Facts program opened: the project's own, or in
    /// dependency-source mode the logical path of the program that extends
    /// it, beside it, whose bytes are a scratch file (ADR 0296).
    project: PathBuf,
    session: NativeIncrementalSession,
    diagnostics: DiagnosticSession,
    sources: Vec<SourceFile>,
    tracking: Tracking,
    last: Option<CachedAnswer>,
    cache_retention: CacheRetention,
    dependency: Option<DependencyState>,
    /// Every file the analyzed program holds, read before its first edit.
    /// `session`'s source set is fixed when it opens, so an edit that
    /// changes this reopens the program (review round 1 of ADR 0296).
    modules: Option<Vec<String>>,
    /// What the open program's module resolution read
    /// ([`ResolutionWitness`]); a difference rebuilds the program from a new
    /// Type Facts session. `None` until the first analysis observes it.
    resolution: Option<ResolutionWitness>,
}

/// What a retained dependency-source program (ADR 0296) holds beside the
/// program it analyzes: the project's own program, which preparation reads,
/// and what preparation and the fold answered when the analyzed program was
/// opened. Each request re-asks both and reuses the analyzed program only
/// when both answer exactly as they did.
struct DependencyState {
    original: OriginalProgram,
    prepared: PreparedDependencies,
    folds: BTreeMap<String, Option<SourceHash>>,
    /// The scratch file holding the analyzed program's configuration,
    /// which this daemon wrote and owns.
    generated: Option<PathBuf>,
}

/// The project's own Type Facts program, retained and kept current with the
/// same freshness machinery as the analyzed one.
struct OriginalProgram {
    project: PathBuf,
    typescript: TypeFactsSession,
    /// Sorted by path, as [`resync_tracked`] keeps them.
    sources: Vec<SourceFile>,
    /// The producer's own order of `sources`, which preparation reads in: it
    /// decides the order notes are printed in.
    order: Vec<String>,
    /// Every file the program holds. `sources` is a path-decided subset of
    /// it, so while this is unchanged so is which files `sources` holds; an
    /// edited import that changes it rebuilds (review round 1).
    modules: Vec<PathBuf>,
    tracking: Tracking,
}

impl OriginalProgram {
    fn open(request: &Request) -> Result<Self, Box<dyn Error>> {
        let project = PathBuf::from(&request.project_id);
        let mut typescript =
            TypeFactsSession::open(&request.typefacts_executable, &request.project_id, &[])?;
        let configured = typescript.configured_sources()?;
        let order = configured
            .iter()
            .map(|source| source.path.clone())
            .collect();
        let modules = dependency_program::program_modules(&mut typescript)?;
        let mut sources = configured;
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        let tracking = Tracking::new(&project, &project, &sources)?;
        Ok(Self {
            project,
            typescript,
            sources,
            order,
            modules,
            tracking,
        })
    }

    /// The configured sources in the producer's order.
    fn ordered_sources(&self) -> Vec<SourceFile> {
        self.order
            .iter()
            .filter_map(|path| {
                self.sources
                    .binary_search_by(|source| source.path.as_str().cmp(path))
                    .ok()
                    .map(|index| self.sources[index].clone())
            })
            .collect()
    }

    fn prepare(
        &mut self,
        request: &Request,
        dialect: &'static solid_facts_backend::dialect::Dialect,
    ) -> Result<PreparedDependencies, Box<dyn Error>> {
        let host = dependency_program::host(&request.runtime)?;
        let (notes, program) = dependency_program::prepare_from(
            &self.project,
            &request.dependency_sources,
            &host,
            dialect,
            &self.ordered_sources(),
            &self.modules,
        );
        Ok(PreparedDependencies {
            notes,
            program: program?,
        })
    }
}

/// The filesystem observations a retained program is reused under.
struct Tracking {
    fingerprints: BTreeMap<String, FileFingerprint>,
    dirs: BTreeMap<PathBuf, Option<FileStamp>>,
    /// The file holding the program's configuration: the project's tsconfig,
    /// or the scratch file a dependency-source program is opened from.
    config: PathBuf,
    tsconfig: FileFingerprint,
    /// What resolving the configuration's `extends` chain consulted
    /// (`tsconfig_extends_witness`): every config and manifest read and every
    /// path probed. `None` when the chain cannot be resolved exactly, and then
    /// no retained program is reused (an edit there could not be seen).
    extended: Option<Vec<(PathBuf, String)>>,
}

impl Tracking {
    fn new(project: &Path, config: &Path, sources: &[SourceFile]) -> Result<Self, Box<dyn Error>> {
        let tsconfig = fingerprint_file(config)?;
        let extended =
            solid_facts_backend::tsconfig_extends_witness(config).map(|(_, witness)| witness);
        let mut fingerprints = BTreeMap::new();
        let mut dirs = BTreeMap::new();
        if let Some(parent) = project.parent() {
            dirs.insert(parent.to_path_buf(), directory_stamp(parent));
        }
        for source in sources {
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
            fingerprints,
            dirs,
            config: config.to_path_buf(),
            tsconfig,
            extended,
        })
    }
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
    /// ADR 0296: the project's own program is current, and preparation
    /// answers differently; reopen the analyzed program for this answer.
    Reprepare(Box<PreparedDependencies>),
}

/// Reconcile one retained program with the filesystem. Content edits to
/// known files become overlay updates at `version`; anything that can change
/// the project's file set demands a full rebuild.
fn resync_tracked(
    tracking: &mut Tracking,
    sources: &mut [SourceFile],
    version: u64,
) -> Result<Sync, Box<dyn Error>> {
    let config = tracking.config.clone();
    match refresh_file_with(&config, &tracking.tsconfig, |path| fs::read(path)) {
        Ok(FileRefresh::Unchanged) => {}
        Ok(FileRefresh::MetadataOnly(fingerprint)) => tracking.tsconfig = fingerprint,
        // A tsconfig that changed or can no longer be read is a new program.
        Ok(FileRefresh::Content { .. }) | Err(_) => return Ok(Sync::Rebuild),
    }
    // So is a change anywhere in its `extends` chain, or a chain this
    // daemon cannot follow exactly.
    let Some(extended) = tracking.extended.as_ref() else {
        return Ok(Sync::Rebuild);
    };
    if solid_facts_backend::tsconfig_extends_witness(&config).map(|(_, witness)| witness)
        != Some(extended.clone())
    {
        return Ok(Sync::Rebuild);
    }
    for (dir, recorded) in &tracking.dirs {
        if directory_stamp(dir) != *recorded {
            return Ok(Sync::Rebuild);
        }
    }
    let mut changes = Vec::new();
    let paths = tracking.fingerprints.keys().cloned().collect::<Vec<_>>();
    for path in paths {
        let recorded = tracking.fingerprints[&path];
        let refresh = match refresh_file_with(Path::new(&path), &recorded, |path| fs::read(path)) {
            Ok(refresh) => refresh,
            Err(_) => return Ok(Sync::Rebuild),
        };
        match refresh {
            FileRefresh::Unchanged => {}
            FileRefresh::MetadataOnly(fingerprint) => {
                tracking.fingerprints.insert(path, fingerprint);
            }
            FileRefresh::Content { fingerprint, bytes } => {
                let text = String::from_utf8(bytes)?;
                changes.push(SourceChange {
                    path: path.clone(),
                    version,
                    source: Some(text.clone()),
                    compiler_options: Default::default(),
                });
                tracking.fingerprints.insert(path.clone(), fingerprint);
                let index = sources
                    .binary_search_by(|source| source.path.as_str().cmp(path.as_str()))
                    .map_err(|_| format!("configured source disappeared from state: {path}"))?;
                sources[index] = SourceFile {
                    path,
                    source: text.into(),
                    compiler_options: Default::default(),
                };
            }
        }
    }
    Ok(Sync::Ready(changes))
}

/// The scratch program file a live daemon owns, removed when it exits.
static OWNED_PROGRAM: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

fn own_program(path: Option<&Path>) {
    if let Ok(mut owned) = OWNED_PROGRAM.lock() {
        *owned = path.map(Path::to_path_buf);
    }
}

fn remove_owned_program() {
    if let Ok(mut owned) = OWNED_PROGRAM.lock()
        && let Some(path) = owned.take()
    {
        let _ = fs::remove_file(path);
    }
}

impl State {
    fn open(request: &Request) -> Result<Self, Box<dyn Error>> {
        let dialect = resolve_dialect(request)?;
        if !request.dependency_sources.is_empty() {
            let original = OriginalProgram::open(request)?;
            return Self::open_dependency(request, dialect, original, None);
        }
        let typescript =
            TypeFactsSession::open(&request.typefacts_executable, &request.project_id, &[])?;
        let (session, configured) = NativeIncrementalSession::open_pipelined(
            dialect,
            request.project_id.clone(),
            typescript,
        )?;
        Self::from_session(
            dialect,
            PathBuf::from(&request.project_id),
            session,
            configured,
            None,
        )
    }

    fn from_session(
        dialect: &'static solid_facts_backend::dialect::Dialect,
        project: PathBuf,
        session: NativeIncrementalSession,
        configured: Vec<SourceFile>,
        dependency: Option<DependencyState>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut sources = configured;
        sources.sort_by(|left, right| left.path.cmp(&right.path));
        let config = dependency
            .as_ref()
            .and_then(|dependency| dependency.generated.clone())
            .unwrap_or_else(|| project.clone());
        let tracking = Tracking::new(&project, &config, &sources)?;
        let cache_retention = cache_retention(sources.len());
        Ok(Self {
            dialect,
            project,
            session,
            diagnostics: DiagnosticSession::new(dialect),
            sources,
            tracking,
            last: None,
            cache_retention,
            dependency,
            modules: None,
            resolution: None,
        })
    }

    /// ADR 0296: open the analyzed program of a dependency-source check from
    /// the project's own retained program, exactly as a one-shot check
    /// prepares, folds and binds it (`dependency_program`). `prepared` is
    /// preparation's answer when the caller already has it.
    fn open_dependency(
        request: &Request,
        dialect: &'static solid_facts_backend::dialect::Dialect,
        mut original: OriginalProgram,
        prepared: Option<PreparedDependencies>,
    ) -> Result<Self, Box<dyn Error>> {
        let prepared = match prepared {
            Some(prepared) => prepared,
            None => original.prepare(request, dialect)?,
        };
        let Some(program) = prepared.program.clone() else {
            // Nothing selected: the ordinary analysis of the project's own
            // tsconfig, as a one-shot check runs it.
            let typescript =
                TypeFactsSession::open(&request.typefacts_executable, &request.project_id, &[])?;
            let (session, configured) = NativeIncrementalSession::open_pipelined(
                dialect,
                request.project_id.clone(),
                typescript,
            )?;
            return Self::from_session(
                dialect,
                original.project.clone(),
                session,
                configured,
                Some(DependencyState {
                    original,
                    prepared,
                    folds: BTreeMap::new(),
                    generated: None,
                }),
            );
        };
        let domain = dependency_domain(request).unwrap_or_default();
        let (logical, generated) = program.write_retained(domain.as_bytes())?;
        let opened = (|| -> Result<Self, Box<dyn Error>> {
            let project_id = logical.to_string_lossy().into_owned();
            let mut typescript = TypeFactsSession::open(
                &request.typefacts_executable,
                &project_id,
                &[format!("-project-config={}", generated.display())],
            )?;
            let configured = typescript.configured_sources()?;
            let folded =
                dependency_program::fold_sources(&configured, &program, &request.runtime, false);
            let mut session = NativeIncrementalSession::open_rewritten(
                dialect,
                project_id,
                folded.sources,
                folded.program_hashes,
                typescript,
            );
            let bound = program.clone();
            session.set_facts_transform(Box::new(move |facts, typescript| {
                dependency_program::bind(facts, typescript, &bound)
                    .map_err(|error| BackendError::Process(error.to_string()))
            }));
            Self::from_session(
                dialect,
                logical.clone(),
                session,
                configured,
                Some(DependencyState {
                    original,
                    prepared,
                    folds: folded.record,
                    generated: Some(generated.clone()),
                }),
            )
        })();
        if opened.is_err() {
            let _ = fs::remove_file(&generated);
        }
        opened
    }

    fn generated(&self) -> Option<&Path> {
        self.dependency
            .as_ref()
            .and_then(|dependency| dependency.generated.as_deref())
    }

    /// The program's own analysis answer may be reused only when this holds:
    /// in dependency-source mode, the project's own program and the fold
    /// answer as they did too.
    fn resync(&mut self, request: &Request) -> Result<Sync, Box<dyn Error>> {
        // The retained session resolved its imports when it opened; if the
        // filesystem would now resolve them differently, only a new session
        // answers as a one-shot check does.
        if self
            .resolution
            .as_ref()
            .is_some_and(|witness| !witness.current())
        {
            return Ok(Sync::Rebuild);
        }
        let changes = match resync_tracked(
            &mut self.tracking,
            &mut self.sources,
            self.session.generation() + 1,
        )? {
            Sync::Ready(changes) => changes,
            other => return Ok(other),
        };
        if request.dependency_sources.is_empty() {
            return Ok(Sync::Ready(changes));
        }
        let Some(dependency) = self.dependency.as_mut() else {
            return Ok(Sync::Rebuild);
        };
        let original = &mut dependency.original;
        match resync_tracked(
            &mut original.tracking,
            &mut original.sources,
            original.typescript.generation() + 1,
        )? {
            Sync::Ready(edits) if edits.is_empty() => {}
            Sync::Ready(edits) => {
                let edits = edits
                    .into_iter()
                    .map(|change| typefacts::v3::FileChange {
                        path: change.path,
                        version: change.version,
                        source: change.source.unwrap_or_default().into_bytes(),
                        deleted: false,
                    })
                    .collect();
                if original.typescript.update(edits).is_err() {
                    return Ok(Sync::Rebuild);
                }
                // An edited import can add a file to the program or drop
                // one, and the inventory preparation reads is the open
                // program's: reacquire it whole rather than patch it.
                let modules = dependency_program::program_modules(&mut original.typescript)?;
                if modules != original.modules {
                    return Ok(Sync::Rebuild);
                }
            }
            _ => return Ok(Sync::Rebuild),
        }
        // A refusal is answered by a rebuild, which refuses as a fresh run
        // does.
        let Ok(prepared) = original.prepare(request, self.dialect) else {
            return Ok(Sync::Rebuild);
        };
        if prepared != dependency.prepared {
            return Ok(Sync::Reprepare(Box::new(prepared)));
        }
        if let Some(program) = &prepared.program {
            // The fold reads more than the analyzed files: the installed
            // runtime it resolves each host constant from. Recompute it, and
            // reuse nothing unless it answers as it did. An edited file is an
            // ordinary edit only when neither its old nor its new text folds,
            // since an edit carries the program's own text.
            let folded =
                dependency_program::fold_sources(&self.sources, program, &request.runtime, false);
            if !folded.record.keys().eq(dependency.folds.keys()) {
                return Ok(Sync::Reprepare(Box::new(prepared)));
            }
            for (path, now) in &folded.record {
                let before = &dependency.folds[path];
                let edited = changes.iter().any(|change| &change.path == path);
                if (edited && (before.is_some() || now.is_some())) || (!edited && before != now) {
                    return Ok(Sync::Reprepare(Box::new(prepared)));
                }
            }
        }
        Ok(Sync::Ready(changes))
    }
}

/// Replace a retained state, removing `previous`, the scratch program file
/// the replaced one owned, when the new one does not use it.
fn replace_state(state: &mut State, next: State, previous: Option<PathBuf>) {
    *state = next;
    if let Some(previous) = previous
        && state.generated() != Some(previous.as_path())
    {
        let _ = fs::remove_file(&previous);
    }
    own_program(state.generated());
}

/// ADR 0296: what a dependency-source daemon serves beside its project: the
/// normalized selection and the whole declared runtime host, both of which
/// decide the analyzed program. `None` outside dependency-source mode.
fn dependency_domain(request: &Request) -> Option<String> {
    if request.dependency_sources.is_empty() {
        return None;
    }
    Some(
        serde_json::json!({
            "project": request.project_id,
            "dependencySources": normalized_selection(&request.dependency_sources),
            "runtime": request.runtime,
        })
        .to_string(),
    )
}

/// The selection as preparation reads it: a set.
fn normalized_selection(selection: &[String]) -> Vec<String> {
    let mut selection = selection.to_vec();
    selection.sort();
    selection.dedup();
    selection
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
    // ADR 0311: first, before the live-socket early success, the bind, any
    // state or any producer. `--serve` is handled before `eligible`, and the
    // implicit default excludes it, so a bare `--serve` stays a base server.
    if !request.dependency_sources.is_empty() {
        return Err(
            "--serve does not retain a --dependency-source program (ADR 0311): \
             a dependency-source check is a one-shot check"
                .into(),
        );
    }
    let implementation = implementation_identity()?;
    let mut request = request.clone();
    request.dependency_sources = normalized_selection(&request.dependency_sources);
    let request = &request;
    let socket = socket_path_in_domain(
        &request.project_id,
        &request.typefacts_executable,
        resolve_dialect(request)?.id,
        implementation,
        dependency_domain(request).as_deref(),
    );
    if UnixStream::connect(&socket).is_ok() {
        return Ok(0); // a live daemon already serves this project
    }
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    let mut state = match State::open(request) {
        Ok(state) => Some(state),
        Err(error) => {
            let _ = fs::remove_file(&socket);
            return Err(error);
        }
    };
    own_program(state.as_ref().and_then(State::generated));
    // Blocking accept keeps request latency free of poll sleeps; a
    // watchdog thread ends the whole process after the idle limit.
    let idle = idle_limit();
    let memory_limit = memory_limit_bytes();
    let process_id = std::process::id();
    let activity = std::sync::Arc::new(std::sync::Mutex::new(Activity {
        last: Instant::now(),
        serving: false,
    }));
    let watchdog_activity = std::sync::Arc::clone(&activity);
    let watchdog_socket = socket.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5).min(idle));
            {
                // Decided and acted on under the lock, so a request the loop
                // has begun to serve is never ended as idle (review round 1
                // of ADR 0296). One accepted after this exit sees no header,
                // which its client answers one-shot.
                let activity = watchdog_activity.lock();
                let idle_for = activity.as_ref().map_or(idle, |activity| {
                    if activity.serving {
                        Duration::ZERO
                    } else {
                        activity.last.elapsed()
                    }
                });
                if idle_for >= idle {
                    let _ = fs::remove_file(&watchdog_socket);
                    remove_owned_program();
                    std::process::exit(0);
                }
            }
            // The memory limit holds even mid-request: its client then reads
            // a short or absent body, which it never takes as an answer.
            if memory_limit.is_some_and(|limit| {
                process_tree_resident_bytes(process_id).is_some_and(|resident| resident > limit)
            }) {
                let _ = fs::remove_file(&watchdog_socket);
                remove_owned_program();
                std::process::exit(0);
            }
        }
    });
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Ok(mut activity) = activity.lock() {
                    activity.serving = true;
                }
                if let Err(error) = handle(&mut state, request, stream) {
                    eprintln!("solid-checker daemon: {error}");
                }
                if let Ok(mut activity) = activity.lock() {
                    activity.serving = false;
                    activity.last = Instant::now();
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&socket);
                remove_owned_program();
                return Err(error.into());
            }
        }
    }
}

/// What the idle watchdog reads: when the last request ended, and whether
/// one is being served now.
struct Activity {
    last: Instant,
    serving: bool,
}

fn handle(
    state: &mut Option<State>,
    request: &Request,
    stream: UnixStream,
) -> Result<(), Box<dyn Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut check: CheckRequest = serde_json::from_str(&line)?;
    normalize_enablement(&mut check);
    let mut stream = reader.into_inner();
    if verify_implementation(implementation_identity()?, &check.implementation).is_err() {
        return respond_error(
            &mut stream,
            "daemon checker implementation identity mismatch",
        );
    }
    if check.project_id != request.project_id {
        return respond_error(&mut stream, "daemon serves a different project");
    }
    if check.dependency_sources != request.dependency_sources {
        return respond_error(
            &mut stream,
            "daemon serves a different dependency selection",
        );
    }
    if !request.dependency_sources.is_empty() && check.runtime != request.runtime {
        return respond_error(&mut stream, "daemon serves a different runtime host");
    }
    let outcome = answer_retained(state, request, &check);
    match outcome {
        Ok(answer) => {
            let materialized = !answer.cache_hit;
            let header = serde_json::to_vec(&CheckHeader {
                implementation: implementation_identity()?.to_owned(),
                ok: true,
                status: answer.status.to_string(),
                error: String::new(),
                cache_hit: answer.cache_hit,
                generation: answer.generation,
                analysis_ns: answer.analysis_ns,
                response_bytes: u64::try_from(answer.body.len()).unwrap_or(u64::MAX),
                notice: Some(answer.notice.to_string()),
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

fn normalize_enablement(check: &mut CheckRequest) {
    check.presets.sort();
    check.presets.dedup();
    check.enable_rules.sort();
    check.enable_rules.dedup();
    check.dependency_sources = normalized_selection(&check.dependency_sources);
}

fn respond_error(stream: &mut UnixStream, message: &str) -> Result<(), Box<dyn Error>> {
    let header = serde_json::to_vec(&CheckHeader {
        implementation: implementation_identity()?.to_owned(),
        ok: false,
        status: String::new(),
        error: message.into(),
        cache_hit: false,
        generation: 0,
        analysis_ns: 0,
        response_bytes: 0,
        notice: None,
    })?;
    stream.write_all(&header)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

/// Answer from the retained state, opening it first when an earlier request
/// discarded it.
///
/// A request that fails part-way may already have read edits it never
/// applied: a refresh records each file as it reads it, and the project's own
/// program takes its edits before preparation can still refuse. So the state
/// a failed request leaves is never trusted again. It is discarded whole --
/// session, trackers, cached answer and program file -- and the next request opens
/// the program from scratch, as a one-shot check would (review round 1 of
/// ADR 0296).
fn answer_retained(
    slot: &mut Option<State>,
    request: &Request,
    check: &CheckRequest,
) -> Result<Answer, Box<dyn Error>> {
    if slot.is_none() {
        let opened = State::open(request)?;
        own_program(opened.generated());
        *slot = Some(opened);
    }
    let Some(state) = slot.as_mut() else {
        return Err("daemon state was not opened".into());
    };
    let outcome = answer(state, request, check);
    if outcome.is_err() {
        if let Some(generated) = slot.take().as_ref().and_then(State::generated) {
            let _ = fs::remove_file(generated);
        }
        own_program(None);
    }
    outcome
}

fn answer(
    state: &mut State,
    request: &Request,
    check: &CheckRequest,
) -> Result<Answer, Box<dyn Error>> {
    let started = Instant::now();
    let changes = match state.resync(request)? {
        Sync::Rebuild => {
            let previous = state.generated().map(Path::to_path_buf);
            replace_state(state, State::open(request)?, previous);
            Vec::new()
        }
        Sync::Reprepare(prepared) => {
            // The project's own program is current; only the analyzed one
            // is reopened.
            let dependency = state
                .dependency
                .take()
                .ok_or("dependency-source state without its original program")?;
            let previous = dependency.generated.clone();
            match State::open_dependency(
                request,
                state.dialect,
                dependency.original,
                Some(*prepared),
            ) {
                Ok(next) => replace_state(state, next, previous),
                Err(error) => {
                    // The next request rebuilds: the retained program has
                    // lost its original, and its program file is gone.
                    if let Some(previous) = previous {
                        let _ = fs::remove_file(previous);
                    }
                    own_program(None);
                    return Err(error);
                }
            }
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
            notice: state
                .last
                .as_ref()
                .map_or_else(|| Arc::from(""), |last| Arc::clone(&last.notice)),
            cache_hit: true,
            generation: state.session.generation(),
            analysis_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        });
    }
    let demand = |state: &mut State| -> Result<(), Box<dyn Error>> {
        let options = semantic_demand_options_for_enablement(
            state.dialect,
            &state.project,
            RequestedRuleEnablement {
                presets: &check.presets,
                rules: &check.enable_rules,
                runtime: check.runtime.clone(),
                feedback_facts: false,
            },
        )?;
        state.session.set_semantic_demand_options(options);
        Ok(())
    };
    let facts = if changes.is_empty() {
        demand(state)?;
        state.session.analyze()?
    } else {
        // Read before the edit reaches the program, so it is the program
        // the session's sources were taken from.
        if state.modules.is_none() {
            state.modules = Some(state.session.program_modules()?);
        }
        demand(state)?;
        let facts = state.session.edit(changes, None)?;
        let modules = state.session.program_modules()?;
        if state.modules.as_ref() == Some(&modules) {
            facts
        } else {
            // An edited import added a file to the program or dropped one,
            // which the session's fixed source set does not follow.
            let previous = state.generated().map(Path::to_path_buf);
            replace_state(state, State::open(request)?, previous);
            demand(state)?;
            state.session.analyze()?
        }
    };
    // Observe what this program's resolution read, for the next request.
    state.resolution = Some(ResolutionWitness::of(&state.session.resolution_graph()?));
    let directory = state
        .project
        .parent()
        .ok_or("tsconfig has no parent directory")?;
    let source_refusals = state
        .dependency
        .as_ref()
        .map(|dependency| dependency.prepared.notes.refusals.clone())
        .unwrap_or_default();
    // The same package inputs the one-shot path reads.
    // Capture discovery's exact input generation before it runs. A refusal
    // needs only its witness; success retains the complete graph-input closure.
    let mut inference_inputs = inference_inputs_for_check(state, check);
    let (
        boundary,
        solid_facts_backend::InferredHost {
            hosts,
            note: inference_note,
        },
    ) = solid_facts_backend::check::package_inputs(
        directory,
        &facts,
        &source_refusals,
        &check.runtime,
        state.dialect,
    )?;
    if let Some(inputs) = &mut inference_inputs
        && let Some(hosts) = &hosts
    {
        // Final package admission can observe additional canonical targets.
        // Retain the identities actually used, rather than a post-analysis
        // rediscovery that could bless another generation of filesystem bytes.
        inputs.extend(
            hosts
                .manifest
                .inputs
                .iter()
                .map(|(path, identity)| (PathBuf::from(path), content_hash(identity.as_bytes()))),
        );
        inputs.sort();
        inputs.dedup();
    }
    // ADR 0296: preparation's notes come first, where a one-shot check
    // prints them.
    let dependency_notes = state
        .dependency
        .as_ref()
        .map(|dependency| dependency.prepared.notes.lines.clone())
        .unwrap_or_default();
    let notice: Arc<str> = dependency_notes
        .into_iter()
        .chain(inference_note)
        .collect::<Vec<_>>()
        .join("\n")
        .into();
    let analysis = state
        .diagnostics
        .analyze_inputs_measured_with_enablement(
            &state.project,
            &state.sources,
            &facts,
            solid_reactive_ir::AnalysisInputs {
                boundary: &boundary,
                hosts: hosts.as_ref(),
            },
            RequestedRuleEnablement {
                presets: &check.presets,
                rules: &check.enable_rules,
                runtime: check.runtime.clone(),
                feedback_facts: false,
            },
        )?
        .0;
    let body: Arc<[u8]> = snapshot_emission::emit(
        resolve_dialect(request)?,
        "json",
        &request.project_id,
        &analysis.snapshot,
        false,
        true,
        Duration::ZERO,
    )?
    .output
    .into();
    let status: Arc<str> = analysis.snapshot.status.as_str().into();
    let modules = imported_package_roots(&facts);
    let router_roots = solid_facts_backend::installed_bytes::router_install_roots(&facts);
    // An unreadable witness, or a change while discovery/analysis ran, prevents
    // caching. The ordinary answer still follows the existing refusal path.
    state.last = match inference_inputs.filter(|inputs| inference_inputs_current(inputs)) {
        Some(inference_inputs) => Some(CachedAnswer {
            generation: state.session.generation(),
            input_files: input_files(state, &modules, &router_roots)?,
            inference_inputs,
            notice: Arc::clone(&notice),
            presets: check.presets.clone(),
            enable_rules: check.enable_rules.clone(),
            runtime: check.runtime.clone(),
            modules,
            router_roots,
            status: Arc::clone(&status),
            body: Arc::clone(&body),
        }),
        None => None,
    };
    state.diagnostics.retain_for_idle(state.cache_retention);
    Ok(Answer {
        status,
        body,
        cache_hit: false,
        generation: state.session.generation(),
        analysis_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        notice,
    })
}

/// Return the cached snapshot only when its inputs still hold: same
/// generation, the same input files, and their contents are unchanged.
fn cached_answer(
    state: &State,
    check: &CheckRequest,
) -> Result<Option<CachedSnapshot>, Box<dyn Error>> {
    let Some(cached) = &state.last else {
        return Ok(None);
    };
    if cached.generation != state.session.generation()
        || cached.presets != check.presets
        || cached.enable_rules != check.enable_rules
        || cached.runtime != check.runtime
        || !inference_inputs_current(&cached.inference_inputs)
    {
        return Ok(None);
    }
    let current = input_files(state, &cached.modules, &cached.router_roots)?;
    Ok(cached.snapshot_if_current(
        state.session.generation(),
        &current,
        &check.presets,
        &check.enable_rules,
        &check.runtime,
    ))
}

fn inference_inputs_for_check(state: &State, check: &CheckRequest) -> Option<Vec<InputFile>> {
    let runtime = &check.runtime;
    if runtime.target.is_some()
        || !runtime.selected_conditions().is_empty()
        || runtime.program_boundary == Some(solid_reactive_ir::ProgramBoundary::Open)
    {
        return Some(Vec::new());
    }
    let directory = state.project.parent()?;
    let sources = state
        .sources
        .iter()
        .map(|source| PathBuf::from(&source.path))
        .collect::<Vec<_>>();
    let mut paths = solid_facts_backend::inferred_host_input_paths_for_sources(
        directory,
        &state.project,
        &sources,
    );
    // Nested execution boundaries are facts even without an accepted catalog.
    paths.extend(state.sources.iter().flat_map(|source| {
        Path::new(&source.path)
            .parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .map(|ancestor| ancestor.join("package.json"))
    }));
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            Some((
                path.clone(),
                solid_facts_backend::inferred_host_input_digest(&path)?,
            ))
        })
        .collect()
}

fn inference_inputs_current(inputs: &[InputFile]) -> bool {
    inputs.iter().all(|(path, expected)| {
        solid_facts_backend::inferred_host_input_digest(path).as_ref() == Some(expected)
    })
}

/// The current on-disk inputs besides the program and the inference closure:
/// the imported packages' manifests (the package boundary reads them), the
/// rule options, and the release manifests, each with its content hash,
/// sorted.
fn input_files(
    state: &State,
    modules: &[String],
    router_roots: &[PathBuf],
) -> Result<Vec<InputFile>, Box<dyn Error>> {
    let directory = state
        .project
        .parent()
        .ok_or("tsconfig has no parent directory")?;
    let mut paths = discovered_contract_paths(directory, modules)?;
    // Rule options are part of every diagnostic identity, and
    // `DiagnosticSession::analyze` re-reads them from disk on every run. The
    // cached-answer short-circuit returns before analyze does, so the file
    // must be in this input set or an options edit keeps serving the old
    // snapshot for a whole generation.
    if let Some(path) = solid_facts_backend::discovered_rule_options_path(directory) {
        paths.push(path);
    }
    // The installed `solid-js` manifest, and the manifest of every other
    // package the dialect names as owning a release-dependent answer
    // (`@solidjs/signals`, `@solidjs/web`), decide the release notice (SC9014)
    // that `DiagnosticSession::analyze` re-reads on every run, so an install
    // moving any of them between an audited and an unaudited release must not
    // keep serving the previous answer for a whole generation.
    paths.extend(solid_facts_backend::dialect::release_manifests(
        &state.project,
    ));
    // ADR 0193: an undeclared program boundary is read from the nearest
    // `package.json` above the project (`default_program_boundary`), which
    // `DiagnosticSession::analyze` re-reads on every run and which decides,
    // among others, whether ADR 0319's invocation flow runs at all. Every
    // candidate on that walk is an input -- an absent one too, since creating
    // a nearer manifest moves the answer -- or a manifest flipping the
    // program between closed and open keeps serving the old snapshot.
    // ADR 0324: a patch recorded by any of these decides whether the router
    // premise applies, so each lockfile on the walk is an input too.
    paths.extend(directory.ancestors().flat_map(|ancestor| {
        solid_facts_backend::installed_bytes::PATCH_RECORD_FILES
            .iter()
            .map(move |name| ancestor.join(name))
    }));
    match solid_facts_backend::dialect::detect_detailed(&state.project) {
        solid_facts_backend::dialect::Detection::Installed { manifest, .. }
        | solid_facts_backend::dialect::Detection::Unsupported { manifest, .. }
        | solid_facts_backend::dialect::Detection::Defaulted {
            manifest: Some(manifest),
        } => paths.push(manifest),
        solid_facts_backend::dialect::Detection::Defaulted { manifest: None } => {}
    }
    paths.sort();
    paths.dedup();
    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        // Absence is a state, not an error: a manifest deleted between runs
        // should invalidate the cache rather than fail the check.
        let hash = match if path.is_dir() {
            solid_facts_backend::inferred_host_directory_digest(&path)
                .ok_or_else(|| "cannot fingerprint inferred host directory".into())
        } else {
            hash_file(&path)
        } {
            Ok(hash) => hash,
            Err(_) if !path.exists() => [0_u8; 32],
            Err(error) => return Err(error),
        };
        files.push((path.clone(), hash));
    }
    // ADR 0324: the premise reads the installed router's bytes, which no
    // tracked source holds (`dist/index.js` is no program file), so each
    // resolved install is an input by the digest the premise compares.
    for root in router_roots {
        let digest = solid_facts_backend::installed_bytes::package_digest(root)
            .map_or([0_u8; 32], |digest| content_hash(digest.as_bytes()));
        files.push((root.join("dist"), digest));
    }
    Ok(files)
}

pub fn check(request: &Request) -> Result<i32, Box<dyn Error>> {
    let started = Instant::now();
    let request = &absolute_paths(request)?;
    // Resolved exactly once and threaded through every use -- the socket
    // hash, the spawned daemon's --dialect, the emission. Detection reads
    // the filesystem, so resolving again later could answer differently (an
    // `npm install` finishing between the calls) and split the client and
    // its daemon across two dialects.
    let dialect = resolve_dialect(request)?;
    let implementation = implementation_identity()?;
    let socket = socket_path_in_domain(
        &request.project_id,
        &request.typefacts_executable,
        dialect.id,
        implementation,
        dependency_domain(request).as_deref(),
    );
    let stream = match UnixStream::connect(&socket) {
        Ok(stream) => stream,
        Err(_) => spawn_and_connect(request, &socket, dialect)?,
    };
    let payload = serde_json::to_vec(&CheckRequest {
        implementation: implementation_identity()?.to_owned(),
        project_id: request.project_id.clone(),
        presets: request.presets.clone(),
        enable_rules: request.enable_rules.clone(),
        runtime: request.runtime.clone(),
        dependency_sources: normalized_selection(&request.dependency_sources),
    })?;
    let mut stream = stream;
    stream.write_all(&payload)?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = BufReader::new(stream);
    let mut header_line = String::new();
    reader.read_line(&mut header_line)?;
    let header: CheckHeader = serde_json::from_str(&header_line)?;
    verify_implementation(implementation, &header.implementation)?;
    if !header.ok {
        return Err(header.error.into());
    }
    // The whole body, and exactly as long as the header says, before anything
    // reaches the terminal: a daemon that exits while writing it (an idle or
    // memory limit) leaves an ordinary EOF after a partial body, and a partial
    // body is a failed answer, which falls back to one-shot with nothing of it
    // printed (review round 1 of ADR 0296).
    let mut body = Vec::new();
    reader.read_to_end(&mut body)?;
    let received = u64::try_from(body.len()).unwrap_or(u64::MAX);
    if received != header.response_bytes {
        return Err(format!(
            "daemon response has {received} of its {} bytes",
            header.response_bytes
        )
        .into());
    }
    let (output, code) = if request.format == "json" {
        // The daemon caches the canonical JSON emission; it is the output.
        // A project-level notice lives in the findings, not the header.
        let notices = request.certify
            && !request.internal
            && header.status == "uncertifiable"
            && serde_json::from_slice::<serde_json::Value>(&body)?["findings"]
                .as_array()
                .is_some_and(|findings| {
                    findings.iter().any(|finding| {
                        finding["kind"] == "uncertifiable" && finding["subjectKind"] == "project"
                    })
                });
        let fails =
            crate::snapshot_emission::certify_fails(&header.status, notices, request.internal);
        (body, i32::from(request.certify && fails))
    } else {
        let snapshot: solid_facts_backend::Snapshot = serde_json::from_slice(&body)?;
        if snapshot.status != header.status {
            return Err("daemon response status does not match snapshot".into());
        }
        let emission = snapshot_emission::emit(
            dialect,
            &request.format,
            &request.project_id,
            &snapshot,
            request.certify,
            request.internal,
            started.elapsed(),
        )?;
        (emission.output, emission.exit_code)
    };
    if let Some(notice) = header.notice.as_deref().filter(|notice| !notice.is_empty()) {
        eprintln!("{notice}");
    }
    // Past here the answer is this check's: a failed write is reported, never
    // followed by a one-shot answer on the same stdout.
    if let Err(error) = io::stdout().write_all(&output) {
        eprintln!("solid-checker: could not write the answer: {error}");
        return Ok(2);
    }
    report_timings(&header, started.elapsed(), received);
    Ok(code)
}

/// `request` with every path the daemon reads for it made absolute against
/// this caller's working directory (review round 1 of ADR 0296). A daemon
/// outlives the client that spawned it and keeps that client's working
/// directory, so a relative path would name the first caller's file, and
/// equal relative spellings from two directories would share a cached answer.
/// The project id is already absolute (`eligible`); `--dependency-source`
/// names bare specifiers, and no other field the daemon reads is a path.
fn absolute_paths(request: &Request) -> Result<Request, Box<dyn Error>> {
    let mut request = request.clone();
    request.typefacts_executable = executable_path(&request.typefacts_executable)?;
    Ok(request)
}

fn absolute(path: &str) -> Result<String, Box<dyn Error>> {
    Ok(std::path::absolute(path)?.to_string_lossy().into_owned())
}

/// The producer this caller would run: a path made absolute, or a bare name
/// resolved through this caller's `PATH` as `Command` would resolve it.
/// The daemon would otherwise search its first caller's.
fn executable_path(executable: &str) -> Result<String, Box<dyn Error>> {
    if executable.contains(std::path::MAIN_SEPARATOR) {
        return absolute(executable);
    }
    let search = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&search)
        .map(|directory| directory.join(executable))
        .find(|candidate| {
            fs::metadata(candidate).is_ok_and(|metadata| {
                metadata.is_file()
                    && std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0
            })
        })
        .ok_or_else(|| format!("Type Facts executable {executable:?} is not on PATH").into())
        .and_then(|found| absolute(&found.to_string_lossy()))
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
    dialect: &'static solid_facts_backend::dialect::Dialect,
) -> Result<UnixStream, Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--serve")
        .arg("--project")
        .arg(&request.project_id)
        .arg("--typefacts")
        .arg(&request.typefacts_executable);
    // `check` hashed this same resolved dialect into the socket path; the
    // spawned daemon must resolve identically. Detection can flip between
    // the client's hash and daemon startup (an `npm install` finishing, a
    // package.json edit), and a daemon that re-detects then binds a socket
    // the client never connects to — so the value `check` resolved once is
    // passed in and forwarded, never re-derived.
    command.arg("--dialect").arg(dialect.id);
    // ADR 0296: a dependency-source daemon prepares its program for exactly
    // this selection and host, which therefore cross verbatim.
    if !request.dependency_sources.is_empty() {
        for specifier in normalized_selection(&request.dependency_sources) {
            command.arg("--dependency-source").arg(specifier);
        }
        command
            .arg("--daemon-runtime")
            .arg(serde_json::to_string(&request.runtime)?);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                // A daemon can fail before it creates the socket (for
                // example, when the platform refuses Unix socket creation).
                // Do not spend the whole startup budget waiting for a child
                // that has already exited; this path is also the normal
                // fallback when a release binary cannot retain a session.
                if let Some(status) = child.try_wait()? {
                    return Err(format!(
                        "daemon exited before socket became ready ({status}): {error}"
                    )
                    .into());
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
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
        path::PathBuf,
        time::Duration,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        CheckHeader, CheckRequest, FileRefresh, ProcessMemory, Request, cache_retention_from,
        eligible, enabled_from, fingerprint_file, hash_file, inference_inputs_current,
        macho_build_identity, normalize_enablement, parse_process_memory,
        process_tree_resident_bytes_from, refresh_file_with, retained_format, serve, socket_path,
        socket_path_in_domain, timing_value, verify_implementation,
    };
    use solid_reactive_ir::{CacheRetention, RuntimeEnvironment};

    /// An ordinary check of an absolute project, daemon-eligible in every
    /// respect but its effective dependency-source selection.
    fn ordinary(selection: &[&str]) -> Request {
        serde_json::from_value(serde_json::json!({
            "projectId": "/project/tsconfig.json",
            "generation": 1,
            "sources": [],
            "typefactsExecutable": "/bin/typefacts",
            "dependencySources": selection,
        }))
        .unwrap()
    }

    #[test]
    fn an_effective_dependency_selection_is_never_eligible() {
        // ADR 0311. The request alone decides: a named list, `*`, a
        // duplicate, a mix and a selection that selects nothing all bypass.
        assert!(eligible(&ordinary(&[])));
        for selection in [
            &["dep"][..],
            &["*"][..],
            &["dep", "dep"][..],
            &["dep", "*"][..],
            &["package-that-selects-nothing"][..],
            &[""][..],
        ] {
            assert!(!eligible(&ordinary(selection)), "{selection:?}");
        }
    }

    #[test]
    fn serve_refuses_a_selection_before_it_reads_anything_else() {
        // The refusal is the first statement: this project, producer and
        // runtime do not exist, and no socket is bound.
        for selection in [&["dep"][..], &["*"][..], &[""][..]] {
            let error = serve(&ordinary(selection)).unwrap_err().to_string();
            assert!(error.contains("ADR 0311"), "{error}");
            assert!(error.contains("one-shot check"), "{error}");
        }
    }

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
    fn absent_daemon_enablement_fields_are_backward_compatible() {
        let mut absent: CheckRequest = serde_json::from_str(r#"{"projectId":"project"}"#).unwrap();
        normalize_enablement(&mut absent);
        assert!(absent.presets.is_empty());
        assert!(absent.enable_rules.is_empty());
    }

    #[test]
    fn retained_inference_closure_observes_content_absence_membership_and_symlinks() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "daemon-inference-inputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        fs::create_dir_all(&scratch).unwrap();
        let source = scratch.join("source.ts");
        fs::write(&source, "one").unwrap();
        let observed = |paths: &[PathBuf]| {
            paths
                .iter()
                .map(|path| {
                    (
                        path.clone(),
                        solid_facts_backend::inferred_host_input_digest(path).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let files = observed(std::slice::from_ref(&source));
        assert!(inference_inputs_current(&files));
        let modified = fs::metadata(&source).unwrap().modified().unwrap();
        fs::write(&source, "two").unwrap();
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert!(
            !inference_inputs_current(&files),
            "same size/mtime/inode cannot bless new bytes"
        );
        let absent = scratch.join("vite.config.js");
        let missing = observed(std::slice::from_ref(&absent));
        fs::write(&absent, "export default {};").unwrap();
        assert!(!inference_inputs_current(&missing));
        let directories = observed(std::slice::from_ref(&scratch));
        fs::write(scratch.join("excluded-server.ts"), "import './source';").unwrap();
        assert!(!inference_inputs_current(&directories));
        let alternate = scratch.join("alternate.ts");
        fs::write(&alternate, "two").unwrap();
        let link = scratch.join("linked.ts");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        let links = observed(std::slice::from_ref(&link));
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&alternate, &link).unwrap();
        assert!(
            !inference_inputs_current(&links),
            "equal target bytes do not imply equal link identity"
        );
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn daemon_enablement_order_and_duplicates_normalize_to_one_key() {
        let mut repeated = CheckRequest {
            implementation: String::from("test-checker"),
            project_id: "project".into(),
            presets: vec!["z".into(), "preferences".into(), "z".into()],
            enable_rules: vec![
                "prefer-show".into(),
                "prefer-for".into(),
                "prefer-show".into(),
            ],
            runtime: RuntimeEnvironment::default(),
            dependency_sources: vec!["dep-b".into(), "*".into(), "dep-b".into()],
        };
        normalize_enablement(&mut repeated);
        assert_eq!(repeated.presets, ["preferences", "z"]);
        assert_eq!(repeated.enable_rules, ["prefer-for", "prefer-show"]);
        assert_eq!(repeated.dependency_sources, ["*", "dep-b"]);
    }

    #[test]
    fn a_dependency_source_domain_is_its_own_daemon() {
        // ADR 0296: the selection and host decide the analyzed program, so
        // another selection or host must not reach this daemon's answers.
        let base = socket_path("/p/tsconfig.json", "/bin/typefacts", "solid-v2", "c");
        assert_eq!(
            base,
            socket_path_in_domain("/p/tsconfig.json", "/bin/typefacts", "solid-v2", "c", None),
            "the ordinary daemon keeps its identity"
        );
        let domain = |text: &str| {
            socket_path_in_domain(
                "/p/tsconfig.json",
                "/bin/typefacts",
                "solid-v2",
                "c",
                Some(text),
            )
        };
        assert_ne!(base, domain("a"));
        assert_ne!(domain("a"), domain("b"));
        assert_eq!(domain("a"), domain("a"));
    }

    #[test]
    fn loaded_macho_identity_requires_one_complete_build_uuid() {
        let mut command = Vec::new();
        command.extend(0x1b_u32.to_le_bytes());
        command.extend(24_u32.to_le_bytes());
        command.extend([7_u8; 16]);
        assert_eq!(
            macho_build_identity(&command, 1).as_deref(),
            Some("macho-uuid:07070707070707070707070707070707")
        );
        assert!(macho_build_identity(&command[..23], 1).is_none());
        assert!(macho_build_identity(&command, 0).is_none());
        assert!(macho_build_identity(&[], 0).is_none());
        let mut duplicate = command.clone();
        duplicate.extend(&command);
        assert!(macho_build_identity(&duplicate, 2).is_none());
        command[4..8].copy_from_slice(&0_u32.to_le_bytes());
        assert!(macho_build_identity(&command, 1).is_none());
    }

    #[test]
    fn same_path_replacement_cannot_reuse_an_older_implementation() {
        let scratch =
            std::env::temp_dir().join(format!("daemon-checker-identity-{}", std::process::id()));
        fs::create_dir_all(&scratch).unwrap();
        let path = scratch.join("checker");
        fs::write(&path, "old checker bytes").unwrap();
        let old = format!("{:x?}", hash_file(&path).unwrap());
        let replacement = scratch.join("replacement");
        fs::write(&replacement, "new checker bytes").unwrap();
        fs::rename(&replacement, &path).unwrap();
        let new = format!("{:x?}", hash_file(&path).unwrap());
        assert_ne!(old, new);
        assert_ne!(
            socket_path("project", "typefacts", "solid-v2", &old),
            socket_path("project", "typefacts", "solid-v2", &new)
        );
        assert!(verify_implementation(&new, &old).is_err());
        assert!(
            verify_implementation(&new, "").is_err(),
            "pre-handshake daemon refused"
        );
        assert!(
            verify_implementation(&old, &new).is_err(),
            "server rejects a foreign client"
        );
        assert!(verify_implementation(&new, &new).is_ok());
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn retained_actor_identity_includes_project_and_typefacts_build() {
        let baseline = socket_path(
            "/project/a/tsconfig.json",
            "/bin/typefacts-a",
            "solid-v2",
            "checker-a",
        );
        assert_ne!(
            baseline,
            socket_path(
                "/project/b/tsconfig.json",
                "/bin/typefacts-a",
                "solid-v2",
                "checker-a"
            )
        );
        assert_ne!(
            baseline,
            socket_path(
                "/project/a/tsconfig.json",
                "/bin/typefacts-b",
                "solid-v2",
                "checker-a"
            )
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
                implementation: String::from("test-checker"),
                ok: true,
                status: "certified".into(),
                error: String::new(),
                cache_hit: true,
                generation: 7,
                analysis_ns: 11,
                response_bytes: 13,
                notice: None,
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
