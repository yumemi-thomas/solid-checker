//! Symbol identity of one project (Go: `idFor`, `symbolFor`,
//! `canonicalSymbol`, `collectExportedIdentities`, `sweepDurableIdentities`
//! and the maps beside them in `tsgo/project.go`).
//!
//! A binder-level symbol's wire id is a pure function of the program (ADR
//! 0366): the hash of its [`Key`] (`binder_key.rs`), which does not depend on
//! which symbol asked first, on the checker's history, or on file order. So is
//! a multi-origin property's (a union, intersection or spread property whose
//! origins all have binder keys and fix its type, ADR 0371): the hash of its
//! `Set` key. Any
//! other symbol the checker derived (an instantiation, a transient clone, a
//! mapped property) or whose binder-ness cannot be proven gets a
//! generation-scoped counter id (`symbol:<generation>:<n>`) that means nothing
//! outside the generation that issued it.
//!
//! # Ownership
//!
//! The maps hold tsrs symbol handles and addresses, which are valid only for
//! the generation (program chain and checker) that produced them
//! (`unsafe_boundary` invariant 1). Every operation that replaces or frees a
//! generation clears them first: an accepted update ([`Identities::after_update`]),
//! a rebase and `ReleaseAnalysisState` ([`Identities::release_pointers`]), and
//! close. Only pointer-free state survives: `keys` (the rehydration boundary)
//! and `exported_by_ref`.
//!

use crate::binder_key::{Key, binder_key, binder_root, global_symbol, intrinsic_symbol, set_key};
use crate::durable::{DurableRef, minimum_declaration_ref};
use crate::gocompat::clean;
use crate::model::SymbolId;
use crate::nodes::node_at;
use crate::shape::{ExportedSymbol, exported_symbols_with};
use crate::unsafe_boundary::CheckerLease;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use tsrs_ast::{Node, SourceFile, Symbol, SymbolFlags, is_external_module};
use tsrs_compiler::{Checker, Program};
use tsrs_core::P;

/// A module-visible symbol's span-insensitive identity (Go:
/// `preservedExportIdentity`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preserved {
    pub id: SymbolId,
    pub reference: DurableRef,
}

/// Go's `declarationExportIdentities`/`currentExports`: a symbol of the
/// accepted candidate generation and the identity its export slot keeps.
pub type CurrentExports = Vec<(P<Symbol>, Preserved)>;

/// One direct write to a symbol (Go: the cases of `runtimeSymbolKindLocked`'s
/// `visit`). `Value` is evaluated lazily, under the evaluation's own cycle
/// guard, so the index holds no kinds.
#[derive(Clone, Copy)]
pub enum WriteSite {
    /// `x = value`, `x ||= value`, ...: the kind of `value`.
    Value(P<Node>),
    /// Arithmetic/bitwise compound assignment and `++`/`--`.
    NonCallable,
    /// Any other direct write (destructuring, `for-in/of` target, ...).
    Open,
}

/// Canonical symbol address to its writes in one file, in source order.
pub type WriteIndex = HashMap<usize, Vec<WriteSite>>;

#[derive(Default)]
pub struct Identities {
    /// The accepted program generation's number (Go: `project.generation`).
    pub generation: u64,
    next_symbol: u64,
    /// Symbol address to its id: a memo of a pure function for a binder-level
    /// symbol, the only record of a counter id.
    ids_by_symbol: HashMap<usize, SymbolId>,
    /// Id to the symbol it names, so a counter (and a binder id) answers
    /// without re-resolving. Not a claim table: several symbols may map to one
    /// binder id (a pre-merge symbol and its merged symbol), and the entry is
    /// always the binder-level root. Several multi-origin properties may share
    /// a `Set` id (one per containing type); the entry is the first minted,
    /// which only answers what all of them share (declarations, references).
    symbols_by_id: HashMap<SymbolId, P<Symbol>>,
    /// Go: `durableRefs`. Maps a binder id back to its key so the symbol can be
    /// re-resolved lazily after a release.
    keys: HashMap<SymbolId, Arc<Key>>,
    /// The module-visible symbols' declaration references (Go:
    /// `exportedIdentitiesByRef`): pointer-free, so it also answers after a
    /// release.
    exported_by_ref: Arc<HashMap<DurableRef, Preserved>>,
    /// Go: `filesByName`. `Program.GetSourceFile` does not round-trip virtual
    /// bundled-lib names, so durable re-resolution of lib-declared symbols
    /// falls back to this index.
    files_by_name: Option<HashMap<String, P<SourceFile>>>,
    /// Go: `runtimePaths`. Filesystem-resolved declaration paths (symlinks
    /// followed). A path's resolution is a file-system fact, not a checker
    /// one, so it is kept for as long as the program chain trusts the file
    /// system: across releases and parse-reusing updates, which reuse every
    /// other file and its module resolutions as read, and dropped by any
    /// rebuild, which reads the file system again (`Realpath`, ADR 0369).
    runtime_paths: HashMap<String, String>,
    /// Write sites per declaring file (by file address), built on first use by
    /// `runtime_facts`. Holds node and symbol addresses: generation-scoped like
    /// `ids_by_symbol`, cleared at every point that clears it.
    pub(crate) write_indexes: HashMap<usize, Rc<WriteIndex>>,
    /// Derived keys: memoized acyclic type keys (with the depth each reaches)
    /// by type address, and the tuple index; address-keyed, so
    /// generation-scoped like `ids_by_symbol` and cleared with it.
    pub(crate) type_keys: HashMap<usize, (Option<crate::derived_key::Digest32>, u32)>,
    pub(crate) tuple_index: crate::derived_key::TupleIndex,
    /// Measurement only (`derived_key::ENV_STATS`): the derivation pointers
    /// and the symbol each derived id was first minted with in this pass, and
    /// why the last symbol had no key.
    pub(crate) derived_signatures: HashMap<SymbolId, Vec<usize>>,
    pub(crate) derived_first: HashMap<SymbolId, P<Symbol>>,
    pub(crate) last_failure: Option<String>,
    /// Derived symbols take exact keys (`derived_key.rs`), and a symbol with
    /// no key carries no id at all instead of a counter (instant-edits P2).
    /// Chosen at open; never changes for a project.
    derived_keys: bool,
}

/// The pointer-free start of a pass's identity state ([`Identities::pass_seed`]).
#[derive(Clone)]
pub(crate) struct PassSeed {
    generation: u64,
    exported_by_ref: Arc<HashMap<DurableRef, Preserved>>,
    runtime_paths: HashMap<String, String>,
    derived_keys: bool,
}

/// What an identity question needs: the checker and program of the generation
/// the symbols come from, and the identity state.
pub struct Env<'a> {
    pub checker: &'a mut CheckerLease,
    pub program: &'static Program,
    pub ids: &'a mut Identities,
    /// Digests of the program inputs retained answers depend on (ADR 0369).
    pub inputs: &'a mut crate::inputs::ProgramInputs,
}

fn same_symbol(left: P<Symbol>, right: P<Symbol>) -> bool {
    left.addr() == right.addr()
}

impl Identities {
    #[must_use]
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            ..Self::default()
        }
    }

    /// Selects exact derived-symbol keys (`derived_key::ENV_FLAG`).
    #[must_use]
    pub fn with_derived_keys(mut self, derived_keys: bool) -> Self {
        self.derived_keys = derived_keys;
        self
    }

    #[cfg(test)]
    pub(crate) fn select_derived_keys_for_test(&mut self) {
        self.derived_keys = true;
    }

    /// Whether derived symbols take exact keys and an unkeyed symbol no id.
    #[must_use]
    pub fn derived_keys(&self) -> bool {
        self.derived_keys
    }

    /// The declaration references of the binder ids held (tests).
    pub fn durable_refs(&self) -> impl Iterator<Item = &DurableRef> {
        self.keys.values().filter_map(|key| key.reference())
    }

    /// Whether any checker-owned map holds an entry (tests).
    #[must_use]
    pub fn holds_symbols(&self) -> bool {
        !self.ids_by_symbol.is_empty() || !self.symbols_by_id.is_empty()
    }

    #[must_use]
    pub fn exported_by_ref(&self) -> &HashMap<DurableRef, Preserved> {
        &self.exported_by_ref
    }

    /// The key a binder id was minted from in this generation. `None` for a
    /// counter and for a multi-origin property's `Set` id: no binder-level
    /// answer may hold one across an update (ADR 0369, 0371).
    #[must_use]
    pub(crate) fn key_of_id(&self, id: &SymbolId) -> Option<&Arc<Key>> {
        self.keys.get(id).filter(|key| key.is_binder())
    }

    /// Whether `id` is a multi-origin property's `Set` id (ADR 0371).
    #[must_use]
    pub(crate) fn is_set_id(&self, id: &SymbolId) -> bool {
        self.keys.get(id).is_some_and(|key| !key.is_binder())
    }

    /// Makes a retained binder id answerable in this generation without
    /// minting it: `symbol_for` re-resolves it lazily through its key, as it
    /// does for an id minted before a release. `false` when the id already
    /// names another key (a collision): the caller must not reuse it.
    pub(crate) fn register_key(&mut self, id: &SymbolId, key: &Arc<Key>) -> bool {
        if !key.is_binder() {
            return false;
        }
        match self.keys.get(id) {
            Some(held) => held == key,
            None => {
                self.keys.insert(Arc::clone(id), Arc::clone(key));
                true
            }
        }
    }

    /// Go: `ReleaseAnalysisState`'s identity half, and the rebase policy's:
    /// drops every pointer-keyed map. Durable refs and the by-ref index are
    /// the rehydration boundary and stay.
    pub fn release_pointers(&mut self) {
        self.ids_by_symbol.clear();
        self.symbols_by_id.clear();
        self.files_by_name = None;
        self.write_indexes.clear();
        self.type_keys.clear();
        self.derived_signatures.clear();
        self.derived_first.clear();
        self.tuple_index = crate::derived_key::TupleIndex::default();
        self.next_symbol = 0;
    }

    /// A file checker's pass ended (per-file checkers, `unsafe_boundary`
    /// invariant 8): drops every map that may hold a handle into the checker's
    /// region, which is freed now. `ids_by_symbol` and `symbols_by_id` hold
    /// derived symbols and the checker's merged symbols (both in its region),
    /// and binder symbols only as a memo of what `keys` re-resolves;
    /// `write_indexes` holds canonical (possibly merged) symbol addresses. An
    /// address in a freed region is reused by the next pass, so an entry left
    /// behind could name another symbol. The counter keeps running, so a
    /// counter id is never issued twice in a generation: a counter id of an
    /// earlier pass stops resolving (fails closed), it never names a later
    /// pass's symbol. `files_by_name` holds program files and stays. The
    /// derived-key memos (type keys, first symbols, pointer signatures, tuple
    /// index) are address-keyed into the region too; a derived id's `keys`
    /// entry is address-free and stays, so the next pass mints the same id.
    pub fn purge_scope_pointers(&mut self) {
        self.ids_by_symbol.clear();
        self.symbols_by_id.clear();
        self.write_indexes.clear();
        self.type_keys.clear();
        self.derived_signatures.clear();
        self.derived_first.clear();
        self.tuple_index = crate::derived_key::TupleIndex::default();
    }

    /// What a pass's own identity state starts from (per-file passes,
    /// instant-edits P3): the project's pointer-free inputs (the derived-key
    /// mode, the exported identities, the resolved runtime paths) and no key.
    /// `Send`, so a worker thread builds its state from it
    /// ([`Identities::from_seed`]).
    #[must_use]
    pub(crate) fn pass_seed(&self) -> PassSeed {
        PassSeed {
            generation: self.generation,
            exported_by_ref: Arc::clone(&self.exported_by_ref),
            runtime_paths: self.runtime_paths.clone(),
            derived_keys: self.derived_keys,
        }
    }

    /// A pass's identity state. A pass mints into its own `keys`, so what it
    /// emits is a function of what its checker read, never of which pass,
    /// table or scratch question minted an id first; the caller merges the
    /// keys afterwards ([`Identities::take_keys`]).
    #[must_use]
    pub(crate) fn from_seed(seed: PassSeed) -> Identities {
        Identities {
            generation: seed.generation,
            exported_by_ref: seed.exported_by_ref,
            runtime_paths: seed.runtime_paths,
            derived_keys: seed.derived_keys,
            ..Identities::default()
        }
    }

    /// The keys minted since the last call, in id order (a pass's output).
    pub(crate) fn take_keys(&mut self) -> Vec<(SymbolId, Arc<Key>)> {
        let mut keys: Vec<(SymbolId, Arc<Key>)> =
            std::mem::take(&mut self.keys).into_iter().collect();
        keys.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        keys
    }

    /// The key `id` names in this generation, if any.
    #[must_use]
    pub(crate) fn key(&self, id: &SymbolId) -> Option<&Arc<Key>> {
        self.keys.get(id)
    }

    /// Records `key` for `id` (the merge of a pass's keys; the caller checked
    /// that `id` names no other key).
    pub(crate) fn insert_key(&mut self, id: &SymbolId, key: &Arc<Key>) {
        self.keys
            .entry(Arc::clone(id))
            .or_insert_with(|| Arc::clone(key));
    }

    /// The runtime paths a pass state resolved (to merge back).
    pub(crate) fn take_runtime_paths(&mut self) -> HashMap<String, String> {
        std::mem::take(&mut self.runtime_paths)
    }

    /// Merges runtime paths a pass state resolved (a memo of a file-system
    /// answer: every pass resolves a path the same way).
    pub(crate) fn absorb_runtime_paths(&mut self, paths: HashMap<String, String>) {
        for (path, resolved) in paths {
            self.runtime_paths.entry(path).or_insert(resolved);
        }
    }

    /// The program chain was rebuilt: every file and module resolution was
    /// read again, so resolved runtime paths are too.
    pub fn forget_runtime_paths(&mut self) {
        self.runtime_paths.clear();
    }

    /// Go: `Close`.
    pub fn close(&mut self) {
        self.release_pointers();
        self.runtime_paths.clear();
        self.keys.clear();
        self.exported_by_ref = Arc::default();
    }

    /// Seeds the exported identities at open (Go: `OpenProject`).
    pub fn seed_exported(&mut self, program: &Program, checker: &mut Checker) {
        self.exported_by_ref = Arc::new(collect_exported_identities(program, checker));
    }

    /// Go: the identity half of `Update` after a candidate is accepted. Nothing
    /// derived from the preceding generation survives: the claims and the
    /// durable references restart empty, and the exported identities are
    /// collected again from the candidate program, so an id minted before the
    /// update fails closed until the new generation mints it, or a retained
    /// binder-level answer holding it proves its dependencies and registers its
    /// key (ADR 0369). (The exports the update already derived, `current`, only
    /// feed its cutoff decisions.)
    pub fn after_update(
        &mut self,
        generation: u64,
        program: &Program,
        checker: &mut Checker,
        _incremental_path: Option<&str>,
        _current: Option<CurrentExports>,
    ) {
        self.generation = generation;
        self.ids_by_symbol.clear();
        self.symbols_by_id.clear();
        self.files_by_name = None;
        self.write_indexes.clear();
        self.type_keys.clear();
        self.derived_signatures.clear();
        self.derived_first.clear();
        self.tuple_index = crate::derived_key::TupleIndex::default();
        self.next_symbol = 0;
        self.keys.clear();
        self.seed_exported(program, checker);
    }
}

/// Go: `collectExportedIdentities`, keyed by declaration reference. A module's
/// export enters only when it is a proven binder-level symbol, under the same
/// minimum-declaration reference its id hashes; two symbols sharing a
/// reference are not safe to canonicalize and neither enters.
pub fn collect_exported_identities(
    program: &Program,
    checker: &mut Checker,
) -> HashMap<DurableRef, Preserved> {
    let mut identities: HashMap<DurableRef, (usize, Preserved)> = HashMap::new();
    let mut conflicted: HashSet<DurableRef> = HashSet::new();
    for file in program.source_files() {
        if !is_external_module(*file) {
            continue;
        }
        let Some(exports) = exported_symbols_with(checker, *file) else {
            continue;
        };
        for ExportedSymbol { symbol, .. } in exports {
            let Some(root) = binder_root(checker, symbol) else {
                continue;
            };
            let Some(reference) = minimum_declaration_ref(root) else {
                continue;
            };
            if conflicted.contains(&reference) {
                continue;
            }
            if let Some((existing, _)) = identities.get(&reference)
                && *existing != root.addr()
            {
                identities.remove(&reference);
                conflicted.insert(reference);
                continue;
            }
            let id: SymbolId = Arc::from(reference.exported_id().as_str());
            identities.insert(
                reference.clone(),
                (root.addr(), Preserved { id, reference }),
            );
        }
    }
    identities
        .into_iter()
        .map(|(reference, (_, preserved))| (reference, preserved))
        .collect()
}

impl Env<'_> {
    /// The key of a binder-level `symbol` and the root it names, or `None` for
    /// a derived or unproven symbol (which takes a counter).
    fn key_of(&self, symbol: P<Symbol>) -> Option<(Key, P<Symbol>)> {
        let exported = &self.ids.exported_by_ref;
        binder_key(self.checker, symbol, |reference| {
            exported.contains_key(reference)
        })
    }

    /// The `Set` key of a multi-origin property (ADR 0371).
    fn set_key_of(&self, symbol: P<Symbol>) -> Option<Key> {
        let exported = &self.ids.exported_by_ref;
        set_key(self.checker, symbol, |reference| {
            exported.contains_key(reference)
        })
    }

    /// Go: `idFor`. A binder-level symbol's id is the hash of its key, a
    /// multi-origin property's the hash of its `Set` key; any other symbol
    /// takes the next counter of the generation.
    pub fn id_for(&mut self, symbol: P<Symbol>) -> SymbolId {
        let address = symbol.addr();
        if let Some(id) = self.ids.ids_by_symbol.get(&address) {
            return Arc::clone(id);
        }
        if let Some(key) = self.set_key_of(symbol) {
            let id: SymbolId = Arc::from(key.id().as_str());
            self.ids.last_failure = Some("set-collision".into());
            if self.ids.keys.get(&id).is_none_or(|held| **held == key) {
                self.ids
                    .keys
                    .entry(Arc::clone(&id))
                    .or_insert_with(|| Arc::new(key));
                self.ids
                    .symbols_by_id
                    .entry(Arc::clone(&id))
                    .or_insert(symbol);
                self.ids.ids_by_symbol.insert(address, Arc::clone(&id));
                return id;
            }
        } else if let Some((key, root)) = self.key_of(symbol) {
            let id: SymbolId = Arc::from(key.id().as_str());
            self.ids.last_failure = Some(format!(
                "binder-collision held={:?} new={key:?}",
                self.ids.keys.get(&id)
            ));
            // Two keys spelling one id (a 96-bit collision) would conflate two
            // symbols: the later one fails closed to a counter.
            if self.ids.keys.get(&id).is_none_or(|held| **held == key) {
                self.ids
                    .keys
                    .entry(Arc::clone(&id))
                    .or_insert_with(|| Arc::new(key));
                self.ids
                    .symbols_by_id
                    .entry(Arc::clone(&id))
                    .or_insert(root);
                self.ids.ids_by_symbol.insert(root.addr(), Arc::clone(&id));
                self.ids.ids_by_symbol.insert(address, Arc::clone(&id));
                return id;
            }
        }
        if self.ids.derived_keys {
            // No exact key: no id. An absent symbol fails closed downstream;
            // a counter would split one symbol across checkers (and, in
            // per-file mode, across passes).
            let id = self.derived_id(symbol).unwrap_or_else(|| {
                if let Some(path) = std::env::var_os(crate::derived_key::ENV_STATS) {
                    let reason = self.ids.last_failure.take().unwrap_or_default();
                    crate::derived_key::record(
                        &path,
                        &format!(
                            "nosymbol name={} flags={:?} check={:?} decls={} :: {reason}",
                            symbol.name(),
                            symbol.flags(),
                            symbol.check_flags(),
                            symbol.declarations().len()
                        ),
                    );
                }
                SymbolId::from("")
            });
            self.ids.ids_by_symbol.insert(address, Arc::clone(&id));
            return id;
        }
        self.ids.next_symbol += 1;
        let id: SymbolId =
            Arc::from(format!("symbol:{}:{}", self.ids.generation, self.ids.next_symbol).as_str());
        if let Some(path) = std::env::var_os(crate::derived_key::ENV_STATS) {
            let reason = self.ids.last_failure.take().unwrap_or_default();
            crate::derived_key::record(
                &path,
                &format!(
                    "counter {id} name={} flags={:?} check={:?} decls={} :: {reason}",
                    symbol.name(),
                    symbol.flags(),
                    symbol.check_flags(),
                    symbol.declarations().len()
                ),
            );
        }
        self.ids.ids_by_symbol.insert(address, Arc::clone(&id));
        self.ids.symbols_by_id.insert(Arc::clone(&id), symbol);
        id
    }

    /// A derived symbol's exact key (`derived_key.rs`, instant-edits P2), or
    /// `None`: no key (the caller gives the symbol no id). With
    /// [`crate::derived_key::ENV_STATS`] set, every decision is appended to
    /// that file, and an id minted again in this pass for a symbol whose
    /// derivation pointers differ is checked for type identity (injectivity).
    fn derived_id(&mut self, symbol: P<Symbol>) -> Option<SymbolId> {
        let stats = std::env::var_os(crate::derived_key::ENV_STATS);
        let (key, outcome) = match crate::derived_key::binder_fallback(symbol) {
            Some(key) => (key, crate::derived_key::Outcome::BinderFallback),
            None => {
                let exported = &self.ids.exported_by_ref;
                let checker: &mut tsrs_compiler::Checker = &mut *self.checker;
                let mut memo = std::mem::take(&mut self.ids.type_keys);
                let mut tuples = std::mem::take(&mut self.ids.tuple_index);
                let mut keyer = crate::derived_key::Keyer::new(
                    checker,
                    |reference: &crate::durable::DurableRef| exported.contains_key(reference),
                    &mut memo,
                    &mut tuples,
                );
                let (key, outcome) = keyer.derived(symbol, 0);
                let failure = keyer.failure.take();
                if let Some(path) = &stats {
                    if let (Some(dump), Some(key)) = (keyer.dump.take(), &key) {
                        crate::derived_key::record(
                            path,
                            &format!("mapperdump {} {dump}", key.id()),
                        );
                    }
                    if keyer.cyclic_work > 50 {
                        crate::derived_key::record(
                            path,
                            &format!("cyclicwork {}", keyer.cyclic_work),
                        );
                    }
                }
                drop(keyer);
                self.ids.type_keys = memo;
                self.ids.tuple_index = tuples;
                let Some(key) = key else {
                    if let Some(path) = &stats {
                        let reason = format!(
                            "{outcome:?} {} {}",
                            crate::derived_key::failure_reason(&mut *self.checker, symbol),
                            failure.as_deref().unwrap_or("-")
                        );
                        crate::derived_key::record(path, &format!("none {reason}"));
                        self.ids.last_failure = Some(reason);
                    }
                    return None;
                };
                (key, outcome)
            }
        };
        let id: SymbolId = Arc::from(key.id().as_str());
        if let Some(path) = &stats {
            self.record_derived(path, symbol, &id, &key, outcome);
        }
        // An instantiation the root answers for is the root: its id is the
        // root's own. The root is minted here too, so the id resolves (to the
        // root) in this pass whether or not anything named the root itself.
        if matches!(&key, Key::Derived { tag: "root", .. }) {
            let root = self
                .checker
                .value_symbol_links
                .try_get_if_id_assigned(symbol)
                .and_then(|links| links.target())?;
            if self.id_for(root) != id {
                self.ids.last_failure = Some("root-mismatch".into());
                return None;
            }
            self.ids
                .ids_by_symbol
                .insert(symbol.addr(), Arc::clone(&id));
            return Some(id);
        }
        // Two keys spelling one id (a 96-bit collision): no id.
        if self.ids.keys.get(&id).is_some_and(|held| **held != key) {
            self.ids.last_failure = Some(format!(
                "derived-collision held={:?} new={key:?}",
                self.ids.keys.get(&id)
            ));
            return None;
        }
        self.ids
            .keys
            .entry(Arc::clone(&id))
            .or_insert_with(|| Arc::new(key));
        self.ids
            .symbols_by_id
            .entry(Arc::clone(&id))
            .or_insert(symbol);
        self.ids
            .ids_by_symbol
            .insert(symbol.addr(), Arc::clone(&id));
        Some(id)
    }

    /// Measurement only ([`crate::derived_key::ENV_STATS`]): the key, and
    /// whether this pass already minted `id` for a symbol whose derivation
    /// pointers differ (then: are the two types identical both ways).
    fn record_derived(
        &mut self,
        path: &std::ffi::OsStr,
        symbol: P<Symbol>,
        id: &SymbolId,
        key: &Key,
        outcome: crate::derived_key::Outcome,
    ) {
        if let Key::Derived {
            tag,
            name,
            root,
            context,
        } = key
        {
            let context: String = context.iter().take(4).map(|b| format!("{b:02x}")).collect();
            crate::derived_key::record(
                path,
                &format!("keydef {id} {tag} {name} root={root} ctx={context}"),
            );
        }
        let signature = crate::derived_key::pointer_signature(&mut *self.checker, symbol);
        let verdict = match self.ids.derived_signatures.get(id) {
            None => "first".to_owned(),
            Some(previous) if *previous == signature => "same".to_owned(),
            Some(_) => {
                let earlier = self.ids.derived_first.get(id).copied();
                let identical = earlier.is_some_and(|earlier| {
                    let a = self.checker.get_type_of_symbol_exported(earlier);
                    let b = self.checker.get_type_of_symbol_exported(symbol);
                    self.checker.is_type_identical_to(a, b)
                        && self.checker.is_type_identical_to(b, a)
                });
                if identical {
                    "DIFFERENT-IDENTICAL".to_owned()
                } else {
                    format!("DIFFERENT-NOT-IDENTICAL {}", symbol.name())
                }
            }
        };
        crate::derived_key::record(path, &format!("keyed {outcome:?} {verdict}"));
        self.ids
            .derived_signatures
            .entry(Arc::clone(id))
            .or_insert(signature);
        self.ids
            .derived_first
            .entry(Arc::clone(id))
            .or_insert(symbol);
    }

    /// Go: `runtimePath`: `path` with symlinks resolved, or `path` itself when
    /// it does not resolve (a virtual or overlay-only file).
    pub fn runtime_path(&mut self, path: &str) -> String {
        if let Some(resolved) = self.ids.runtime_paths.get(path) {
            return resolved.clone();
        }
        // wasm32-wasip1 resolves links the same way: std's `canonicalize` is
        // wasi-libc's `realpath` over `path_readlink` there (pinned by the
        // symlinked-root case in packages/wasm/test/producer-reactor.test.mjs).
        let resolved = std::fs::canonicalize(path)
            .map_or_else(|_| path.to_owned(), |real| clean(&real.to_string_lossy()));
        self.ids
            .runtime_paths
            .insert(path.to_owned(), resolved.clone());
        resolved
    }

    /// Go: `symbolFor`: resolves `id` in the current generation, lazily
    /// re-resolving a binder id minted earlier through its key. The resolved
    /// symbol's own key must equal the id's, so a failed re-resolution reports
    /// not-found, exactly as a stale id always has.
    pub fn symbol_for(&mut self, id: &SymbolId) -> Option<P<Symbol>> {
        if let Some(symbol) = self.ids.symbols_by_id.get(id) {
            return Some(*symbol);
        }
        let key = Arc::clone(self.ids.keys.get(id)?);
        let candidate = match &*key {
            Key::Decl(reference) | Key::Export(reference) => self.symbol_at_reference(reference)?,
            Key::Global(name) => global_symbol(self.checker, name)?,
            Key::Intrinsic(which) => intrinsic_symbol(self.checker, *which),
            // A multi-origin property is made per containing type: no
            // position names it, so after a release it fails closed, like a
            // counter, until this generation mints it again.
            Key::Set { .. } | Key::Derived { .. } => return None,
        };
        let (found, root) = self.key_of(candidate)?;
        if found != *key {
            return None;
        }
        self.ids.ids_by_symbol.insert(root.addr(), Arc::clone(id));
        self.ids
            .ids_by_symbol
            .insert(candidate.addr(), Arc::clone(id));
        self.ids.symbols_by_id.insert(Arc::clone(id), root);
        Some(root)
    }

    fn symbol_at_reference(&mut self, reference: &DurableRef) -> Option<P<Symbol>> {
        let file = match self.program.get_source_file(&reference.path) {
            Some(file) => file,
            None => {
                let index = self.ids.files_by_name.get_or_insert_with(|| {
                    self.program
                        .source_files()
                        .iter()
                        .map(|file| (clean(file.file_name()), *file))
                        .collect()
                });
                *index.get(&reference.path)?
            }
        };
        if reference.start_byte >= reference.end_byte
            || reference.end_byte as usize > file.text().len()
        {
            return None;
        }
        let node = node_at(file, reference.start_byte);
        self.checker.get_symbol_at_location_exported(node)
    }

    /// Go: `canonicalSymbol`. `GetAliasedSymbol` normally resolves the
    /// complete chain. The bounded loop also handles compiler versions that
    /// expose one hop without allocating a visited map on every identifier in
    /// the retained reference scan.
    pub fn canonical_symbol(&mut self, mut symbol: P<Symbol>) -> P<Symbol> {
        for _ in 0..64 {
            if !symbol.flags().intersects(SymbolFlags::Alias) {
                break;
            }
            let original = self.checker.get_aliased_symbol(symbol);
            if same_symbol(original, symbol) {
                break;
            }
            symbol = original;
        }
        symbol
    }
}
