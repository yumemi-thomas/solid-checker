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
//! **What is not recorded.** A memo computation's own reads are its memo's
//! (the store records them), so nothing inside a memo computation reaches a
//! chunk ([`suppress`]); a section's own computation likewise (its value is
//! the fingerprint). Reads of another file's facts outside the lookup's
//! tables are not recorded yet: a chunk that makes them keeps its stage's
//! file-level gate (`interproc::call_role_syntax`).
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
    /// The exported module-level constants proven primitive (ADR 0325).
    ExportedPrimitiveConsts,
}

impl Section {
    pub(crate) const ALL: [Self; 13] = [
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
        Self::ExportedPrimitiveConsts,
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Reads {
    sections: u32,
    /// Sorted, without duplicates.
    memos: Vec<MemoRead>,
}

impl Reads {
    pub(crate) fn is_empty(&self) -> bool {
        self.sections == 0 && self.memos.is_empty()
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

    #[cfg(test)]
    pub(crate) fn memos(&self) -> &[MemoRead] {
        &self.memos
    }

    /// Adds `other`'s reads.
    pub(crate) fn extend(&mut self, other: &Self) {
        self.sections |= other.sections;
        if other.memos.is_empty() {
            return;
        }
        self.memos.extend_from_slice(&other.memos);
        self.memos.sort_unstable();
        self.memos.dedup();
    }
}

/// One thread's recording: the innermost frame receives every note.
#[derive(Default)]
struct Frame {
    sections: u32,
    memos: Vec<MemoRead>,
    /// The replayed [`Recorded`] read sets, by address, so a cache read a
    /// thousand times is copied once.
    replayed: BTreeSet<usize>,
}

impl Frame {
    fn finish(mut self) -> Reads {
        self.memos.sort_unstable();
        self.memos.dedup();
        Reads {
            sections: self.sections,
            memos: self.memos,
        }
    }

    fn add(&mut self, reads: &Reads) {
        self.sections |= reads.sections;
        self.memos.extend_from_slice(&reads.memos);
    }
}

thread_local! {
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
}

/// Whether some computation on this thread is recording.
pub(crate) fn recording() -> bool {
    FRAMES.with(|frames| !frames.borrow().is_empty())
}

/// Records that the current computation read `section`.
pub(crate) fn note(section: Section) {
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.sections |= section.bit();
        }
    });
}

/// Records that the current computation read a memo answer.
pub(crate) fn note_memo(read: MemoRead) {
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.memos.push(read);
        }
    });
}

/// Adds `reads` to the current computation's, as if it had made them.
pub(crate) fn replay(reads: &Reads) {
    if reads.is_empty() {
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
    if reads.is_empty() {
        return;
    }
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            if reads.memos.is_empty() {
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
    }
}

fn push() -> FrameGuard {
    FRAMES.with(|frames| frames.borrow_mut().push(Frame::default()));
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
            memos.sort_unstable();
            memos.dedup();
            Reads {
                sections: frame.sections,
                memos,
            }
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
    /// A memo read, named for a diagnostic.
    fn describe_memo(&self, read: MemoRead) -> String {
        format!("{read:?}")
    }
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

    /// Whether every read answers in this build as it did when recorded.
    pub(crate) fn holds(&self, answers: &dyn ReadAnswers) -> bool {
        match self.first_miss(answers) {
            None => true,
            Some(miss) => {
                if report_misses() {
                    eprintln!("{}", serde_json::json!({ "irReadMiss": miss }));
                }
                false
            }
        }
    }

    /// The first read that no longer answers as recorded, named.
    pub(crate) fn first_miss(&self, answers: &dyn ReadAnswers) -> Option<String> {
        if self.sections.len() != self.reads.sections().count() {
            return Some("an unfinished chunk".to_owned());
        }
        if self.reads.is_empty() {
            return None;
        }
        for (section, recorded) in self.reads.sections().zip(&self.sections) {
            if answers.section_fingerprint(section) != *recorded {
                return Some(format!("section {section:?}"));
            }
        }
        if self.reads.memos.is_empty() {
            return None;
        }
        if answers.memo_store_id() != self.store {
            return Some("another memo store".to_owned());
        }
        self.reads
            .memos
            .iter()
            .find(|read| answers.memo_changed_at(self.store, **read) != Some(read.changed_at))
            .map(|read| format!("memo {}", answers.describe_memo(*read)))
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
}
