//! The proof log (ADR 0933): `SOLID_CHECKER_IR_PROOF_LOG=<path>` records how
//! each site's verdict came about, so a gate can fail on a clean that no fact
//! stands under (`scripts/proof-gate.mjs`).
//!
//! A site's verdict is made in four places, and each writes its own record
//! kind here, keyed by the file and the byte span of the site:
//!
//! - the producers: a row with a role, or an end with no row (`silent`; ADR
//!   0936: the producer records the end in its result and
//!   [`crate::site_accounting`] writes it, so the log and the runtime agree);
//! - the projection of rows to findings (`row`, with its typed reason, and
//!   `finding`), and the passes inside it that drop or demote one;
//! - the backend passes over the findings (`drop`, `reclass`, `verdict`);
//! - the accounting passes of SC4001 (ADR 0886) and the pending rules (ADR
//!   0838), whose endings are already typed (`silent`, `owner`, `pending`).
//!
//! Beside them, `decision` lines forward the role classifier's decisions
//! ([`crate::role_log`] frames them whenever either log is on), and `site`
//! lines list the sites of an independent universe (`proof_universe.rs`).
//! `scripts/fuzz/proofs.mjs` folds the lines into one entry per site.
//!
//! Every drop and demotion names a typed proof ([`DropPass`]) and the typed
//! facts it rests on ([`PremiseKind`]). A pass that removes a finding without
//! one cannot be written: the log has no other way to record it.
//!
//! One process writes each file path once, as `{"k":"path","q":pid,"p":id,
//! "f":path}` ahead of the first line that names it by `"q"` and `"p"`, the
//! role log's convention (`SOLID_CHECKER_IR_PROOF_LOG_PROGRAM=<id>` adds `"g"`
//! to it). A directory gets one `proof-<pid>.jsonl` per process.
//!
//! Write-only: nothing reads the log back, and every hook returns what it was
//! given. Unset, each hook is one atomic load at a coarse point (a projection,
//! the end of a build, a backend pass); the decisions the role log frames cost
//! what they did, because the role log's flag is the one flag
//! ([`crate::role_log::track_for_proof_log`]). Exact for a one-shot run
//! (`SOLID_CHECKER_DAEMON=0`): a retained daemon replays a memoized
//! computation's role-log lines only, so its proof log can miss a reused
//! decision.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use solid_facts::core::Span;
use typefacts::Location;

use crate::{ExecutionRole, Finding};

/// Lines are flushed when this much is buffered, and at the end of a build.
const FLUSH_AT: usize = 1 << 16;

type Fields = Map<String, Value>;

/// Where the proof log goes: a file (appended to) or a directory (a file per
/// process), and the program its lines belong to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofLogTarget {
    pub path: PathBuf,
    pub program: Option<String>,
}

impl ProofLogTarget {
    /// `SOLID_CHECKER_IR_PROOF_LOG` and `SOLID_CHECKER_IR_PROOF_LOG_PROGRAM`
    /// of this process; none when the first is unset or empty.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let path =
            std::env::var_os("SOLID_CHECKER_IR_PROOF_LOG").filter(|path| !path.is_empty())?;
        let program = std::env::var("SOLID_CHECKER_IR_PROOF_LOG_PROGRAM")
            .ok()
            .filter(|program| !program.is_empty());
        Some(Self {
            path: PathBuf::from(path),
            program,
        })
    }
}

struct Sink {
    file: std::fs::File,
    buffer: Vec<u8>,
    /// The program every path line names (`"g"`), if one was given.
    program: Option<String>,
    /// The id each path got, in the order its path line was written.
    paths: HashMap<String, u32>,
}

/// Whether a sink is set: read by every hook, so the log off costs a load.
static ON: AtomicBool = AtomicBool::new(false);
static SINK: Mutex<Option<Sink>> = Mutex::new(None);
/// The environment is read once, before the first hook or [`configure`].
static FROM_ENV: std::sync::Once = std::sync::Once::new();
/// Path ids are never given twice in a process.
static NEXT_PATH_ID: AtomicU32 = AtomicU32::new(0);

fn open_sink(target: &ProofLogTarget) -> Option<Sink> {
    let path = if target.path.is_dir() {
        target.path.join(format!("proof-{}.jsonl", process_id()?))
    } else {
        target.path.clone()
    };
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    Some(Sink {
        file,
        buffer: Vec::new(),
        program: target.program.clone(),
        paths: HashMap::new(),
    })
}

fn lock_sink() -> std::sync::MutexGuard<'static, Option<Sink>> {
    SINK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Reads the environment once. Run first by [`crate::role_log`]'s own
/// initialization too, so its frames are on before the first decision.
pub(crate) fn init_from_env() {
    FROM_ENV.call_once(|| {
        if let Some(sink) = ProofLogTarget::from_env().as_ref().and_then(open_sink) {
            *lock_sink() = Some(sink);
            ON.store(true, Ordering::Release);
            crate::role_log::track_for_proof_log(true);
        }
    });
}

/// Points the proof log at `target` (none: off), in place of this process's
/// environment, until the next call. What the previous target buffered is
/// written out first.
pub fn configure(target: Option<&ProofLogTarget>) {
    FROM_ENV.call_once(|| {});
    let on = {
        let mut sink = lock_sink();
        if let Some(previous) = sink.as_mut() {
            flush_locked(previous);
        }
        *sink = target.and_then(open_sink);
        sink.is_some()
    };
    ON.store(on, Ordering::Release);
    crate::role_log::track_for_proof_log(on);
}

/// The process id a log directory's file is named by; none where the platform
/// has no process ids (WASI), which leaves the log off.
fn process_id() -> Option<u32> {
    #[cfg(any(unix, windows))]
    {
        Some(std::process::id())
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Whether the proof log is on for this process.
#[inline]
pub(crate) fn enabled() -> bool {
    init_from_env();
    ON.load(Ordering::Acquire)
}

/// The process (`q`) and the id (`p`) a line names its file by.
#[derive(Clone, Copy)]
struct PathRef {
    process: u32,
    id: u32,
}

/// `path`'s id in this sink, writing its path line the first time.
fn path_ref(path: &str) -> Option<PathRef> {
    let mut guard = lock_sink();
    let sink = guard.as_mut()?;
    let process = process_id().unwrap_or(0);
    if let Some(id) = sink.paths.get(path) {
        return Some(PathRef { process, id: *id });
    }
    let id = NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed);
    sink.paths.insert(path.to_owned(), id);
    let path_ref = PathRef { process, id };
    let line = path_line(path_ref, path, sink.program.as_deref());
    push_locked(sink, &line);
    Some(path_ref)
}

/// The line that gives `path` its id, and names the program it belongs to.
fn path_line(path_ref: PathRef, path: &str, program: Option<&str>) -> Vec<u8> {
    let mut fields = Fields::new();
    fields.insert("k".to_owned(), json!("path"));
    fields.insert("q".to_owned(), json!(path_ref.process));
    fields.insert("p".to_owned(), json!(path_ref.id));
    fields.insert("f".to_owned(), json!(path));
    if let Some(program) = program {
        fields.insert("g".to_owned(), json!(program));
    }
    to_line(fields)
}

fn to_line(fields: Fields) -> Vec<u8> {
    let mut line = serde_json::to_vec(&Value::Object(fields)).unwrap_or_default();
    line.push(b'\n');
    line
}

/// Writes `fields` as one line about the site `span` of `path` (a line with no
/// span is about the file).
fn emit(path: &str, span: Option<(u64, u64)>, mut fields: Fields) {
    let Some(path) = path_ref(path) else {
        return;
    };
    fields.insert("q".to_owned(), json!(path.process));
    fields.insert("p".to_owned(), json!(path.id));
    if let Some((start, end)) = span {
        fields.insert("s".to_owned(), json!(start));
        fields.insert("e".to_owned(), json!(end));
    }
    append(&to_line(fields));
}

fn append(line: &[u8]) {
    if let Some(sink) = lock_sink().as_mut() {
        push_locked(sink, line);
    }
}

fn push_locked(sink: &mut Sink, line: &[u8]) {
    sink.buffer.extend_from_slice(line);
    if sink.buffer.len() >= FLUSH_AT {
        flush_locked(sink);
    }
}

fn flush_locked(sink: &mut Sink) {
    if sink.buffer.is_empty() {
        return;
    }
    // Whole lines only, under the file's exclusive advisory lock, as the role
    // log writes them: a grid's concurrent checkers share one file.
    let Sink { file, buffer, .. } = sink;
    let locked = file.lock().is_ok();
    let _ = file.write_all(buffer);
    if locked {
        let _ = file.unlock();
    }
    buffer.clear();
}

/// Writes out what is buffered (the end of each build, the end of the backend's
/// assembly).
pub(crate) fn flush() {
    if let Some(sink) = lock_sink().as_mut() {
        flush_locked(sink);
    }
}

/// [`flush`], for the backend.
pub fn flush_proof_log() {
    flush();
}

fn object(value: Value) -> Fields {
    match value {
        Value::Object(fields) => fields,
        _ => Fields::new(),
    }
}

fn span_pair(span: Span) -> (u64, u64) {
    (u64::from(span.start), u64::from(span.end))
}

fn location_pair(location: &Location) -> (u64, u64) {
    (location.start_byte, location.end_byte)
}

/// A typed fact a silence, a drop or a demotion rests on. The set is closed:
/// a proof names the kinds it rests on, and the gate counts a proof that
/// names none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PremiseKind {
    /// The user's rule options disable the rule (ADR 0440): a choice, not a
    /// fact about the code.
    UserRuleOption,
    /// The finding's file is under a dependency source root; the package is
    /// not the project's code (ADR 0284, `ProjectFacts::dependency_source_root`).
    DependencySourceRoot,
    /// Another enabled rule's finding stands at the same site and owns the
    /// claim (SC5001 for a pending read, SC1004, SC1007).
    OwningFindingAtSite,
    /// A violation stands at the same site (ADR 0822).
    ViolationAtSite,
    /// An equal finding is already in the list.
    EqualFinding,
    /// An authored suppression comment names the rule at the site (ADR 0429).
    SuppressionComment,
    /// The whole-program reach query proves no execution reaches the site of
    /// an absent prop (ADR 0696).
    SiteReachAbsentProp,
    /// The site-reach query's answer for the site: unreached, undecided,
    /// lazy, refused creation or dead code (ADR 0458).
    SiteReach,
    /// The site is in an instance field initializer, which runs when an
    /// instance is constructed.
    InstanceFieldInitializer,
    /// The site is in the callback of a resolved setter call whose write guard
    /// provably throws first: `ownedWrite` is proven off and the call's role
    /// reports a disallowed write, so the callback never runs and the write is
    /// the claim (SC2001, ADR 0932). Unknown or `ownedWrite: true` never
    /// carry it. The span is the setter call, where the violation stands.
    SetterGuardThrowsFirst,
    /// The site is in a function that returns JSX and is also handed to code
    /// that may call it as a plain function.
    UndecidedComponent,
    /// The site names a binding that may not be initialized there (ADR 0464).
    BindingBeforeInitialization,
    /// The compiler refused the file, so it has no compiler facts (ADR 0360).
    CompilerRefusedFile,
    /// The rule claims a client runtime window and the program's execution
    /// paths decide whether the site is in one (ADR 0659).
    ClientRuntimeReach,
    /// The runtime configuration is not the standard one (ADRs 0266, 0268).
    RuntimeConfiguration,
    /// An effect's apply is proven never to run: an exact `defer: true` and a
    /// compute that reads nothing (`effect_execution`, ADR 0590).
    ApplyNeverRuns,
    /// The compiler deleted the code: a `Value(Elided)` discarded region.
    CompilerDiscardedRegion,
    /// A closed program, every way into the helper a call expression the
    /// graph resolves (ADR 0203).
    ClosedProgramCallers,
    /// Every call site of the component passes the prop a static value, so
    /// the property read is not a reactive read at all (the props index,
    /// `PropUse::Static`).
    PropsCallersStatic,
    /// A `merge` view serves the key from a plain source (ADR 0630).
    MergeViewServesPlain,
    /// A view (`omit`, `merge`) hides the key the read names.
    ViewHidesKey,
    /// The read is a component's `ref` prop called once to publish its handle:
    /// not a reactive read.
    RefPropCall,
    /// A store `then` Get the thenable protocol makes exempt (ADR 0293).
    StoreThenGetExempt,
    /// The read sits in a named function at a tracked position (an effect's
    /// compute or another place the compiler and the dialect track), so it is
    /// a tracked read there (`named_at_tracked_position`).
    TrackedPosition,
    /// The read sits in a function handed by name to a carrier that runs it
    /// where no read rule reports; the premise's span is the carrier call,
    /// whose own decision says so (ADR 0531, `handed_read_role`).
    CarrierPlacesRead,
    /// `createMutable`: its proxy is designed to be written through.
    CreateMutableProxy,
    /// The write has an exempt standing: the store's own setter writing to
    /// its draft, a raw or shallow value (`WriteStanding::Exempt`).
    StoreWriteExemptStanding,
    /// `tsc` reports a diagnostic at the site, so the claim is TypeScript's
    /// (invariant 1).
    TscReportsAtSite,
    /// The store is shallow: a write below its root lands.
    ShallowStoreRoot,
    /// A logical assignment proven not to write (ADR 0718).
    LogicalAssignmentSkipsWrite,
    /// The dialect answers that an optimistic setter meets no owned-scope
    /// write guard (`optimistic_signal_setter_guarded`,
    /// `optimistic_store_setter_guarded`: rc.0, rc.17).
    OptimisticSetterUnguarded,
    /// An exact call edge of the call graph enters the function that holds the
    /// read: the summaries attribute its reads at the call, where the call's
    /// own role decides (`function_call_site_references`). The span is the
    /// callee reference at the call.
    CallEdgePlaces,
    /// The function's binding is handed to a reviewed fresh-stack scheduler
    /// (`setTimeout(f)`, `promise.then(f)`): it runs after the call with no
    /// strict-read window (`HandOff::FreshStack`). The span is the reference.
    FreshStackScheduler,
    /// The function that holds the read is a proven component, so its render
    /// enters it. The span is the function.
    RenderEntersComponent,
    /// The site is in a function argument that a known primitive provably never
    /// invokes (a stored-value position, a lazy adapter whose result is
    /// discarded), so the code in it never runs. The span is the primitive call.
    DormantCallbackArgument,
    /// Type Facts resolves a mutator call's callee to a standard-library
    /// declaration that is not the `Array` method the rule judges (a typed
    /// array's `fill`). The span is the callee.
    CalleeNotArrayMethod,
    /// The written root is a binding of an object pattern over the props
    /// (`const { program } = props`): it holds one prop's value, which the
    /// caller owns, not the props container the rule judges. The span is the
    /// root.
    DestructuredPropValue,
    /// The site is at module scope or in a static initializer: no function or
    /// instance field owns it, and no owner exists there to reject the access;
    /// the rc.21 runtime installs strict-read contexts only in component and
    /// effect bodies (the module-initialization role). The span is the site.
    ModuleScope,
    /// The site follows an `await` of its own async function that every path
    /// reaches, or that dominates it: it runs on a later task, off every
    /// caller's stack, in no render window and under no tracking scope (ADR
    /// 0320, 0323). The span is the `await`.
    AfterAwait,
    /// Nothing enters the function that holds the site: no call edge, carrier
    /// or scheduler names it, no render enters it, no unseen code calls it (a
    /// closed program's export, a private function: ADR 0203, 0934), and its
    /// binding is referenced nowhere, so its code never runs. The span is the
    /// function.
    NothingEnters,
    /// The role classifier decided the site's role (the compiler's and the
    /// dialect's facts place the code) and that role reports nothing for the
    /// rule: an effect's apply, a leaf scope, a handler. The span is the site.
    RoleDoesNotReport,
    /// A container a caller handed in holds a plain value on this route: a
    /// literal, a function or a `new` object, or a props declaration every
    /// call site of which passes one (ADR 0919). The span is that value.
    PlainValueOnEveryRoute,
    /// The function is handed whole to a position the compiler and the dialect
    /// place exactly and whose role reports nothing (an event handler slot, a
    /// tracked JSX expression): its reads run there. The span is the reference.
    HandedToSilentSlot,
}

/// A premise: a typed fact kind, the code the fact is about (`at`, in the
/// line's own file or in `file`), or none for a fact that is not about code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Premise {
    pub(crate) kind: PremiseKind,
    pub(crate) at: Option<Span>,
    /// The file `at` is in when it is not the line's own.
    pub(crate) file: Option<Arc<str>>,
}

impl Premise {
    /// A fact about the code at `at` in the line's own file.
    pub(crate) fn at(kind: PremiseKind, at: Option<Span>) -> Self {
        Self {
            kind,
            at,
            file: None,
        }
    }

    /// A fact about the code at `at` in `file`.
    pub(crate) fn in_file(kind: PremiseKind, at: Span, file: Arc<str>) -> Self {
        Self {
            kind,
            at: Some(at),
            file: Some(file),
        }
    }
}

fn premises_value(premises: &[Premise]) -> Value {
    Value::Array(
        premises
            .iter()
            .map(|premise| {
                let mut fields = object(json!({
                    "kind": format!("{:?}", premise.kind),
                    "at": premise.at.map(|at| [at.start, at.end]),
                }));
                if let Some(file) = &premise.file {
                    fields.insert("in".to_owned(), Value::String(file.to_string()));
                }
                Value::Object(fields)
            })
            .collect(),
    )
}

/// A pass that removes a finding or demotes a violation to an obligation, and
/// the typed facts it rests on. A finding that leaves the list leaves it
/// through a pass named here, or the gate fails the row that reported it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropPass {
    /// `retain_enabled`: the rule is disabled in the project's options.
    RuleDisabled,
    /// The finding sits in a dependency source root (ADR 0284).
    DependencySource,
    /// `suppress_findings_owned_by_enabled_rules`: a more specific enabled
    /// rule's finding stands at the site.
    OwnedByEnabledRule,
    /// An authored suppression comment (ADR 0429) removes a violation.
    HostSuppression,
    /// ADR 0696: a violation at a site no execution reaches, an absent prop.
    AbsentProp,
    /// An instance field initializer demotes a violation.
    InstanceField,
    /// An undecided component demotes a violation.
    UndecidedComponent,
    /// ADR 0458: the site-reach query demotes a violation.
    SiteUnreached,
    /// ADR 0464: a binding may not be initialized yet.
    BindingBeforeInitialization,
    /// ADR 0360: the compiler refused the file.
    CompilerRefusedFile,
    /// ADR 0659 and the deferred-mount rule: the program's execution paths.
    ExecutionPaths,
    /// ADR 0822: a violation at the same site makes a pending-value
    /// obligation redundant.
    SupersededByViolation,
    /// ADRs 0266/0268: the runtime configuration demotes a violation.
    RuntimeConfiguration,
    /// `finish_findings`: an equal finding is already in the list.
    EqualFinding,
}

impl DropPass {
    /// The typed proof's name in the log.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::RuleDisabled => "RuleDisabled",
            Self::DependencySource => "DependencySource",
            Self::OwnedByEnabledRule => "OwnedByEnabledRule",
            Self::HostSuppression => "HostSuppression",
            Self::AbsentProp => "AbsentProp",
            Self::InstanceField => "InstanceField",
            Self::UndecidedComponent => "UndecidedComponent",
            Self::SiteUnreached => "SiteUnreached",
            Self::BindingBeforeInitialization => "BindingBeforeInitialization",
            Self::CompilerRefusedFile => "CompilerRefusedFile",
            Self::ExecutionPaths => "ExecutionPaths",
            Self::SupersededByViolation => "SupersededByViolation",
            Self::RuntimeConfiguration => "RuntimeConfiguration",
            Self::EqualFinding => "EqualFinding",
        }
    }

    /// The fact kinds the pass rests on, each about the finding's site.
    pub(crate) const fn premise_kinds(self) -> &'static [PremiseKind] {
        match self {
            Self::RuleDisabled => &[PremiseKind::UserRuleOption],
            Self::DependencySource => &[PremiseKind::DependencySourceRoot],
            Self::OwnedByEnabledRule => &[PremiseKind::OwningFindingAtSite],
            Self::HostSuppression => &[PremiseKind::SuppressionComment],
            Self::AbsentProp => &[PremiseKind::SiteReachAbsentProp],
            Self::InstanceField => &[PremiseKind::InstanceFieldInitializer],
            Self::UndecidedComponent => &[PremiseKind::UndecidedComponent],
            Self::SiteUnreached => &[PremiseKind::SiteReach],
            Self::BindingBeforeInitialization => &[PremiseKind::BindingBeforeInitialization],
            Self::CompilerRefusedFile => &[PremiseKind::CompilerRefusedFile],
            Self::ExecutionPaths => &[PremiseKind::ClientRuntimeReach],
            Self::SupersededByViolation => &[PremiseKind::ViolationAtSite],
            Self::RuntimeConfiguration => &[PremiseKind::RuntimeConfiguration],
            Self::EqualFinding => &[PremiseKind::EqualFinding],
        }
    }
}

/// One finding as a stage sees it: enough to tell a finding that stayed from
/// one that left or changed kind.
#[derive(Clone, Debug, Eq, PartialEq)]
struct StageEntry {
    code: String,
    kind: String,
    context: String,
    path: String,
    start: u64,
    end: u64,
}

impl StageEntry {
    fn of(finding: &Finding) -> Self {
        Self {
            code: finding.id.clone(),
            kind: finding.kind.clone(),
            context: finding.analysis_context.clone(),
            path: finding.primary_location.path.to_string(),
            start: finding.primary_location.start_byte,
            end: finding.primary_location.end_byte,
        }
    }

    fn key(&self) -> (&str, &str, u64, u64) {
        (&self.code, &self.path, self.start, self.end)
    }
}

/// What a stage did to one finding it began with.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Change {
    /// The finding is gone.
    Dropped,
    /// The finding stayed at its site and changed kind (`to`).
    Reclassified { to: String },
}

/// What happened to each entry of `before` in `after`, by index into `before`
/// (an entry that stayed as it was is absent). The findings are matched by
/// code and site, one-to-one, a finding of the same kind first: a pass that
/// keeps the order or reorders is told the same.
fn changes(before: &[StageEntry], after: &[StageEntry]) -> Vec<(usize, Change)> {
    let mut remaining: HashMap<(&str, &str, u64, u64), Vec<&str>> = HashMap::new();
    for entry in after {
        remaining.entry(entry.key()).or_default().push(&entry.kind);
    }
    let mut moved = Vec::new();
    for (index, entry) in before.iter().enumerate() {
        let Some(kinds) = remaining.get_mut(&entry.key()) else {
            moved.push((index, Change::Dropped));
            continue;
        };
        if let Some(same) = kinds.iter().position(|kind| *kind == entry.kind) {
            kinds.swap_remove(same);
        } else if let Some(other) = kinds.pop() {
            moved.push((
                index,
                Change::Reclassified {
                    to: other.to_owned(),
                },
            ));
        } else {
            moved.push((index, Change::Dropped));
        }
    }
    moved
}

/// The findings before a pass that may drop or demote some. `begin` is none
/// with the log off; `end` writes what the pass moved, under its typed proof.
pub struct ProofStage {
    before: Vec<StageEntry>,
}

impl ProofStage {
    #[must_use]
    pub fn begin(findings: &[Finding]) -> Option<Self> {
        enabled().then(|| Self {
            before: findings.iter().map(StageEntry::of).collect(),
        })
    }

    /// Writes a `drop` line for each finding of `begin` that is not in
    /// `findings`, a `reclass` line for each that changed kind.
    pub fn end(self, pass: DropPass, findings: &[Finding]) {
        let after = findings.iter().map(StageEntry::of).collect::<Vec<_>>();
        for (index, change) in changes(&self.before, &after) {
            let entry = &self.before[index];
            emit(
                &entry.path,
                Some((entry.start, entry.end)),
                stage_fields(pass, entry, &change),
            );
        }
    }
}

/// The rows of a draft table before a pass that may remove some (ADR 0933):
/// a pass that runs between the producers and the program, which a finding's
/// stage never sees. `begin` is none with the log off; `end` writes a `silent`
/// line for each row the pass removed, under its typed proof.
pub(crate) struct RowStage {
    table: &'static str,
    before: Vec<Location>,
}

impl RowStage {
    pub(crate) fn begin<T>(
        table: &'static str,
        rows: &[T],
        at: impl Fn(&T) -> &Location,
    ) -> Option<Self> {
        enabled().then(|| Self {
            table,
            before: rows.iter().map(|row| at(row).clone()).collect(),
        })
    }

    pub(crate) fn end<T>(
        self,
        proof: &str,
        kinds: &[PremiseKind],
        rows: &[T],
        at: impl Fn(&T) -> &Location,
    ) {
        let mut remaining: HashMap<&Location, usize> = HashMap::new();
        for row in rows {
            *remaining.entry(at(row)).or_default() += 1;
        }
        for location in &self.before {
            if let Some(count) = remaining.get_mut(location)
                && *count > 0
            {
                *count -= 1;
                continue;
            }
            let span = u32::try_from(location.start_byte)
                .ok()
                .zip(u32::try_from(location.end_byte).ok())
                .map(|(start, end)| Span::new(start, end));
            let premises = kinds
                .iter()
                .map(|kind| Premise::at(*kind, span))
                .collect::<Vec<_>>();
            silent(&format!("row:{}", self.table), location, proof, &premises);
        }
    }
}

fn stage_fields(pass: DropPass, entry: &StageEntry, change: &Change) -> Fields {
    let at = u32::try_from(entry.start)
        .ok()
        .zip(u32::try_from(entry.end).ok())
        .map(|(start, end)| Span::new(start, end));
    let premises = pass
        .premise_kinds()
        .iter()
        .map(|kind| Premise::at(*kind, at))
        .collect::<Vec<_>>();
    let mut fields = object(json!({
        "code": entry.code,
        "from": entry.kind,
        "ctx": entry.context,
        "pass": pass.name(),
        "premises": premises_value(&premises),
    }));
    match change {
        Change::Dropped => {
            fields.insert("k".to_owned(), json!("drop"));
        }
        Change::Reclassified { to } => {
            fields.insert("k".to_owned(), json!("reclass"));
            fields.insert("to".to_owned(), json!(to));
        }
    }
    fields
}

/// The fields of a `decision` line: the role classifier answered `role` for a
/// `kind` query, through the tagged arms `arms`, the last returning `role`
/// being `arm` (with whether it is a self-declared default).
fn decision_fields(
    kind: &str,
    role: ExecutionRole,
    arm: Option<(&'static str, bool)>,
    arms: &[&'static str],
    silent: Option<bool>,
) -> Fields {
    object(json!({
        "k": "decision",
        "t": kind,
        "role": format!("{role:?}"),
        "arm": arm.map(|(id, _)| id),
        "arms": arms,
        "default": arm.is_some_and(|(_, default)| default),
        "silent": silent,
    }))
}

/// One outermost decision of the role classifier ([`crate::role_log`]).
pub(crate) fn decision(
    kind: &str,
    path: &str,
    span: Span,
    role: ExecutionRole,
    arm: Option<(&'static str, bool)>,
    arms: &[&'static str],
    silent: Option<bool>,
) {
    if !enabled() {
        return;
    }
    emit(
        path,
        Some(span_pair(span)),
        decision_fields(kind, role, arm, arms, silent),
    );
}

/// One owner answer of `owners::owner_emission` ([`crate::role_log::owner`]).
pub(crate) fn owner_decision(
    rule: &str,
    path: &str,
    span: Span,
    arm: (&'static str, bool),
    context: u8,
    silent: bool,
) {
    if !enabled() {
        return;
    }
    emit(
        path,
        Some(span_pair(span)),
        object(json!({
            "k": "owner",
            "rule": rule,
            "arm": arm.0,
            "default": arm.1,
            "ctx": context,
            "silent": silent,
        })),
    );
}

/// A pending rule's claim ended with no account ([`crate::role_log::pending`]).
pub(crate) fn pending_exit(rule: &str, path: &str, span: Span, exit: &str) {
    if !enabled() {
        return;
    }
    emit(
        path,
        Some(span_pair(span)),
        object(json!({ "k": "pending", "rule": rule, "exit": exit })),
    );
}

/// What a row of the program's tables is, for the log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RowTable {
    Read,
    Write,
    Action,
    AsyncRead,
}

impl RowTable {
    const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Action => "action",
            Self::AsyncRead => "async-read",
        }
    }
}

/// One row of the program: `reported` when projection turns it into a
/// finding, else `why` names the typed reason it does not.
pub(crate) fn row(
    table: RowTable,
    location: &Location,
    role: Option<ExecutionRole>,
    reported: bool,
    why: Option<&str>,
) {
    row_found_at(table, location, role, reported, why, None);
}

/// [`row`] for a row whose finding stands at another place of the same file
/// (a read reported at its origin, ADR 0930): `found_at` is that place.
pub(crate) fn row_found_at(
    table: RowTable,
    location: &Location,
    role: Option<ExecutionRole>,
    reported: bool,
    why: Option<&str>,
    found_at: Option<&Location>,
) {
    if !enabled() {
        return;
    }
    let mut fields = object(json!({
        "k": "row",
        "t": table.name(),
        "role": role.map(|role| format!("{role:?}")),
        "reported": reported,
        "why": why,
    }));
    if let Some(found) = found_at.filter(|found| found.path == location.path) {
        let (start, end) = location_pair(found);
        fields.insert("fa".to_owned(), json!([start, end]));
    }
    emit(
        location.path.as_ref(),
        Some(location_pair(location)),
        fields,
    );
}

/// The findings `project_findings` returns, after its own passes.
pub(crate) fn findings(findings: &[Finding]) {
    if !enabled() {
        return;
    }
    for finding in findings {
        emit_finding("finding", finding);
    }
}

/// The findings the backend returns, after every pass: the verdicts.
/// Flushes, as the last record of an assembly.
pub fn record_verdicts(findings: &[Finding]) {
    if !enabled() {
        return;
    }
    for finding in findings {
        emit_finding("verdict", finding);
    }
    flush();
}

fn emit_finding(kind: &str, finding: &Finding) {
    emit(
        finding.primary_location.path.as_ref(),
        Some(location_pair(&finding.primary_location)),
        object(json!({
            "k": kind,
            "code": finding.id,
            "kind": finding.kind,
            "ctx": finding.analysis_context,
        })),
    );
}

/// What a site of the independent universe is, and how it ends in the final
/// program (`proof_universe.rs`).
pub(crate) struct SiteRecord<'a> {
    /// `read`, `write`, `action` or `store-write`.
    pub(crate) table: &'a str,
    /// What the source is: `accessor`, `store`, `props`, `setter` or `action`.
    pub(crate) source: &'a str,
    /// The syntactic form: `call`, `member`, `assignment`, `delete`, `mutator`.
    pub(crate) via: &'a str,
    pub(crate) location: &'a Location,
    /// The other spans a row, obligation or silence that answers for the site
    /// may sit at (the links of a member chain, a call that has it as callee).
    pub(crate) alternates: &'a [Span],
    /// `row`, `census`, `discarded` or `none`.
    pub(crate) end: &'a str,
    /// The row's table and role, or the census's blocker.
    pub(crate) detail: &'a str,
}

pub(crate) fn site(record: &SiteRecord<'_>) {
    if !enabled() {
        return;
    }
    emit(
        record.location.path.as_ref(),
        Some(location_pair(record.location)),
        object(json!({
            "k": "site",
            "t": record.table,
            "source": record.source,
            "via": record.via,
            "alt": record.alternates.iter().map(|span| [span.start, span.end]).collect::<Vec<_>>(),
            "end": record.end,
            "detail": record.detail,
        })),
    );
}

/// A silent end a rule's account names: `proof` is the typed proof's name
/// (an enum variant, `ContextProof::OwnedContext`), `premises` the typed facts
/// it rests on. The accounting passes of ADRs 0886 and 0838 and
/// [`crate::site_accounting`] (the producers' named ends, ADR 0936) write it.
pub(crate) fn silent(rule: &str, location: &Location, proof: &str, premises: &[Premise]) {
    if !enabled() {
        return;
    }
    emit(
        location.path.as_ref(),
        Some(location_pair(location)),
        object(json!({
            "k": "silent",
            "rule": rule,
            "proof": proof,
            "premises": premises_value(premises),
        })),
    );
}

/// The span a location names, when it fits.
pub(crate) fn span_of(location: &Location) -> Option<Span> {
    let start = u32::try_from(location.start_byte).ok()?;
    let end = u32::try_from(location.end_byte).ok()?;
    Some(Span::new(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(code: &str, kind: &str, start: u64) -> StageEntry {
        StageEntry {
            code: code.to_owned(),
            kind: kind.to_owned(),
            context: String::new(),
            path: "/p/App.tsx".to_owned(),
            start,
            end: start + 3,
        }
    }

    #[test]
    fn a_stage_tells_a_drop_from_a_demotion_from_a_finding_that_stayed() {
        let before = vec![
            entry("SC1001", "violation", 10),
            entry("SC1001", "violation", 20),
            entry("SC2001", "violation", 30),
            entry("SC2001", "uncertifiable", 40),
        ];
        let after = vec![
            entry("SC2001", "violation", 30),
            entry("SC1001", "uncertifiable", 20),
            entry("SC2001", "uncertifiable", 40),
            entry("SC4001", "uncertifiable", 50),
        ];
        assert_eq!(
            changes(&before, &after),
            vec![
                (0, Change::Dropped),
                (
                    1,
                    Change::Reclassified {
                        to: "uncertifiable".to_owned()
                    }
                ),
            ]
        );
    }

    #[test]
    fn a_stage_matches_equal_findings_one_to_one() {
        let before = vec![
            entry("SC1001", "violation", 10),
            entry("SC1001", "violation", 10),
        ];
        let after = vec![entry("SC1001", "violation", 10)];
        assert_eq!(changes(&before, &after), vec![(1, Change::Dropped)]);
        assert!(changes(&after, &after).is_empty());
    }

    #[test]
    fn a_drop_line_names_its_proof_and_the_facts_it_rests_on() {
        let moved = entry("SC1001", "violation", 10);
        let fields = stage_fields(DropPass::AbsentProp, &moved, &Change::Dropped);
        let value = Value::Object(fields);
        assert_eq!(value["k"], "drop");
        assert_eq!(value["pass"], "AbsentProp");
        assert_eq!(value["code"], "SC1001");
        assert_eq!(value["from"], "violation");
        assert_eq!(value["premises"][0]["kind"], "SiteReachAbsentProp");
        assert_eq!(value["premises"][0]["at"], json!([10, 13]));
    }

    #[test]
    fn a_reclass_line_names_both_kinds() {
        let moved = entry("SC2001", "violation", 10);
        let fields = stage_fields(
            DropPass::InstanceField,
            &moved,
            &Change::Reclassified {
                to: "uncertifiable".to_owned(),
            },
        );
        let value = Value::Object(fields);
        assert_eq!(value["k"], "reclass");
        assert_eq!(value["from"], "violation");
        assert_eq!(value["to"], "uncertifiable");
    }

    #[test]
    fn every_pass_names_a_proof_and_at_least_one_premise() {
        let passes = [
            DropPass::RuleDisabled,
            DropPass::DependencySource,
            DropPass::OwnedByEnabledRule,
            DropPass::HostSuppression,
            DropPass::AbsentProp,
            DropPass::InstanceField,
            DropPass::UndecidedComponent,
            DropPass::SiteUnreached,
            DropPass::BindingBeforeInitialization,
            DropPass::CompilerRefusedFile,
            DropPass::ExecutionPaths,
            DropPass::SupersededByViolation,
            DropPass::RuntimeConfiguration,
            DropPass::EqualFinding,
        ];
        let mut names = std::collections::BTreeSet::new();
        for pass in passes {
            assert!(!pass.name().is_empty());
            assert!(
                !pass.premise_kinds().is_empty(),
                "{pass:?} rests on no fact"
            );
            assert!(names.insert(pass.name()), "{pass:?} repeats a name");
        }
        assert_eq!(names.len(), passes.len());
    }

    #[test]
    fn a_decision_line_carries_the_arm_and_whether_it_reports() {
        let fields = decision_fields(
            "read",
            ExecutionRole::UntrackedRendering,
            Some(("a.first", false)),
            &["a.first", "a.second"],
            Some(false),
        );
        let value = Value::Object(fields);
        assert_eq!(value["k"], "decision");
        assert_eq!(value["t"], "read");
        assert_eq!(value["role"], "UntrackedRendering");
        assert_eq!(value["arm"], "a.first");
        assert_eq!(value["arms"], json!(["a.first", "a.second"]));
        assert_eq!(value["default"], false);
        assert_eq!(value["silent"], false);
        let untagged = Value::Object(decision_fields(
            "role",
            ExecutionRole::Unknown,
            None,
            &[],
            None,
        ));
        assert_eq!(untagged["arm"], Value::Null);
        assert_eq!(untagged["silent"], Value::Null);
    }

    #[test]
    fn a_path_line_names_its_program() {
        let path = PathRef { process: 41, id: 2 };
        let plain = path_line(path, "/p/\"App\".tsx", None);
        assert_eq!(plain.last(), Some(&b'\n'));
        assert_eq!(
            serde_json::from_slice::<Value>(&plain).unwrap(),
            json!({ "k": "path", "q": 41, "p": 2, "f": "/p/\"App\".tsx" })
        );
        let named = path_line(path, "/p/App.tsx", Some("fuzz-7"));
        assert_eq!(
            serde_json::from_slice::<Value>(&named).unwrap(),
            json!({ "k": "path", "q": 41, "p": 2, "f": "/p/App.tsx", "g": "fuzz-7" })
        );
    }

    #[test]
    fn premises_are_written_with_their_spans() {
        let premises = [
            Premise::at(PremiseKind::UserRuleOption, None),
            Premise::at(PremiseKind::SiteReach, Some(Span::new(3, 9))),
            Premise::in_file(
                PremiseKind::CallEdgePlaces,
                Span::new(4, 5),
                "/p/B.tsx".into(),
            ),
        ];
        assert_eq!(
            premises_value(&premises),
            json!([
                { "kind": "UserRuleOption", "at": null },
                { "kind": "SiteReach", "at": [3, 9] },
                { "kind": "CallEdgePlaces", "at": [4, 5], "in": "/p/B.tsx" },
            ])
        );
    }
}
