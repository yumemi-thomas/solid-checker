//! Symbol identity of one project (Go: `idFor`, `symbolFor`,
//! `canonicalSymbol`, `collectExportedIdentities`, `sweepDurableIdentities`
//! and the maps beside them in `tsgo/project.go`).
//!
//! A symbol's wire id is durable when it hashes a declaration (ADR 0001): the
//! name span of the first declaration plus the symbol's name, or, for a symbol
//! a module exports, the span-insensitive hash of declaring path and name.
//! Durable ids survive program rebuilds, so retained facts stay resolvable.
//! A symbol without a declaration gets a generation-scoped counter id
//! (`symbol:<generation>:<n>`) that means nothing outside the generation that
//! issued it.
//!
//! # Ownership
//!
//! The maps hold tsrs symbol handles and addresses, which are valid only for
//! the generation (program chain and checker) that produced them
//! (`unsafe_boundary` invariant 1). Every operation that replaces or frees a
//! generation clears them first: an accepted update ([`Identities::after_update`]),
//! a rebase and `ReleaseAnalysisState` ([`Identities::release_pointers`]), and
//! close. Only pointer-free state survives: `durable_refs` (the rehydration
//! boundary) and `exported_by_ref`.
//!
//! One deliberate difference from Go: Go's pointer-keyed `exportedIdentities`
//! keeps entries of replaced generations, which are harmless there because a
//! freed Go pointer is never reused. A freed tsrs address is reused, so here
//! the pointer-keyed map is only ever a cache of the current generation, and
//! `exported_by_ref` (patched exactly as Go patches it) answers for every
//! symbol it does not hold. Both are derived from the same
//! `durable_ref_for(symbol)`, so the answers are equal.

use crate::durable::{DurableRef, durable_ref_for};
use crate::gocompat::clean;
use crate::model::SymbolId;
use crate::nodes::node_at;
use crate::shape::{ExportedSymbol, exported_symbols_with};
use crate::unsafe_boundary::CheckerLease;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tsrs_ast::{SourceFile, Symbol, SymbolFlags, is_external_module};
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

#[derive(Default)]
pub struct Identities {
    /// The accepted program generation's number (Go: `project.generation`).
    pub generation: u64,
    next_symbol: u64,
    ids_by_symbol: HashMap<usize, SymbolId>,
    symbols_by_id: HashMap<SymbolId, P<Symbol>>,
    /// Go: `durableRefs`. Maps a durable id back to its declaration so the
    /// symbol can be re-resolved lazily in a later generation.
    durable_refs: HashMap<SymbolId, DurableRef>,
    /// Go: `exportedIdentities`, keyed by symbol address; `None` after a
    /// release and after any update.
    exported: Option<HashMap<usize, Preserved>>,
    /// Go: `exportedIdentitiesByRef`.
    exported_by_ref: HashMap<DurableRef, Preserved>,
    /// Go: `filesByName`. `Program.GetSourceFile` does not round-trip virtual
    /// bundled-lib names, so durable re-resolution of lib-declared symbols
    /// falls back to this index.
    files_by_name: Option<HashMap<String, P<SourceFile>>>,
    /// Go: `runtimePaths`. Filesystem-resolved declaration paths for one
    /// generation (symlinks followed), dropped with the other path caches on
    /// release and on every accepted update.
    runtime_paths: HashMap<String, String>,
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

    /// The durable declaration references held (tests).
    #[must_use]
    pub fn durable_refs(&self) -> &HashMap<SymbolId, DurableRef> {
        &self.durable_refs
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
        self.exported = None;
        self.files_by_name = None;
        self.runtime_paths.clear();
        self.next_symbol = 0;
    }

    /// Go: `Close`.
    pub fn close(&mut self) {
        self.release_pointers();
        self.durable_refs.clear();
        self.exported_by_ref.clear();
    }

    /// Go: `sweepDurableIdentities`. Drops identity bookkeeping for files the
    /// program no longer contains. This is the one eviction that cannot change
    /// behavior: an absent file's declarations cannot re-resolve regardless,
    /// and a file that re-enters the program re-mints identical entries
    /// through the location scans that precede any by-id query. Identities
    /// whose declaring file stays in the program are never dropped, even when
    /// the file is affected: recomputing an edited file's facts resolves
    /// enqueued symbols by id, and evicting those entries would silently cost
    /// their declarations.
    pub fn sweep_durable(&mut self, program: &Program) {
        let present: HashSet<String> = program
            .source_files()
            .iter()
            .map(|file| clean(file.file_name()))
            .collect();
        self.durable_refs
            .retain(|_, reference| present.contains(&reference.path));
    }

    /// Seeds the exported identities at open (Go: `OpenProject`).
    pub fn seed_exported(&mut self, program: &Program, checker: &mut Checker) {
        let identities = collect_exported_identities(program, checker);
        self.exported_by_ref = index_by_ref(&identities);
        self.exported = Some(identities);
    }

    /// Go: the identity half of `Update` after a candidate is accepted.
    ///
    /// `current` is `Some` when the candidate's exports for `incremental_path`
    /// are known: they replace that file's by-ref entries. Otherwise the
    /// whole program's exports are collected again. Either way the
    /// pointer-keyed maps restart from the candidate's own symbols.
    pub fn after_update(
        &mut self,
        generation: u64,
        program: &Program,
        checker: &mut Checker,
        incremental_path: Option<&str>,
        current: Option<CurrentExports>,
    ) {
        self.generation = generation;
        self.ids_by_symbol.clear();
        self.symbols_by_id.clear();
        self.files_by_name = None;
        self.runtime_paths.clear();
        self.next_symbol = 0;
        match (incremental_path, current) {
            (Some(path), Some(current)) => {
                self.exported_by_ref
                    .retain(|reference, _| reference.path != path);
                let mut exported = HashMap::with_capacity(current.len());
                for (symbol, preserved) in &current {
                    exported.insert(symbol.addr(), preserved.clone());
                    self.exported_by_ref
                        .insert(preserved.reference.clone(), preserved.clone());
                }
                self.exported = Some(exported);
                self.seed_current(&current);
            }
            _ => {
                self.seed_exported(program, checker);
            }
        }
    }

    /// Go: `for symbol, preserved := range currentExports`.
    fn seed_current(&mut self, current: &CurrentExports) {
        for (symbol, preserved) in current {
            self.ids_by_symbol
                .insert(symbol.addr(), Arc::clone(&preserved.id));
            self.symbols_by_id
                .insert(Arc::clone(&preserved.id), *symbol);
            self.durable_refs
                .insert(Arc::clone(&preserved.id), preserved.reference.clone());
        }
    }
}

/// Go: `collectExportedIdentities`.
pub fn collect_exported_identities(
    program: &Program,
    checker: &mut Checker,
) -> HashMap<usize, Preserved> {
    let mut identities: HashMap<usize, Preserved> = HashMap::new();
    for file in program.source_files() {
        if !is_external_module(*file) {
            continue;
        }
        let Some(exports) = exported_symbols_with(checker, *file) else {
            continue;
        };
        for ExportedSymbol { export, symbol } in exports {
            let key = symbol.addr();
            let id: SymbolId = Arc::from(export.id.as_str());
            if let Some(existing) = identities.get(&key)
                && existing.id != id
            {
                // The same target reached through incompatible deterministic
                // identities is not safe to canonicalize globally.
                identities.remove(&key);
                continue;
            }
            identities.insert(
                key,
                Preserved {
                    id,
                    reference: export.reference,
                },
            );
        }
    }
    identities
}

/// Go: `indexExportedIdentitiesByRef`.
fn index_by_ref(identities: &HashMap<usize, Preserved>) -> HashMap<DurableRef, Preserved> {
    identities
        .values()
        .map(|identity| (identity.reference.clone(), identity.clone()))
        .collect()
}

impl Env<'_> {
    /// Records `id` for `symbol` unless another symbol already holds it.
    fn claim(&mut self, symbol: P<Symbol>, id: &SymbolId) -> bool {
        match self.ids.symbols_by_id.get(id) {
            Some(existing) if !same_symbol(*existing, symbol) => false,
            _ => {
                self.ids.ids_by_symbol.insert(symbol.addr(), Arc::clone(id));
                self.ids.symbols_by_id.insert(Arc::clone(id), symbol);
                true
            }
        }
    }

    /// Go: `idFor`.
    pub fn id_for(&mut self, symbol: P<Symbol>) -> SymbolId {
        let key = symbol.addr();
        if let Some(id) = self.ids.ids_by_symbol.get(&key) {
            return Arc::clone(id);
        }
        if let Some(exported) = self.ids.exported.as_ref().and_then(|map| map.get(&key)) {
            let (id, reference) = (Arc::clone(&exported.id), exported.reference.clone());
            if self.claim(symbol, &id) {
                self.ids.durable_refs.insert(Arc::clone(&id), reference);
                return id;
            }
        }
        if let Some(reference) = durable_ref_for(symbol) {
            // Checker rebuilds can expose an equivalent declaration through a
            // different symbol from the one `GetExportsOfModule` returned.
            // The durable declaration ref is the cross-generation proof and
            // therefore the authoritative secondary key.
            if let Some(exported) = self.ids.exported_by_ref.get(&reference) {
                let id = Arc::clone(&exported.id);
                if self.claim(symbol, &id) {
                    self.ids.durable_refs.insert(Arc::clone(&id), reference);
                    return id;
                }
            }
            let id: SymbolId = Arc::from(reference.id().as_str());
            if self.claim(symbol, &id) {
                self.ids.durable_refs.insert(Arc::clone(&id), reference);
                return id;
            }
        }
        self.ids.next_symbol += 1;
        let id: SymbolId =
            Arc::from(format!("symbol:{}:{}", self.ids.generation, self.ids.next_symbol).as_str());
        self.ids.ids_by_symbol.insert(key, Arc::clone(&id));
        self.ids.symbols_by_id.insert(Arc::clone(&id), symbol);
        id
    }

    /// Go: `runtimePath`: `path` with symlinks resolved, or `path` itself when
    /// it does not resolve (a virtual or overlay-only file).
    pub fn runtime_path(&mut self, path: &str) -> String {
        if let Some(resolved) = self.ids.runtime_paths.get(path) {
            return resolved.clone();
        }
        let resolved = std::fs::canonicalize(path)
            .map_or_else(|_| path.to_owned(), |real| clean(&real.to_string_lossy()));
        self.ids
            .runtime_paths
            .insert(path.to_owned(), resolved.clone());
        resolved
    }

    /// Go: `symbolFor`: resolves `id` in the current generation, lazily
    /// re-resolving a durable id minted in an earlier generation through its
    /// declaration. A failed re-resolution reports not-found, exactly as a
    /// stale id always has.
    pub fn symbol_for(&mut self, id: &SymbolId) -> Option<P<Symbol>> {
        if let Some(symbol) = self.ids.symbols_by_id.get(id) {
            return Some(*symbol);
        }
        let reference = self.ids.durable_refs.get(id)?.clone();
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
        let symbol = self.checker.get_symbol_at_location_exported(node)?;
        match durable_ref_for(symbol) {
            Some(resolved) if resolved == reference => {}
            _ => return None,
        }
        if let Some(canonical) = self.ids.ids_by_symbol.get(&symbol.addr())
            && canonical != id
        {
            // A shape-equivalent update may deliberately preserve an older
            // canonical id for this symbol. A historical span-derived id must
            // not displace that choice if it is queried later.
            return None;
        }
        self.ids.ids_by_symbol.insert(symbol.addr(), Arc::clone(id));
        self.ids.symbols_by_id.insert(Arc::clone(id), symbol);
        Some(symbol)
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
