//! Symbol identity of one project (Go: `idFor`, `symbolFor`,
//! `canonicalSymbol`, `collectExportedIdentities`, `sweepDurableIdentities`
//! and the maps beside them in `tsgo/project.go`).
//!
//! A binder-level symbol's wire id is a pure function of the program (ADR
//! 0366): the hash of its [`Key`] (`binder_key.rs`), which does not depend on
//! which symbol asked first, on the checker's history, or on file order. A
//! symbol the checker derived (an instantiation, a transient clone, a mapped
//! or spread property) or whose binder-ness cannot be proven gets a
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

use crate::binder_key::{Key, binder_key, binder_root, global_symbol, intrinsic_symbol};
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
    /// always the binder-level root.
    symbols_by_id: HashMap<SymbolId, P<Symbol>>,
    /// Go: `durableRefs`. Maps a binder id back to its key so the symbol can be
    /// re-resolved lazily after a release.
    keys: HashMap<SymbolId, Key>,
    /// The module-visible symbols' declaration references (Go:
    /// `exportedIdentitiesByRef`): pointer-free, so it also answers after a
    /// release.
    exported_by_ref: HashMap<DurableRef, Preserved>,
    /// Go: `filesByName`. `Program.GetSourceFile` does not round-trip virtual
    /// bundled-lib names, so durable re-resolution of lib-declared symbols
    /// falls back to this index.
    files_by_name: Option<HashMap<String, P<SourceFile>>>,
    /// Go: `runtimePaths`. Filesystem-resolved declaration paths for one
    /// generation (symlinks followed), dropped with the other path caches on
    /// release and on every accepted update.
    runtime_paths: HashMap<String, String>,
    /// Write sites per declaring file (by file address), built on first use by
    /// `runtime_facts`. Holds node and symbol addresses: generation-scoped like
    /// `ids_by_symbol`, cleared at every point that clears it.
    pub(crate) write_indexes: HashMap<usize, Rc<WriteIndex>>,
}

/// What an identity question needs: the checker and program of the generation
/// the symbols come from, and the identity state.
pub struct Env<'a> {
    pub checker: &'a mut CheckerLease,
    pub program: &'static Program,
    pub ids: &'a mut Identities,
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

    /// The declaration references of the binder ids held (tests).
    pub fn durable_refs(&self) -> impl Iterator<Item = &DurableRef> {
        self.keys.values().filter_map(Key::reference)
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

    /// Go: `ReleaseAnalysisState`'s identity half, and the rebase policy's:
    /// drops every pointer-keyed map. Durable refs and the by-ref index are
    /// the rehydration boundary and stay.
    pub fn release_pointers(&mut self) {
        self.ids_by_symbol.clear();
        self.symbols_by_id.clear();
        self.files_by_name = None;
        self.runtime_paths.clear();
        self.write_indexes.clear();
        self.next_symbol = 0;
    }

    /// Go: `Close`.
    pub fn close(&mut self) {
        self.release_pointers();
        self.keys.clear();
        self.exported_by_ref.clear();
    }

    /// Seeds the exported identities at open (Go: `OpenProject`).
    pub fn seed_exported(&mut self, program: &Program, checker: &mut Checker) {
        self.exported_by_ref = collect_exported_identities(program, checker);
    }

    /// Go: the identity half of `Update` after a candidate is accepted. Nothing
    /// derived from the preceding generation survives: the claims and the
    /// durable references restart empty, and the exported identities are
    /// collected again from the candidate program, so an id minted before the
    /// update fails closed until the new generation mints it. (The exports the
    /// update already derived, `current`, only feed its cutoff decisions.)
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
        self.runtime_paths.clear();
        self.write_indexes.clear();
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

    /// Go: `idFor`. A binder-level symbol's id is the hash of its key; any
    /// other symbol takes the next counter of the generation.
    pub fn id_for(&mut self, symbol: P<Symbol>) -> SymbolId {
        let address = symbol.addr();
        if let Some(id) = self.ids.ids_by_symbol.get(&address) {
            return Arc::clone(id);
        }
        if let Some((key, root)) = self.key_of(symbol) {
            let id: SymbolId = Arc::from(key.id().as_str());
            // Two keys spelling one id (a 96-bit collision) would conflate two
            // symbols: the later one fails closed to a counter.
            if self.ids.keys.get(&id).is_none_or(|held| *held == key) {
                self.ids.keys.entry(Arc::clone(&id)).or_insert(key);
                self.ids
                    .symbols_by_id
                    .entry(Arc::clone(&id))
                    .or_insert(root);
                self.ids.ids_by_symbol.insert(root.addr(), Arc::clone(&id));
                self.ids.ids_by_symbol.insert(address, Arc::clone(&id));
                return id;
            }
        }
        self.ids.next_symbol += 1;
        let id: SymbolId =
            Arc::from(format!("symbol:{}:{}", self.ids.generation, self.ids.next_symbol).as_str());
        self.ids.ids_by_symbol.insert(address, Arc::clone(&id));
        self.ids.symbols_by_id.insert(Arc::clone(&id), symbol);
        id
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
        let key = self.ids.keys.get(id)?.clone();
        let candidate = match &key {
            Key::Decl(reference) | Key::Export(reference) => self.symbol_at_reference(reference)?,
            Key::Global(name) => global_symbol(self.checker, name)?,
            Key::Intrinsic(which) => intrinsic_symbol(self.checker, *which),
        };
        let (found, root) = self.key_of(candidate)?;
        if found != key {
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
