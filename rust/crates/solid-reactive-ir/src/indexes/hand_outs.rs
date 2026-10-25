//! What the lookup hands a recording computation of other files
//! (ADR 0408).
//!
//! The execution-role machine, and every other reader of the lookup, reaches
//! another file through a few lookup questions: which function a symbol
//! names, which binding a reference resolves to, which file a path is, who
//! calls a function. It then reads that file's syntax, compiler map and
//! entity rows directly. A stage chunk records neither (`crate::base::reads`),
//! so a same-length edit there could leave a kept chunk stale.
//!
//! Each such question, asked while a chunk records, is now a recorded read:
//! - its answer, absent answers included: which function a symbol names as
//!   the answers of its bucket of symbols ([`FunctionSymbolBucket`]), a
//!   binding as every binding resolution of the asking file
//!   ([`BindingRowsOf`]), the rest as a memo over the probe that answers it
//!   ([`ProbeRead`]). Keyed coarser than one question, so a build holds a
//!   few thousand such entries, not a hundred thousand;
//! - every file the answer hands out, as a memo over that file's section
//!   fingerprints ([`FileFactsRead`]): the read moves on any change of the
//!   file's source, compiler map, entity rows or call signatures.
//!
//! Nothing is noted outside a recording, inside a memo computation (its own
//! reads are the store's) or while a probe is asked.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use super::*;
use crate::base::FileId;
use crate::base::memo::build::{BuildSchema, BuildTables, Input, Section};
use crate::base::memo::probes::{PolledKey, Probe, ProbeAnswer};
use crate::base::memo::{Canonical, Ctx, Cycle, ErasedTable, Memo, Table, TableId};
use crate::base::reads::MemoRead;

/// The hand-out tables hold ids 640 to 703.
pub(crate) const HAND_OUT_TABLES_FROM: u16 = 640;
pub(crate) const HAND_OUT_TABLES_END: u16 = 704;
/// Every hand-out table's id.
pub(crate) const HAND_OUT_TABLES: std::ops::Range<u16> = HAND_OUT_TABLES_FROM..HAND_OUT_TABLES_END;
/// [`FileFactsRead`]'s table.
const FILE_FACTS_READ: TableId = TableId(HAND_OUT_TABLES_FROM);

/// The hand-out memo tables, inside the build's.
#[derive(Default)]
pub(crate) struct HandOutTables {
    files: Table<BuildSchema, FileFactsRead>,
    probes: Table<BuildSchema, ProbeRead>,
    function_buckets: Table<BuildSchema, FunctionSymbolBucket>,
    binding_rows: Table<BuildSchema, BindingRowsOf>,
}

impl HandOutTables {
    pub(crate) fn table(&self, id: TableId) -> Option<&dyn ErasedTable<BuildSchema>> {
        match id {
            FileFactsRead::ID => Some(&self.files),
            ProbeRead::ID => Some(&self.probes),
            FunctionSymbolBucket::ID => Some(&self.function_buckets),
            BindingRowsOf::ID => Some(&self.binding_rows),
            _ => None,
        }
    }

    pub(crate) fn tables(&self) -> Vec<&dyn ErasedTable<BuildSchema>> {
        vec![
            &self.files,
            &self.probes,
            &self.function_buckets,
            &self.binding_rows,
        ]
    }
}

/// How many buckets the symbols that name a project function fall in.
const FUNCTION_BUCKETS: u16 = 1024;

/// The bucket of `symbol`: a function of its text alone, so the same in
/// every build.
fn function_bucket_of(symbol: &str) -> u16 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    symbol.hash(&mut hasher);
    u16::try_from(hasher.finish() % u64::from(FUNCTION_BUCKETS)).expect("a bucket below 2^16")
}

/// One bucket's answers of `function_for_symbol`: each symbol of the bucket
/// that names a function (or aborted, `None`), with the function's file,
/// slot rank and ordinal, sorted by symbol. A symbol outside it names none.
pub(super) type FunctionBucket = Arc<Vec<(Arc<str>, Option<(FileId, u32, u32)>)>>;

/// [`FunctionBucket`] of one bucket: what a reader of `function_for_symbol`
/// records. The answers are the lookup's whole-program table
/// ([`Ctx::project`]): every build recomputes the buckets read, and a
/// reader reruns only when its bucket's answers moved.
pub(crate) struct FunctionSymbolBucket;

impl Memo<BuildSchema> for FunctionSymbolBucket {
    type Key = u16;
    type Value = FunctionBucket;
    const NAME: &'static str = "hand_out_function_bucket";
    const ID: TableId = TableId(HAND_OUT_TABLES_FROM + 2);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        &tables.hand_outs().function_buckets
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, bucket: &u16) -> Result<FunctionBucket, Cycle> {
        Ok(ctx.project(|lookup| lookup.function_bucket(*bucket)))
    }

    fn on_cycle(_: &u16) -> FunctionBucket {
        Arc::default()
    }
}

/// Every binding resolution of one file's references
/// ([`SemanticLookup::binding_resolution_rows`]): what a reader of
/// `binding_at_reference` records for the file it asks about.
pub(crate) struct BindingRowsOf;

impl Memo<BuildSchema> for BindingRowsOf {
    type Key = FileId;
    type Value = Arc<super::BindingResolutionRows>;
    const NAME: &'static str = "hand_out_binding_rows";
    const ID: TableId = TableId(HAND_OUT_TABLES_FROM + 3);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        &tables.hand_outs().binding_rows
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        id: &FileId,
    ) -> Result<Arc<super::BindingResolutionRows>, Cycle> {
        let path = ctx.path_of(*id);
        Ok(ctx.project(|lookup| Arc::new(lookup.binding_resolution_rows(path))))
    }

    fn on_cycle(_: &FileId) -> Arc<super::BindingResolutionRows> {
        Arc::default()
    }
}

impl Canonical for Probe {}
impl Canonical for PolledKey {}
impl Canonical for ProbeAnswer {}

/// A file's section fingerprints (source, compiler map, entity rows, call
/// signatures): what a reader that was handed the file records.
pub(crate) struct FileFactsRead;

impl Memo<BuildSchema> for FileFactsRead {
    type Key = FileId;
    type Value = (u64, u64, u64, u64);
    const NAME: &'static str = "file_facts_read";
    const ID: TableId = FILE_FACTS_READ;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        &tables.hand_outs().files
    }

    fn compute(ctx: &Ctx<'_, '_, BuildSchema>, id: &FileId) -> Result<(u64, u64, u64, u64), Cycle> {
        Ok(ctx.file_fingerprints(*id))
    }

    fn on_cycle(_: &FileId) -> (u64, u64, u64, u64) {
        (0, 0, 0, 0)
    }

    fn implied(id: &FileId) -> Vec<Input> {
        vec![Input::File(*id, Section::Source)]
    }
}

/// A lookup question's answer, as its probe gives it.
pub(crate) struct ProbeRead;

impl Memo<BuildSchema> for ProbeRead {
    type Key = (Probe, PolledKey);
    type Value = ProbeAnswer;
    const NAME: &'static str = "hand_out_probe";
    const ID: TableId = TableId(HAND_OUT_TABLES_FROM + 1);

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        &tables.hand_outs().probes
    }

    fn compute(
        ctx: &Ctx<'_, '_, BuildSchema>,
        (probe, key): &(Probe, PolledKey),
    ) -> Result<ProbeAnswer, Cycle> {
        Ok(ctx.probe(*probe, key.clone()))
    }

    fn on_cycle(_: &(Probe, PolledKey)) -> ProbeAnswer {
        ProbeAnswer::Flag(false)
    }
}

/// This build's reads of the per-file and per-bucket hand-out memos, each
/// asked of the store once: a memo's answer keeps the revision it last
/// changed in for the whole build.
#[derive(Default)]
pub(crate) struct HandOutReads {
    files: OnceLock<Vec<OnceLock<MemoRead>>>,
    rows: OnceLock<Vec<OnceLock<MemoRead>>>,
    buckets: OnceLock<Vec<OnceLock<MemoRead>>>,
    component_spans: OnceLock<Vec<OnceLock<MemoRead>>>,
    member_targets: OnceLock<Vec<OnceLock<MemoRead>>>,
    literal_receivers: OnceLock<Vec<OnceLock<MemoRead>>>,
    never_returning: OnceLock<Vec<OnceLock<MemoRead>>>,
}

/// The cached read of slot `index` of `cells` (sized `len` on first use),
/// asking `read` the first time; a cycle's answer is noted, never kept.
fn cached_read(
    cells: &OnceLock<Vec<OnceLock<MemoRead>>>,
    len: usize,
    index: usize,
    read: impl FnOnce() -> Option<MemoRead>,
) {
    let cells = cells.get_or_init(|| (0..len).map(|_| OnceLock::new()).collect());
    let Some(cell) = cells.get(index) else {
        if let Some(read) = read() {
            crate::base::reads::note_memo(read);
        }
        return;
    };
    if let Some(read) = cell.get() {
        crate::base::reads::note_memo(*read);
        return;
    }
    match read() {
        Some(read) => {
            let _ = cell.set(read);
            crate::base::reads::note_memo(read);
        }
        None => crate::base::reads::note_memo(MemoRead {
            table: TableId(HAND_OUT_TABLES_FROM),
            query: crate::base::memo::QueryId(u32::MAX),
            changed_at: crate::base::memo::Revision::MAX,
        }),
    }
}

/// What a frame already noted, so a hand-out asked a thousand times in one
/// chunk asks its memo once: the frame's serial and its notes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Noted {
    File(FileId),
    Probe(Probe, PolledKey),
    FunctionBucket(u16),
    BindingRows(FileId),
    ComponentSpans(FileId),
    MemberTargets(FileId),
    LiteralReceivers(FileId),
    NeverReturning(FileId),
    NamespaceHandOuts(FileId),
}

thread_local! {
    static NOTED: RefCell<(u64, HashSet<Noted>)> = RefCell::new((0, HashSet::default()));
}

/// Whether `note` is new to the frame `serial` (and remembers it).
fn first_in_frame(serial: u64, note: Noted) -> bool {
    NOTED.with(|noted| {
        let mut noted = noted.borrow_mut();
        if noted.0 != serial {
            noted.0 = serial;
            noted.1.clear();
        }
        noted.1.insert(note)
    })
}

impl<'a> SemanticLookup<'a> {
    /// The frame a hand-out made now is recorded in: some chunk records on
    /// this thread, not inside a memo computation or a probe.
    fn hand_out_frame(&self) -> Option<u64> {
        if probing() {
            return None;
        }
        crate::base::reads::recording_frame()
    }

    /// Records, for the computation recording, that it was handed `file`:
    /// it may read any of the file's facts.
    pub(crate) fn note_file_read(&self, file: &FileFacts) {
        let Some(frame) = self.hand_out_frame() else {
            return;
        };
        if let Some(slot) = self.file_table.last_slot(file.path.as_str()) {
            self.note_slot_in(frame, slot);
        }
    }

    /// [`Self::note_file_read`] of the file in slot `slot`.
    pub(crate) fn note_slot_read(&self, slot: usize) {
        let Some(frame) = self.hand_out_frame() else {
            return;
        };
        self.note_slot_in(frame, slot);
    }

    /// Notes the file in slot `slot` (its every slot: the memo is the id's).
    fn note_slot_in(&self, frame: u64, slot: usize) {
        let (id, _) = self.memo_slot_of(slot);
        if first_in_frame(frame, Noted::File(id)) {
            cached_read(
                &self.hand_out_reads.files,
                self.file_table.slot_count(),
                slot,
                || self.memo_world().memo_read::<FileFactsRead>(&id),
            );
        }
    }

    /// Records, for the computation recording, the answer of `probe` at
    /// `key`: the lookup question it asked.
    pub(crate) fn note_probe_read(&self, probe: Probe, key: impl FnOnce() -> PolledKey) {
        let Some(frame) = self.hand_out_frame() else {
            return;
        };
        let key = key();
        if first_in_frame(frame, Noted::Probe(probe, key.clone())) {
            let _ = self.memo_world().answer::<ProbeRead>(&(probe, key));
        }
    }

    /// Records, for the computation recording, which function `symbol`
    /// names, as its bucket's answers ([`FunctionSymbolBucket`]).
    pub(crate) fn note_function_for_symbol(&self, symbol: &str) {
        let Some(frame) = self.hand_out_frame() else {
            return;
        };
        let bucket = function_bucket_of(symbol);
        if first_in_frame(frame, Noted::FunctionBucket(bucket)) {
            cached_read(
                &self.hand_out_reads.buckets,
                usize::from(FUNCTION_BUCKETS),
                usize::from(bucket),
                || self.memo_world().memo_read::<FunctionSymbolBucket>(&bucket),
            );
        }
    }

    /// Records, for the computation recording, which binding a reference of
    /// the file at `path` resolves to, as the file's rows ([`BindingRowsOf`]).
    pub(crate) fn note_binding_at(&self, path: &str) {
        let Some(frame) = self.hand_out_frame() else {
            return;
        };
        if let Some(slot) = self.file_table.last_slot(path) {
            let (id, _) = self.memo_slot_of(slot);
            if first_in_frame(frame, Noted::BindingRows(id)) {
                cached_read(
                    &self.hand_out_reads.rows,
                    self.file_table.slot_count(),
                    slot,
                    || self.memo_world().memo_read::<BindingRowsOf>(&id),
                );
            }
        }
    }

    /// Records, for the computation recording, the component identities of
    /// the functions of the file at `path` ([`super::census::ComponentSpansOf`]):
    /// what a question about one of them reads of the census. `false` when no
    /// such read stands for it (outside a recording, or a path the build does
    /// not hold), so the reader records the whole census instead.
    pub(crate) fn note_component_spans(&self, path: &str) -> bool {
        if probing() {
            return false;
        }
        let Some(frame) = crate::base::reads::recording_frame() else {
            return true;
        };
        let Some(slot) = self.file_table.last_slot(path) else {
            return false;
        };
        let (id, _) = self.memo_slot_of(slot);
        if first_in_frame(frame, Noted::ComponentSpans(id)) {
            cached_read(
                &self.hand_out_reads.component_spans,
                self.file_table.slot_count(),
                slot,
                || {
                    self.memo_world()
                        .memo_read::<super::census::ComponentSpansOf>(&id)
                },
            );
        }
        true
    }

    /// Records, for the computation recording, the exact object-member
    /// targets of every slot of the file at `path`
    /// ([`crate::object_members::ObjectMemberTargets`]): what a question
    /// about one call site of that file reads of the whole map. `false` when
    /// no such read stands for it, as [`Self::note_component_spans`].
    pub(crate) fn note_member_targets_of(&self, path: &str) -> bool {
        self.note_slot_memos::<crate::object_members::ObjectMemberTargets>(
            path,
            Noted::MemberTargets,
            &self.hand_out_reads.member_targets,
        )
    }

    /// [`Self::note_member_targets_of`] for the literal-receiver census
    /// ([`crate::object_members::LiteralReceivers`]).
    pub(crate) fn note_literal_receivers_of(&self, path: &str) -> bool {
        self.note_slot_memos::<crate::object_members::LiteralReceivers>(
            path,
            Noted::LiteralReceivers,
            &self.hand_out_reads.literal_receivers,
        )
    }

    /// Records, for the computation recording, the `never`-returning calls
    /// of the file at `path` ([`super::project_memos::NeverReturning`]):
    /// what a question about that file reads of the whole table. `false`
    /// when no such read stands for it, as [`Self::note_component_spans`].
    pub(crate) fn note_never_returning_of(&self, path: &str) -> bool {
        if probing() {
            return false;
        }
        let Some(frame) = crate::base::reads::recording_frame() else {
            return true;
        };
        let Some(slot) = self.file_table.last_slot(path) else {
            return false;
        };
        let (id, _) = self.memo_slot_of(slot);
        if first_in_frame(frame, Noted::NeverReturning(id)) {
            cached_read(
                &self.hand_out_reads.never_returning,
                self.file_table.slot_count(),
                slot,
                || {
                    self.memo_world()
                        .memo_read::<super::project_memos::NeverReturning>(&id)
                },
            );
        }
        true
    }

    /// Notes `M`'s answer of every slot of the file at `path`: the entries
    /// a whole-program map assembled from those answers holds for the path
    /// (a later slot's entry replaces an earlier one's, so each may decide).
    fn note_slot_memos<M>(
        &self,
        path: &str,
        noted: fn(FileId) -> Noted,
        cells: &OnceLock<Vec<OnceLock<MemoRead>>>,
    ) -> bool
    where
        M: Memo<BuildSchema, Key = (FileId, u32)>,
    {
        if probing() {
            return false;
        }
        let Some(frame) = crate::base::reads::recording_frame() else {
            return true;
        };
        let Some(slot) = self.file_table.last_slot(path) else {
            return false;
        };
        let (id, _) = self.memo_slot_of(slot);
        if first_in_frame(frame, noted(id)) {
            for &slot in self.memos.table().slots_of(id) {
                cached_read(cells, self.file_table.slot_count(), slot, || {
                    self.memo_world().memo_read::<M>(&self.memo_slot_of(slot))
                });
            }
        }
        true
    }

    /// Records, for the computation recording, the closed program's
    /// namespace census of the module at `path` ([`super::census::HandOuts`]:
    /// whether it escapes unnamed, which functions it hands out by name):
    /// what `hands_out_by_name` and `escapes_unnamed` read of every file's
    /// namespace uses.
    pub(crate) fn note_namespace_hand_outs_of(&self, path: &str) {
        let Some(frame) = self.hand_out_frame() else {
            return;
        };
        let Some(id) = self.memos.table().id_of(path) else {
            return;
        };
        if first_in_frame(frame, Noted::NamespaceHandOuts(id)) {
            let _ = self.memo_world().answer::<super::census::HandOuts>(&id);
        }
    }

    /// Records, for the computation recording, `symbol`'s declarations and
    /// every analyzed file that holds one: what a question about the
    /// bindings that name the symbol reads, as a binding naming a symbol is
    /// one of its declarations.
    pub(crate) fn note_symbol_declarations(&self, symbol: &str) {
        if self.hand_out_frame().is_none() {
            return;
        }
        self.note_probe_read(Probe::SymbolInfo, || Self::symbol_key(symbol));
        for declaration in self.symbol_declarations(symbol).into_iter().flatten() {
            if let Some(slot) = self
                .file_table
                .last_slot(declaration.location.path.as_ref())
            {
                self.note_slot_read(slot);
            }
        }
    }

    /// [`FunctionBucket`] of `bucket`, from the whole-program table; every
    /// bucket is sorted out of it once per build.
    fn function_bucket(&self, bucket: u16) -> FunctionBucket {
        let buckets = self.function_buckets.get_or_init(|| {
            let mut buckets = vec![Vec::new(); usize::from(FUNCTION_BUCKETS)];
            for (symbol, answer) in self.functions_by_symbol() {
                let answer = match answer {
                    SymbolFunction::Resolved { file, function } => {
                        let (id, rank) = self.memo_slot_of(*file);
                        Some((
                            id,
                            rank,
                            u32::try_from(*function).expect("fewer than 2^32 functions"),
                        ))
                    }
                    SymbolFunction::Aborted => None,
                };
                buckets[usize::from(function_bucket_of(symbol))]
                    .push((Arc::<str>::from(*symbol), answer));
            }
            buckets
                .into_iter()
                .map(|mut answers| {
                    answers.sort_unstable();
                    Arc::new(answers)
                })
                .collect()
        });
        Arc::clone(&buckets[usize::from(bucket)])
    }

    /// The probe key of a symbol's text.
    pub(crate) fn symbol_key(symbol: &str) -> PolledKey {
        PolledKey::Text(Arc::from(symbol))
    }
}
