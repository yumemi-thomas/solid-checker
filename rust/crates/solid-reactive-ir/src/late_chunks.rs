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
            files: &ctx.facts.files,
            unchanged_paths: ctx.unchanged_paths,
            lookup: ctx.semantic_lookup,
            discovery: DiscoveryTables::of(ctx),
        }
    }
}

impl<T: Send + Sync> FileChunks<T> {
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
        let outcomes = crate::pipeline::parallel_slice_results(inputs.files, |file| {
            if inputs.unchanged_paths.contains(file.path.as_str())
                && held_once(file)
                && let Some(chunk) = chunks.get(file.path.as_str())
                && chunk.source_hash == file.source_hash
                && (Arc::ptr_eq(&chunk.compiler, &file.compiler)
                    || crate::cache::same_compiler_semantics(&chunk.compiler, &file.compiler))
                && inputs.discovery.answer_as_recorded(&chunk.discovery)
                && chunk.proof.holds(inputs.lookup)
            {
                crate::base::reads::replay(&chunk.proof.reads);
                return Outcome::Kept(Arc::clone(&chunk.value));
            }
            let ((value, discovery), reads) =
                crate::base::reads::record(|| record_discovery(|| compute(file)));
            Outcome::Computed(Arc::new(value), reads, discovery)
        });
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
}

impl<'a> DiscoveryTables<'a> {
    pub(crate) fn of(ctx: &AnalysisContext<'a>) -> Self {
        Self {
            prop_sources: ctx.prop_sources,
            uncertain_prop_sources: ctx.uncertain_prop_sources,
            source_kinds: ctx.source_kinds,
            props_reactivity: ctx.props_reactivity,
        }
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
            };
            current == *answer
        })
    }
}
