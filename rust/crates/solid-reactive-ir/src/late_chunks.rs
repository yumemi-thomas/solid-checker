//! Per-file chunks of the static prepass's file rules, which were
//! whole-program loops over every file (ADR 0413).
//!
//! Each pass is a function of one file's facts and of what it asks the
//! rest of the program. A file's output is a chunk ([`FileChunks`]) reused
//! while the file is one of the build's `unchanged_paths` (its source, its
//! TypeFacts rows and the symbols they name are the last build's), its
//! compiler map has the same semantics, and every read it recorded answers
//! as it did ([`crate::base::reads`]: memo answers, whole-table sections and
//! what the lookup hands out of other files, ADR 0408). What a pass asks of
//! source discovery's tables is recorded here, entry by entry, with its
//! answer ([`DiscoveryRead`]): props reactivity is decided at call sites in
//! other files, so a moved entry reruns only the chunks that asked it.
//!
//! A path held by two slots is computed for each and kept for none; a
//! one-shot build keeps nothing. A pass that runs inside a recording hands
//! its chunks' reads to it, reused or not.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::core::{SourceHash, SourcePath};
use typefacts::Location;

use crate::identity::SymbolId;
use crate::indexes::SemanticLookup;
use crate::pipeline::AnalysisContext;
use crate::{HashMap, HashSet, ReactiveSourceKind};

/// One file's output of one pass and what computing it read.
struct Chunk<T> {
    source_hash: SourceHash,
    compiler: Arc<solid_facts::compiler::ExecutionMap>,
    proof: crate::base::reads::ReadProof,
    discovery: Vec<(DiscoveryRead, u64)>,
    value: Arc<T>,
}

/// One pass's chunks, by path.
pub(crate) struct FileChunks<T> {
    chunks: HashMap<SourcePath, Chunk<T>>,
}

impl<T> Default for FileChunks<T> {
    fn default() -> Self {
        Self {
            chunks: HashMap::default(),
        }
    }
}

/// The chunks of every chunked pass, kept with the late stages
/// ([`crate::cache::CachedLateStages`]).
#[derive(Default)]
pub(crate) struct LateChunks {
    pub(crate) props_destructure: FileChunks<Vec<crate::StaticDefect>>,
    pub(crate) value_references: FileChunks<Vec<crate::StaticDefect>>,
    pub(crate) result_methods: FileChunks<Vec<crate::StaticDefect>>,
    /// The leaf-owner operations of each file (`cleanup`).
    pub(crate) leaf_operations: FileChunks<Vec<crate::LeafOwnerOperation>>,
    /// The static API checks of each file (`static_api`).
    pub(crate) static_api: FileChunks<crate::static_api::StaticDirectiveFileResult>,
    /// The compatibility pass's diagnostics of each file (`upstream_compat`).
    pub(crate) upstream: FileChunks<crate::upstream_compat::FileDiagnostics>,
}

/// What a pass's chunks are checked against.
pub(crate) struct ChunkInputs<'a, 'l> {
    pub(crate) files: &'a [FileFacts],
    pub(crate) unchanged_paths: &'a HashSet<String>,
    pub(crate) lookup: &'a SemanticLookup<'l>,
    pub(crate) discovery: DiscoveryTables<'a>,
}

impl<'a, 'l> ChunkInputs<'a, 'l> {
    pub(crate) fn of(ctx: &'a AnalysisContext<'l>) -> Self
    where
        'l: 'a,
    {
        Self {
            files: crate::base::reads::file_slots(ctx.facts),
            unchanged_paths: ctx.unchanged_paths,
            lookup: ctx.semantic_lookup,
            discovery: DiscoveryTables::of(ctx),
        }
    }
}

impl<T: Send + Sync + PartialEq> FileChunks<T> {
    /// Each file's output, in file order: the kept chunk where it holds,
    /// computed (recording what it reads) elsewhere.
    pub(crate) fn run(
        cache: Option<&mut Self>,
        inputs: &ChunkInputs<'_, '_>,
        compute: impl Fn(&FileFacts) -> T + Sync,
    ) -> Vec<Arc<T>> {
        let Some(cache) = cache else {
            return crate::pipeline::parallel_file_results(inputs.files, |file| {
                Arc::new(compute(file))
            });
        };
        let mut slots = HashMap::<&str, usize>::default();
        for file in inputs.files {
            *slots.entry(file.path.as_str()).or_default() += 1;
        }
        cache
            .chunks
            .retain(|path, _| slots.get(path.as_str()) == Some(&1));
        let chunks = &cache.chunks;
        let held_once = |file: &FileFacts| slots.get(file.path.as_str()) == Some(&1);
        // Shadow and verify-reused modes compute every kept chunk again: a
        // difference is a read the chunk did not record, named here.
        let store = inputs.lookup.memo_store();
        let check = store.check_solvers();
        // Each worker asks a memo once for its slice (`reads::PassAnswers`).
        let outcomes = crate::pipeline::parallel_slice_with_state(
            inputs.files,
            || crate::base::reads::PassAnswers::new(inputs.lookup),
            |answers, file| {
                if inputs.unchanged_paths.contains(file.path.as_str())
                    && held_once(file)
                    && let Some(chunk) = chunks.get(file.path.as_str())
                    && chunk.source_hash == file.source_hash
                    && (Arc::ptr_eq(&chunk.compiler, &file.compiler)
                        || crate::cache::same_compiler_semantics(&chunk.compiler, &file.compiler))
                    && inputs.discovery.answer_as_recorded(&chunk.discovery)
                    && chunk.proof.holds(answers)
                {
                    if check && crate::base::reads::suppress(|| compute(file)) != *chunk.value {
                        store.poison(&format!(
                            "IR late chunk verify mismatch: the kept chunk of {} differs from a fresh one",
                            file.path.as_str()
                        ));
                    } else {
                        crate::base::reads::replay(&chunk.proof.reads);
                        return Outcome::Kept(Arc::clone(&chunk.value));
                    }
                }
                let ((value, discovery), reads) =
                    crate::base::reads::record(|| record_discovery(|| compute(file)));
                Outcome::Computed(Arc::new(value), reads, discovery)
            },
        );
        inputs
            .files
            .iter()
            .zip(outcomes)
            .map(|(file, outcome)| match outcome {
                Outcome::Kept(value) => value,
                Outcome::Computed(value, reads, discovery) => {
                    if held_once(file) {
                        cache.chunks.insert(
                            file.path.clone(),
                            Chunk {
                                source_hash: file.source_hash.clone(),
                                compiler: Arc::clone(&file.compiler),
                                proof: crate::base::reads::ReadProof::new(reads, inputs.lookup),
                                discovery,
                                value: Arc::clone(&value),
                            },
                        );
                    }
                    value
                }
            })
            .collect()
    }
}

enum Outcome<T> {
    Kept(Arc<T>),
    Computed(Arc<T>, crate::base::reads::Reads, Vec<(DiscoveryRead, u64)>),
}

/// An entry of source discovery's tables a chunk asked.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DiscoveryRead {
    /// `prop_sources[symbol]`.
    PropSource(SymbolId),
    /// `uncertain_prop_sources ∋ symbol`.
    UncertainPropSource(SymbolId),
    /// `source_kinds[symbol]`.
    SourceKind(SymbolId),
    /// The props reactivity of the whole object declared at a location.
    PropsObject(Location),
    /// The props reactivity of named slots of the object declared at a
    /// location.
    PropsNames(Location, Vec<String>),
    /// ADR 0823: whether a destructure of named slots (every slot, for
    /// `None`) may read a pending getter.
    PropsPending(Location, Option<Vec<String>>),
    /// Whether `symbol` names an accessor, a setter or an action: a call of
    /// it opens no leaf scope (`cleanup`).
    SafeCall(SymbolId),
    /// `setters ∋ symbol`.
    Setter(SymbolId),
    /// `actions ∋ symbol`.
    Action(SymbolId),
    /// The callee transport resolved at a location (the interprocedural
    /// pass's `source_call_targets`).
    SourceCallTarget(Location),
    /// `accessors[symbol]`.
    Accessor(SymbolId),
    /// `value_form_stores ∋ symbol`.
    ValueFormStore(SymbolId),
    /// `source_owned_write[symbol]`.
    SourceOwnedWrite(SymbolId),
    /// How many times reachable calls run the callee at a location
    /// (reachability's whole-program answer).
    ReachableCall(Location),
    /// Whether a client build may run: no server build selected, or a client
    /// root in some file (`static_api::context_of`).
    ClientBuild,
    /// One entry of a table read through [`Noted`] or [`NotedSet`].
    Entry(TableId, SymbolId),
    /// The props reactivity of one named slot of the object declared at a
    /// location ([`NotedProps::prop_use`]).
    PropUse(Location, String),
    /// Whether the props object declared at a location has a reactivity row
    /// ([`NotedProps::declared`]).
    PropsDeclared(Location),
    /// [`NotedProps::accessor_value_use`] of one named slot.
    AccessorValueUse(Location, String),
    /// The primitive of a source whose accessor is declared at a location
    /// ([`primitive_declared_at`]).
    PrimitiveDeclaredAt(Location),
    /// A question a pass's own whole-program structure answers
    /// ([`ExtraAnswers`]).
    Extra(ExtraRead),
}

/// A question about a structure a pass builds over the whole program before
/// its chunks run, keyed so a chunk records only what it asked.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ExtraRead {
    /// The compatibility pass's read rows of one path (`ReactiveReadIndex`).
    ReactiveReadsOf(String),
    /// Whether any analyzed parameter reaches an undescribed callee
    /// (`ForwardedUncaptured`).
    ForwardedEmpty,
    /// The undescribed callee a parameter of a function reaches.
    ForwardedReached(String, solid_facts::core::Span, usize),
    /// The source a TypeScript reference at a range names
    /// (`source_reference_locations`).
    SourceReference(String, u64, u64),
    /// The program's `strictNullChecks`.
    StrictNullChecks,
}

/// Answers [`ExtraRead`]s in the current build: the pass that recorded them.
pub(crate) trait ExtraAnswers: Sync {
    fn answer(&self, read: &ExtraRead) -> u64;
}

/// Records, for the chunk computing on this thread, that it asked `read`
/// and was answered `answer`.
pub(crate) fn note_extra(read: ExtraRead, answer: u64) {
    if recording_tables() {
        note(DiscoveryRead::Extra(read), answer);
    }
}

/// The answer a recorded read keeps of `value`.
pub(crate) fn answer_hash(value: &impl Hash) -> u64 {
    answer_of(value)
}

/// The primitive that created a source whose accessor is declared at
/// `declaration`: the first such source the primitive table lists. A whole
/// table scan, recorded as one question with its answer.
pub(crate) fn primitive_declared_at<'a>(
    accessors: Noted<'a, (SymbolId, Location)>,
    primitives: Noted<'a, SymbolId>,
    declaration: &Location,
) -> Option<&'a SymbolId> {
    let answer = primitive_declared_at_in(accessors.map, primitives.map, declaration);
    if recording_tables() {
        note(
            DiscoveryRead::PrimitiveDeclaredAt(declaration.clone()),
            answer_of(&answer),
        );
    }
    answer
}

fn primitive_declared_at_in<'a>(
    accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    primitives: &'a HashMap<SymbolId, SymbolId>,
    declaration: &Location,
) -> Option<&'a SymbolId> {
    primitives.iter().find_map(|(source, primitive)| {
        (accessors.get(source).map(|(_, found)| found) == Some(declaration)).then_some(primitive)
    })
}

/// Source discovery's props reactivity read through recorded questions, as
/// [`Noted`] reads a table.
#[derive(Clone, Copy)]
pub(crate) struct NotedProps<'a> {
    index: &'a crate::source_discovery::PropsReactivityIndex,
}

impl<'a> NotedProps<'a> {
    pub(crate) const fn new(index: &'a crate::source_discovery::PropsReactivityIndex) -> Self {
        Self { index }
    }

    pub(crate) fn prop_use(
        &self,
        declaration: &Location,
        name: &str,
    ) -> crate::source_discovery::PropUse {
        let answer = self.index.prop_use(declaration, name);
        if recording_tables() {
            note(
                DiscoveryRead::PropUse(declaration.clone(), name.to_owned()),
                use_answer(answer),
            );
        }
        answer
    }

    /// Whether the props object declared at `declaration` has a reactivity
    /// row.
    pub(crate) fn declared(&self, declaration: &Location) -> bool {
        let answer = self.index.for_declaration(declaration).is_some();
        if recording_tables() {
            note(
                DiscoveryRead::PropsDeclared(declaration.clone()),
                u64::from(answer),
            );
        }
        answer
    }

    pub(crate) fn accessor_value_use(
        &self,
        declaration: &Location,
        name: &str,
    ) -> crate::source_discovery::PropUse {
        let answer = self.index.accessor_value_use(declaration, name);
        if recording_tables() {
            note(
                DiscoveryRead::AccessorValueUse(declaration.clone(), name.to_owned()),
                use_answer(answer),
            );
        }
        answer
    }
}

/// A table of source discovery's a pass reads by symbol through [`Noted`]
/// or [`NotedSet`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TableId {
    Accessors,
    SourceKinds,
    SourcePrimitives,
    PropSources,
    UncertainPropSources,
    TypedSources,
    Setters,
    Actions,
}

/// Whether a chunk is recording its table reads on this thread.
fn recording_tables() -> bool {
    LOG.with(|log| log.borrow().is_some())
}

/// A table read by symbol whose every lookup a computing chunk records with
/// its answer (the entry, or its absence): the same lookups as the map's,
/// so a pass reads it as it read the map. Outside a chunk nothing is noted.
pub(crate) struct Noted<'a, V> {
    table: TableId,
    map: &'a HashMap<SymbolId, V>,
}

impl<V> Clone for Noted<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> Copy for Noted<'_, V> {}

impl<'a, V: Hash> Noted<'a, V> {
    pub(crate) const fn new(table: TableId, map: &'a HashMap<SymbolId, V>) -> Self {
        Self { table, map }
    }

    pub(crate) fn get(&self, symbol: &str) -> Option<&'a V> {
        let entry = self.map.get(symbol);
        if recording_tables() {
            note(
                DiscoveryRead::Entry(self.table, SymbolId::from(symbol)),
                answer_of(&entry),
            );
        }
        entry
    }

    pub(crate) fn contains_key(&self, symbol: &str) -> bool {
        self.get(symbol).is_some()
    }
}

/// [`Noted`] for a set of symbols.
#[derive(Clone, Copy)]
pub(crate) struct NotedSet<'a> {
    table: TableId,
    set: &'a HashSet<SymbolId>,
}

impl<'a> NotedSet<'a> {
    pub(crate) const fn new(table: TableId, set: &'a HashSet<SymbolId>) -> Self {
        Self { table, set }
    }

    pub(crate) fn contains(&self, symbol: &str) -> bool {
        let answer = self.set.contains(symbol);
        if recording_tables() {
            note(
                DiscoveryRead::Entry(self.table, SymbolId::from(symbol)),
                u64::from(answer),
            );
        }
        answer
    }
}

thread_local! {
    static LOG: RefCell<Option<Vec<(DiscoveryRead, u64)>>> = const { RefCell::new(None) };
}

fn note(read: DiscoveryRead, answer: u64) {
    LOG.with(|log| {
        if let Some(log) = log.borrow_mut().as_mut() {
            log.push((read, answer));
        }
    });
}

/// Runs a chunk, recording its discovery reads, each once.
fn record_discovery<R>(f: impl FnOnce() -> R) -> (R, Vec<(DiscoveryRead, u64)>) {
    let outer = LOG.with(|log| log.replace(Some(Vec::new())));
    let value = f();
    let reads = LOG.with(|log| log.replace(outer)).unwrap_or_default();
    let reads = reads
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    // A chunk computing inside another's hands its reads to it as well.
    LOG.with(|log| {
        if let Some(log) = log.borrow_mut().as_mut() {
            log.extend(reads.iter().cloned());
        }
    });
    (value, reads)
}

fn answer_of(value: &impl Hash) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

fn kind_answer(kind: Option<&ReactiveSourceKind>) -> u64 {
    match kind {
        None => 0,
        Some(ReactiveSourceKind::Accessor) => 1,
        Some(ReactiveSourceKind::Store) => 2,
    }
}

fn use_answer(answer: crate::source_discovery::PropUse) -> u64 {
    match answer {
        crate::source_discovery::PropUse::Static => 0,
        crate::source_discovery::PropUse::Reactive => 1,
        crate::source_discovery::PropUse::Unknown => 2,
    }
}

/// Source discovery's tables as a chunked pass reads them: each entry
/// asked is recorded with its answer.
#[derive(Clone, Copy)]
pub(crate) struct DiscoveryTables<'a> {
    prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    uncertain_prop_sources: &'a HashSet<SymbolId>,
    source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    props_reactivity: &'a crate::source_discovery::PropsReactivityIndex,
    accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    setters: &'a HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            ReactiveSourceKind,
        ),
    >,
    actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// The interprocedural pass's transport targets, for the passes that run
    /// after it; `None` before, where a recorded read of them never holds.
    source_call_targets:
        Option<&'a HashMap<Location, crate::interproc::accessor_provenance::Callable>>,
    value_form_stores: &'a HashSet<SymbolId>,
    source_owned_write: &'a HashMap<SymbolId, crate::OwnedWriteOption>,
    reachable_calls: &'a HashMap<Location, u64>,
    /// The static API pass's client-build answer; `None` elsewhere, where a
    /// recorded read of it never holds.
    client_build: Option<bool>,
    source_primitives: &'a HashMap<SymbolId, SymbolId>,
    typed_sources: &'a HashMap<SymbolId, crate::source_discovery::TypedSource>,
    /// The answers of a pass's own structures; `None` where a recorded
    /// [`ExtraRead`] never holds.
    extra: Option<&'a dyn ExtraAnswers>,
}

impl<'a> DiscoveryTables<'a> {
    pub(crate) fn of(ctx: &AnalysisContext<'a>) -> Self {
        Self {
            prop_sources: ctx.prop_sources,
            uncertain_prop_sources: ctx.uncertain_prop_sources,
            source_kinds: ctx.source_kinds,
            props_reactivity: ctx.props_reactivity,
            accessors: ctx.accessors,
            setters: ctx.setters,
            actions: ctx.actions,
            source_call_targets: None,
            value_form_stores: ctx.value_form_stores,
            source_owned_write: ctx.source_owned_write,
            reachable_calls: ctx.reachable_calls,
            client_build: None,
            source_primitives: ctx.source_primitives,
            typed_sources: ctx.typed_sources,
            extra: None,
        }
    }

    /// These tables with a pass's own answers.
    pub(crate) fn with_extra(mut self, extra: &'a dyn ExtraAnswers) -> Self {
        self.extra = Some(extra);
        self
    }

    /// The answer of one [`DiscoveryRead::Entry`] in these tables.
    fn entry_answer(&self, table: TableId, symbol: &str) -> u64 {
        match table {
            TableId::Accessors => answer_of(&self.accessors.get(symbol)),
            TableId::SourceKinds => answer_of(&self.source_kinds.get(symbol)),
            TableId::SourcePrimitives => answer_of(&self.source_primitives.get(symbol)),
            TableId::PropSources => answer_of(&self.prop_sources.get(symbol)),
            TableId::UncertainPropSources => {
                u64::from(self.uncertain_prop_sources.contains(symbol))
            }
            TableId::TypedSources => answer_of(&self.typed_sources.get(symbol)),
            TableId::Setters => answer_of(&self.setters.get(symbol)),
            TableId::Actions => answer_of(&self.actions.get(symbol)),
        }
    }

    /// These tables with the static API pass's client-build answer.
    pub(crate) fn with_client_build(mut self, client_build: bool) -> Self {
        self.client_build = Some(client_build);
        self
    }

    /// `accessors[symbol]`, recorded.
    pub(crate) fn accessor(&self, symbol: &SymbolId) -> Option<&'a (SymbolId, Location)> {
        let entry = self.accessors.get(symbol);
        note(DiscoveryRead::Accessor(symbol.clone()), answer_of(&entry));
        entry
    }

    /// Whether `symbol` is a value-form store, recorded.
    pub(crate) fn value_form_store(&self, symbol: &SymbolId) -> bool {
        let answer = self.value_form_stores.contains(symbol);
        note(
            DiscoveryRead::ValueFormStore(symbol.clone()),
            u64::from(answer),
        );
        answer
    }

    /// `source_owned_write[symbol]`, recorded.
    pub(crate) fn source_owned_write(&self, symbol: &SymbolId) -> Option<crate::OwnedWriteOption> {
        let entry = self.source_owned_write.get(symbol).copied();
        note(
            DiscoveryRead::SourceOwnedWrite(symbol.clone()),
            answer_of(&entry),
        );
        entry
    }

    /// How many times reachable calls run the callee at `callee`, recorded.
    pub(crate) fn reachable_call(&self, callee: &Location) -> Option<u64> {
        let answer = self.reachable_calls.get(callee).copied();
        note(
            DiscoveryRead::ReachableCall(callee.clone()),
            answer_of(&answer),
        );
        answer
    }

    /// Records that a pass read the client-build answer `answer`; it holds
    /// only where these tables carry the same answer (`with_client_build`).
    pub(crate) fn note_client_build(&self, answer: bool) {
        note(DiscoveryRead::ClientBuild, u64::from(answer));
    }

    /// These tables with the interprocedural pass's transport targets.
    pub(crate) fn with_source_call_targets(
        mut self,
        targets: &'a HashMap<Location, crate::interproc::accessor_provenance::Callable>,
    ) -> Self {
        self.source_call_targets = Some(targets);
        self
    }

    fn safe_call_answer(&self, symbol: &str) -> bool {
        self.accessors.contains_key(symbol)
            || self.setters.contains_key(symbol)
            || self.actions.contains_key(symbol)
    }

    /// Whether `symbol` names an accessor, a setter or an action, recorded.
    pub(crate) fn safe_call(&self, symbol: &SymbolId) -> bool {
        let answer = self.safe_call_answer(symbol);
        note(DiscoveryRead::SafeCall(symbol.clone()), u64::from(answer));
        answer
    }

    /// Whether `symbol` names a setter, recorded.
    pub(crate) fn setter(&self, symbol: &SymbolId) -> bool {
        let answer = self.setters.contains_key(symbol);
        note(DiscoveryRead::Setter(symbol.clone()), u64::from(answer));
        answer
    }

    /// Whether `symbol` names an action, recorded.
    pub(crate) fn action(&self, symbol: &SymbolId) -> bool {
        let answer = self.actions.contains_key(symbol);
        note(DiscoveryRead::Action(symbol.clone()), u64::from(answer));
        answer
    }

    /// The callee transport resolved at `callee`, recorded; nothing (and a
    /// read that never holds) before the interprocedural pass.
    pub(crate) fn source_call_target(
        &self,
        callee: &Location,
    ) -> Option<&'a crate::interproc::accessor_provenance::Callable> {
        let Some(targets) = self.source_call_targets else {
            note(DiscoveryRead::SourceCallTarget(callee.clone()), u64::MAX);
            return None;
        };
        let target = targets.get(callee);
        note(
            DiscoveryRead::SourceCallTarget(callee.clone()),
            answer_of(&target),
        );
        target
    }

    pub(crate) fn prop_source(&self, symbol: &SymbolId) -> Option<&'a (SymbolId, Location)> {
        let entry = self.prop_sources.get(symbol);
        note(DiscoveryRead::PropSource(symbol.clone()), answer_of(&entry));
        entry
    }

    pub(crate) fn uncertain_prop_source(&self, symbol: &SymbolId) -> bool {
        let uncertain = self.uncertain_prop_sources.contains(symbol);
        note(
            DiscoveryRead::UncertainPropSource(symbol.clone()),
            u64::from(uncertain),
        );
        uncertain
    }

    pub(crate) fn source_kind(&self, symbol: &SymbolId) -> Option<&'a ReactiveSourceKind> {
        let kind = self.source_kinds.get(symbol);
        note(DiscoveryRead::SourceKind(symbol.clone()), kind_answer(kind));
        kind
    }

    pub(crate) fn props_object_use(
        &self,
        declaration: &Location,
    ) -> crate::source_discovery::PropUse {
        let answer = self.props_reactivity.object_use(declaration);
        note(
            DiscoveryRead::PropsObject(declaration.clone()),
            use_answer(answer),
        );
        answer
    }

    /// [`crate::source_discovery::PropsReactivityIndex::pending_answer`],
    /// recorded.
    pub(crate) fn props_pending(&self, declaration: &Location, names: Option<Vec<String>>) -> u64 {
        let answer = self
            .props_reactivity
            .pending_answer(declaration, names.as_deref());
        note(
            DiscoveryRead::PropsPending(declaration.clone(), names),
            answer,
        );
        answer
    }

    pub(crate) fn props_names_use<'n>(
        &self,
        declaration: &Location,
        names: impl Iterator<Item = &'n str>,
    ) -> crate::source_discovery::PropUse {
        let names = names.map(str::to_owned).collect::<Vec<_>>();
        let answer = self
            .props_reactivity
            .names_use(declaration, names.iter().map(String::as_str));
        note(
            DiscoveryRead::PropsNames(declaration.clone(), names),
            use_answer(answer),
        );
        answer
    }

    /// Whether every recorded entry answers as it did.
    fn answer_as_recorded(&self, reads: &[(DiscoveryRead, u64)]) -> bool {
        reads.iter().all(|(read, answer)| {
            let current = match read {
                DiscoveryRead::PropSource(symbol) => answer_of(&self.prop_sources.get(symbol)),
                DiscoveryRead::UncertainPropSource(symbol) => {
                    u64::from(self.uncertain_prop_sources.contains(symbol))
                }
                DiscoveryRead::SourceKind(symbol) => kind_answer(self.source_kinds.get(symbol)),
                DiscoveryRead::PropsObject(declaration) => {
                    use_answer(self.props_reactivity.object_use(declaration))
                }
                DiscoveryRead::PropsNames(declaration, names) => use_answer(
                    self.props_reactivity
                        .names_use(declaration, names.iter().map(String::as_str)),
                ),
                DiscoveryRead::PropsPending(declaration, names) => self
                    .props_reactivity
                    .pending_answer(declaration, names.as_deref()),
                DiscoveryRead::SafeCall(symbol) => u64::from(self.safe_call_answer(symbol)),
                DiscoveryRead::Setter(symbol) => u64::from(self.setters.contains_key(symbol)),
                DiscoveryRead::Action(symbol) => u64::from(self.actions.contains_key(symbol)),
                // Recorded before the pass's targets existed: never holds.
                DiscoveryRead::SourceCallTarget(callee) => match self.source_call_targets {
                    Some(targets) => answer_of(&targets.get(callee)),
                    None => return false,
                },
                DiscoveryRead::Accessor(symbol) => answer_of(&self.accessors.get(symbol)),
                DiscoveryRead::ValueFormStore(symbol) => {
                    u64::from(self.value_form_stores.contains(symbol))
                }
                DiscoveryRead::SourceOwnedWrite(symbol) => {
                    answer_of(&self.source_owned_write.get(symbol).copied())
                }
                DiscoveryRead::ReachableCall(callee) => {
                    answer_of(&self.reachable_calls.get(callee).copied())
                }
                DiscoveryRead::ClientBuild => match self.client_build {
                    Some(client_build) => u64::from(client_build),
                    None => return false,
                },
                DiscoveryRead::Entry(table, symbol) => self.entry_answer(*table, symbol),
                DiscoveryRead::PropUse(declaration, name) => {
                    use_answer(self.props_reactivity.prop_use(declaration, name))
                }
                DiscoveryRead::PropsDeclared(declaration) => {
                    u64::from(self.props_reactivity.for_declaration(declaration).is_some())
                }
                DiscoveryRead::AccessorValueUse(declaration, name) => {
                    use_answer(self.props_reactivity.accessor_value_use(declaration, name))
                }
                DiscoveryRead::PrimitiveDeclaredAt(declaration) => answer_of(
                    &primitive_declared_at_in(self.accessors, self.source_primitives, declaration),
                ),
                DiscoveryRead::Extra(read) => match self.extra {
                    Some(extra) => extra.answer(read),
                    None => return false,
                },
            };
            current == *answer
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(start: u64) -> Location {
        Location {
            path: "/p/a.ts".into(),
            start_byte: start,
            end_byte: start + 1,
        }
    }

    /// Each table entry a chunk asks is recorded with its answer, and the
    /// record holds exactly while every entry answers the same: an entry
    /// added, a reachability count moved, a transport target or the
    /// client-build answer absent where it was asked fail it.
    #[test]
    fn recorded_table_reads_hold_until_an_asked_entry_moves() {
        let symbol = SymbolId::from("s");
        let other = SymbolId::from("t");
        let empty_props = HashMap::default();
        let empty_set = HashSet::default();
        let kinds = HashMap::default();
        let props = crate::source_discovery::PropsReactivityIndex::default();
        let empty_primitives = HashMap::default();
        let empty_typed = HashMap::default();
        let mut accessors = HashMap::default();
        accessors.insert(symbol.clone(), (symbol.clone(), location(1)));
        let setters = HashMap::default();
        let actions = HashMap::default();
        let owned = HashMap::default();
        let reachable = HashMap::from_iter([(location(5), 2)]);
        let targets = HashMap::default();
        let tables = |actions, reachable| DiscoveryTables {
            prop_sources: &empty_props,
            uncertain_prop_sources: &empty_set,
            source_kinds: &kinds,
            props_reactivity: &props,
            accessors: &accessors,
            setters: &setters,
            actions,
            source_call_targets: None,
            value_form_stores: &empty_set,
            source_owned_write: &owned,
            reachable_calls: reachable,
            client_build: None,
            source_primitives: &empty_primitives,
            typed_sources: &empty_typed,
            extra: None,
        };
        let before = tables(&actions, &reachable)
            .with_source_call_targets(&targets)
            .with_client_build(true);
        let ((), reads) = record_discovery(|| {
            assert!(before.safe_call(&symbol));
            assert!(!before.action(&other));
            assert!(!before.setter(&symbol));
            assert_eq!(before.reachable_call(&location(5)), Some(2));
            assert!(before.source_call_target(&location(7)).is_none());
            assert!(before.accessor(&other).is_none());
            before.note_client_build(true);
        });
        assert!(before.answer_as_recorded(&reads));
        // Without the transport targets or the client-build answer the reads
        // that asked them do not hold.
        assert!(!tables(&actions, &reachable).answer_as_recorded(&reads));
        let flipped = before.with_client_build(false);
        assert!(!flipped.answer_as_recorded(&reads));
        // An action added under an asked symbol, or a count moved.
        let mut with_action = actions.clone();
        with_action.insert(other.clone(), (other.clone(), location(3)));
        let added = tables(&with_action, &reachable)
            .with_source_call_targets(&targets)
            .with_client_build(true);
        assert!(!added.answer_as_recorded(&reads));
        let mut recounted = reachable.clone();
        recounted.insert(location(5), 3);
        let moved = tables(&actions, &recounted)
            .with_source_call_targets(&targets)
            .with_client_build(true);
        assert!(!moved.answer_as_recorded(&reads));
        let same = tables(&actions, &reachable)
            .with_source_call_targets(&targets)
            .with_client_build(true);
        assert!(same.answer_as_recorded(&reads));
    }
}
