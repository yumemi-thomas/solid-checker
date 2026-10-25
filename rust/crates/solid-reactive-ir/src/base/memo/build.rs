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
//! set of changed keys), and the constants are compared. A file's TypeFacts
//! parts are digested again only when their rows are not the rows digested
//! last build ([`FileParts`]); the producer's list of changed paths is not
//! trusted for it.

use super::locks::RwLock;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// its path, in table order, its location and its whole resolved call
    /// (validity, kind, targets, declaration, arguments, flags), since
    /// `FileRead::resolved_callee_call` hands the whole call to its reader.
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
    /// The spans where a symbol's value is used like a returned callback
    /// ([`super::super::contributions::use_spans`]).
    UseSpans,
    /// Uses of entity symbols by the symbol ([`super::super::contributions::symbol_uses`]).
    SymbolUses,
    /// Property spellings by name ([`super::super::contributions::spellings`]).
    Spellings,
    /// The global bindings some assignment writes, by name
    /// ([`super::super::contributions::global_writes`]).
    GlobalWrites,
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
    /// One key of a project pass's published output ([`super::derived`]),
    /// by its interned id.
    Derived(super::derived::DerivedId, u32),
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
    /// Tables 128 to 159: returned-callback use sites (study S5e).
    returned: crate::indexes::ReturnedTables,
    /// Tables 96 to 127 and 256 to 319: the invocation-flow answers (study
    /// S5a) and their site roles.
    flow: crate::execution_role::FlowTables,
    /// Tables 160 to 191: returned-function invocations and returned sources
    /// (study S6a).
    invocations: crate::indexes::InvocationTables,
    /// Tables 240 to 255: the accessor-provenance resolver (S7b).
    resolver: crate::interproc::accessor_provenance::ResolverTables,
    /// Tables 384 to 447: accessor transport (S7c).
    transport: crate::interproc::accessor_provenance::TransportTables,
    /// Tables 640 to 703: what the lookup hands a recording chunk
    /// (ADR 0408).
    hand_outs: crate::indexes::HandOutTables,
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
                .contains(&id.0)
                || (crate::execution_role::FLOW_NARROW_TABLES_FROM
                    ..crate::execution_role::FLOW_NARROW_TABLES_END)
                    .contains(&id.0) =>
            {
                self.flow
                    .table(id)
                    .unwrap_or_else(|| unreachable!("memo table {id:?}"))
            }
            id if (crate::interproc::accessor_provenance::TRANSPORT_TABLES_FROM
                ..crate::interproc::accessor_provenance::TRANSPORT_TABLES_END)
                .contains(&id.0) =>
            {
                self.transport
                    .table(id)
                    .unwrap_or_else(|| unreachable!("memo table {id:?}"))
            }
            id if (crate::indexes::HAND_OUT_TABLES_FROM..crate::indexes::HAND_OUT_TABLES_END)
                .contains(&id.0) =>
            {
                self.hand_outs
                    .table(id)
                    .unwrap_or_else(|| unreachable!("memo table {id:?}"))
            }
            id if id.0 >= crate::interproc::accessor_provenance::RESOLVER_TABLES_FROM => self
                .resolver
                .table(id)
                .unwrap_or_else(|| unreachable!("memo table {id:?}")),
            id if id.0 >= crate::indexes::INVOCATION_TABLES_FROM => self
                .invocations
                .table(id)
                .unwrap_or_else(|| unreachable!("memo table {id:?}")),
            id if id.0 >= crate::indexes::RETURNED_TABLES_FROM => self
                .returned
                .table(id)
                .unwrap_or_else(|| unreachable!("memo table {id:?}")),
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
        tables.extend(self.returned.tables());
        tables.extend(self.flow.tables());
        tables.extend(self.invocations.tables());
        tables.extend(self.resolver.tables());
        tables.extend(self.transport.tables());
        tables.extend(self.hand_outs.tables());
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

    pub(crate) fn hand_outs(&self) -> &crate::indexes::HandOutTables {
        &self.hand_outs
    }

    pub(crate) fn census(&self) -> &crate::indexes::CensusTables {
        &self.census
    }

    pub(crate) fn returned(&self) -> &crate::indexes::ReturnedTables {
        &self.returned
    }

    pub(crate) fn flow(&self) -> &crate::execution_role::FlowTables {
        &self.flow
    }

    pub(crate) fn invocations(&self) -> &crate::indexes::InvocationTables {
        &self.invocations
    }

    pub(crate) fn resolver(&self) -> &crate::interproc::accessor_provenance::ResolverTables {
        &self.resolver
    }

    pub(crate) fn transport(&self) -> &crate::interproc::accessor_provenance::TransportTables {
        &self.transport
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

/// CPU time of one file's fingerprints by part, when measured
/// ([`crate::base::reads::cost`]): source and compiler, symbol rows,
/// declarations and signatures, references, and the parts reused.
type FingerprintCost = [u64; 5];

/// What a file's Entities and Signatures fingerprints are made of, kept
/// between builds with the identity of the rows each part digests
/// (ADR 0406). A part is digested again only when its rows are
/// not the very rows it digested last build:
/// - the TypeFacts entities of the path (`entities_for_path`), by address
///   and length. The store keeps the last build's table
///   ([`MemoBase::typescript`]), so every allocation those addresses name is
///   alive and immutable: a slice at the same address with the same length
///   is the same rows. A replaced path's rows are a new allocation, so its
///   parts are digested again whatever the producer named; a full table
///   replace digests every file again.
/// - the path's entity-symbol rows ([`crate::indexes::PathSymbols`]), which
///   the parts hold: a holder of a clone sees the rows change only through a
///   copy (`Arc::make_mut`), so the same pointer is the same rows.
///
/// The references located in the file are digested every build: they are
/// assembled per build from the whole symbol table, with no identity to
/// key on.
#[derive(Clone, Default)]
struct FileParts {
    entities_at: (usize, usize),
    /// The declaration each entity's resolved call names: part of Entities.
    declarations: [u8; 32],
    signatures: [u8; 32],
    rows: (
        Option<crate::indexes::PathSymbols>,
        Option<crate::indexes::PathSymbols>,
    ),
    rows_digest: [u8; 32],
}

fn same_rows(
    left: Option<&crate::indexes::PathSymbols>,
    right: Option<&crate::indexes::PathSymbols>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        _ => false,
    }
}

/// `id`'s fingerprints in this build, and the parts they are made of. With
/// `previous` (the last build's parts, valid against the table the store
/// kept), a part whose rows are the same rows is not digested again;
/// `reused` counts those.
fn fingerprints(
    views: &Views<'_>,
    id: FileId,
    previous: Option<&FileParts>,
    cost: &mut FingerprintCost,
    reused: &mut [usize; 2],
) -> (FileFingerprints, FileParts) {
    let measure = crate::base::reads::cost::on();
    let mut started = measure.then(crate::base::reads::cost::now_ns);
    let mut lap = |part: usize| {
        if let Some(at) = started.as_mut() {
            let now = crate::base::reads::cost::now_ns();
            cost[part] += now.saturating_sub(*at);
            *at = now;
        }
    };
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
    lap(0);
    let mut parts = FileParts::default();
    let mut references = Sha256::new();
    // Entity rows are per path: every slot of the id reads the same ones.
    if let Some(slot) = slots.first() {
        let view = views.view(*slot);
        let path = views.table.path_of(id);
        parts.rows = (
            views.sources.symbols.by_path.get(path).cloned(),
            views.sources.symbols.exact_by_path.get(path).cloned(),
        );
        match previous.filter(|previous| {
            same_rows(previous.rows.0.as_ref(), parts.rows.0.as_ref())
                && same_rows(previous.rows.1.as_ref(), parts.rows.1.as_ref())
        }) {
            Some(previous) => {
                parts.rows_digest = previous.rows_digest;
                reused[0] += 1;
            }
            None => parts.rows_digest = symbol_rows_digest(&view),
        }
        lap(1);
        let entities = view.typescript_entities();
        parts.entities_at = (entities.as_ptr() as usize, entities.len());
        match previous.filter(|previous| previous.entities_at == parts.entities_at) {
            Some(previous) => {
                parts.declarations = previous.declarations;
                parts.signatures = previous.signatures;
                reused[1] += 1;
            }
            None => {
                parts.declarations = declarations_digest(entities);
                parts.signatures = signatures_digest(entities);
            }
        }
        lap(2);
        // The references located here (the reference index's entries).
        for (symbol, span) in view.references() {
            references.update(
                u64::try_from(symbol.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            references.update(symbol.as_bytes());
            references.update(span.start.to_le_bytes());
            references.update(span.end.to_le_bytes());
        }
        lap(3);
    }
    let references: [u8; 32] = references.finalize().into();
    let mut entities = Sha256::new();
    entities.update(parts.rows_digest);
    entities.update(parts.declarations);
    entities.update(references);
    // The file's import resolutions (`file_table::resolution_digests`).
    match views
        .sources
        .resolutions
        .and_then(|digests| digests.get(views.table.path_of(id)))
    {
        Some(digest) => {
            entities.update([1]);
            entities.update(digest);
        }
        None => entities.update([0]),
    }
    let fingerprints = FileFingerprints {
        source: source.finalize().into(),
        compiler: compiler.finalize().into(),
        entities: entities.finalize().into(),
        signatures: parts.signatures,
        slots,
    };
    (fingerprints, parts)
}

/// `id`'s fingerprints with every part digested.
fn fingerprints_fresh(views: &Views<'_>, id: FileId) -> (FileFingerprints, FileParts) {
    fingerprints(views, id, None, &mut [0; 5], &mut [0; 2])
}

/// The entity-symbol rows of a file's view, digested.
fn symbol_rows_digest(view: &FileView<'_>) -> [u8; 32] {
    let mut rows = Sha256::new();
    for ((start, end), symbol, exact) in view.symbol_rows() {
        rows.update(start.to_le_bytes());
        rows.update(end.to_le_bytes());
        rows.update(
            u64::try_from(symbol.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        rows.update(symbol.as_bytes());
        rows.update([u8::from(exact)]);
    }
    rows.finalize().into()
}

/// The declaration each resolved call of `entities` names (a callee's
/// symbol), digested.
fn declarations_digest(entities: &[typefacts::EntityFact]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for entity in entities {
        let declaration = entity.resolved_call.as_deref().map(|call| {
            call.declaration
                .as_ref()
                .map(|declaration| &*declaration.symbol)
        });
        digest.update(entity.location.start_byte.to_le_bytes());
        digest.update(entity.location.end_byte.to_le_bytes());
        match declaration {
            None => digest.update([0]),
            Some(None) => digest.update([1]),
            Some(Some(symbol)) => {
                digest.update([2]);
                digest.update(
                    u64::try_from(symbol.len())
                        .unwrap_or(u64::MAX)
                        .to_le_bytes(),
                );
                digest.update(symbol.as_bytes());
            }
        }
    }
    digest.finalize().into()
}

/// Each entity's location and whole resolved call, digested: the
/// Signatures section. The rows are written as self-delimiting binary
/// ([`super::serial_digest`]) and digested at once.
fn signatures_digest(entities: &[typefacts::EntityFact]) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(entities.len() * 64);
    for entity in entities {
        bytes.extend_from_slice(&entity.location.start_byte.to_le_bytes());
        bytes.extend_from_slice(&entity.location.end_byte.to_le_bytes());
        match entity.resolved_call.as_deref() {
            None => bytes.push(0),
            Some(call) => {
                bytes.push(1);
                // The call's canonical serialization (field order is the
                // struct's; no map iterates).
                super::serial_digest::append(&mut bytes, call);
            }
        }
    }
    Sha256::digest(&bytes).into()
}

/// Runs the four folds at once, each on a quarter of the analysis workers;
/// with one worker (wasm32-wasip1) one after the other.
#[allow(clippy::disallowed_methods)]
fn fold_side_by_side<A: Send, B: Send, C: Send, D: Send>(
    a: impl FnOnce() -> A + Send,
    b: impl FnOnce() -> B + Send,
    c: impl FnOnce() -> C + Send,
    d: impl FnOnce() -> D + Send,
) -> (A, B, C, D) {
    if crate::pipeline::available_analysis_workers() <= 1 {
        return (a(), b(), c(), d());
    }
    let limit = crate::pipeline::analysis_worker_limit_for_lanes(4);
    std::thread::scope(|scope| {
        let b = scope.spawn(move || {
            let _limit = crate::pipeline::AnalysisWorkerLimit::enter(limit);
            b()
        });
        let c = scope.spawn(move || {
            let _limit = crate::pipeline::AnalysisWorkerLimit::enter(limit);
            c()
        });
        let d = scope.spawn(move || {
            let _limit = crate::pipeline::AnalysisWorkerLimit::enter(limit);
            d()
        });
        let a = {
            let _limit = crate::pipeline::AnalysisWorkerLimit::enter(limit);
            a()
        };
        (
            a,
            b.join().expect("index fold panicked"),
            c.join().expect("index fold panicked"),
            d.join().expect("index fold panicked"),
        )
    })
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
    K: Clone + Eq + std::hash::Hash + Send + Sync + std::fmt::Debug,
    C: Clone + PartialEq + Send + Sync + std::hash::Hash,
{
    /// Brings the index to `views` from the last build's: the changed and
    /// removed files' contributions are replaced, and each changed key some
    /// entry reads is pushed onto `changed`. A store with no index folds one.
    /// With `check`, the update is compared with a refold; a difference is
    /// reported (the store is poisoned) and the refold kept. A `lazy` index
    /// no build has read stays unfolded: its first read folds it
    /// ([`Ctx::symbol_uses`]), and until then no entry reads a key of it.
    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        id: IndexId,
        lazy: bool,
        folded: Option<BaseIndex<K, C>>,
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
            None if lazy => {}
            None => self.index = Some(folded.unwrap_or_else(|| views.fold(contribute))),
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
    /// What each file's fingerprints are made of ([`FileParts`]).
    parts: BTreeMap<FileId, FileParts>,
    /// The last build's TypeFacts table: it keeps alive every allocation
    /// [`FileParts::entities_at`] names.
    typescript: Option<solid_facts::TypeScriptTable>,
    wrapper_candidates: Option<BaseIndex<Arc<str>, WrapperCandidate>>,
    wrapper_keys: IndexKeys<Arc<str>>,
    program_closed: Option<bool>,
    /// The component census's indexes ([`crate::indexes::CallSitesOf`]).
    references: KeptIndex<Arc<str>, Span>,
    callees: KeptIndex<Arc<str>, crate::base::CalleeSite>,
    use_spans: KeptIndex<Arc<str>, Span>,
    /// The object-member census's indexes, read by key instead of asked as
    /// probes at every build (ADR 0406).
    symbol_uses: KeptIndex<Arc<str>, crate::base::SymbolUse>,
    spellings: KeptIndex<Arc<str>, crate::base::Spelling>,
    global_writes: KeptIndex<Arc<str>, ()>,
    runtime_redirects: Option<bool>,
    /// The analyzed ids in the order of their first slot ([`Input::Files`]).
    ids: Vec<FileId>,
    /// Bumped by every build in which anything of the program may differ from
    /// the last: [`Input::Project`]'s answer.
    project_epoch: u64,
}

/// Polled keys are sharded by hash; a key's id is its shard's id times
/// [`POLLED_SHARDS`] plus the shard, so an id names its shard.
const POLLED_SHARDS: usize = 16;

#[derive(Default)]
struct PolledShard {
    keys: IndexKeys<super::probes::PolledKey>,
    /// Read in no order: `begin` re-asks each and `end` releases by key.
    answers: HashMap<Input, super::probes::AnswerFingerprint>,
}

fn polled_shard_of(key: &super::probes::PolledKey) -> usize {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    key.hash(&mut hasher);
    // The high bits: FxHash's low bits are weak for small integer keys.
    (hasher.finish() >> 60) as usize % POLLED_SHARDS
}

fn polled_id(shard: usize, local: u32) -> u32 {
    local
        .checked_mul(POLLED_SHARDS as u32)
        .and_then(|id| id.checked_add(shard as u32))
        .expect("fewer than 2^28 polled keys per shard")
}

fn polled_local(id: u32) -> (usize, u32) {
    (id as usize % POLLED_SHARDS, id / POLLED_SHARDS as u32)
}

/// What one [`MemoStore::begin`] found and cost.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BeginReport {
    pub(crate) fingerprint_ns: u64,
    pub(crate) index_ns: u64,
    pub(crate) mark_ns: u64,
    /// Re-asking the recorded probes: the wall time, how many, how many moved.
    pub(crate) probe_ns: u64,
    pub(crate) probes: usize,
    pub(crate) probes_moved: usize,
    /// Files whose entity-symbol rows, and whose TypeFacts entities, were
    /// the last build's rows: their parts were not digested again.
    pub(crate) rows_reused: usize,
    pub(crate) entities_reused: usize,
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
    /// Unique in the process: a chunk's recorded memo reads name the store
    /// they were made against ([`crate::base::reads::ReadProof`]).
    id: u64,
    base: RwLock<MemoBase>,
    /// The keys of polled inputs, and each recorded polled input's answer
    /// fingerprint as of the last build ([`super::probes`]), sharded by key
    /// and apart from `base`: every probe of a parallel stage reads them,
    /// and a first-seen key writes them, which must not stall the stage's
    /// fingerprint reads of `base`.
    polled: Box<[RwLock<PolledShard>; POLLED_SHARDS]>,
    /// Shadow or verify-reused mode: the updated indexes are compared with a
    /// refold every build.
    check_indexes: bool,
    /// Whether a later build will read this store. A store that serves one
    /// build (a one-shot or shadow-fresh build) keeps no fingerprints: there
    /// is nothing to compare them with.
    retained: bool,
    builds: AtomicU64,
    /// Project passes' kept solvers and published outputs.
    derived: super::derived::DerivedTables,
    /// This build's begin costs, when measured ([`crate::base::reads::cost`]).
    cost: std::sync::Mutex<Option<serde_json::Value>>,
    /// A finished build whose [`Self::end`] has not run yet
    /// ([`LookupMemos::defer_end`]), with the begin report it will print:
    /// run by [`Self::finish_deferred_end`], or by the next build's begin.
    deferred_end: std::sync::Mutex<Option<BeginReport>>,
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
        static NEXT_STORE: AtomicU64 = AtomicU64::new(1);
        Self {
            store: Store::new(verify_reused, report),
            id: NEXT_STORE.fetch_add(1, Ordering::Relaxed),
            base: RwLock::default(),
            polled: Default::default(),
            check_indexes: check_indexes || verify_reused,
            retained: true,
            builds: AtomicU64::new(0),
            derived: super::derived::DerivedTables::default(),
            cost: std::sync::Mutex::new(None),
            deferred_end: std::sync::Mutex::new(None),
        }
    }

    /// Runs the end of a build that deferred it, if one is due: the sweep of
    /// the entries that build did not reach. The report the build would have
    /// returned, or `None` when no end was due.
    pub(crate) fn finish_deferred_end(&self) -> Option<MemoReport> {
        let begin = self
            .deferred_end
            .lock()
            .expect("memo deferred end lock")
            .take()?;
        Some(MemoReport {
            begin,
            tables: self.end(),
        })
    }

    /// A store configured from the environment (verify-reused and shadow
    /// modes).
    pub(crate) fn from_env() -> Self {
        Self::new(
            verify_reused_from_env(),
            crate::shadow::ShadowMode::from_env() != crate::shadow::ShadowMode::Off,
        )
    }

    #[cfg(test)]
    pub(crate) fn poisoned(&self) -> bool {
        self.store.poisoned()
    }

    /// The store's identity in the process.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// A query of `table`, named for a diagnostic.
    pub(crate) fn describe(&self, table: TableId, query: super::QueryId) -> String {
        self.store.describe(table, query)
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
        // The last build's sweep comes first: a build reads the store as the
        // sweep left it.
        let _ = self.finish_deferred_end();
        let mut report = BeginReport::default();
        let fresh = self.store.poisoned() || self.builds.fetch_add(1, Ordering::AcqRel) == 0;
        let mut base = self.base.write().expect("memo base lock");
        if self.store.poisoned() {
            *base = MemoBase::default();
            for shard in &self.polled {
                *shard.write().expect("memo polled lock") = PolledShard::default();
            }
            self.derived.clear();
        }
        base.interner = views.table.interner().clone();
        let started = Instant::now();
        let ids = if self.retained {
            views.table.ids().to_vec()
        } else {
            debug_assert!(fresh, "a store of one build begins once");
            Vec::new()
        };
        // Measured, the phase runs on one thread: each file's time is CPU.
        let one_thread =
            crate::base::reads::cost::on().then(|| crate::pipeline::AnalysisWorkerLimit::enter(1));
        // The last build's parts are valid only against the table it kept.
        let previous = base.typescript.as_ref().map(|_| &base.parts);
        let check = self.check_indexes;
        let costed = crate::pipeline::parallel_slice_results(&ids, |id| {
            let mut cost = [0; 5];
            let mut reused = [0; 2];
            let (fingerprints, parts) = fingerprints(
                views,
                *id,
                previous.and_then(|parts| parts.get(id)),
                &mut cost,
                &mut reused,
            );
            // Shadow and verify-reused modes digest every part again; a
            // difference is reported and the fresh digest kept.
            if check && reused != [0; 2] {
                let (fresh, fresh_parts) = fingerprints_fresh(views, *id);
                if fresh != fingerprints {
                    return ((*id, fresh, fresh_parts), cost, reused, Some(*id));
                }
            }
            ((*id, fingerprints, parts), cost, reused, None)
        });
        let mut fingerprint_cost: FingerprintCost = [0; 5];
        let mut current = Vec::with_capacity(costed.len());
        let mut parts = BTreeMap::new();
        for ((id, fingerprints, file_parts), cost, reused, mismatch) in costed {
            for (sum, part) in fingerprint_cost.iter_mut().zip(cost) {
                *sum += part;
            }
            report.rows_reused += reused[0];
            report.entities_reused += reused[1];
            if let Some(id) = mismatch {
                self.store.poison(&format!(
                    "IR memo mismatch: the kept fingerprint parts of `{}` differ from a fresh digest",
                    views.table.path_of(id)
                ));
            }
            current.push((id, fingerprints));
            parts.insert(id, file_parts);
        }
        drop(one_thread);
        if self.retained {
            base.parts = parts;
            base.typescript = Some(views.sources.typescript.clone());
        }
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
        // A store's first build folds its four eager indexes side by side;
        // each is then installed below as the fold it would have made.
        let mut first_folds = if base.wrapper_candidates.is_none()
            && base.references.index.is_none()
            && base.callees.index.is_none()
            && base.use_spans.index.is_none()
        {
            let (wrappers, references, callees, use_spans) = fold_side_by_side(
                || views.fold(contribute),
                || views.fold(crate::base::contributions::references),
                || views.fold(crate::base::contributions::callees),
                || views.fold(crate::base::contributions::use_spans),
            );
            (
                Some(wrappers),
                Some(references),
                Some(callees),
                Some(use_spans),
            )
        } else {
            (None, None, None, None)
        };
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
            None => {
                base.wrapper_candidates = Some(
                    first_folds
                        .0
                        .take()
                        .unwrap_or_else(|| views.fold(contribute)),
                );
            }
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
            use_spans,
            symbol_uses,
            spellings,
            global_writes,
            ..
        } = &mut *base;
        report.census_delta_keys += references.update(
            IndexId::References,
            false,
            first_folds.1.take(),
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
            false,
            first_folds.2.take(),
            views,
            crate::base::contributions::callees,
            &changed_files,
            &removed,
            check,
            &self.store,
            &mut changed_inputs,
        );
        report.census_delta_keys += use_spans.update(
            IndexId::UseSpans,
            false,
            first_folds.3.take(),
            views,
            crate::base::contributions::use_spans,
            &changed_files,
            &removed,
            check,
            &self.store,
            &mut changed_inputs,
        );
        report.census_delta_keys += symbol_uses.update(
            IndexId::SymbolUses,
            true,
            None,
            views,
            crate::base::contributions::symbol_uses,
            &changed_files,
            &removed,
            check,
            &self.store,
            &mut changed_inputs,
        );
        report.census_delta_keys += spellings.update(
            IndexId::Spellings,
            true,
            None,
            views,
            crate::base::contributions::spellings,
            &changed_files,
            &removed,
            check,
            &self.store,
            &mut changed_inputs,
        );
        report.census_delta_keys += global_writes.update(
            IndexId::GlobalWrites,
            true,
            None,
            views,
            crate::base::contributions::global_writes,
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
        drop(base);
        let recorded = self
            .polled
            .iter()
            .flat_map(|shard| {
                let shard = shard.read().expect("memo polled lock");
                shard
                    .answers
                    .iter()
                    .filter_map(|(input, last)| match input {
                        Input::Polled(probe, id) => shard
                            .keys
                            .keys
                            .get(&polled_local(*id).1)
                            .cloned()
                            .map(|key| (*input, *probe, key, *last)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let measure = crate::base::reads::cost::on();
        let one_thread = measure.then(|| crate::pipeline::AnalysisWorkerLimit::enter(1));
        let started = Instant::now();
        let now =
            crate::pipeline::parallel_slice_results(&recorded, |(input, probe, key, last)| {
                let asked = measure.then(crate::base::reads::cost::now_ns);
                let fingerprint = super::probes::fingerprint(probes, *probe, key);
                let cost = asked.map_or(0, |asked| {
                    crate::base::reads::cost::now_ns().saturating_sub(asked)
                });
                (*input, *probe, fingerprint, fingerprint != *last, cost)
            });
        report.probe_ns = nanos(started.elapsed());
        drop(one_thread);
        report.probes = now.len();
        let mut probe_cost = BTreeMap::<String, [u64; 3]>::new();
        {
            for (input, probe, fingerprint, changed, cost) in now {
                if changed {
                    changed_inputs.push(input);
                    report.probes_moved += 1;
                }
                if measure {
                    let kind = match probe {
                        super::probes::Probe::Hazard(_) => "Hazard".to_owned(),
                        probe => format!("{probe:?}"),
                    };
                    let entry = probe_cost.entry(kind).or_default();
                    entry[0] += 1;
                    entry[1] += u64::from(changed);
                    entry[2] += cost;
                }
                if let Input::Polled(_, id) = input {
                    self.polled[polled_local(id).0]
                        .write()
                        .expect("memo polled lock")
                        .answers
                        .insert(input, fingerprint);
                }
            }
        }
        if measure {
            *self.cost.lock().expect("memo cost lock") = Some(serde_json::json!({
                "fingerprintCpuNs": {
                    "sourceCompiler": fingerprint_cost[0],
                    "symbolRows": fingerprint_cost[1],
                    "declarationsAndSignatures": fingerprint_cost[2],
                    "references": fingerprint_cost[3],
                },
                "fingerprintReused": {
                    "rows": report.rows_reused,
                    "entities": report.entities_reused,
                },
                "fingerprintWallNs": report.fingerprint_ns,
                "probeWallNs": report.probe_ns,
                "probeCpuNs": probe_cost.values().map(|cost| cost[2]).sum::<u64>(),
                "probes": probe_cost,
            }));
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

    /// The polled input of `probe` at `key`, interned, with `fingerprint`
    /// remembered as its answer if no build has one yet: the next build asks
    /// the probe again and compares.
    pub(super) fn poll(
        &self,
        probe: super::probes::Probe,
        key: &super::probes::PolledKey,
        fingerprint: super::probes::AnswerFingerprint,
    ) -> Input {
        let shard = polled_shard_of(key);
        let lock = &self.polled[shard];
        {
            let polled = lock.read().expect("memo polled lock");
            if let Some(local) = polled.keys.ids.get(key) {
                let input = Input::Polled(probe, polled_id(shard, *local));
                if polled.answers.contains_key(&input) {
                    return input;
                }
            }
        }
        let mut polled = lock.write().expect("memo polled lock");
        let input = Input::Polled(probe, polled_id(shard, polled.keys.intern(key)));
        polled.answers.entry(input).or_insert(fingerprint);
        input
    }

    /// Ends a build: sweeps entries this build did not reach and releases
    /// the index keys no entry reads any more.
    pub(crate) fn end(&self) -> Vec<(&'static str, TableStats)> {
        self.log_cost();
        let (stats, released) = self.store.end();
        let mut base = self.base.write().expect("memo base lock");
        let mut polled_released = [false; POLLED_SHARDS];
        for input in released {
            match input {
                Input::Index(IndexId::WrapperCandidates, id) => base.wrapper_keys.release(id),
                Input::Index(IndexId::References, id) => base.references.keys.release(id),
                Input::Index(IndexId::Callees, id) => base.callees.keys.release(id),
                Input::Index(IndexId::UseSpans, id) => base.use_spans.keys.release(id),
                Input::Index(IndexId::SymbolUses, id) => base.symbol_uses.keys.release(id),
                Input::Index(IndexId::Spellings, id) => base.spellings.keys.release(id),
                Input::Index(IndexId::GlobalWrites, id) => base.global_writes.keys.release(id),
                Input::Polled(_, id) => {
                    let shard = polled_local(id).0;
                    self.polled[shard]
                        .write()
                        .expect("memo polled lock")
                        .answers
                        .remove(&input);
                    polled_released[shard] = true;
                }
                Input::Derived(id, key) => self.derived.release(id, key),
                _ => {}
            }
        }
        drop(base);
        // Several probes share a key: it is released when no polled input
        // names it.
        for (shard, released) in self.polled.iter().zip(polled_released) {
            if !released {
                continue;
            }
            let mut polled = shard.write().expect("memo polled lock");
            let live = polled
                .answers
                .keys()
                .filter_map(|input| match input {
                    Input::Polled(_, id) => Some(polled_local(*id).1),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            let dead = polled
                .keys
                .keys
                .keys()
                .copied()
                .filter(|id| !live.contains(id))
                .collect::<Vec<_>>();
            for id in dead {
                polled.keys.release(id);
            }
        }
        stats
    }

    /// Appends the build's bookkeeping costs to
    /// `SOLID_CHECKER_IR_BOOKKEEPING_LOG`: the begin's and the read sets'.
    fn log_cost(&self) {
        if !crate::base::reads::cost::on() {
            return;
        }
        let begin = self.cost.lock().expect("memo cost lock").take();
        let reads = crate::base::reads::cost::take()
            .into_iter()
            .map(|(kind, calls, elements, nanos)| {
                (
                    kind.to_owned(),
                    serde_json::json!({ "calls": calls, "elements": elements, "cpuNs": nanos }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let line =
            serde_json::json!({ "begin": begin, "readSets": reads, "retained": self.retained });
        if let Some(path) = std::env::var_os("SOLID_CHECKER_IR_BOOKKEEPING_LOG")
            && let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
        {
            use std::io::Write;
            let _ = writeln!(file, "{line}");
        }
    }

    /// Every key of the wrapper-candidate index, sorted.
    #[cfg(test)]
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

impl super::derived::DerivedSchema for BuildSchema {
    fn derived<'a>(world: &'a BuildWorld<'_>) -> &'a super::derived::DerivedTables {
        &world.store.derived
    }

    fn input(id: super::derived::DerivedId, key: u32) -> Input {
        Input::Derived(id, key)
    }
}

impl MemoStore {
    /// The kept solvers and published outputs of project passes.
    pub(crate) fn derived(&self) -> &super::derived::DerivedTables {
        &self.derived
    }

    /// Publishes a pass's moved keys to the memos that read them
    /// ([`super::Store::publish`]).
    pub(crate) fn publish<D: super::derived::Derived>(
        &self,
        changes: impl IntoIterator<Item = (D::Key, Option<D::Value>)>,
    ) -> Invalidation {
        self.store.publish::<D>(&self.derived, changes)
    }

    /// The store's current build ([`super::Store::revision`]).
    pub(crate) fn revision(&self) -> super::Revision {
        self.store.revision()
    }

    /// Shadow or verify-reused mode: a kept solver is checked against a
    /// fresh solve every build ([`crate::base::fixpoint`]).
    pub(crate) fn check_solvers(&self) -> bool {
        self.check_indexes
    }

    /// Whether the store is kept between builds (a retained session's), so
    /// state kept in it is reused.
    pub(crate) fn is_retained(&self) -> bool {
        self.retained
    }

    /// Reports `message` and poisons the store: the next build starts empty.
    pub(crate) fn poison(&self, message: &str) {
        self.store.poison(message);
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

    /// The answer of `M` for `key`, to a reader outside every memo. A stage
    /// chunk computing on this thread records it ([`crate::base::reads`]).
    pub(crate) fn answer<M: Memo<BuildSchema>>(&self, key: &M::Key) -> M::Value {
        let (value, read) = self.store.store.answer_read::<M>(self, key);
        match read {
            Some((query, changed_at)) => {
                crate::base::reads::note_memo(crate::base::reads::MemoRead {
                    table: M::ID,
                    query,
                    changed_at,
                })
            }
            // A cycle's fail-closed answer: the store is poisoned, and a
            // chunk that read it must not be trusted again.
            None => crate::base::reads::note_memo(crate::base::reads::MemoRead {
                table: M::ID,
                query: super::QueryId(u32::MAX),
                changed_at: super::Revision::MAX,
            }),
        }
        value
    }

    /// The read a reader outside every memo records of `M`'s answer for
    /// `key` ([`Self::answer`]), without recording it: `None` for a cycle's
    /// fail-closed answer, which poisons the store.
    pub(crate) fn memo_read<M: Memo<BuildSchema>>(
        &self,
        key: &M::Key,
    ) -> Option<crate::base::reads::MemoRead> {
        let (_, read) = self.store.store.answer_read::<M>(self, key);
        read.map(|(query, changed_at)| crate::base::reads::MemoRead {
            table: M::ID,
            query,
            changed_at,
        })
    }

    /// The revision `query` of `table` last changed in, for a chunk's
    /// recorded read ([`crate::base::reads::ReadAnswers`]).
    pub(crate) fn changed_at(
        &self,
        table: TableId,
        query: super::QueryId,
    ) -> Option<super::Revision> {
        self.changed_at_checked(table, query).0
    }

    /// [`Self::changed_at`], and whether bringing the memo up to date needed
    /// a derived table not published yet in this build: then the answer is
    /// `None` and nothing was installed, nor the store poisoned
    /// ([`super::derived::deferring`]).
    pub(crate) fn changed_at_checked(
        &self,
        table: TableId,
        query: super::QueryId,
    ) -> (Option<super::Revision>, bool) {
        super::derived::computing(|| {
            crate::base::reads::suppress(|| {
                super::derived::deferring(|| self.store.store.changed_at(self, table, query))
            })
        })
    }
}

impl World<BuildSchema> for BuildWorld<'_> {}

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

    /// The file's TypeFacts references, `(symbol, span)` (its entity rows).
    pub(crate) fn references(&self) -> &'w [(Arc<str>, Span)] {
        self.ctx
            .record_input(Input::File(self.id, Section::Entities));
        self.view.references()
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

    /// The fingerprint of each [`Section`] of the file `id`, all recorded:
    /// what a reader of the whole file records
    /// (`indexes::hand_outs::FileFactsRead`). Not its slots, whose positions
    /// move with every file added before it.
    pub(crate) fn file_fingerprints(&self, id: FileId) -> (u64, u64, u64, u64) {
        let inputs = [
            Input::File(id, Section::Source),
            Input::File(id, Section::Compiler),
            Input::File(id, Section::Entities),
            Input::File(id, Section::Signatures),
        ];
        let [source, compiler, entities, signatures] = {
            let base = self.world().store.base.read().expect("memo base lock");
            inputs.map(|input| {
                let Input::File(_, section) = input else {
                    unreachable!("file sections only")
                };
                base.files.get(&id).map_or(0, |file| {
                    u64::from_le_bytes(file.section(section)[..8].try_into().expect("eight bytes"))
                })
            })
        };
        for input in inputs {
            self.record_input(input);
        }
        (source, compiler, entities, signatures)
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

    /// [`Self::project_files`] for a computation about the file `id` that
    /// reads the rest of the program only through the lookup's recorded
    /// questions ([`crate::base::reads`]: memo answers and what the lookup
    /// hands out, ADR 0408): it records the file's sections and slots, the
    /// build constants, and what `f` recorded, not the whole program. A
    /// whole-table section read (`reads::Section`), a cycle's answer, or a
    /// build under runtime redirects (where the lookup answers callees
    /// unrecorded) reads the whole program ([`Input::Project`]).
    pub(crate) fn project_file_recorded<R>(
        &self,
        id: FileId,
        f: impl for<'x> FnOnce(&crate::indexes::SemanticLookup<'x>, &FileTable) -> R,
    ) -> R {
        if self.constant(Constant::RuntimeRedirects) {
            return self.project_files(f);
        }
        let _ = self.constant(Constant::ProgramClosed);
        for section in SECTIONS {
            self.record_input(Input::File(id, section));
        }
        self.record_input(Input::Slots(id));
        let table = self.world().views.table;
        let access = self
            .world()
            .project
            .expect("a build world with a lookup (`with_project`)");
        let mut f = Some(f);
        let mut out = None;
        access.run(&mut |lookup| {
            if let Some(f) = f.take() {
                out = Some(crate::base::reads::record_detached(|| f(lookup, table)));
            }
        });
        let (value, reads) = out.expect("the lookup ran the closure");
        let (memos, sections) = reads.into_memo_reads();
        // A hazard section is the hazard's answer
        // (`SemanticLookup::section_fingerprint`): its probe stands for it.
        let mut whole = memos
            .iter()
            .any(|read| read.query == super::QueryId(u32::MAX));
        for section in sections {
            match section {
                crate::base::reads::Section::SynchronousDispatch => {
                    let _ = self.hazard(crate::base::Hazard::SynchronousDispatch);
                }
                crate::base::reads::Section::ReflectiveDefinition => {
                    let _ = self.hazard(crate::base::Hazard::ReflectiveDefinitionSpelling);
                }
                // The section's value, polled.
                crate::base::reads::Section::ProjectPrimitives => {
                    let _ = self.probe(
                        super::probes::Probe::ProjectPrimitives,
                        super::probes::PolledKey::Unit,
                    );
                }
                _ => whole = true,
            }
        }
        if whole {
            self.record_input(Input::Project);
        }
        if self.frame.current.is_some() {
            let mut recorded = self.reads.borrow_mut();
            recorded.extend(
                memos
                    .into_iter()
                    .filter(|read| read.query != super::QueryId(u32::MAX))
                    .map(|read| super::Read::Memo(read.query, read.changed_at)),
            );
        }
        value
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
            crate::base::contributions::references,
        )
    }

    /// The spans where `symbol`'s value is used like a returned callback
    /// ([`super::super::contributions::use_spans`]): `(file, slot rank, span)`.
    pub(crate) fn use_spans(&self, symbol: &Arc<str>) -> Vec<(FileId, usize, Span)> {
        self.index_values(
            IndexId::UseSpans,
            symbol,
            |base| &base.use_spans,
            |base| &mut base.use_spans,
            crate::base::contributions::use_spans,
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
            crate::base::contributions::callees,
        )
    }

    /// The uses of the entity symbol `symbol`
    /// ([`super::super::contributions::symbol_uses`]): `(file, slot rank,
    /// use)`, sorted.
    pub(crate) fn symbol_uses(&self, symbol: &str) -> Vec<(FileId, u32, crate::base::SymbolUse)> {
        ranked(self.index_values(
            IndexId::SymbolUses,
            &Arc::from(symbol),
            |base| &base.symbol_uses,
            |base| &mut base.symbol_uses,
            crate::base::contributions::symbol_uses,
        ))
    }

    /// Where `name` is spelled as a property
    /// ([`super::super::contributions::spellings`]): `(file, slot rank,
    /// spelling)`, sorted.
    pub(crate) fn spellings(&self, name: &str) -> Vec<(FileId, u32, crate::base::Spelling)> {
        ranked(self.index_values(
            IndexId::Spellings,
            &Arc::from(name),
            |base| &base.spellings,
            |base| &mut base.spellings,
            crate::base::contributions::spellings,
        ))
    }

    /// Whether some assignment writes the global binding of `name`
    /// ([`super::super::contributions::global_writes`]).
    pub(crate) fn global_written(&self, name: &str) -> bool {
        !self
            .index_values(
                IndexId::GlobalWrites,
                &Arc::from(name),
                |base| &base.global_writes,
                |base| &mut base.global_writes,
                crate::base::contributions::global_writes,
            )
            .is_empty()
    }

    fn index_values<C>(
        &self,
        id: IndexId,
        key: &Arc<str>,
        kept: fn(&MemoBase) -> &KeptIndex<Arc<str>, C>,
        kept_mut: fn(&mut MemoBase) -> &mut KeptIndex<Arc<str>, C>,
        contribute: Contribute<Arc<str>, C>,
    ) -> Vec<(FileId, usize, C)>
    where
        C: Clone + PartialEq + Send + Sync + std::hash::Hash,
    {
        let store = self.world().store;
        let interned = {
            let base = store.base.read().expect("memo base lock");
            let index = kept(&base);
            index
                .index
                .is_some()
                .then(|| index.keys.ids.get(key).copied())
                .flatten()
        };
        let interned = interned.unwrap_or_else(|| {
            let mut base = store.base.write().expect("memo base lock");
            let index = kept_mut(&mut base);
            if index.index.is_none() {
                // A lazy index's first read in the store's life: folded over
                // this build, and updated from it at every later begin.
                index.index = Some(self.world().views.fold(contribute));
            }
            index.keys.intern(key)
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

/// Index values with `u32` slot ranks, sorted: a canonical answer.
fn ranked<C: Ord>(values: Vec<(FileId, usize, C)>) -> Vec<(FileId, u32, C)> {
    let mut values = values
        .into_iter()
        .map(|(file, rank, value)| {
            (
                file,
                u32::try_from(rank).expect("fewer than 2^32 slots of one path"),
                value,
            )
        })
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
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

    /// Ends the build if it read any memo, but leaves the sweep to
    /// [`MemoStore::finish_deferred_end`] (or to the next build's begin,
    /// which runs it first): the store is not read in between, and the answer
    /// the build made does not depend on what the sweep drops. Whether the
    /// build read a memo.
    pub(crate) fn defer_end(&self) -> bool {
        let Some(begin) = self.begun.get() else {
            return false;
        };
        *self
            .store()
            .deferred_end
            .lock()
            .expect("memo deferred end lock") = Some(*begin);
        true
    }
}
