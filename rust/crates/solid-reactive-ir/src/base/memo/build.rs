//! The build's memo schema: what a memo computation may read of one Reactive
//! IR build, and the store a retained session keeps between builds.
//!
//! Inputs ([`Input`]):
//! - a section of one file ([`Section`]): its source (the AST, the text and
//!   everything else `FileFacts` derives from it), its compiler map, or its
//!   entity-symbol rows;
//! - the slots a file occupies in `ProjectFacts::files` (file order);
//! - one key of an L1 index the store keeps ([`IndexId`]);
//! - a build constant ([`Constant`]);
//! - the analyzed ids in order ([`Input::Files`]).
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
    /// The file's TypeFacts rows: its entity-symbol rows (`EntitySymbols`
    /// for its path), the declaration symbol of each of its entities'
    /// resolved calls (a callee's symbol), and the references located in it.
    Entities,
    /// What the file's TypeFacts entities say about calls: for each entity of
    /// its path, in table order, its location and whether it carries a
    /// resolved call that never returns or asserts. A resolved call is read
    /// through `FileRead::resolved_callee_call`.
    Signatures,
}

const SECTIONS: [Section; 4] = [
    Section::Source,
    Section::Compiler,
    Section::Entities,
    Section::Signatures,
];

/// The L1 indexes the memo store keeps between builds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum IndexId {
    /// Identity-wrapper candidates by declaration symbol
    /// ([`crate::indexes::identity_wrappers`]).
    WrapperCandidates,
    /// TypeFacts references by symbol ([`super::super::contributions::references`]).
    References,
    /// Calls and JSX elements by the symbol their callee or tag names
    /// ([`super::super::contributions::callees`]).
    Callees,
}

/// A build constant a computation may read. Constants are part of the
/// cache domain too (a change there is a fresh build); recording them keeps
/// the store exact on its own.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Constant {
    ProgramClosed,
    /// Whether the build carries runtime symbol redirects (dependency-source
    /// analysis), under which a callee's symbol can be the redirected export.
    RuntimeRedirects,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Input {
    File(FileId, Section),
    /// The answer of an unported project table or a base index at a key
    /// ([`super::probes`]), by interned key: re-asked at every build.
    Polled(super::probes::Probe, u32),
    /// The slots of the file in `ProjectFacts::files`, in file order.
    Slots(FileId),
    /// One key of an index, by its interned id.
    Index(IndexId, u32),
    Constant(Constant),
    /// The analyzed ids in the order of their first slot: changed when a
    /// file is added, removed or reordered.
    Files,
    /// The whole analyzed program: every file's every section and slots, the
    /// TypeScript table's change flag and the build constants. A coarse read
    /// for a computation that goes through lookup tables not yet ported onto
    /// memos ([`Ctx::project`]): exact, because it changes whenever any
    /// input of any of those tables can, and it reuses only across builds in
    /// which nothing changed.
    Project,
}

/// Runs a closure over the build's `SemanticLookup`, whose lifetime a memo
/// computation cannot name. Implemented by the lookup; used by
/// [`Ctx::project`].
pub(crate) trait ProjectAccess: Sync {
    fn run(&self, f: &mut dyn for<'x> FnMut(&crate::indexes::SemanticLookup<'x>));
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

/// [`crate::object_members::ObjectMemberTargets`]'s table.
pub(crate) const OBJECT_MEMBER_TARGETS: TableId = TableId(20);
/// [`crate::object_members::SpreadTargetsAt`]'s table.
pub(crate) const SPREAD_TARGETS_AT: TableId = TableId(21);
/// [`crate::object_members::LiteralReceivers`]'s table.
pub(crate) const LITERAL_RECEIVERS: TableId = TableId(22);
/// [`crate::object_members::ReceiverClosed`]'s table.
pub(crate) const RECEIVER_CLOSED: TableId = TableId(23);
/// [`crate::object_members::AssignedMembersAnswer`]'s table.
pub(crate) const ASSIGNED_MEMBERS: TableId = TableId(24);
/// [`crate::object_members::PrototypeOwnerAt`]'s table.
pub(crate) const PROTOTYPE_OWNER_AT: TableId = TableId(25);

/// The build's memo tables.
#[derive(Default)]
pub(crate) struct BuildTables {
    identity_wrappers: super::Table<BuildSchema, crate::indexes::IdentityWrapper>,
    wrapper_at: super::Table<BuildSchema, crate::indexes::WrapperAt>,
    object_member_targets: super::Table<BuildSchema, crate::object_members::ObjectMemberTargets>,
    spread_targets_at: super::Table<BuildSchema, crate::object_members::SpreadTargetsAt>,
    literal_receivers: super::Table<BuildSchema, crate::object_members::LiteralReceivers>,
    receiver_closed: super::Table<BuildSchema, crate::object_members::ReceiverClosed>,
    assigned_members: super::Table<BuildSchema, crate::object_members::AssignedMembersAnswer>,
    prototype_owner_at: super::Table<BuildSchema, crate::object_members::PrototypeOwnerAt>,
    /// Tables 32 to 63: the project-wide answers (study S5d).
    project: crate::indexes::ProjectTables,
    /// Tables 64 and up: the component census (study S5c).
    census: crate::indexes::CensusTables,
    /// Tables 96 to 127: the invocation-flow answers (study S5a).
    flow: crate::execution_role::FlowTables,
}

impl TableSet<BuildSchema> for BuildTables {
    fn table(&self, id: TableId) -> &dyn super::ErasedTable<BuildSchema> {
        let table = self.table_by_id(id);
        debug_assert_eq!(
            table.id(),
            id,
            "memo table {} answers for another id",
            table.name()
        );
        table
    }

    fn tables(&self) -> Vec<&dyn super::ErasedTable<BuildSchema>> {
        self.all_tables()
    }
}

impl BuildTables {
    fn table_by_id(&self, id: TableId) -> &dyn super::ErasedTable<BuildSchema> {
        match id {
            IDENTITY_WRAPPERS => &self.identity_wrappers,
            WRAPPER_AT => &self.wrapper_at,
            OBJECT_MEMBER_TARGETS => &self.object_member_targets,
            SPREAD_TARGETS_AT => &self.spread_targets_at,
            LITERAL_RECEIVERS => &self.literal_receivers,
            RECEIVER_CLOSED => &self.receiver_closed,
            ASSIGNED_MEMBERS => &self.assigned_members,
            PROTOTYPE_OWNER_AT => &self.prototype_owner_at,
            id if (crate::execution_role::FLOW_TABLES_FROM
                ..crate::execution_role::FLOW_TABLES_END)
                .contains(&id.0) =>
            {
                self.flow
                    .table(id)
                    .unwrap_or_else(|| unreachable!("memo table {id:?}"))
            }
            id if id.0 >= crate::indexes::CENSUS_TABLES_FROM => self
                .census
                .table(id)
                .unwrap_or_else(|| unreachable!("memo table {id:?}")),
            id if id.0 >= crate::indexes::PROJECT_TABLES_FROM => self.project.table(id),
            _ => unreachable!("memo table {id:?}"),
        }
    }

    fn all_tables(&self) -> Vec<&dyn super::ErasedTable<BuildSchema>> {
        let mut tables: Vec<&dyn super::ErasedTable<BuildSchema>> = vec![
            &self.identity_wrappers,
            &self.wrapper_at,
            &self.object_member_targets,
            &self.spread_targets_at,
            &self.literal_receivers,
            &self.receiver_closed,
            &self.assigned_members,
            &self.prototype_owner_at,
        ];
        tables.extend(self.project.tables());
        tables.extend(self.census.tables());
        tables.extend(self.flow.tables());
        tables
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

    pub(crate) fn object_member_targets(
        &self,
    ) -> &super::Table<BuildSchema, crate::object_members::ObjectMemberTargets> {
        &self.object_member_targets
    }

    pub(crate) fn spread_targets_at(
        &self,
    ) -> &super::Table<BuildSchema, crate::object_members::SpreadTargetsAt> {
        &self.spread_targets_at
    }

    pub(crate) fn literal_receivers(
        &self,
    ) -> &super::Table<BuildSchema, crate::object_members::LiteralReceivers> {
        &self.literal_receivers
    }

    pub(crate) fn receiver_closed(
        &self,
    ) -> &super::Table<BuildSchema, crate::object_members::ReceiverClosed> {
        &self.receiver_closed
    }

    pub(crate) fn assigned_members(
        &self,
    ) -> &super::Table<BuildSchema, crate::object_members::AssignedMembersAnswer> {
        &self.assigned_members
    }

    pub(crate) fn prototype_owner_at(
        &self,
    ) -> &super::Table<BuildSchema, crate::object_members::PrototypeOwnerAt> {
        &self.prototype_owner_at
    }

    pub(crate) fn project(&self) -> &crate::indexes::ProjectTables {
        &self.project
    }

    pub(crate) fn census(&self) -> &crate::indexes::CensusTables {
        &self.census
    }

    pub(crate) fn flow(&self) -> &crate::execution_role::FlowTables {
        &self.flow
    }
}

/// A file's section fingerprints and slots in one build.
#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFingerprints {
    source: [u8; 32],
    compiler: [u8; 32],
    entities: [u8; 32],
    signatures: [u8; 32],
    slots: Vec<usize>,
}

impl FileFingerprints {
    fn section(&self, section: Section) -> &[u8; 32] {
        match section {
            Section::Source => &self.source,
            Section::Compiler => &self.compiler,
            Section::Entities => &self.entities,
            Section::Signatures => &self.signatures,
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
        let view = views.view(*slot);
        // The declaration each resolved call names (a callee's symbol), and
        // the references located here (the reference index's entries).
        for entity in view.typescript_entities() {
            let declaration = entity.resolved_call.as_deref().map(|call| {
                call.declaration
                    .as_ref()
                    .map(|declaration| &*declaration.symbol)
            });
            entities.update(entity.location.start_byte.to_le_bytes());
            entities.update(entity.location.end_byte.to_le_bytes());
            match declaration {
                None => entities.update([0]),
                Some(None) => entities.update([1]),
                Some(Some(symbol)) => {
                    entities.update([2]);
                    entities.update(
                        u64::try_from(symbol.len())
                            .unwrap_or(u64::MAX)
                            .to_le_bytes(),
                    );
                    entities.update(symbol.as_bytes());
                }
            }
        }
        entities.update([0xff]);
        for (symbol, span) in view.references() {
            entities.update(
                u64::try_from(symbol.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            entities.update(symbol.as_bytes());
            entities.update(span.start.to_le_bytes());
            entities.update(span.end.to_le_bytes());
        }
    }
    let mut signatures = Sha256::new();
    if let Some(slot) = slots.first() {
        for (start, end, call) in views.view(*slot).call_signature_rows() {
            signatures.update(start.to_le_bytes());
            signatures.update(end.to_le_bytes());
            signatures.update([call]);
        }
    }
    FileFingerprints {
        source: source.finalize().into(),
        compiler: compiler.finalize().into(),
        entities: entities.finalize().into(),
        signatures: signatures.finalize().into(),
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

/// An L1 index the store keeps between builds, with its interned keys.
struct KeptIndex<K, C> {
    index: Option<BaseIndex<K, C>>,
    keys: IndexKeys<K>,
}

impl<K, C> Default for KeptIndex<K, C> {
    fn default() -> Self {
        Self {
            index: None,
            keys: IndexKeys {
                ids: HashMap::default(),
                keys: HashMap::default(),
                next: 0,
            },
        }
    }
}

impl<K, C> KeptIndex<K, C>
where
    K: Clone + Eq + std::hash::Hash + Send + std::fmt::Debug,
    C: Clone + PartialEq + Send + std::hash::Hash,
{
    /// Brings the index to `views` from the last build's: the changed and
    /// removed files' contributions are replaced, and each changed key some
    /// entry reads is pushed onto `changed`. A store with no index folds one.
    /// With `check`, the update is compared with a refold; a difference is
    /// reported (the store is poisoned) and the refold kept.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        id: IndexId,
        views: &Views<'_>,
        contribute: Contribute<K, C>,
        changed_files: &[FileId],
        removed: &[FileId],
        check: bool,
        store: &Store<BuildSchema>,
        changed: &mut Vec<Input>,
    ) -> usize {
        let mut count = 0;
        match self.index.as_mut() {
            Some(index) => {
                let delta = index.update(
                    crate::pipeline::parallel_slice_results(changed_files, |file| {
                        (*file, Some(views.contribution(*file, contribute)))
                    })
                    .into_iter()
                    .chain(removed.iter().map(|file| (*file, None))),
                );
                count = delta.len();
                for key in delta {
                    if let Some(key) = self.keys.ids.get(&key) {
                        changed.push(Input::Index(id, *key));
                    }
                }
            }
            None => self.index = Some(views.fold(contribute)),
        }
        if check && let Some(index) = &self.index {
            let fold = views.fold(contribute);
            if *index != fold {
                store.poison(&format!(
                    "IR memo mismatch: index `{id:?}`: the update of {} changed and {} removed files differs from a refold",
                    changed_files.len(),
                    removed.len()
                ));
                self.index = Some(fold);
            }
        }
        count
    }

    fn fingerprint(&self, key: u32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        if let (Some(index), Some(key)) = (&self.index, self.keys.keys.get(&key)) {
            for (file, rank, value) in index.ranked_values(key) {
                (file, rank, value).hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn values(&self, key: &K) -> Vec<(FileId, usize, C)> {
        self.index
            .as_ref()
            .map(|index| {
                index
                    .ranked_values(key)
                    .map(|(file, rank, value)| (file, rank, value.clone()))
                    .collect()
            })
            .unwrap_or_default()
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
    /// The component census's indexes ([`crate::indexes::CallSitesOf`]).
    references: KeptIndex<Arc<str>, Span>,
    callees: KeptIndex<Arc<str>, crate::base::CalleeSite>,
    runtime_redirects: Option<bool>,
    /// The analyzed ids in the order of their first slot ([`Input::Files`]).
    ids: Vec<FileId>,
    /// The keys of polled inputs, and each recorded polled input's answer
    /// fingerprint as of the last build ([`super::probes`]).
    polled_keys: IndexKeys<super::probes::PolledKey>,
    polled: BTreeMap<Input, u64>,
    /// Bumped by every build in which anything of the program may differ from
    /// the last: [`Input::Project`]'s answer.
    project_epoch: u64,
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
    /// The census indexes' changed keys (references, callees).
    pub(crate) census_delta_keys: usize,
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
    /// Whether a later build will read this store. A store that serves one
    /// build (a one-shot or shadow-fresh build) keeps no fingerprints: there
    /// is nothing to compare them with.
    retained: bool,
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
            retained: true,
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
    #[cfg(test)]
    pub(crate) fn begin(
        &self,
        views: &Views<'_>,
        program_closed: bool,
        probes: &dyn super::probes::Probes,
    ) -> BeginReport {
        self.begin_with(views, program_closed, probes, false, false)
    }

    /// [`Self::begin`], told whether the producer says the TypeScript table
    /// is the last build's (`cache::typescript_unchanged`; without that word,
    /// the table is taken to have changed) and whether the build carries
    /// runtime redirects ([`Constant::RuntimeRedirects`]).
    pub(crate) fn begin_with(
        &self,
        views: &Views<'_>,
        program_closed: bool,
        probes: &dyn super::probes::Probes,
        typescript_unchanged: bool,
        runtime_redirects: bool,
    ) -> BeginReport {
        let mut report = BeginReport::default();
        let fresh = self.store.poisoned() || self.builds.fetch_add(1, Ordering::AcqRel) == 0;
        let mut base = self.base.write().expect("memo base lock");
        if self.store.poisoned() {
            *base = MemoBase::default();
        }
        base.interner = views.table.interner().clone();
        let started = Instant::now();
        let ids = if self.retained {
            views.table.ids().to_vec()
        } else {
            debug_assert!(fresh, "a store of one build begins once");
            Vec::new()
        };
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
        if base.ids != views.table.ids() {
            changed_inputs.push(Input::Files);
            base.ids = views.table.ids().to_vec();
        }
        let project_moved = fresh
            || !changed_files.is_empty()
            || !removed.is_empty()
            || !typescript_unchanged
            || base
                .program_closed
                .is_some_and(|closed| closed != program_closed);
        if project_moved {
            base.project_epoch += 1;
            changed_inputs.push(Input::Project);
        }
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
        let check = self.check_indexes;
        let MemoBase {
            references,
            callees,
            ..
        } = &mut *base;
        report.census_delta_keys += references.update(
            IndexId::References,
            views,
            crate::base::contributions::references,
            &changed_files,
            &removed,
            check,
            &self.store,
            &mut changed_inputs,
        );
        report.census_delta_keys += callees.update(
            IndexId::Callees,
            views,
            crate::base::contributions::callees,
            &changed_files,
            &removed,
            check,
            &self.store,
            &mut changed_inputs,
        );
        report.index_ns = nanos(started.elapsed());
        if base
            .runtime_redirects
            .is_some_and(|redirects| redirects != runtime_redirects)
        {
            changed_inputs.push(Input::Constant(Constant::RuntimeRedirects));
        }
        base.runtime_redirects = Some(runtime_redirects);
        if base
            .program_closed
            .is_some_and(|closed| closed != program_closed)
        {
            changed_inputs.push(Input::Constant(Constant::ProgramClosed));
        }
        base.program_closed = Some(program_closed);
        // Polled inputs: every recorded one is asked again, and the ones
        // whose answer moved are changed inputs.
        let recorded = base
            .polled
            .iter()
            .filter_map(|(input, last)| match input {
                Input::Polled(probe, id) => base
                    .polled_keys
                    .keys
                    .get(id)
                    .cloned()
                    .map(|key| (*input, *probe, key, *last)),
                _ => None,
            })
            .collect::<Vec<_>>();
        drop(base);
        let now =
            crate::pipeline::parallel_slice_results(&recorded, |(input, probe, key, last)| {
                let fingerprint = super::probes::fingerprint(probes, *probe, key);
                (*input, fingerprint, fingerprint != *last)
            });
        {
            let mut base = self.base.write().expect("memo base lock");
            for (input, fingerprint, changed) in now {
                if changed {
                    changed_inputs.push(input);
                }
                base.polled.insert(input, fingerprint);
            }
        }
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

    /// The interned id of a polled input's key.
    pub(super) fn polled_key(&self, key: &super::probes::PolledKey) -> u32 {
        if let Some(id) = self
            .base
            .read()
            .expect("memo base lock")
            .polled_keys
            .ids
            .get(key)
        {
            return *id;
        }
        self.base
            .write()
            .expect("memo base lock")
            .polled_keys
            .intern(key)
    }

    /// Remembers a polled input's answer fingerprint, if no build has yet:
    /// the next build asks the probe again and compares.
    pub(super) fn note_polled(&self, input: Input, fingerprint: u64) {
        let known = self
            .base
            .read()
            .expect("memo base lock")
            .polled
            .contains_key(&input);
        if !known {
            self.base
                .write()
                .expect("memo base lock")
                .polled
                .entry(input)
                .or_insert(fingerprint);
        }
    }

    /// Ends a build: sweeps entries this build did not reach and releases
    /// the index keys no entry reads any more.
    pub(crate) fn end(&self) -> Vec<(&'static str, TableStats)> {
        let (stats, released) = self.store.end();
        let mut base = self.base.write().expect("memo base lock");
        let mut polled_released = false;
        for input in released {
            match input {
                Input::Index(IndexId::WrapperCandidates, id) => base.wrapper_keys.release(id),
                Input::Index(IndexId::References, id) => base.references.keys.release(id),
                Input::Index(IndexId::Callees, id) => base.callees.keys.release(id),
                Input::Polled(..) => {
                    base.polled.remove(&input);
                    polled_released = true;
                }
                _ => {}
            }
        }
        // Several probes share a key: it is released when no polled input
        // names it.
        if polled_released {
            let live = base
                .polled
                .keys()
                .filter_map(|input| match input {
                    Input::Polled(_, id) => Some(*id),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            let dead = base
                .polled_keys
                .keys
                .keys()
                .copied()
                .filter(|id| !live.contains(id))
                .collect::<Vec<_>>();
            for id in dead {
                base.polled_keys.release(id);
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
    pub(super) store: &'w MemoStore,
    program_closed: bool,
    pub(super) probes: &'w dyn super::probes::Probes,
    project: Option<&'w dyn ProjectAccess>,
    runtime_redirects: bool,
}

impl<'w> BuildWorld<'w> {
    pub(crate) fn new(
        views: Views<'w>,
        store: &'w MemoStore,
        program_closed: bool,
        probes: &'w dyn super::probes::Probes,
    ) -> Self {
        Self {
            views,
            store,
            program_closed,
            probes,
            project: None,
            runtime_redirects: false,
        }
    }

    /// Whether the build carries runtime redirects ([`Constant::RuntimeRedirects`]).
    #[must_use]
    pub(crate) fn with_runtime_redirects(mut self, runtime_redirects: bool) -> Self {
        self.runtime_redirects = runtime_redirects;
        self
    }

    /// The lookup coarse reads go through ([`Ctx::project`]).
    #[must_use]
    pub(crate) fn with_project(mut self, project: &'w dyn ProjectAccess) -> Self {
        self.project = Some(project);
        self
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
            Input::Index(IndexId::References, key) => base.references.fingerprint(key),
            Input::Index(IndexId::Callees, key) => base.callees.fingerprint(key),
            Input::Constant(Constant::ProgramClosed) => u64::from(self.program_closed),
            Input::Constant(Constant::RuntimeRedirects) => u64::from(self.runtime_redirects),
            Input::Files => {
                use std::hash::{Hash, Hasher};
                let mut hasher = rustc_hash::FxHasher::default();
                self.views.table.ids().hash(&mut hasher);
                hasher.finish()
            }
            Input::Polled(probe, id) => {
                let key = base.polled_keys.keys.get(&id).cloned();
                drop(base);
                key.map_or(0, |key| {
                    super::probes::fingerprint(self.probes, probe, &key)
                })
            }
            Input::Project => base.project_epoch,
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
    pub(crate) fn compiler(&self) -> &'w ExecutionMap {
        self.ctx
            .record_input(Input::File(self.id, Section::Compiler));
        &self.view.file.compiler
    }

    /// The resolved call the file's entities carry for a callee
    /// ([`FileView::resolved_callee_call`]): reads the source and the
    /// file's call signatures.
    pub(crate) fn resolved_callee_call(&self, callee: Span) -> Option<&'w typefacts::ResolvedCall> {
        self.ctx.record_input(Input::File(self.id, Section::Source));
        self.ctx
            .record_input(Input::File(self.id, Section::Signatures));
        self.view.resolved_callee_call(callee)
    }

    /// The symbol whose name covers exactly `span` in this file.
    pub(crate) fn exact_symbol(&self, span: Span) -> Option<&'w SymbolId> {
        self.ctx
            .record_input(Input::File(self.id, Section::Entities));
        self.view.exact_symbol(span)
    }

    /// The symbol of the entity at exactly `span` of this file, whatever
    /// its claim ([`FileView::symbol_at`]).
    pub(crate) fn symbol_at(&self, span: Span) -> Option<&'w SymbolId> {
        self.ctx
            .record_input(Input::File(self.id, Section::Entities));
        self.view.symbol_at(span)
    }

    /// The symbol a callee resolves to, runtime redirects aside
    /// ([`FileView::callee_symbol`]): reads the AST and the TypeFacts rows.
    pub(crate) fn callee_symbol(&self, callee: Span) -> Option<&'w str> {
        self.ctx
            .record_input(Input::File(self.id, Section::Entities));
        self.ctx.record_input(Input::File(self.id, Section::Source));
        self.view.callee_symbol(callee)
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

    /// The Solid-version vocabulary the build analyzes with: a build constant
    /// (a different dialect is a different cache domain), so it is not recorded.
    pub(crate) fn dialect(&self) -> &'w dyn solid_dialect::Dialect {
        self.world().views.constants.dialect
    }

    /// Runs `f` over the build's lookup, recorded as a read of the whole
    /// program ([`Input::Project`]): the way a computation reaches a table
    /// not yet ported onto memos. `f` must return owned data.
    pub(crate) fn project<R>(
        &self,
        f: impl for<'x> FnOnce(&crate::indexes::SemanticLookup<'x>) -> R,
    ) -> R {
        self.record_input(Input::Project);
        let access = self
            .world()
            .project
            .expect("a build world with a lookup (`with_project`)");
        let mut f = Some(f);
        let mut out = None;
        access.run(&mut |lookup| {
            if let Some(f) = f.take() {
                out = Some(f(lookup));
            }
        });
        out.expect("the lookup ran the closure")
    }

    /// [`Self::project`] with the build's file table (the store's ids), for
    /// a computation that names files by id.
    pub(crate) fn project_files<R>(
        &self,
        f: impl for<'x> FnOnce(&crate::indexes::SemanticLookup<'x>, &FileTable) -> R,
    ) -> R {
        let table = self.world().views.table;
        self.project(|lookup| f(lookup, table))
    }

    /// The path a file id names: the key's identity, not a read of the file.
    pub(crate) fn path_of(&self, id: FileId) -> &'w str {
        self.world().views.table.path_of(id)
    }

    /// Every analyzed id, in the order of its first slot.
    pub(crate) fn ids(&self) -> Vec<FileId> {
        self.record_input(Input::Files);
        self.world().views.table.ids().to_vec()
    }

    /// The TypeFacts references of `symbol` located in analyzed files: `(file,
    /// slot rank, span)`, by file id then production order (each slot of a
    /// file lists them).
    pub(crate) fn references(&self, symbol: &Arc<str>) -> Vec<(FileId, usize, Span)> {
        self.index_values(
            IndexId::References,
            symbol,
            |base| &base.references,
            |base| &mut base.references,
        )
    }

    /// The calls and JSX elements whose callee or tag resolves to `symbol`
    /// ([`super::super::contributions::callees`]): `(file, slot rank, site)`.
    pub(crate) fn callees(
        &self,
        symbol: &Arc<str>,
    ) -> Vec<(FileId, usize, crate::base::CalleeSite)> {
        self.index_values(
            IndexId::Callees,
            symbol,
            |base| &base.callees,
            |base| &mut base.callees,
        )
    }

    fn index_values<C>(
        &self,
        id: IndexId,
        key: &Arc<str>,
        kept: fn(&MemoBase) -> &KeptIndex<Arc<str>, C>,
        kept_mut: fn(&mut MemoBase) -> &mut KeptIndex<Arc<str>, C>,
    ) -> Vec<(FileId, usize, C)>
    where
        C: Clone + PartialEq + Send + std::hash::Hash,
    {
        let store = self.world().store;
        let interned = {
            let base = store.base.read().expect("memo base lock");
            kept(&base).keys.ids.get(key).copied()
        };
        let interned = interned.unwrap_or_else(|| {
            let mut base = store.base.write().expect("memo base lock");
            kept_mut(&mut base).keys.intern(key)
        });
        self.record_input(Input::Index(id, interned));
        let base = store.base.read().expect("memo base lock");
        kept(&base).values(key)
    }

    /// A build constant.
    pub(crate) fn constant(&self, constant: Constant) -> bool {
        self.record_input(Input::Constant(constant));
        match constant {
            Constant::ProgramClosed => self.world().program_closed,
            Constant::RuntimeRedirects => self.world().runtime_redirects,
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
        let mut store = Box::new(MemoStore::from_env());
        store.retained = false;
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
