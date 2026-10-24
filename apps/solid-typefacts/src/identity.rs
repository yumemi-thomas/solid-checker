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
//! mapped property) or whose binder-ness cannot be proven gets its exact
//! derived key's hash (`derived_key.rs`), and a symbol with no exact key gets
//! no id (the empty id: an absent fact). No id is a counter, so an id names
//! one symbol in every checker of the program (ADR 0398).
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
    /// Symbol address to its id: a memo of a pure function of the symbol.
    ids_by_symbol: HashMap<usize, SymbolId>,
    /// Id to the symbol it names, so an id answers without re-resolving. Not a claim table: several symbols may map to one
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
    /// What each module contributed to `exported_by_ref`, so an update that
    /// replaced one file rescans only the modules that can read it
    /// ([`ExportScan`]).
    export_scan: ExportScan,
    /// Go: `filesByName`. `Program.GetSourceFile` does not round-trip virtual
    /// bundled-lib names, so durable re-resolution of lib-declared symbols
    /// falls back to this index.
    files_by_name: Option<HashMap<String, P<SourceFile>>>,
    /// Go: `runtimePaths`. Filesystem-resolved declaration paths (symlinks
    /// followed). A path's resolution is a file-system fact, not a checker
    /// one, so it is kept for as long as the program chain trusts the file
    /// system: across releases and parse-reusing updates, which reuse every
    /// other file and its module resolutions as read, and dropped by any
    /// rebuild, which reads the file system again (`Realpath`).
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
    /// Per-file passes (instant-edits P3): the keys the last table's passes
    /// minted, merged (with how many passes minted each), consulted after
    /// `keys` by every lookup, so a scratch question resolves a pass's id and
    /// never mints one id for another key. Shared with the merge that owns
    /// it; detached at every program change, as `keys` is cleared then.
    table_keys: Option<Arc<TableKeys>>,
}

/// The keys of a table's passes, each with how many passes minted it.
pub(crate) type TableKeys = HashMap<SymbolId, (Arc<Key>, u32)>;

/// The pointer-free start of a pass's identity state ([`Identities::pass_seed`]).
#[derive(Clone)]
pub(crate) struct PassSeed {
    generation: u64,
    exported_by_ref: Arc<HashMap<DurableRef, Preserved>>,
    runtime_paths: HashMap<String, String>,
}

/// What an identity question needs: the checker and program of the generation
/// the symbols come from, and the identity state.
pub struct Env<'a> {
    pub checker: &'a mut CheckerLease,
    pub program: &'static Program,
    pub ids: &'a mut Identities,
    /// Digests of the program inputs (`inputs`).
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

    /// Whether `id` was minted from an exact derived key (`derived_key.rs`)
    /// in this generation: a derived symbol, not a binder-level one.
    #[must_use]
    pub fn is_derived_id(&self, id: &SymbolId) -> bool {
        self.key(id)
            .is_some_and(|key| matches!(**key, Key::Derived { .. }))
    }

    /// Whether `id` is a multi-origin property's `Set` id (ADR 0371).
    #[must_use]
    pub(crate) fn is_set_id(&self, id: &SymbolId) -> bool {
        self.key(id).is_some_and(|key| !key.is_binder())
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
    }

    /// A file checker's pass ended (per-file checkers, `unsafe_boundary`
    /// invariant 8): drops every map that may hold a handle into the checker's
    /// region, which is freed now. `ids_by_symbol` and `symbols_by_id` hold
    /// derived symbols and the checker's merged symbols (both in its region),
    /// and binder symbols only as a memo of what `keys` re-resolves;
    /// `write_indexes` holds canonical (possibly merged) symbol addresses. An
    /// address in a freed region is reused by the next pass, so an entry left
    /// behind could name another symbol. `files_by_name` holds program files
    /// and stays. The
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
    /// instant-edits P3): the project's pointer-free inputs (the exported
    /// identities, the resolved runtime paths) and no key.
    /// `Send`, so a worker thread builds its state from it
    /// ([`Identities::from_seed`]).
    #[must_use]
    pub(crate) fn pass_seed(&self) -> PassSeed {
        PassSeed {
            generation: self.generation,
            exported_by_ref: Arc::clone(&self.exported_by_ref),
            runtime_paths: self.runtime_paths.clone(),
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

    /// The key `id` names in this generation, if any: minted under this
    /// state, or by the last table's passes.
    #[must_use]
    pub(crate) fn key(&self, id: &SymbolId) -> Option<&Arc<Key>> {
        self.keys.get(id).or_else(|| {
            self.table_keys
                .as_ref()
                .and_then(|keys| keys.get(id))
                .map(|(key, _)| key)
        })
    }

    /// The keys minted under this state itself (not the table's).
    pub(crate) fn own_keys(&self) -> impl Iterator<Item = (&SymbolId, &Arc<Key>)> {
        self.keys.iter()
    }

    /// Attaches (or detaches) the last table's merged keys.
    pub(crate) fn set_table_keys(&mut self, keys: Option<Arc<TableKeys>>) {
        self.table_keys = keys;
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
        self.table_keys = None;
        self.exported_by_ref = Arc::default();
        self.export_scan = ExportScan::default();
    }

    /// Seeds the exported identities at open (Go: `OpenProject`).
    pub fn seed_exported(&mut self, program: &Program, checker: &mut Checker) {
        self.seed_exported_with(program, checker, None);
    }

    /// [`Identities::seed_exported`], with the modules `scanned` already
    /// answered elsewhere ([`scan_exports_on_workers`]); a module it left
    /// out, or answered with roots that are not lasting, is scanned here.
    pub(crate) fn seed_exported_with(
        &mut self,
        program: &Program,
        checker: &mut Checker,
        scanned: Option<Vec<Option<Option<FileExports>>>>,
    ) {
        let files = program.source_files();
        let mut answers: Vec<(String, Option<FileExports>)> = Vec::with_capacity(files.len());
        let mut scanned = scanned.filter(|scanned| scanned.len() == files.len());
        for (at, file) in files.iter().enumerate() {
            let held = scanned.as_mut().and_then(|scanned| scanned[at].take());
            let now = match held {
                Some(now) if now.as_ref().is_none_or(|now| now.kept) => now,
                _ => file_exports(checker, *file),
            };
            answers.push((clean(file.file_name()), now));
        }
        self.apply_every_export(answers);
        self.export_scan.valid = true;
    }

    /// Rescans `files` (their earlier contributions replaced) and patches
    /// `exported_by_ref` at the references they held or hold.
    fn rescan_exports(&mut self, checker: &mut Checker, files: &[P<SourceFile>]) {
        let scanned = files
            .iter()
            .map(|file| (clean(file.file_name()), file_exports(checker, *file)))
            .collect();
        self.apply_exports(scanned);
    }

    /// Replaces every module's contribution by `answers` (a scan of every
    /// module) and builds `exported_by_ref` from them. An identity the
    /// previous scan held keeps its value (its id hashes only its path and
    /// name).
    fn apply_every_export(&mut self, answers: Vec<(String, Option<FileExports>)>) {
        let previous = std::mem::take(&mut self.exported_by_ref);
        let mut scan = ExportScan {
            valid: self.export_scan.valid,
            ..ExportScan::default()
        };
        for (path, now) in answers {
            let Some(now) = now else {
                continue;
            };
            for (reference, root) in &now.entries {
                let roots = scan.roots.entry(reference.clone()).or_default();
                match roots.iter_mut().find(|(held, _)| held == root) {
                    Some(entry) => entry.1 += 1,
                    None => roots.push((*root, 1)),
                }
            }
            scan.files.insert(path, now);
        }
        let mut exported: HashMap<DurableRef, Preserved> = HashMap::with_capacity(scan.roots.len());
        for (reference, roots) in &scan.roots {
            if roots.len() != 1 {
                continue;
            }
            let preserved = previous
                .get(reference)
                .cloned()
                .unwrap_or_else(|| Preserved {
                    id: Arc::from(reference.exported_id().as_str()),
                    reference: reference.clone(),
                });
            scan.by_path
                .entry(reference.path.clone())
                .or_default()
                .insert((
                    reference.start_byte,
                    reference.end_byte,
                    reference.name.clone(),
                ));
            exported.insert(reference.clone(), preserved);
        }
        self.exported_by_ref = Arc::new(exported);
        self.export_scan = scan;
    }

    /// Replaces the contributions of the scanned modules and patches
    /// `exported_by_ref` at the references they held or hold.
    fn apply_exports(&mut self, scanned: Vec<(String, Option<FileExports>)>) {
        let mut touched: HashSet<DurableRef> = HashSet::new();
        for (path, now) in scanned {
            let before = self.export_scan.files.remove(&path);
            if let Some(before) = &before {
                self.export_scan.remove(before, &mut touched);
            }
            if let Some(now) = now {
                self.export_scan.add(&now, &mut touched);
                self.export_scan.files.insert(path, now);
            }
        }
        let exported = Arc::make_mut(&mut self.exported_by_ref);
        let scan = &mut self.export_scan;
        for reference in touched {
            let moved = match scan.roots.get(&reference) {
                // Exactly one root: the reference is a safe export identity.
                Some(roots) if roots.len() == 1 => {
                    let entered = !exported.contains_key(&reference);
                    if entered {
                        let id: SymbolId = Arc::from(reference.exported_id().as_str());
                        exported.insert(
                            reference.clone(),
                            Preserved {
                                id,
                                reference: reference.clone(),
                            },
                        );
                        scan.by_path
                            .entry(reference.path.clone())
                            .or_default()
                            .insert((
                                reference.start_byte,
                                reference.end_byte,
                                reference.name.clone(),
                            ));
                    }
                    entered
                }
                _ => {
                    let left = exported.remove(&reference).is_some();
                    if left && let Some(held) = scan.by_path.get_mut(&reference.path) {
                        held.remove(&(
                            reference.start_byte,
                            reference.end_byte,
                            reference.name.clone(),
                        ));
                        if held.is_empty() {
                            scan.by_path.remove(&reference.path);
                        }
                    }
                    left
                }
            };
            if moved {
                scan.digests.remove(&reference.path);
                if let Some(changed) = &mut scan.changed {
                    changed.insert(reference.path.clone());
                }
            }
        }
    }

    /// `Exports(path)` (`inputs`, `passes`): a digest of the export
    /// identities declared in `path`, in (start, end, name) order; zero for
    /// none.
    pub(crate) fn export_digest(&mut self, path: &str) -> crate::inputs::Digest {
        let scan = &mut self.export_scan;
        if let Some(digest) = scan.digests.get(path) {
            return *digest;
        }
        let Some(references) = scan.by_path.get(path) else {
            return crate::inputs::Digest::default();
        };
        use sha2::Digest as _;
        let mut hasher = sha2::Sha256::new();
        for (start, end, name) in references {
            hasher.update(start.to_le_bytes());
            hasher.update(end.to_le_bytes());
            hasher.update(name.as_bytes());
            hasher.update([0]);
        }
        let digest: crate::inputs::Digest = hasher.finalize().into();
        scan.digests.insert(path.to_owned(), digest);
        digest
    }

    /// The paths whose `Exports` may have changed since the last call, or
    /// `None` when every path's may have (a scan of every module).
    pub(crate) fn take_export_changes(&mut self) -> Option<HashSet<String>> {
        self.export_scan.changed.replace(HashSet::new())
    }

    /// The modules an update must rescan: `None` for every module, else the
    /// given ones and every module whose roots the scan could not keep.
    fn exports_to_rescan(
        &self,
        program: &Program,
        rescan: Option<&[String]>,
    ) -> Option<Vec<P<SourceFile>>> {
        let rescan = rescan?;
        if !self.export_scan.valid {
            return None;
        }
        let mut paths: HashSet<&str> = rescan.iter().map(String::as_str).collect();
        paths.extend(
            self.export_scan
                .files
                .iter()
                .filter(|(_, exports)| !exports.kept)
                .map(|(path, _)| path.as_str()),
        );
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            files.push(program.get_source_file(path)?);
        }
        Some(files)
    }

    /// The program chain was replaced (a rebase): every root address the
    /// export scan holds is dead, so the next update rescans every module.
    pub(crate) fn forget_export_roots(&mut self) {
        self.export_scan.valid = false;
    }

    /// Go: the identity half of `Update` after a candidate is accepted. Nothing
    /// derived from the preceding generation survives: the claims and the
    /// durable references restart empty, and the exported identities are
    /// collected again from the candidate program, so an id minted before the
    /// update fails closed until a pass of the new generation mints it (a kept
    /// pass's keys rejoin through the table's merged keys). (The exports the
    /// update already derived, `current`, only feed its cutoff decisions.)
    ///
    /// `rescan` names the modules whose exports can have changed when the
    /// update replaced one file in place (every other file kept its parse,
    /// its binder symbols and its resolutions): the file and every module
    /// that reaches it over a resolution edge. `None` rescans every module.
    ///
    /// `scanned` is every module's half scanned beforehand on workers
    /// ([`scan_exports_on_workers`]) for when every module is rescanned.
    pub(crate) fn after_update(
        &mut self,
        generation: u64,
        program: &Program,
        checker: &mut Checker,
        rescan: Option<&[String]>,
        scanned: Option<Vec<Option<Option<FileExports>>>>,
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
        self.keys.clear();
        self.table_keys = None;
        match self.exports_to_rescan(program, rescan) {
            Some(files) => self.rescan_exports(checker, &files),
            None => self.seed_exported_with(program, checker, scanned),
        }
        self.export_scan.valid = true;
        if crate::passes::verify_reuse() {
            let fresh = collect_exported_identities(program, checker);
            if fresh != *self.exported_by_ref {
                crate::per_file::reuse_mismatch("the exported identities");
                self.seed_exported(program, checker);
            }
        }
    }
}

/// What the modules of a program export, per module (`collect_exported_identities`
/// kept per file). A reference is an export identity when every module that
/// exports it names one root.
///
/// A root is held by a lasting stand-in (`binder_key::lasting_root`): a
/// program-owned binder symbol's address, which names the same symbol in
/// every checker of the chain while its file keeps its parse. A module's
/// entry is `kept` when every root has one; otherwise its roots are held by
/// address and the module is rescanned at every update, in the same checker
/// as every other rescanned module. A stand-in never equals another root's
/// address: a root merges into nothing, a stand-in is a symbol that merged. An update that replaces one file
/// in place rescans that file and every module reaching it over resolution
/// edges: the others resolve their exports through files that kept their
/// parse, so their roots are the same live symbols.
#[derive(Default)]
struct ExportScan {
    files: HashMap<String, FileExports>,
    /// Per reference, each distinct root address and how many exports name it.
    roots: HashMap<DurableRef, Vec<(usize, u32)>>,
    /// The export identities by declaring path, and their digests.
    by_path: HashMap<String, std::collections::BTreeSet<(u32, u32, String)>>,
    digests: HashMap<String, crate::inputs::Digest>,
    /// Paths whose identities moved since the last table; `None` for all.
    changed: Option<HashSet<String>>,
    /// Whether the root addresses belong to the current chain.
    valid: bool,
}

pub(crate) struct FileExports {
    entries: Vec<(DurableRef, usize)>,
    kept: bool,
}

impl ExportScan {
    fn remove(&mut self, exports: &FileExports, touched: &mut HashSet<DurableRef>) {
        for (reference, root) in &exports.entries {
            if let Some(roots) = self.roots.get_mut(reference) {
                if let Some(at) = roots.iter().position(|(held, _)| held == root) {
                    roots[at].1 -= 1;
                    if roots[at].1 == 0 {
                        roots.swap_remove(at);
                    }
                }
                if roots.is_empty() {
                    self.roots.remove(reference);
                }
            }
            touched.insert(reference.clone());
        }
    }

    fn add(&mut self, exports: &FileExports, touched: &mut HashSet<DurableRef>) {
        for (reference, root) in &exports.entries {
            let roots = self.roots.entry(reference.clone()).or_default();
            match roots.iter_mut().find(|(held, _)| held == root) {
                Some(entry) => entry.1 += 1,
                None => roots.push((*root, 1)),
            }
            touched.insert(reference.clone());
        }
    }
}

/// Every file's [`file_exports`] (index-aligned with `files`), on up to
/// `workers` threads, each in a file checker of its own; a file the
/// workers did not reach is `None`. A module whose roots are not all
/// lasting is answered, but its root addresses die with the worker's
/// checker: the caller scans it again ([`Identities::seed_exported_with`]).
pub(crate) fn scan_exports_on_workers(
    generation: &crate::unsafe_boundary::Generation,
    files: &[P<SourceFile>],
    workers: usize,
) -> Vec<Option<Option<FileExports>>> {
    let workers = workers.min(files.len() / 256);
    if workers <= 1 {
        return Vec::new();
    }
    generation.prime_caches();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let next = &next;
    let mut answers: Vec<Option<Option<FileExports>>> = (0..files.len()).map(|_| None).collect();
    let done: Vec<Vec<(usize, Option<FileExports>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .filter_map(|_| {
                std::thread::Builder::new()
                    .name("typefacts-exports".to_owned())
                    .stack_size(crate::passes::WORKER_STACK)
                    .spawn_scoped(scope, move || {
                        let mut local = Vec::new();
                        let checker = generation.file_checker();
                        generation.with_checker(|lease| {
                            loop {
                                let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(file) = files.get(at) else {
                                    break;
                                };
                                local.push((at, file_exports(lease, *file)));
                            }
                        });
                        drop(checker);
                        local
                    })
                    .ok()
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    for (at, exports) in done.into_iter().flatten() {
        answers[at] = Some(exports);
    }
    answers
}

/// One module's half of [`collect_exported_identities`]; `None` for a file
/// that is not a module or whose exports have no durable identity.
fn file_exports(checker: &mut Checker, file: P<SourceFile>) -> Option<FileExports> {
    if !is_external_module(file) {
        return None;
    }
    let exports = exported_symbols_with(checker, file)?;
    let mut entries = Vec::with_capacity(exports.len());
    let mut kept = true;
    for ExportedSymbol { symbol, .. } in exports {
        let Some(root) = binder_root(checker, symbol) else {
            continue;
        };
        let Some(reference) = minimum_declaration_ref(root) else {
            continue;
        };
        let identity = match crate::binder_key::lasting_root(checker, root) {
            Some(lasting) => lasting,
            None => {
                kept = false;
                root.addr()
            }
        };
        entries.push((reference, identity));
    }
    Some(FileExports { entries, kept })
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
    /// a derived or unproven symbol (keyed by `derived_id`).
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
    /// multi-origin property's the hash of its `Set` key, any other symbol's
    /// the hash of its exact derived key; a symbol with none has the empty
    /// id.
    pub fn id_for(&mut self, symbol: P<Symbol>) -> SymbolId {
        let address = symbol.addr();
        if let Some(id) = self.ids.ids_by_symbol.get(&address) {
            return Arc::clone(id);
        }
        if let Some(key) = self.set_key_of(symbol) {
            let id: SymbolId = Arc::from(key.id().as_str());
            self.ids.last_failure = Some("set-collision".into());
            if self.ids.key(&id).is_none_or(|held| **held == key) {
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
                self.ids.key(&id)
            ));
            // Two keys spelling one id (a 96-bit collision) would conflate two
            // symbols: the later one is keyed as derived, or has no id.
            if self.ids.key(&id).is_none_or(|held| **held == key) {
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
        // No exact key: no id. An absent symbol fails closed downstream; a
        // counter would split one symbol across checkers and passes.
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
        if self.ids.key(&id).is_some_and(|held| **held != key) {
            self.ids.last_failure = Some(format!(
                "derived-collision held={:?} new={key:?}",
                self.ids.key(&id)
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
        let key = Arc::clone(self.ids.key(id)?);
        let candidate = match &*key {
            Key::Decl(reference) | Key::Export(reference) => self.symbol_at_reference(reference)?,
            Key::Global(name) => global_symbol(self.checker, name)?,
            Key::Intrinsic(which) => intrinsic_symbol(self.checker, *which),
            // A multi-origin property or a derived symbol is made per
            // containing type or instantiation: no position names it, so
            // after a release it fails closed until a pass mints it again.
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
