//! L3: what a stage chunk read of the rest of the program (P5 study §3.2 L3).
//!
//! A per-file stage cache (a chunk: one stage's output for one file) is
//! reused when its own file is unchanged and every cross-file answer it read
//! answers as it did. The reads are recorded here, on the computing thread,
//! while the chunk runs ([`record`]):
//! - a memo answer read outside every memo ([`note_memo`], from
//!   [`crate::base::memo::build::BuildWorld::answer`]): its table, query and
//!   the revision its value last changed in, the store's own fingerprint;
//! - a whole-table view the lookup assembles from many answers or from other
//!   files ([`Section`], [`note`]): fingerprinted per build by the lookup
//!   from its value ([`crate::indexes::SemanticLookup::section_fingerprint`]).
//!
//! **Derived caches replay their reads.** A per-build cache filled by one
//! chunk and read by another ([`Recorded`]) records what its own
//! initialization read and hands that to every reader, so a reader that
//! finds the cache filled records the same reads as the one that filled it.
//! Work handed to worker threads ([`crate::pipeline`]'s parallel helpers)
//! records on each worker and merges at the join, so a chunk's record is the
//! same whatever thread computes what.
//!
//! **Other files' facts.** The lookup hands a chunk another file through a
//! few questions (which function a symbol names, a binding, a path, a
//! function's callers); each hand-out is recorded as the question's answer
//! and the handed file's section fingerprints (`indexes::hand_outs`), so a
//! chunk reruns when anything it may have read of that file moves.
//!
//! **What is not recorded.** A memo computation's own reads are its memo's
//! (the store records them), so nothing inside a memo computation reaches a
//! chunk ([`suppress`]); a section's own computation likewise (its value is
//! the fingerprint).
//!
//! **Soundness** is the trace-replay lemma (study §3.3): a deterministic
//! chunk whose recorded reads all answer as recorded re-traces the same
//! path and returns the same value.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use super::memo::{QueryId, Revision, TableId};

/// A whole-table view of the lookup that a chunk reads as one value. Each is
/// fingerprinted from its value, per build, on the first check that needs
/// it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub(crate) enum Section {
    /// Proven, escaped and possible components (ADR 0331, 0388).
    Components,
    /// Exact object-member targets (ADR 0299, 0386).
    MemberTargets,
    /// What each intrinsic element's spread supplies (ADR 0358).
    SpreadTargets,
    /// Members whose receiver is an exact object literal (ADR 0386).
    LiteralReceivers,
    /// The invocation-flow answer of every literal (ADR 0319, 0391).
    FlowAnswers,
    /// The literals every invocation of which is an event delivery (ADR 0327).
    EventLiterals,
    /// The calls that never return normally (ADR 0278, 0279).
    NeverReturning,
    /// The returned-source answers and what a fragment reads of each
    /// creating call (ADR 0222, 0287, 0392).
    ReturnedSources,
    /// The returned-callback invocations and their sites (ADR 0390, 0392).
    ReturnedCallbacks,
    /// The synchronous event-dispatch hazard (ADR 0356).
    SynchronousDispatch,
    /// The reflective-definition hazard (ADR 0293, 0314).
    ReflectiveDefinition,
    /// A reflective write through a `prototype` (ADR 0292).
    ReflectivePrototypeWrite,
    /// The route component values (ADR 0334), whose router configurations
    /// may sit in another file than the value (ADR 0408).
    RouteComponents,
    /// The bindings a `createContext` call initializes, in any file, and
    /// TypeScript's references to each (`is_context_reference`).
    ContextSymbols,
    /// The dialect primitives the project names anywhere, which decide the
    /// project-wide callback capability flags and their early exits.
    ProjectPrimitives,
}

impl Section {
    pub(crate) const ALL: [Self; 15] = [
        Self::Components,
        Self::MemberTargets,
        Self::SpreadTargets,
        Self::LiteralReceivers,
        Self::FlowAnswers,
        Self::EventLiterals,
        Self::NeverReturning,
        Self::ReturnedSources,
        Self::ReturnedCallbacks,
        Self::SynchronousDispatch,
        Self::ReflectiveDefinition,
        Self::ReflectivePrototypeWrite,
        Self::RouteComponents,
        Self::ContextSymbols,
        Self::ProjectPrimitives,
    ];

    const fn bit(self) -> u32 {
        1 << (self as u8)
    }
}

/// A memo answer read outside every memo: the store's own fingerprint of
/// it, the revision in which its value last changed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MemoRead {
    pub(crate) table: TableId,
    pub(crate) query: QueryId,
    pub(crate) changed_at: Revision,
}

/// Everything one computation read of the rest of the program.
///
/// The memo reads are kept as sorted runs without duplicates, shared by
/// `Arc` with the sets they were added from, and a short unsorted tail
/// (ADR 0406). Adding a set ([`Self::extend`]) adds its runs by
/// pointer, skipping one already held, so a set added to a thousand others
/// (the reads a stage made before its chunks ran) is neither copied nor
/// sorted again for each; small runs are merged once there are many. A read
/// may sit in more than one run: a check of the set asks it again, with the
/// same answer.
#[derive(Clone, Debug, Default)]
pub(crate) struct Reads {
    sections: u32,
    /// Each sorted, without duplicates, never empty.
    runs: Vec<Arc<[MemoRead]>>,
    /// At most [`TAIL_MAX`] reads not in a run yet, in the order added.
    tail: Vec<MemoRead>,
}

impl PartialEq for Reads {
    fn eq(&self, other: &Self) -> bool {
        self.sections == other.sections && self.distinct_memos() == other.distinct_memos()
    }
}

impl Eq for Reads {}

/// A tail longer than this becomes a run.
const TAIL_MAX: usize = 64;
/// More runs than this: the smaller half is merged into one.
const RUNS_MAX: usize = 16;

impl Reads {
    pub(crate) fn is_empty(&self) -> bool {
        self.sections == 0 && self.runs.is_empty() && self.tail.is_empty()
    }

    pub(crate) fn sections(&self) -> impl Iterator<Item = Section> + '_ {
        Section::ALL
            .into_iter()
            .filter(|section| self.sections & section.bit() != 0)
    }

    #[cfg(test)]
    pub(crate) fn reads_section(&self, section: Section) -> bool {
        self.sections & section.bit() != 0
    }

    /// The memo reads, sorted without duplicates.
    #[cfg(test)]
    pub(crate) fn memos(&self) -> Vec<MemoRead> {
        self.distinct_memos()
    }

    /// The memo reads, sorted without duplicates, and the whole-table
    /// sections read: what a memo computation that recorded its lookup
    /// reads records in its own context
    /// ([`crate::base::memo::Ctx::project_file_recorded`]).
    pub(crate) fn into_memo_reads(self) -> (Vec<MemoRead>, Vec<Section>) {
        (self.distinct_memos(), self.sections().collect())
    }

    /// Every memo read, a read held twice listed twice.
    fn memo_reads(&self) -> impl Iterator<Item = &MemoRead> + '_ {
        self.runs
            .iter()
            .flat_map(|run| run.iter())
            .chain(&self.tail)
    }

    fn has_memos(&self) -> bool {
        !self.runs.is_empty() || !self.tail.is_empty()
    }

    fn distinct_memos(&self) -> Vec<MemoRead> {
        let mut memos = self.memo_reads().copied().collect::<Vec<_>>();
        memos.sort_unstable();
        memos.dedup();
        memos
    }

    /// The reads of a finished recording: its own notes (sorted, without
    /// duplicates) and the runs it replayed.
    fn of(sections: u32, mut runs: Vec<Arc<[MemoRead]>>, own: Vec<MemoRead>) -> Self {
        if !own.is_empty() {
            runs.push(Arc::from(own));
        }
        let mut reads = Self {
            sections,
            runs,
            tail: Vec::new(),
        };
        reads.bound_runs();
        reads
    }

    /// These reads without the answers of the tables in `tables`: what a
    /// reader keeps of another computation's reads when it compares those
    /// tables' inputs itself (`indexes::hand_outs`).
    pub(crate) fn without_tables(&self, tables: std::ops::Range<u16>) -> Self {
        let kept = |read: &MemoRead| !tables.contains(&read.table.0);
        if self.memo_reads().all(kept) {
            return self.clone();
        }
        let runs = self
            .runs
            .iter()
            .filter_map(|run| {
                if run.iter().all(kept) {
                    return Some(Arc::clone(run));
                }
                let run = run
                    .iter()
                    .filter(|read| kept(read))
                    .copied()
                    .collect::<Vec<_>>();
                (!run.is_empty()).then(|| Arc::from(run))
            })
            .collect();
        Self {
            sections: self.sections,
            runs,
            tail: self
                .tail
                .iter()
                .filter(|read| kept(read))
                .copied()
                .collect(),
        }
    }

    /// Adds `other`'s reads.
    pub(crate) fn extend(&mut self, other: &Self) {
        self.sections |= other.sections;
        for run in &other.runs {
            self.add_run(run);
        }
        if other.tail.is_empty() {
            self.bound_runs();
            return;
        }
        self.tail.extend_from_slice(&other.tail);
        if self.tail.len() > TAIL_MAX {
            let mut tail = std::mem::take(&mut self.tail);
            cost::timed(cost::EXTEND, tail.len(), || {
                tail.sort_unstable();
                tail.dedup();
            });
            self.runs.push(Arc::from(tail));
        }
        self.bound_runs();
    }

    fn add_run(&mut self, run: &Arc<[MemoRead]>) {
        if !self.runs.iter().any(|held| Arc::ptr_eq(held, run)) {
            self.runs.push(Arc::clone(run));
        }
    }

    /// Merges the smaller half of the runs into one while there are more
    /// than [`RUNS_MAX`]: a read is merged again only with runs at least as
    /// large as its own, and a large shared run stays shared.
    fn bound_runs(&mut self) {
        if self.runs.len() <= RUNS_MAX {
            return;
        }
        self.runs.sort_by_key(|run| std::cmp::Reverse(run.len()));
        let small = self.runs.split_off(RUNS_MAX / 2);
        let elements = small.iter().map(|run| run.len()).sum::<usize>();
        let merged = cost::timed(cost::MERGE, elements, || {
            let mut merged = Vec::with_capacity(elements);
            for run in &small {
                merged.extend_from_slice(run);
            }
            // Each run is sorted: the stable sort merges the runs it finds
            // instead of sorting the whole from scratch.
            merged.sort();
            merged.dedup();
            merged
        });
        self.runs.push(Arc::from(merged));
    }
}

/// What keeping read sets costs, for `SOLID_CHECKER_IR_BOOKKEEPING_LOG`
/// (off unless that names a file): per kind of normalization, the calls,
/// the elements they normalized and their CPU time, summed over threads.
pub(crate) mod cost {
    use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

    pub(crate) const FINISH: usize = 0;
    pub(crate) const EXTEND: usize = 1;
    pub(crate) const SO_FAR: usize = 2;
    pub(crate) const MERGE: usize = 3;
    pub(crate) const KINDS: [&str; 4] = ["finish", "tail", "soFar", "merge"];

    static ON: AtomicU8 = AtomicU8::new(2);
    static CALLS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
    static ELEMENTS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
    static NANOS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

    /// Whether the costs are measured.
    pub(crate) fn on() -> bool {
        match ON.load(Ordering::Relaxed) {
            0 => false,
            1 => true,
            _ => {
                let on = std::env::var_os("SOLID_CHECKER_IR_BOOKKEEPING_LOG").is_some();
                ON.store(u8::from(on), Ordering::Relaxed);
                on
            }
        }
    }

    /// Nanoseconds since the first call: what a measured computation took.
    /// Measured phases run on one thread ([`super::super::memo::build`]),
    /// so a duration there is CPU time, not a wait on another thread.
    pub(crate) fn now_ns() -> u64 {
        static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        u64::try_from(
            EPOCH
                .get_or_init(std::time::Instant::now)
                .elapsed()
                .as_nanos(),
        )
        .unwrap_or(u64::MAX)
    }

    pub(crate) fn timed<R>(kind: usize, elements: usize, f: impl FnOnce() -> R) -> R {
        if !on() {
            return f();
        }
        let started = now_ns();
        let value = f();
        let nanos = now_ns().saturating_sub(started);
        CALLS[kind].fetch_add(1, Ordering::Relaxed);
        ELEMENTS[kind].fetch_add(elements as u64, Ordering::Relaxed);
        NANOS[kind].fetch_add(nanos, Ordering::Relaxed);
        value
    }

    /// The costs since the last call, `(kind, calls, elements, nanos)`.
    pub(crate) fn take() -> Vec<(&'static str, u64, u64, u64)> {
        KINDS
            .iter()
            .enumerate()
            .map(|(kind, name)| {
                (
                    *name,
                    CALLS[kind].swap(0, Ordering::Relaxed),
                    ELEMENTS[kind].swap(0, Ordering::Relaxed),
                    NANOS[kind].swap(0, Ordering::Relaxed),
                )
            })
            .collect()
    }
}

/// One thread's recording: the innermost frame receives every note.
#[derive(Default)]
struct Frame {
    /// Opened by [`suppress`]: what it receives goes nowhere.
    suppressed: bool,
    /// This thread's count of frames opened when this one was: tells a
    /// frame apart from the one before it at the same depth.
    serial: u64,
    sections: u32,
    memos: Vec<MemoRead>,
    /// The runs of the read sets replayed into this frame, each once.
    runs: Vec<Arc<[MemoRead]>>,
    /// The replayed [`Recorded`] read sets, by address, so a cache read a
    /// thousand times is added once.
    replayed: BTreeSet<usize>,
}

impl Frame {
    fn finish(mut self) -> Reads {
        cost::timed(cost::FINISH, self.memos.len(), || {
            self.memos.sort_unstable();
            self.memos.dedup();
        });
        Reads::of(self.sections, self.runs, self.memos)
    }

    fn add(&mut self, reads: &Reads) {
        self.sections |= reads.sections;
        for run in &reads.runs {
            if !self.runs.iter().any(|held| Arc::ptr_eq(held, run)) {
                self.runs.push(Arc::clone(run));
            }
        }
        self.memos.extend_from_slice(&reads.tail);
    }
}

thread_local! {
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    /// `FRAMES`' length: the notes outside every recording (most lookup
    /// reads of a build) test one cell instead of borrowing the stack.
    static DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// How many frames this thread has opened ([`Frame::serial`]).
    static OPENED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Whether some computation on this thread is recording.
pub(crate) fn recording() -> bool {
    DEPTH.with(std::cell::Cell::get) != 0
}

/// The recording a note made now reaches, by its frame's serial: `None`
/// outside every recording and inside a [`suppress`]. A reader whose note
/// costs a lookup (an answer it must ask) asks this first, and may skip a
/// note it already made in the same frame.
pub(crate) fn recording_frame() -> Option<u64> {
    if !recording() {
        return None;
    }
    FRAMES.with(|frames| {
        frames
            .borrow()
            .last()
            .filter(|frame| !frame.suppressed)
            .map(|frame| frame.serial)
    })
}

/// Records that the current computation read `section`.
pub(crate) fn note(section: Section) {
    if !recording() {
        return;
    }
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.sections |= section.bit();
        }
    });
}

/// Records that the current computation read a memo answer.
pub(crate) fn note_memo(read: MemoRead) {
    if !recording() {
        return;
    }
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.memos.push(read);
        }
    });
}

/// Adds `reads` to the current computation's, as if it had made them.
pub(crate) fn replay(reads: &Reads) {
    if reads.is_empty() || !recording() {
        return;
    }
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.add(reads);
        }
    });
}

/// [`replay`] for a shared read set, copied once per frame.
fn replay_shared(reads: &Arc<Reads>) {
    if reads.is_empty() || !recording() {
        return;
    }
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            if !reads.has_memos() {
                frame.sections |= reads.sections;
            } else if frame.replayed.insert(Arc::as_ptr(reads) as usize) {
                frame.add(reads);
            }
        }
    });
}

/// Pops the frame [`push`] opened, even on unwind.
struct FrameGuard;

impl Drop for FrameGuard {
    fn drop(&mut self) {
        FRAMES.with(|frames| {
            frames.borrow_mut().pop();
        });
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

fn push() -> FrameGuard {
    let serial = OPENED.with(|opened| {
        opened.set(opened.get() + 1);
        opened.get()
    });
    FRAMES.with(|frames| {
        frames.borrow_mut().push(Frame {
            serial,
            ..Frame::default()
        });
    });
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    FrameGuard
}

fn take_top() -> Frame {
    FRAMES.with(|frames| {
        std::mem::take(
            frames
                .borrow_mut()
                .last_mut()
                .expect("a recording frame is open"),
        )
    })
}

/// Runs `f` recording what it reads, returns the reads, and hands them to
/// the enclosing computation too.
pub(crate) fn record<R>(f: impl FnOnce() -> R) -> (R, Reads) {
    let guard = push();
    let value = f();
    let reads = take_top().finish();
    drop(guard);
    replay(&reads);
    (value, reads)
}

/// [`record`], without handing the reads to the enclosing computation: a
/// chunk whose reads its stage replays itself once it has read what the
/// stage's own proof needs (`interproc::reads`).
pub(crate) fn record_detached<R>(f: impl FnOnce() -> R) -> (R, Reads) {
    let guard = push();
    let value = f();
    let reads = take_top().finish();
    drop(guard);
    (value, reads)
}

/// An open recording ([`begin`]): what the computation read so far can be
/// read without ending it.
pub(crate) struct Recording {
    _guard: FrameGuard,
}

/// Starts recording a computation that spans several steps; [`Recording::finish`]
/// ends it. Recordings and [`record`]s inside it must be balanced, as they
/// are by construction.
pub(crate) fn begin() -> Recording {
    Recording { _guard: push() }
}

impl Recording {
    /// What the computation read so far.
    pub(crate) fn so_far(&self) -> Reads {
        FRAMES.with(|frames| {
            let frames = frames.borrow();
            let frame = frames.last().expect("a recording frame is open");
            let mut memos = frame.memos.clone();
            cost::timed(cost::SO_FAR, memos.len(), || {
                memos.sort_unstable();
                memos.dedup();
            });
            Reads::of(frame.sections, frame.runs.clone(), memos)
        })
    }

    /// Ends the recording: its reads, handed to the enclosing computation too.
    pub(crate) fn finish(self) -> Reads {
        let reads = take_top().finish();
        drop(self);
        replay(&reads);
        reads
    }
}

/// Runs `f` without letting its reads reach the enclosing computation: a
/// memo computation (the store records its reads) or a section's value.
pub(crate) fn suppress<R>(f: impl FnOnce() -> R) -> R {
    let _guard = push();
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.suppressed = true;
        }
    });
    f()
}

/// A per-build cache that replays the reads its initialization made to
/// every reader (see the module docs): `OnceLock`'s interface.
pub(crate) struct Recorded<T> {
    cell: OnceLock<(T, Arc<Reads>)>,
}

impl<T> Default for Recorded<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Recorded<T> {
    pub(crate) const fn new() -> Self {
        Self {
            cell: OnceLock::new(),
        }
    }

    pub(crate) fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        let (value, reads) = self.cell.get_or_init(|| {
            let guard = push();
            let value = f();
            let reads = take_top().finish();
            drop(guard);
            (value, Arc::new(reads))
        });
        replay_shared(reads);
        value
    }

    pub(crate) fn get(&self) -> Option<&T> {
        self.cell.get().map(|(value, reads)| {
            replay_shared(reads);
            value
        })
    }

    /// Sets a value computed elsewhere with the reads it made; `Err` with the
    /// value when the cache is already set.
    pub(crate) fn set(&self, value: T, reads: Reads) -> Result<(), T> {
        self.cell
            .set((value, Arc::new(reads)))
            .map_err(|(value, _)| value)
    }
}

/// What a chunk keeps of its reads: the reads, the memo store they were made
/// against, and each section's fingerprint when it ran.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReadProof {
    pub(crate) reads: Reads,
    store: u64,
    /// One per read section, in [`Section::ALL`] order.
    sections: Vec<[u8; 32]>,
}

/// Answers a recorded read in the current build.
pub(crate) trait ReadAnswers {
    /// The fingerprint of `section`'s value in this build.
    fn section_fingerprint(&self, section: Section) -> [u8; 32];
    /// The revision `read`'s answer last changed in, brought up to date in
    /// this build; `None` when the store no longer holds it, or is another
    /// store than `store`, the one the read was made against.
    fn memo_changed_at(&self, store: u64, read: MemoRead) -> Option<Revision>;
    /// The identity of the memo store this build reads.
    fn memo_store_id(&self) -> u64;
    /// [`Self::memo_changed_at`], telling apart a read this build cannot
    /// answer yet: bringing the memo up to date needs a derived table its
    /// pass has not published (`base::memo::derived::deferring`).
    fn memo_check(&self, store: u64, read: MemoRead) -> MemoCheck {
        if self.memo_changed_at(store, read) == Some(read.changed_at) {
            MemoCheck::Same
        } else {
            MemoCheck::Moved
        }
    }
    /// A memo read, named for a diagnostic.
    fn describe_memo(&self, read: MemoRead) -> String {
        format!("{read:?}")
    }
}

/// One recorded memo read checked in the current build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MemoCheck {
    Same,
    Moved,
    /// Its memo needs a derived table not yet published in this build.
    Undecided,
}

/// What a proof's check found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Verdict {
    Holds,
    Moved(String),
    /// Every read decided so far holds; these need a pass that has not run
    /// yet in this build.
    Undecided(Vec<MemoRead>),
}

/// `SOLID_CHECKER_IR_READ_MISSES`: report the first read that no longer
/// answers as recorded, for each chunk that reruns.
fn report_misses() -> bool {
    static REPORT: OnceLock<bool> = OnceLock::new();
    *REPORT.get_or_init(|| {
        std::env::var_os("SOLID_CHECKER_IR_READ_MISSES").is_some_and(|value| value != "0")
    })
}

impl ReadProof {
    /// The proof of `reads` against the build `answers` describes.
    pub(crate) fn new(reads: Reads, answers: &dyn ReadAnswers) -> Self {
        Self {
            store: answers.memo_store_id(),
            sections: reads
                .sections()
                .map(|section| answers.section_fingerprint(section))
                .collect(),
            reads,
        }
    }

    /// Reads gathered for a chunk still computing: never holds.
    pub(crate) fn pending(reads: Reads) -> Self {
        Self {
            reads,
            store: 0,
            sections: vec![[0; 32]; Section::ALL.len() + 1],
        }
    }

    /// Whether every read answers in this build as it did when recorded. A
    /// read this build cannot answer yet counts as moved: the chunk reruns.
    pub(crate) fn holds(&self, answers: &dyn ReadAnswers) -> bool {
        match self.verdict(answers) {
            Verdict::Holds => true,
            Verdict::Moved(miss) => {
                report_miss(&miss);
                false
            }
            Verdict::Undecided(reads) => {
                report_miss(&format!(
                    "memo {} (undecided before its pass publishes)",
                    answers.describe_memo(reads[0])
                ));
                false
            }
        }
    }

    /// Whether every read answers as recorded, moved (the first, named), or
    /// holds so far but for reads a pass not yet run must decide.
    pub(crate) fn verdict(&self, answers: &dyn ReadAnswers) -> Verdict {
        if self.sections.len() != self.reads.sections().count() {
            return Verdict::Moved("an unfinished chunk".to_owned());
        }
        if self.reads.is_empty() {
            return Verdict::Holds;
        }
        for (section, recorded) in self.reads.sections().zip(&self.sections) {
            if answers.section_fingerprint(section) != *recorded {
                return Verdict::Moved(format!("section {section:?}"));
            }
        }
        if !self.reads.has_memos() {
            return Verdict::Holds;
        }
        if answers.memo_store_id() != self.store {
            return Verdict::Moved("another memo store".to_owned());
        }
        let mut undecided = Vec::new();
        for read in self.reads.memo_reads() {
            match answers.memo_check(self.store, *read) {
                MemoCheck::Same => {}
                MemoCheck::Moved => {
                    return Verdict::Moved(format!("memo {}", answers.describe_memo(*read)));
                }
                MemoCheck::Undecided => undecided.push(*read),
            }
        }
        if undecided.is_empty() {
            Verdict::Holds
        } else {
            Verdict::Undecided(undecided)
        }
    }
}

fn report_miss(miss: &str) {
    if report_misses() {
        eprintln!("{}", serde_json::json!({ "irReadMiss": miss }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(query: u32) -> MemoRead {
        MemoRead {
            table: TableId(1),
            query: QueryId(query),
            changed_at: 1,
        }
    }

    #[test]
    fn a_recorded_cache_replays_its_reads_to_every_reader() {
        let cache = Recorded::<u32>::new();
        let ((), first) = record(|| {
            cache.get_or_init(|| {
                note(Section::Components);
                note_memo(read(3));
                7
            });
        });
        // The second reader finds the cache filled, and still records what
        // its initialization read.
        let ((), second) = record(|| {
            assert_eq!(*cache.get_or_init(|| unreachable!()), 7);
        });
        assert_eq!(first, second);
        assert!(second.reads_section(Section::Components));
        assert_eq!(second.memos(), &[read(3)]);
    }

    #[test]
    fn suppressed_reads_and_nested_records_stay_where_they_belong() {
        let ((), outer) = record(|| {
            suppress(|| note(Section::FlowAnswers));
            let ((), inner) = record(|| note(Section::MemberTargets));
            assert!(inner.reads_section(Section::MemberTargets));
            note_memo(read(2));
            note_memo(read(2));
        });
        assert!(!outer.reads_section(Section::FlowAnswers));
        // A nested record hands its reads to the enclosing one.
        assert!(outer.reads_section(Section::MemberTargets));
        assert_eq!(outer.memos(), &[read(2)]);
        // Outside every recording, a note goes nowhere.
        note(Section::Components);
        assert!(!recording());
    }

    fn recorded(queries: impl IntoIterator<Item = u32>) -> Reads {
        record(|| {
            for query in queries {
                note_memo(read(query));
            }
        })
        .1
    }

    /// ADR 0406: a set added to many others is shared, not
    /// copied; however many sets are added, the runs stay few and the set
    /// is exactly the union; equality ignores how the runs fall.
    #[test]
    fn extended_sets_share_their_runs_and_stay_exact() {
        let shared = recorded(0..500);
        let mut per_chunk = Vec::new();
        for chunk in 0..40 {
            let mut reads = recorded([1000 + chunk, 3]);
            reads.extend(&shared);
            assert!(
                reads
                    .runs
                    .iter()
                    .any(|run| Arc::ptr_eq(run, &shared.runs[0])),
                "the shared run is held, not copied"
            );
            per_chunk.push(reads);
        }
        let mut aggregate = Reads::default();
        for reads in &per_chunk {
            aggregate.extend(reads);
        }
        assert!(aggregate.runs.len() <= RUNS_MAX, "{}", aggregate.runs.len());
        let expected = (0..500).chain(1000..1040).map(read).collect::<Vec<_>>();
        assert_eq!(aggregate.memos(), expected);
        assert_eq!(aggregate, recorded((0..500).chain(1000..1040)));
        // A replayed set's runs reach the recording that replays it.
        let ((), replayed) = record(|| replay(&aggregate));
        assert_eq!(replayed, aggregate);
        // A long tail becomes a run.
        let mut tailed = Reads::default();
        for query in 0..(TAIL_MAX as u32 * 3) {
            tailed.extend(&Reads {
                sections: 0,
                runs: Vec::new(),
                tail: vec![read(query)],
            });
        }
        assert!(tailed.tail.len() <= TAIL_MAX);
        assert_eq!(tailed, recorded(0..(TAIL_MAX as u32 * 3)));
    }

    /// Answers over fixed memo revisions; `undecided` holds the queries
    /// whose derived table is not published yet.
    struct Fixed {
        revisions: std::collections::BTreeMap<u32, Revision>,
        undecided: std::cell::RefCell<BTreeSet<u32>>,
    }

    impl ReadAnswers for Fixed {
        fn section_fingerprint(&self, _: Section) -> [u8; 32] {
            [0; 32]
        }
        fn memo_changed_at(&self, _: u64, read: MemoRead) -> Option<Revision> {
            self.revisions.get(&read.query.0).copied()
        }
        fn memo_store_id(&self) -> u64 {
            1
        }
        fn memo_check(&self, store: u64, read: MemoRead) -> MemoCheck {
            if self.undecided.borrow().contains(&read.query.0) {
                return MemoCheck::Undecided;
            }
            if self.memo_changed_at(store, read) == Some(read.changed_at) {
                MemoCheck::Same
            } else {
                MemoCheck::Moved
            }
        }
    }

    /// A proof checked before a pass publishes what some of its memos read
    /// is undecided, not moved, while every decided read holds, and reruns
    /// its chunk. A decided read that moved wins over an undecided one.
    #[test]
    fn a_proof_is_undecided_until_its_pass_runs() {
        let ((), reads) = record(|| {
            note_memo(read(1));
            note_memo(read(2));
        });
        let mut answers = Fixed {
            revisions: [(1, 1), (2, 1)].into_iter().collect(),
            undecided: std::cell::RefCell::new([2].into_iter().collect()),
        };
        let proof = ReadProof::new(reads, &answers);
        let Verdict::Undecided(pending) = proof.verdict(&answers) else {
            panic!("undecided before the pass")
        };
        assert_eq!(pending, [read(2)]);
        assert!(!proof.holds(&answers), "an undecided read reruns the chunk");
        answers.revisions.insert(1, 7);
        assert!(matches!(proof.verdict(&answers), Verdict::Moved(_)));
    }
}
