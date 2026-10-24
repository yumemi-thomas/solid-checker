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
    SourceChange, SourceFile, TypeFactsSession, imported_package_manifest_paths,
    imported_package_roots, semantic_demand_options_for_enablement,
};
use solid_reactive_ir::CacheRetention;
use solid_reactive_ir::RuntimeEnvironment;

use super::{Request, snapshot_emission};
use crate::daemon_cache::{CachedAnswer, CachedSnapshot, InputFile};
use crate::daemon_resolution::ResolutionWitness;
use crate::idle_memory;
use solid_facts_backend::dependency_program::{self, PreparedDependencies};

#[derive(Clone, Serialize, Deserialize)]
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
    /// The idle check found the previous answer differed from a fresh build
    /// (ADR 0394). The retained state was dropped; this answer
    /// is the fresh one, and a client that kept the earlier answer should
    /// replace it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    recovered: bool,
}

struct Answer {
    status: Arc<str>,
    body: Arc<[u8]>,
    cache_hit: bool,
    generation: u64,
    analysis_ns: u64,
    notice: Arc<str>,
    /// The reactive IR program the answer came from, for the idle check.
    program: Option<Arc<solid_reactive_ir::Program>>,
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
    // LC_UUID is the linker-issued build identity carried in the loaded image.
    let (commands, count) = crate::unsafe_boundary::loaded_executable_load_commands()?;
    macho_build_identity(commands, count)
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

/// The free memory the idle check keeps beyond its own second build
/// (`SOLID_CHECKER_IDLE_SHADOW_MIN_FREE_MB`, default 1 GiB).
fn idle_check_headroom_bytes() -> u64 {
    std::env::var("SOLID_CHECKER_IDLE_SHADOW_MIN_FREE_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1024)
        .saturating_mul(1024 * 1024)
}

/// Whether the machine has room for the idle check's second build (about
/// the daemon's own resident size) plus the headroom: `None` if it has, else
/// why not. Unknown memory counts as no room: a skipped check is only a
/// missed verification, never a served answer.
fn idle_check_room(root: u32) -> Option<String> {
    let Some(available) = available_memory_bytes() else {
        return Some("available memory unknown".to_owned());
    };
    let Some(resident) = process_tree_resident_bytes(root) else {
        return Some("resident memory unknown".to_owned());
    };
    let needed = resident.saturating_add(idle_check_headroom_bytes());
    (available < needed).then(|| {
        format!(
            "{} MiB available, {} MiB needed",
            available / (1024 * 1024),
            needed / (1024 * 1024)
        )
    })
}

/// The memory the system can hand out without swapping: Linux
/// `MemAvailable`; macOS free, inactive, speculative and purgeable pages.
fn available_memory_bytes() -> Option<u64> {
    if cfg!(target_os = "linux") {
        let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
        return parse_meminfo_available(&meminfo);
    }
    if cfg!(target_os = "macos") {
        let output = Command::new("vm_stat").output().ok()?;
        return output
            .status
            .success()
            .then(|| parse_vm_stat_available(&String::from_utf8_lossy(&output.stdout)))
            .flatten();
    }
    None
}

fn parse_meminfo_available(meminfo: &str) -> Option<u64> {
    meminfo.lines().find_map(|line| {
        let rest = line.strip_prefix("MemAvailable:")?;
        let kib = rest.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()?;
        Some(kib.saturating_mul(1024))
    })
}

fn parse_vm_stat_available(output: &str) -> Option<u64> {
    let mut lines = output.lines();
    let page = lines
        .next()?
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    let mut pages = 0_u64;
    let mut seen = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if matches!(
            name.trim(),
            "Pages free" | "Pages inactive" | "Pages speculative" | "Pages purgeable"
        ) {
            let count = value.trim().trim_end_matches('.').parse::<u64>().ok()?;
            pages = pages.saturating_add(count);
            seen = true;
        }
    }
    seen.then(|| pages.saturating_mul(page))
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
    modules: dependency_program::ProgramModules,
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
    let idle_check = idle_shadow::IdleShadow::from_env(&socket);
    idle_check.spawn(request.clone(), move || idle_check_room(process_id));
    let watchdog_check = Arc::clone(&idle_check);
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
                // The idle check's second build is transient: while the
                // machine still has room for it, it is not held to the
                // daemon's limit. Short of room it is the first thing to
                // give up: abandon it, and end the daemon only if the next
                // sample is still over the limit.
                if watchdog_check.running() && idle_check_room(process_id).is_none() {
                    continue;
                }
                if watchdog_check.relieve_memory() {
                    continue;
                }
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
                let recovered = idle_check.begin_request();
                if recovered {
                    // The idle check proved the retained state wrong: every
                    // retained cache goes with it, and this request opens a
                    // state from scratch.
                    if let Some(generated) = state.take().as_ref().and_then(State::generated) {
                        let _ = fs::remove_file(generated);
                    }
                    own_program(None);
                }
                if let Err(error) = handle(&mut state, request, stream, &idle_check, recovered) {
                    eprintln!("solid-checker daemon: {error}");
                }
                idle_check.end_request();
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
    idle_check: &idle_shadow::IdleShadow,
    recovered: bool,
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
    let outcome = answer_retained(state, request, &check, idle_check);
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
                recovered,
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
        recovered: false,
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
    idle_check: &idle_shadow::IdleShadow,
) -> Result<Answer, Box<dyn Error>> {
    if slot.is_none() {
        let opened = State::open(request)?;
        own_program(opened.generated());
        *slot = Some(opened);
    }
    let Some(state) = slot.as_mut() else {
        return Err("daemon state was not opened".into());
    };
    let outcome = answer_with(state, request, check, true, None);
    let Ok(mut answered) = outcome else {
        if let Some(generated) = slot.take().as_ref().and_then(State::generated) {
            let _ = fs::remove_file(generated);
        }
        own_program(None);
        idle_check.forget();
        return outcome;
    };
    if !answered.cache_hit && idle_check.enabled() {
        // The test fault hook perturbs the retained answer itself, so a
        // cache hit keeps serving it until the idle check drops the state.
        if let Some(perturbed) = idle_shadow::fault(&answered.body) {
            answered.body = Arc::clone(&perturbed);
            if let Some(last) = state_last(slot) {
                last.body = perturbed;
            }
        }
        if let Some(witness) = slot.as_ref().and_then(idle_shadow::Witness::of) {
            idle_check.record(idle_shadow::Served {
                check: check.clone(),
                status: Arc::clone(&answered.status),
                body: Arc::clone(&answered.body),
                notice: Arc::clone(&answered.notice),
                program: answered.program.clone(),
                witness,
            });
        } else {
            idle_check.forget();
        }
    }
    Ok(answered)
}

fn state_last(slot: &mut Option<State>) -> Option<&mut CachedAnswer> {
    slot.as_mut().and_then(|state| state.last.as_mut())
}

/// The reason an answer was abandoned because its idle check was cancelled.
const CANCELLED: &str = "cancelled";

/// Answer a check from `state`. `timings` reports the phases under
/// `SOLID_CHECKER_TIMINGS`; `cancel` abandons the answer between phases (the
/// idle check's fresh build only).
fn answer_with(
    state: &mut State,
    request: &Request,
    check: &CheckRequest,
    timings: bool,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Answer, Box<dyn Error>> {
    let started = Instant::now();
    let mut phases = PhaseClock::new(timings);
    let checkpoint = || -> Result<(), Box<dyn Error>> {
        if cancel.is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Relaxed)) {
            return Err(CANCELLED.into());
        }
        Ok(())
    };
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
            program: None,
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
    phases.mark("resync");
    checkpoint()?;
    // The files an overlay update edited while the program kept its files.
    let mut edited = None;
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
        let paths = changes
            .iter()
            .map(|change| change.path.clone())
            .collect::<Vec<_>>();
        let facts = state.session.edit(changes, None);
        let modules = state.session.program_modules();
        // An edited import added a file to the program or dropped one, which
        // the session's fixed source set does not follow. A dropped file can
        // fail the update's own analysis (its source is still demanded): the
        // program's files moved all the same, so that error is not the answer.
        let moved = modules
            .as_ref()
            .is_ok_and(|modules| state.modules.as_ref() != Some(modules));
        if moved {
            let previous = state.generated().map(Path::to_path_buf);
            replace_state(state, State::open(request)?, previous);
            demand(state)?;
            state.session.analyze()?
        } else {
            modules?;
            edited = Some(paths);
            facts?
        }
    };
    phases.mark("facts");
    checkpoint()?;
    // Review of ADR 0368: a project call's validity is the project's own
    // program's (the retained original, current: `resync` ran), and a package
    // whose own source has a call error is unproven.
    let mut package_refusals = Vec::new();
    let facts = match state.dependency.as_mut() {
        Some(DependencyState {
            original,
            prepared:
                PreparedDependencies {
                    program: Some(program),
                    ..
                },
            ..
        }) => {
            package_refusals = dependency_program::package_call_errors(&facts, program);
            let calls = dependency_program::project_calls(&facts, program);
            let own = if calls.is_empty() {
                Vec::new()
            } else {
                original.typescript.call_validities(&calls)?
            };
            let validity = dependency_program::project_call_validity(&facts, calls, own);
            if validity.is_empty() {
                facts
            } else {
                let mut owned = (*facts).clone();
                validity.apply(&mut owned);
                Arc::new(owned)
            }
        }
        _ => facts,
    };
    // Observe what this program's resolution read, for the next request. The
    // tracked sources are `resync_tracked`'s: their edits are overlay updates.
    // A retained witness was found current by `resync` this request, so it
    // still holds for the same program, and after an update that kept the
    // program's files it needs only the edited files' imports on top. A new
    // session (rebuilt, reprepared, or reopened by the update) has none.
    let tracked = &state.tracking.fingerprints;
    let tracked = |path: &str| tracked.contains_key(path);
    match (state.resolution.as_mut(), edited) {
        (Some(_), None) => {}
        (Some(witness), Some(paths)) => {
            let graph = state.session.resolution_graph_of(&paths)?;
            if graph.unknown_import_paths.is_empty() {
                witness.extend(&graph, tracked);
            } else {
                let graph = state.session.resolution_graph()?;
                state.resolution = Some(ResolutionWitness::of(&graph, tracked));
            }
        }
        (None, _) => {
            let graph = state.session.resolution_graph()?;
            state.resolution = Some(ResolutionWitness::of(&graph, tracked));
        }
    }
    phases.mark("resolution");
    let directory = state
        .project
        .parent()
        .ok_or("tsconfig has no parent directory")?;
    let mut source_refusals = state
        .dependency
        .as_ref()
        .map(|dependency| dependency.prepared.notes.refusals.clone())
        .unwrap_or_default();
    source_refusals.extend(package_refusals);
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
    phases.mark("package-inputs");
    checkpoint()?;
    let (analysis, diagnostic_timings) =
        state.diagnostics.analyze_inputs_measured_with_enablement(
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
        )?;
    phases.mark("analyze");
    checkpoint()?;
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
    // ADR 0334: the route-components premise reads the TanStack router's and
    // its router-core's bytes the same way.
    let mut router_roots = solid_facts_backend::installed_bytes::router_install_roots(&facts);
    router_roots.extend(solid_facts_backend::installed_bytes::route_install_roots(
        &facts,
    ));
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
    phases.mark("emit-and-cache");
    phases.report(
        state.session.generation(),
        &state.session.last_build_timings(),
        &diagnostic_timings,
    );
    state.diagnostics.retain_for_idle(state.cache_retention);
    Ok(Answer {
        status,
        body,
        cache_hit: false,
        generation: state.session.generation(),
        analysis_ns: u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        notice,
        program: Some(Arc::clone(&analysis.program)),
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
    let digests = solid_facts_backend::inferred_host_input_digests(
        &paths.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
    );
    paths
        .into_iter()
        .zip(digests)
        .map(|(path, digest)| Some((path, digest?)))
        .collect()
}

/// Every input still digests as recorded. Each is read again (never trusted
/// on metadata), in parallel: a project tree is thousands of inputs.
fn inference_inputs_current(inputs: &[InputFile]) -> bool {
    let paths = inputs
        .iter()
        .map(|(path, _)| path.as_path())
        .collect::<Vec<_>>();
    solid_facts_backend::inferred_host_input_digests(&paths)
        .iter()
        .zip(inputs)
        .all(|(digest, (_, expected))| digest.as_ref() == Some(expected))
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
    let mut paths = imported_package_manifest_paths(directory, modules)?;
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
    // ADR 0334: which router-core a TanStack router loads is the nearest
    // install on a walk from the router's real directory; a nearer one
    // appearing moves the answer, so every candidate is an input.
    paths.extend(
        router_roots
            .iter()
            .flat_map(|root| solid_facts_backend::installed_bytes::route_core_candidates(root)),
    );
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
    if header.recovered {
        eprintln!(
            "solid-checker: the daemon's earlier answer differed from a fresh build and was \
             dropped; this answer is the fresh one"
        );
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

/// The phases of one uncached daemon answer, emitted under
/// `SOLID_CHECKER_TIMINGS` on the daemon's stderr.
struct PhaseClock {
    emit: bool,
    last: Instant,
    phases: Vec<(&'static str, Duration)>,
}

impl PhaseClock {
    fn new(enabled: bool) -> Self {
        Self {
            emit: enabled && std::env::var_os("SOLID_CHECKER_TIMINGS").is_some(),
            last: Instant::now(),
            phases: Vec::new(),
        }
    }

    fn mark(&mut self, phase: &'static str) {
        if self.emit {
            self.phases.push((phase, self.last.elapsed()));
            self.last = Instant::now();
        }
    }

    fn report(
        &self,
        generation: u64,
        facts: &solid_facts_backend::NativeBuildTimings,
        diagnostics: &solid_facts_backend::DiagnosticTimings,
    ) {
        if !self.emit {
            return;
        }
        let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
        let phases = self
            .phases
            .iter()
            .map(|(phase, duration)| ((*phase).to_owned(), serde_json::json!(ms(*duration))))
            .collect::<serde_json::Map<_, _>>();
        eprintln!(
            "{}",
            serde_json::json!({
                "daemonPhases": {
                    "generation": generation,
                    "phasesMs": phases,
                    "facts": {
                        "sourceAnalysisMs": ms(facts.source_analysis),
                        "sourceFiles": [facts.source_files_reused, facts.source_files_recomputed],
                        "typeFactsMs": ms(facts.type_facts),
                        "exchangeRoundtripMs": ms(facts.exchange.roundtrip),
                        "serverAnalyzeMs": ms(facts.exchange.server_analyze),
                        "responseDecodeMs": ms(facts.exchange.response_decode),
                        "closeSymbolsMs": ms(facts.exchange.client_close_symbols),
                        "closurePatched": facts.exchange.client_closure_patched,
                        "hydrateMs": ms(facts.hydrate),
                        "joinMs": ms(facts.join),
                    },
                    "reactiveIrMs": ms(diagnostics.reactive_ir),
                    "solveAndSnapshotMs": ms(diagnostics.solve_and_snapshot),
                }
            })
        );
    }
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

mod idle_shadow;
#[cfg(test)]
mod tests;
