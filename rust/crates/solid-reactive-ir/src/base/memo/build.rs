//! The build's memo schema: what a memo computation may read of one Reactive
//! IR build, and the store a retained session keeps between builds.
//!
//! Inputs ([`Input`]):
//! - a section of one file ([`Section`]): its source (the AST, the text and
//!   everything else `FileFacts` derives from it), its compiler map, or its
//!   entity-symbol rows;
//! - the slots a file occupies in `ProjectFacts::files` (file order);
//! - one key of an L1 index the store keeps ([`IndexId`]);
//! - a build constant ([`Constant`]).
//!
//! [`MemoStore::begin`] finds the changed inputs exactly: each file's section
//! fingerprints are compared with the last build's, the store's L1 indexes
//! are updated with the changed files' contributions (their Δ is the exact
//! set of changed keys), and the constants are compared. Today every file is
//! fingerprinted per build; a TypeFacts delta naming the edited files
//! (study S0) replaces that scan.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use solid_facts::FileFacts;
use solid_facts::compiler::ExecutionMap;
use solid_facts::core::Span;

use super::super::file_table::{FileId, FileInterner, FileTable, FileView};
use super::super::index::BaseIndex;
use super::super::{Contribute, Views};
use super::{Ctx, Invalidation, Memo, Schema, Store, TableId, TableSet, TableStats, World};
use crate::{HashMap, SymbolId};

/// A file's part the memo layer fingerprints separately.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Section {
    /// The file's text and everything `FileFacts` derives from it: the AST,
    /// spans, the source itself.
    Source,
    /// The Solid compiler's execution map: a function of the source under
    /// the pinned compiler, fingerprinted by the source hash it records.
    Compiler,
    /// The file's entity-symbol rows (`EntitySymbols` for its path).
    Entities,
}

const SECTIONS: [Section; 3] = [Section::Source, Section::Compiler, Section::Entities];

/// The L1 indexes the memo store keeps between builds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum IndexId {
    /// Identity-wrapper candidates by declaration symbol
    /// ([`crate::indexes::identity_wrappers`]).
    WrapperCandidates,
}

/// A build constant a computation may read. Constants are part of the
/// cache domain too (a change there is a fresh build); recording them keeps
/// the store exact on its own.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Constant {
    ProgramClosed,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Input {
    File(FileId, Section),
    /// The slots of the file in `ProjectFacts::files`, in file order.
    Slots(FileId),
    /// One key of an index, by its interned id.
    Index(IndexId, u32),
    Constant(Constant),
}

/// The build's memo schema.
pub(crate) struct BuildSchema;

impl Schema for BuildSchema {
    type Input = Input;
    type Tables = BuildTables;
    type World<'w> = BuildWorld<'w>;
}

/// [`crate::indexes::IdentityWrapper`]'s table.
pub(crate) const IDENTITY_WRAPPERS: TableId = TableId(0);
/// [`crate::indexes::WrapperAt`]'s table.
pub(crate) const WRAPPER_AT: TableId = TableId(1);

/// The build's memo tables.
#[derive(Default)]
pub(crate) struct BuildTables {
    identity_wrappers: super::Table<BuildSchema, crate::indexes::IdentityWrapper>,
    wrapper_at: super::Table<BuildSchema, crate::indexes::WrapperAt>,
}

impl TableSet<BuildSchema> for BuildTables {
    fn table(&self, id: TableId) -> &dyn super::ErasedTable<BuildSchema> {
        match id {
            IDENTITY_WRAPPERS => &self.identity_wrappers,
            WRAPPER_AT => &self.wrapper_at,
            _ => unreachable!("memo table {id:?}"),
        }
    }

    fn tables(&self) -> Vec<&dyn super::ErasedTable<BuildSchema>> {
        vec![&self.identity_wrappers, &self.wrapper_at]
    }
}

impl BuildTables {
    pub(crate) fn identity_wrappers(
        &self,
    ) -> &super::Table<BuildSchema, crate::indexes::IdentityWrapper> {
        &self.identity_wrappers
    }

    pub(crate) fn wrapper_at(&self) -> &super::Table<BuildSchema, crate::indexes::WrapperAt> {
        &self.wrapper_at
    }
}

/// A file's section fingerprints and slots in one build.
#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFingerprints {
    source: [u8; 32],
    compiler: [u8; 32],
    entities: [u8; 32],
    slots: Vec<usize>,
}

impl FileFingerprints {
    fn section(&self, section: Section) -> &[u8; 32] {
        match section {
            Section::Source => &self.source,
            Section::Compiler => &self.compiler,
            Section::Entities => &self.entities,
        }
    }
}

fn low_bits(digest: &[u8; 32]) -> u64 {
    u64::from_le_bytes(digest[..8].try_into().expect("eight bytes"))
}

fn fingerprints(views: &Views<'_>, id: FileId) -> FileFingerprints {
    let slots = views.table.slots_of(id).to_vec();
    let mut source = Sha256::new();
    let mut compiler = Sha256::new();
    for slot in &slots {
        let file = &views.files[*slot];
        source.update(file.source_hash.as_str().as_bytes());
        source.update([0]);
        compiler.update(file.compiler.compiler_facts_protocol.to_le_bytes());
        compiler.update(file.compiler.source_hash.as_str().as_bytes());
        compiler.update([0]);
    }
    // Entity rows are per path: every slot of the id reads the same ones.
    let mut entities = Sha256::new();
    if let Some(slot) = slots.first() {
        for ((start, end), symbol, exact) in views.view(*slot).symbol_rows() {
            entities.update(start.to_le_bytes());
            entities.update(end.to_le_bytes());
            entities.update(
                u64::try_from(symbol.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            entities.update(symbol.as_bytes());
            entities.update([u8::from(exact)]);
        }
    }
    FileFingerprints {
        source: source.finalize().into(),
        compiler: compiler.finalize().into(),
        entities: entities.finalize().into(),
        slots,
    }
}

/// Interned keys of one index: an [`Input::Index`] names a key by id.
#[derive(Default)]
struct IndexKeys<K> {
    ids: HashMap<K, u32>,
    keys: HashMap<u32, K>,
    next: u32,
}

impl<K: Clone + Eq + std::hash::Hash> IndexKeys<K> {
    fn intern(&mut self, key: &K) -> u32 {
        if let Some(id) = self.ids.get(key) {
            return *id;
        }
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .expect("fewer than 2^32 index keys");
        self.ids.insert(key.clone(), id);
        self.keys.insert(id, key.clone());
        id
    }

    fn release(&mut self, id: u32) {
        if let Some(key) = self.keys.remove(&id) {
            self.ids.remove(&key);
        }
    }
}

/// An identity-wrapper candidate: the `ordinal`-th function of its file's
/// slot, bound to the indexed symbol.
pub(crate) type WrapperCandidate = u32;

/// What the store retains of the last build besides its memo tables.
#[derive(Default)]
struct MemoBase {
    interner: FileInterner,
    files: BTreeMap<FileId, FileFingerprints>,
    wrapper_candidates: Option<BaseIndex<Arc<str>, WrapperCandidate>>,
    wrapper_keys: IndexKeys<Arc<str>>,
    program_closed: Option<bool>,
}

/// What one [`MemoStore::begin`] found and cost.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BeginReport {
    pub(crate) fingerprint_ns: u64,
    pub(crate) index_ns: u64,
    pub(crate) mark_ns: u64,
    pub(crate) changed_files: usize,
    pub(crate) removed_files: usize,
    pub(crate) delta_keys: usize,
    pub(crate) changed_inputs: usize,
    pub(crate) dirty: usize,
    pub(crate) maybe_dirty: usize,
    /// The store was empty (first build, or emptied after a cycle or a
    /// mismatch).
    pub(crate) fresh: bool,
}

fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

/// The memo store a retained session keeps between builds, and a one-shot
/// build owns for itself.
pub(crate) struct MemoStore {
    store: Store<BuildSchema>,
    base: RwLock<MemoBase>,
    /// Shadow or verify-reused mode: the updated indexes are compared with a
    /// refold every build.
    check_indexes: bool,
    builds: AtomicU64,
}

fn report(message: &str) {
    let mode = match crate::shadow::ShadowMode::from_env() {
        crate::shadow::ShadowMode::Off => crate::shadow::ShadowMode::Serve,
        mode => mode,
    };
    crate::shadow::report_message(mode, message);
}

/// `SOLID_CHECKER_IR_VERIFY_REUSED`: unset, empty, `0`, `off` or `false`
/// is off; anything else recomputes every reused memo answer and compares.
pub(crate) fn verify_reused_from_env() -> bool {
    !matches!(
        std::env::var("SOLID_CHECKER_IR_VERIFY_REUSED")
            .ok()
            .as_deref()
            .map(str::trim),
        None | Some("" | "0" | "off" | "false")
    )
}

impl MemoStore {
    pub(crate) fn new(verify_reused: bool, check_indexes: bool) -> Self {
        Self {
            store: Store::new(verify_reused, report),
            base: RwLock::default(),
            check_indexes: check_indexes || verify_reused,
            builds: AtomicU64::new(0),
        }
    }

    /// A store configured from the environment (verify-reused and shadow
    /// modes).
    pub(crate) fn from_env() -> Self {
        Self::new(
            verify_reused_from_env(),
            crate::shadow::ShadowMode::from_env() != crate::shadow::ShadowMode::Off,
        )
    }

    /// A build's file table under this store's interner, so every id the
    /// store holds keeps its meaning.
    pub(crate) fn table(&self, table: &FileTable) -> FileTable {
        let interner = self.base.read().expect("memo base lock").interner.clone();
        table.reinterned(interner)
    }

    /// Starts a build over `views` (whose table is [`Self::table`]'s): finds
    /// the inputs that changed since the last build and marks their readers.
    pub(crate) fn begin(&self, views: &Views<'_>, program_closed: bool) -> BeginReport {
        let mut report = BeginReport::default();
        let fresh = self.store.poisoned() || self.builds.fetch_add(1, Ordering::AcqRel) == 0;
        let mut base = self.base.write().expect("memo base lock");
        if self.store.poisoned() {
            *base = MemoBase::default();
        }
        base.interner = views.table.interner().clone();
        let started = Instant::now();
        let ids = views.table.ids().to_vec();
        let current =
            crate::pipeline::parallel_slice_results(&ids, |id| (*id, fingerprints(views, *id)));
        report.fingerprint_ns = nanos(started.elapsed());
        let mut changed_inputs = Vec::new();
        let mut changed_files = Vec::new();
        for (id, next) in &current {
            let Some(previous) = base.files.get(id) else {
                changed_files.push(*id);
                continue;
            };
            if previous == next {
                continue;
            }
            changed_files.push(*id);
            for section in SECTIONS {
                if previous.section(section) != next.section(section) {
                    changed_inputs.push(Input::File(*id, section));
                }
            }
            if previous.slots != next.slots {
                changed_inputs.push(Input::Slots(*id));
            }
        }
        let current = current.into_iter().collect::<BTreeMap<_, _>>();
        let removed = base
            .files
            .keys()
            .copied()
            .filter(|id| !current.contains_key(id))
            .collect::<Vec<_>>();
        for id in &removed {
            changed_inputs.extend(SECTIONS.map(|section| Input::File(*id, section)));
            changed_inputs.push(Input::Slots(*id));
        }
        report.changed_files = changed_files.len();
        report.removed_files = removed.len();
        base.files = current;
        let started = Instant::now();
        let contribute: Contribute<Arc<str>, WrapperCandidate> =
            crate::indexes::identity_wrapper_candidates;
        match base.wrapper_candidates.as_mut() {
            Some(index) => {
                let delta = index.update(
                    crate::pipeline::parallel_slice_results(&changed_files, |id| {
                        (*id, Some(views.contribution(*id, contribute)))
                    })
                    .into_iter()
                    .chain(removed.iter().map(|id| (*id, None))),
                );
                report.delta_keys = delta.len();
                let mut delta = delta.into_iter().collect::<Vec<_>>();
                delta.sort_unstable();
                for key in delta {
                    if let Some(id) = base.wrapper_keys.ids.get(&key) {
                        changed_inputs.push(Input::Index(IndexId::WrapperCandidates, *id));
                    }
                }
            }
            None => base.wrapper_candidates = Some(views.fold(contribute)),
        }
        if self.check_indexes
            && let Some(index) = &base.wrapper_candidates
            && *index != views.fold(contribute)
        {
            self.store.poison(&format!(
                "IR memo mismatch: index `{:?}`: the update of {} changed and {} removed files differs from a refold",
                IndexId::WrapperCandidates,
                changed_files.len(),
                removed.len()
            ));
            base.wrapper_candidates = Some(views.fold(contribute));
        }
        report.index_ns = nanos(started.elapsed());
        if base
            .program_closed
            .is_some_and(|closed| closed != program_closed)
        {
            changed_inputs.push(Input::Constant(Constant::ProgramClosed));
        }
        base.program_closed = Some(program_closed);
        drop(base);
        let started = Instant::now();
        changed_inputs.sort_unstable();
        let Invalidation {
            changed_inputs: found,
            dirty,
            maybe_dirty,
        } = self.store.begin(&changed_inputs);
        report.mark_ns = nanos(started.elapsed());
        report.changed_inputs = found;
        report.dirty = dirty;
        report.maybe_dirty = maybe_dirty;
        report.fresh = fresh;
        report
    }

    /// Ends a build: sweeps entries this build did not reach and releases
    /// the index keys no entry reads any more.
    pub(crate) fn end(&self) -> Vec<(&'static str, TableStats)> {
        let (stats, released) = self.store.end();
        let mut base = self.base.write().expect("memo base lock");
        for input in released {
            if let Input::Index(IndexId::WrapperCandidates, id) = input {
                base.wrapper_keys.release(id);
            }
        }
        stats
    }

    /// Every key of the wrapper-candidate index, sorted: the keys a reader
    /// of the whole table (the cross-file digest) asks for.
    pub(crate) fn wrapper_candidate_keys(&self) -> Vec<Arc<str>> {
        let base = self.base.read().expect("memo base lock");
        let mut keys = base
            .wrapper_candidates
            .as_ref()
            .map(|index| index.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        keys.sort_unstable();
        keys
    }
}

/// One build as the memo computations see it.
pub(crate) struct BuildWorld<'w> {
    views: Views<'w>,
    store: &'w MemoStore,
    program_closed: bool,
}

impl<'w> BuildWorld<'w> {
    pub(crate) fn new(views: Views<'w>, store: &'w MemoStore, program_closed: bool) -> Self {
        Self {
            views,
            store,
            program_closed,
        }
    }

    pub(crate) fn views(&self) -> &Views<'w> {
        &self.views
    }

    /// The answer of `M` for `key`, to a reader outside every memo.
    pub(crate) fn answer<M: Memo<BuildSchema>>(&self, key: &M::Key) -> M::Value {
        self.store.store.answer::<M>(self, key)
    }
}

impl World<BuildSchema> for BuildWorld<'_> {
    fn fingerprint(&self, input: Input) -> u64 {
        let base = self.store.base.read().expect("memo base lock");
        match input {
            Input::File(id, section) => base
                .files
                .get(&id)
                .map_or(0, |file| low_bits(file.section(section))),
            Input::Slots(id) => {
                use std::hash::{Hash, Hasher};
                let mut hasher = rustc_hash::FxHasher::default();
                self.views.table.slots_of(id).hash(&mut hasher);
                hasher.finish()
            }
            Input::Index(IndexId::WrapperCandidates, key) => {
                use std::hash::{Hash, Hasher};
                let mut hasher = rustc_hash::FxHasher::default();
                if let (Some(index), Some(key)) =
                    (&base.wrapper_candidates, base.wrapper_keys.keys.get(&key))
                {
                    for (file, rank, candidate) in index.ranked_values(key) {
                        (file, rank, candidate).hash(&mut hasher);
                    }
                }
                hasher.finish()
            }
            Input::Constant(Constant::ProgramClosed) => u64::from(self.program_closed),
        }
    }
}

/// One slot of one file, read through a memo context: each accessor records
/// the section it reads.
pub(crate) struct FileRead<'r, 'c, 'w: 'c> {
    ctx: &'r Ctx<'c, 'w, BuildSchema>,
    id: FileId,
    view: FileView<'w>,
}

impl<'w> FileRead<'_, '_, 'w> {
    /// The file's facts. Everything in them is a function of its source
    /// (the compiler map included, under the pinned compiler).
    pub(crate) fn facts(&self) -> &'w FileFacts {
        self.ctx.record_input(Input::File(self.id, Section::Source));
        self.view.file
    }

    /// The compiler's execution map.
    #[allow(dead_code, reason = "the first table reading it ports in study S5")]
    pub(crate) fn compiler(&self) -> &'w ExecutionMap {
        self.ctx
            .record_input(Input::File(self.id, Section::Compiler));
        &self.view.file.compiler
    }

    /// The symbol whose name covers exactly `span` in this file.
    pub(crate) fn exact_symbol(&self, span: Span) -> Option<&'w SymbolId> {
        self.ctx
            .record_input(Input::File(self.id, Section::Entities));
        self.view.exact_symbol(span)
    }

    /// The symbol that is the value at `span` ([`FileView::value_symbol`]):
    /// reads the entity rows and the AST.
    pub(crate) fn value_symbol(&self, span: Span) -> Option<&'w SymbolId> {
        self.ctx
            .record_input(Input::File(self.id, Section::Entities));
        self.ctx.record_input(Input::File(self.id, Section::Source));
        self.view.value_symbol(span)
    }
}

impl<'c, 'w> Ctx<'c, 'w, BuildSchema> {
    /// The `rank`-th slot of the file `id`, if it has one (recorded as a
    /// read of the file's source: its slot count is part of it).
    pub(crate) fn file<'r>(&'r self, id: FileId, rank: usize) -> Option<FileRead<'r, 'c, 'w>> {
        self.record_input(Input::File(id, Section::Source));
        let world = self.world();
        let slot = *world.views.table.slots_of(id).get(rank)?;
        Some(FileRead {
            ctx: self,
            id,
            view: world.views.view(slot),
        })
    }

    /// The slots of `id` in file order: what a first-wins choice across
    /// files reads.
    pub(crate) fn slots(&self, id: FileId) -> Vec<usize> {
        self.record_input(Input::Slots(id));
        self.world().views.table.slots_of(id).to_vec()
    }

    /// The identity-wrapper candidates of `symbol`: `(file, slot rank,
    /// function ordinal)`, by file id then production order.
    pub(crate) fn wrapper_candidates(&self, symbol: &Arc<str>) -> Vec<(FileId, usize, u32)> {
        let store = self.world().store;
        let id = {
            let base = store.base.read().expect("memo base lock");
            base.wrapper_keys.ids.get(symbol).copied()
        };
        let id = id.unwrap_or_else(|| {
            store
                .base
                .write()
                .expect("memo base lock")
                .wrapper_keys
                .intern(symbol)
        });
        self.record_input(Input::Index(IndexId::WrapperCandidates, id));
        let base = store.base.read().expect("memo base lock");
        base.wrapper_candidates
            .as_ref()
            .map(|index| {
                index
                    .ranked_values(symbol)
                    .map(|(file, rank, ordinal)| (file, rank, *ordinal))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A build constant.
    #[allow(dead_code, reason = "the first table reading one ports in study S5")]
    pub(crate) fn constant(&self, constant: Constant) -> bool {
        self.record_input(Input::Constant(constant));
        match constant {
            Constant::ProgramClosed => self.world().program_closed,
        }
    }
}

/// The memo store one lookup reads: a retained session's, or its own.
enum StoreRef<'a> {
    Owned(Box<MemoStore>),
    Shared(&'a MemoStore),
}

/// What one build reported of its memos.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoReport {
    pub(crate) begin: BeginReport,
    pub(crate) tables: Vec<(&'static str, TableStats)>,
}

/// A lookup's memo state: the store, the build's file table over the
/// store's interner, and whether the build has begun (its invalidation runs
/// once, before the first read).
pub(crate) struct LookupMemos<'a> {
    store: StoreRef<'a>,
    table: FileTable,
    begun: std::sync::OnceLock<BeginReport>,
}

impl<'a> LookupMemos<'a> {
    /// A store of the lookup's own (a one-shot or fresh build), over the
    /// lookup's `table`.
    pub(crate) fn owned(table: &FileTable) -> Self {
        let store = Box::new(MemoStore::from_env());
        let table = store.table(table);
        Self {
            store: StoreRef::Owned(store),
            table,
            begun: std::sync::OnceLock::new(),
        }
    }

    /// A retained session's store, over the lookup's `table`.
    pub(crate) fn shared(store: &'a MemoStore, table: &FileTable) -> Self {
        Self {
            table: store.table(table),
            store: StoreRef::Shared(store),
            begun: std::sync::OnceLock::new(),
        }
    }

    pub(crate) fn store(&self) -> &MemoStore {
        match &self.store {
            StoreRef::Owned(store) => store,
            StoreRef::Shared(store) => store,
        }
    }

    pub(crate) fn table(&self) -> &FileTable {
        &self.table
    }

    /// Runs `begin` once, before the build's first memo read.
    pub(crate) fn begin_once(&self, begin: impl FnOnce() -> BeginReport) {
        self.begun.get_or_init(begin);
    }

    /// Ends the build if it read any memo: sweeps the store and returns what
    /// the build found.
    pub(crate) fn end(&self) -> Option<MemoReport> {
        let begin = *self.begun.get()?;
        Some(MemoReport {
            begin,
            tables: self.store().end(),
        })
    }
}
