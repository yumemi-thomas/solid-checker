//! The reference index (Go: `tsgo/references.go`).
//!
//! It owns the generation-scoped merged index and its per-file contributions.
//! The scan is the producer's own walk over syntax, not a language-service
//! call: every non-declaration identifier is asked for its symbol, and a
//! shorthand property's name is also a read of its value (ADR 0336,
//! `GetShorthandAssignmentValueSymbol`).
//!
//! # Retention (ADR 0369)
//!
//! An identifier the checker resolves by scope lookup alone
//! (`binder_resolution`) has an answer that depends on the program's inputs
//! and not on checker history. A file's binder-resolved half (its spans by
//! canonical id, its reference spaces) is kept across updates in a
//! [`BinderFile`] with the digests it read, and reused while all of them are
//! unchanged; its type-resolved half (property names, JSX intrinsics, alias
//! positions resolved with the alias followed, JSDoc) is resolved again on
//! every build, in the same walk order as a fresh scan, so the checker sees
//! the same type questions in the same order whether or not a file is reused.
//! Alias targets are re-resolved at the first binder-resolved use of each
//! alias and compared: that one check covers the target module's exports,
//! module resolution, files entering the program and every re-export hop.
//! The merged index and the spaces union are always rebuilt from the current
//! program's files, so a departed file's references leave with it.

use crate::binder_key::Key;
use crate::binder_resolution::reference_resolves_without_types;
use crate::gocompat::clean;
use crate::identity::Env;
use crate::inputs::Digest;
use crate::model::{SymbolId, durable_symbol_id};
use std::collections::{HashMap, HashSet};
use tsrs_ast::{
    Kind, Node, SourceFile, SymbolFlags, is_declaration_name_or_import_property_name,
    is_identifier, is_part_of_type_node, is_qualified_name, is_shorthand_property_assignment,
};
use tsrs_core::P;
use tsrs_scanner::skip_trivia;
use typefacts::{Location, ReferenceSpace, SymbolClaim};

/// Per-file spans omit the file path (`ReferenceIndex::files` owns it once).
/// Compiler positions fit in 32 bits, so a retained reference is one compact
/// eight-byte span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Indexed {
    path: u32,
    start: u32,
    end: u32,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: SymbolId,
    /// Emptied once merged into the index, so the index does not duplicate
    /// what a retained contribution holds.
    pub spans: Vec<Span>,
}

/// One file's contribution to the reference index: every resolvable
/// non-declaration identifier in the file, grouped by the durable id of its
/// alias-canonicalized symbol, each group in ascending byte order. `durable`
/// reports whether every grouped id is durable; an entry with
/// generation-scoped counter ids cannot outlive its generation.
#[derive(Clone, Debug)]
pub struct FileReferences {
    pub refs: Vec<Group>,
    spaces: Vec<(SymbolId, u8)>,
    pub durable: bool,
    /// The spans were moved into a merged index and cleared. Such an entry
    /// still carries exact symbol evidence but cannot rebuild a dropped
    /// index; `build` rescans it.
    merged: bool,
    /// Which scan produced this entry; tests use it to tell a retained
    /// contribution from a replaced one (Go compares the pointers).
    pub serial: u64,
}

/// How much of a build's binder-level work was reused (ADR 0369).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BinderRetention {
    /// Files whose binder-resolved references were reused.
    pub reused_reference_files: usize,
    /// Files scanned whole.
    pub scanned_reference_files: usize,
    /// Entity symbols answered from a retained file.
    pub reused_symbols: usize,
    /// Entity symbols resolved by the checker.
    pub resolved_symbols: usize,
    /// Symbol facts (declarations) answered from an earlier generation.
    pub reused_symbol_facts: usize,
    /// Symbol facts resolved through the checker.
    pub resolved_symbol_facts: usize,
    /// Time spent building the reference index.
    pub build: std::time::Duration,
}

/// An identifier a reuse walk must resolve again, in walk order: every
/// type-resolved one (its answer is history-dependent) or one whose answer
/// holds a generation-scoped id, and every
/// binder-resolved one whose name an alias could answer, with the answer it
/// had. Scope lookup that meets an alias resolves the alias to test its
/// meaning (`get_symbol` reads the target's flags), so such an answer depends
/// on other modules and the lookup can do type work (an `export =` target's
/// type): it is checked, and replayed in the same order, on every reuse.
#[derive(Clone, Debug)]
struct Target {
    start: u32,
    /// `None` for an identifier resolved again into the typed half; else the
    /// binder-resolved answer (reference id, canonical id), or no symbol.
    binder: Option<Option<(SymbolId, SymbolId)>>,
}

/// A file's binder-resolved references.
#[derive(Clone, Debug)]
struct BinderReferences {
    groups: Vec<Group>,
    spaces: Vec<(SymbolId, u8)>,
    targets: Vec<Target>,
}

/// One file's binder-level answers kept across updates, with the dependencies
/// they were computed under (ADR 0369). Pointer-free.
#[derive(Debug)]
pub struct BinderFile {
    /// The input epoch the dependencies were last proven at.
    epoch: u64,
    /// `Text(file)`, `Options`, `Ambient`.
    text: Digest,
    options: Digest,
    ambient: Digest,
    /// `Text(path)` and `Exports(path)` of every file declaring a symbol whose
    /// id a retained answer holds (the file itself, an augmentation), by
    /// interned path, sorted.
    declaring: Vec<(u32, Digest, Digest)>,
    /// The key of every binder id held below (alias targets excluded: they
    /// are re-resolved), registered with the generation on reuse so that
    /// `symbol_for` answers for an id this generation did not mint.
    keys: HashMap<SymbolId, std::sync::Arc<Key>>,
    refs: Option<BinderReferences>,
    /// Entity symbols at binder-resolved positions, by demanded span.
    symbols: HashMap<(u64, u64), (SymbolId, SymbolClaim)>,
    /// Whether anything may be kept for the file ([`retainable`]), and the
    /// names its aliases may answer (syntax: a function of `Text(file)`).
    retainable: bool,
    aliases: std::sync::Arc<HashSet<String>>,
}

impl BinderFile {
    fn baseline(env: &mut Env<'_>, file: P<SourceFile>) -> Self {
        Self {
            epoch: env.inputs.epoch(),
            text: env.inputs.text(file),
            options: env.inputs.options(env.program),
            ambient: env.inputs.ambient(env.program),
            declaring: Vec::new(),
            keys: HashMap::new(),
            refs: None,
            symbols: HashMap::new(),
            retainable: retainable(env, file),
            aliases: std::sync::Arc::new(file_alias_names(file)),
        }
    }

    fn alias_names(&self, env: &mut Env<'_>) -> AliasNames {
        AliasNames {
            file: std::sync::Arc::clone(&self.aliases),
            global: env.inputs.global_aliases(env.checker),
        }
    }

    /// Whether every recorded dependency still holds; on success the held
    /// ids are made answerable in this generation.
    fn still_valid(&mut self, env: &mut Env<'_>, file: P<SourceFile>) -> bool {
        let epoch = env.inputs.epoch();
        if self.epoch == epoch {
            return true;
        }
        if env.inputs.text(file) != self.text
            || env.inputs.options(env.program) != self.options
            || env.inputs.ambient(env.program) != self.ambient
        {
            return false;
        }
        for (path, text, exports) in &self.declaring {
            if env
                .inputs
                .declared(env.program, env.ids.exported_by_ref(), *path)
                != (Some(*text), *exports)
            {
                return false;
            }
        }
        for (id, key) in &self.keys {
            if !env.ids.register_key(id, key) {
                return false;
            }
        }
        self.epoch = epoch;
        true
    }

    /// Records the dependencies of holding `id`: its key, and the text and
    /// exports of the file declaring it. `false` (hold nothing) for an id
    /// without a key in this generation (a counter, or unknown).
    fn depend_on(&mut self, env: &mut Env<'_>, id: &SymbolId) -> bool {
        if id.is_empty() {
            return true;
        }
        if !durable_symbol_id(id) {
            return false;
        }
        if self.keys.contains_key(id) {
            return true;
        }
        let Some(key) = env.ids.key_of_id(id).cloned() else {
            return false;
        };
        if let Some(reference) = key.reference() {
            let path = env.inputs.path_id(&reference.path);
            if let Err(at) = self
                .declaring
                .binary_search_by_key(&path, |(path, _, _)| *path)
            {
                let (Some(text), exports) =
                    env.inputs
                        .declared(env.program, env.ids.exported_by_ref(), path)
                else {
                    return false;
                };
                self.declaring.insert(at, (path, text, exports));
            }
        }
        self.keys.insert(id.clone(), key);
        true
    }
}

/// A binder-level, non-alias symbol's declarations, kept across updates with
/// the dependencies of its id and of every declaration (ADR 0369): the text
/// and exports of each declaring file, the global scope (merges) and the
/// options. An alias is never kept: its target is resolved with the alias
/// followed, which can read types.
#[derive(Debug)]
struct RetainedSymbol {
    epoch: u64,
    key: std::sync::Arc<Key>,
    declarations: std::sync::Arc<[typefacts::Declaration]>,
    options: Digest,
    ambient: Digest,
    declaring: Vec<(u32, Digest, Digest)>,
}

impl RetainedSymbol {
    fn still_valid(&mut self, env: &mut Env<'_>, id: &SymbolId) -> bool {
        let epoch = env.inputs.epoch();
        if self.epoch == epoch {
            return true;
        }
        if env.inputs.options(env.program) != self.options
            || env.inputs.ambient(env.program) != self.ambient
        {
            return false;
        }
        for (path, text, exports) in &self.declaring {
            if env
                .inputs
                .declared(env.program, env.ids.exported_by_ref(), *path)
                != (Some(*text), *exports)
            {
                return false;
            }
        }
        if !env.ids.register_key(id, &self.key) {
            return false;
        }
        self.epoch = epoch;
        true
    }
}

/// How many program changes a retained symbol fact survives unasked.
const SYMBOL_FACT_EPOCHS: u64 = 4;

#[derive(Default)]
pub struct ReferenceIndex {
    /// `None` until the first reference query of a generation.
    merged: Option<HashMap<SymbolId, Vec<Indexed>>>,
    spaces: Option<HashMap<SymbolId, u8>>,
    paths: Vec<String>,
    /// The canonical symbols whose merged rows escaped into a retained fact.
    /// `ReleaseAnalysisState` prunes every other merged bucket; per-file
    /// symbol evidence remains sufficient for exact invalidation.
    used: HashSet<SymbolId>,
    /// `Some` once `release_analysis_state` pruned merged buckets: the only
    /// buckets still known whole. A bucket absent from it may be missing or
    /// hold just the rows an incremental refresh added, so a query for it
    /// rebuilds the merged index first.
    complete: Option<HashSet<SymbolId>>,
    /// Affected files removed from an already-materialized merged index. They
    /// are rescanned lazily at the established reference closure point so
    /// symbol counter minting retains its order.
    refresh_paths: HashSet<String>,
    /// The exact union removed from and added to refreshed contributions.
    /// `delta_exact` distinguishes known-empty from unavailable.
    changed_symbols: HashSet<SymbolId>,
    delta_exact: bool,
    /// This generation's per-file contributions.
    files: HashMap<String, FileReferences>,
    next_serial: u64,
    /// Binder-level answers kept across generations (ADR 0369): slots, and
    /// the slot of each clean path. A departed file's slot is emptied and
    /// the slots are compacted between tables.
    records: Vec<Option<BinderFile>>,
    by_path: HashMap<String, usize>,
    /// The last file looked up (address, input epoch, slot), so the entity
    /// demands of one run do not hash its path once per demand. File
    /// addresses are reused after a program change, which advances the epoch.
    current: Option<(usize, u64, usize)>,
    /// Binder-level symbol facts kept across generations, by id (ADR 0369),
    /// and the latest input epoch one was asked at: a fact nobody asked for in
    /// the last [`SYMBOL_FACT_EPOCHS`] epochs is dropped between tables.
    symbol_facts: HashMap<SymbolId, RetainedSymbol>,
    symbol_epoch: u64,
    retention: BinderRetention,
    /// Whether answers are retained at all: only once the session has
    /// accepted an update. A one-shot run (CLI, CI, a WASM check) never
    /// updates and pays nothing for retention (ADR 0369).
    retaining: bool,
    /// The alias names of a run that retains nothing: none is recorded.
    no_aliases: Option<AliasNames>,
}

fn space_from_bits(bits: u8) -> ReferenceSpace {
    match bits {
        1 => ReferenceSpace::Value,
        2 => ReferenceSpace::Type,
        3 => ReferenceSpace::Both,
        _ => ReferenceSpace::Neither,
    }
}

/// Go: `isTypeSpaceReference`. `IsPartOfTypeNode` handles the right side of a
/// `QualifiedName`, but an imported namespace is commonly its leftmost
/// identifier. Classify the complete qualified name so its enclosing
/// `TypeReference` (or `TypeQuery`) determines the reference space for every
/// identifier in the chain.
fn is_type_space_reference(mut node: P<Node>) -> bool {
    while let Some(parent) = node.parent()
        && is_qualified_name(parent)
    {
        node = parent;
    }
    is_part_of_type_node(node)
}

/// What the scan asks the checker at `node`, and whether the answer is
/// binder-level: `None` for an identifier the scan skips (a declaration name
/// other than a shorthand property's).
fn scanned_identifier(node: P<Node>) -> Option<bool> {
    if !is_declaration_name_or_import_property_name(node) {
        return Some(reference_resolves_without_types(node));
    }
    let parent = node.parent()?;
    // `{ value }` reads the value binding by scope lookup with the alias
    // kept (`resolve_entity_name(Value | Alias)`).
    (is_shorthand_property_assignment(parent) && parent.name() == Some(node)).then_some(true)
}

fn resolve(env: &mut Env<'_>, node: P<Node>) -> Option<P<tsrs_ast::Symbol>> {
    if is_declaration_name_or_import_property_name(node) {
        env.checker
            .get_shorthand_assignment_value_symbol(node.parent())
    } else {
        env.checker.get_symbol_at_location_exported(node)
    }
}

/// The names an alias may answer in one file: every name an import or export
/// declaration of the file mentions (a superset of its aliases), and the
/// aliases of the global scope.
pub(crate) struct AliasNames {
    file: std::sync::Arc<HashSet<String>>,
    global: std::sync::Arc<HashSet<String>>,
}

impl AliasNames {
    fn may_answer(&self, name: &str) -> bool {
        self.file.contains(name) || self.global.contains(name)
    }
}

/// Every identifier an import or export declaration of `file` holds, in any
/// namespace body: a superset of the names of the file's aliases. Syntax only.
fn file_alias_names(file: P<SourceFile>) -> HashSet<String> {
    fn identifiers(node: P<Node>, names: &mut HashSet<String>) {
        if is_identifier(node) {
            names.insert(node.text().to_owned());
        }
        node.for_each_child(&mut |child| {
            identifiers(child, names);
            false
        });
    }
    fn statements(node: P<Node>, names: &mut HashSet<String>) {
        match node.kind() {
            Kind::ImportDeclaration
            | Kind::JSImportDeclaration
            | Kind::ImportEqualsDeclaration
            | Kind::ExportDeclaration => identifiers(node, names),
            Kind::ModuleDeclaration | Kind::ModuleBlock => {
                node.for_each_child(&mut |child| {
                    statements(child, names);
                    false
                });
            }
            _ => {}
        }
    }
    let mut names = HashSet::new();
    file.as_node().for_each_child(&mut |child| {
        statements(child, &mut names);
        false
    });
    names
}

/// A TypeScript source or declaration file: JavaScript binds aliases,
/// exports and expando declarations from `require`, `module.exports` and
/// property assignments, which this module does not enumerate.
fn is_typescript(file: P<SourceFile>) -> bool {
    let name = file.file_name();
    [".ts", ".tsx", ".mts", ".cts"]
        .iter()
        .any(|extension| name.ends_with(extension))
}

/// Whether a file's binder-level answers may be kept at all: a TypeScript
/// file (a JavaScript file's `require` and `module.exports` bind aliases and
/// exports this module does not enumerate) whose module symbol no other file
/// augments (an augmentation's exports join the file's scope).
fn retainable(env: &mut Env<'_>, file: P<SourceFile>) -> bool {
    if !is_typescript(file) {
        return false;
    }
    let Some(symbol) = file.symbol() else {
        return true;
    };
    let merged = env.checker.get_merged_symbol(symbol);
    merged.declarations().iter().all(|declaration| {
        tsrs_ast::get_source_file_of_node(*declaration)
            .is_some_and(|other| other.addr() == file.addr())
    })
}

/// A symbol's two identities in one file, and its accumulator slots.
struct SymbolSlot {
    reference: SymbolId,
    canonical: SymbolId,
    binder: Option<(usize, usize)>,
    typed: Option<(usize, usize)>,
}

struct GroupAcc {
    id: SymbolId,
    spans: Vec<Span>,
    /// A span arrived before an earlier-starting one.
    unordered: bool,
}

/// One half of a file's contribution.
struct Acc {
    groups: Vec<GroupAcc>,
    group_slots: HashMap<SymbolId, usize>,
    spaces: Vec<(SymbolId, u8)>,
    space_slots: HashMap<SymbolId, usize>,
    durable: bool,
}

impl Acc {
    fn new() -> Self {
        Self {
            groups: Vec::new(),
            group_slots: HashMap::new(),
            spaces: Vec::new(),
            space_slots: HashMap::new(),
            durable: true,
        }
    }

    fn slots(&mut self, reference: &SymbolId, canonical: &SymbolId) -> (usize, usize) {
        if !durable_symbol_id(reference) || !durable_symbol_id(canonical) {
            self.durable = false;
        }
        let space = *self
            .space_slots
            .entry(reference.clone())
            .or_insert_with(|| {
                self.spaces.push((reference.clone(), 0));
                self.spaces.len() - 1
            });
        let group = *self
            .group_slots
            .entry(canonical.clone())
            .or_insert_with(|| {
                self.groups.push(GroupAcc {
                    id: canonical.clone(),
                    spans: Vec::new(),
                    unordered: false,
                });
                self.groups.len() - 1
            });
        (space, group)
    }

    fn push(&mut self, (space, group): (usize, usize), span: Span, bits: u8) {
        let group = &mut self.groups[group];
        // Parsed children normally arrive in source order. Preserve the
        // ordering contract without sorting every bucket, but detect and
        // repair any non-lexical compiler traversal defensively.
        if group
            .spans
            .last()
            .is_some_and(|last| last.start > span.start)
        {
            group.unordered = true;
        }
        group.spans.push(span);
        self.spaces[space].1 |= bits;
    }

    fn finish(self) -> (Vec<Group>, Vec<(SymbolId, u8)>, bool) {
        let mut groups: Vec<Group> = self
            .groups
            .into_iter()
            .map(|mut group| {
                if group.unordered {
                    group.spans.sort_by_key(|span| span.start);
                }
                Group {
                    id: group.id,
                    spans: group.spans,
                }
            })
            .collect();
        groups.sort_by(|left, right| left.id.cmp(&right.id));
        let mut spaces = self.spaces;
        spaces.sort_by(|left, right| left.0.cmp(&right.0));
        (groups, spaces, self.durable)
    }
}

/// Both halves of a file, each sorted by id, as one contribution.
fn merge_halves(
    binder: (&[Group], &[(SymbolId, u8)]),
    typed: (Vec<Group>, Vec<(SymbolId, u8)>),
) -> (Vec<Group>, Vec<(SymbolId, u8)>) {
    let (binder_groups, binder_spaces) = binder;
    let (typed_groups, typed_spaces) = typed;
    let mut groups = Vec::with_capacity(binder_groups.len() + typed_groups.len());
    let mut left = binder_groups.iter().peekable();
    let mut right = typed_groups.into_iter().peekable();
    loop {
        match (left.peek(), right.peek()) {
            (Some(l), Some(r)) if l.id == r.id => {
                let l = left.next().expect("peeked");
                let r = right.next().expect("peeked");
                let mut spans = Vec::with_capacity(l.spans.len() + r.spans.len());
                spans.extend_from_slice(&l.spans);
                spans.extend(r.spans);
                spans.sort_by_key(|span| span.start);
                groups.push(Group { id: r.id, spans });
            }
            (Some(l), Some(r)) if l.id < r.id => groups.push(left.next().expect("peeked").clone()),
            (Some(_), Some(_)) | (None, Some(_)) => groups.push(right.next().expect("peeked")),
            (Some(_), None) => groups.push(left.next().expect("peeked").clone()),
            (None, None) => break,
        }
    }
    let mut spaces: Vec<(SymbolId, u8)> =
        Vec::with_capacity(binder_spaces.len() + typed_spaces.len());
    let mut left = binder_spaces.iter().peekable();
    let mut right = typed_spaces.into_iter().peekable();
    loop {
        match (left.peek(), right.peek()) {
            (Some(l), Some(r)) if l.0 == r.0 => {
                let bits = l.1 | r.1;
                left.next();
                let (id, _) = right.next().expect("peeked");
                spaces.push((id, bits));
            }
            (Some(l), Some(r)) if l.0 < r.0 => spaces.push(left.next().expect("peeked").clone()),
            (Some(_), Some(_)) | (None, Some(_)) => spaces.push(right.next().expect("peeked")),
            (Some(_), None) => spaces.push(left.next().expect("peeked").clone()),
            (None, None) => break,
        }
    }
    (groups, spaces)
}

/// One walk over a file. In full mode every scanned identifier is resolved
/// and lands in its half, and the identifiers a reuse must resolve again are
/// recorded. In reuse mode only those are resolved, in the order the full walk
/// resolved them, and subtrees holding none are skipped: the checker sees the
/// same type questions in the same order either way.
struct Walk<'r> {
    slots: HashMap<usize, SymbolSlot>,
    binder: Acc,
    typed: Acc,
    /// Names an alias may answer: the file's own aliases and the global ones.
    aliases: &'r AliasNames,
    /// Reuse mode: the retained targets, and the next one.
    expected: Option<(&'r [Target], usize)>,
    /// Full mode: what a reuse must resolve again.
    targets: Vec<Target>,
    failed: bool,
}

/// A symbol's identities, resolved the first time the file names it: later
/// identifiers of the same symbol add their span through the slots, without
/// hashing identity strings again. The first sight makes exactly the `id_for`
/// calls the identifier would, in the same order, so counter-minted ids come
/// out unchanged.
fn slot_of<'s>(
    slots: &'s mut HashMap<usize, SymbolSlot>,
    env: &mut Env<'_>,
    symbol: P<tsrs_ast::Symbol>,
) -> &'s mut SymbolSlot {
    slots.entry(symbol.addr()).or_insert_with(|| {
        let reference = env.id_for(symbol);
        let canonical = env.canonical_symbol(symbol);
        let canonical = env.id_for(canonical);
        SymbolSlot {
            reference,
            canonical,
            binder: None,
            typed: None,
        }
    })
}

impl<'r> Walk<'r> {
    /// Resolves a type-resolved identifier into the typed half.
    fn resolve_typed(&mut self, env: &mut Env<'_>, node: P<Node>, span: Span, bits: u8) {
        if let Some(symbol) = resolve(env, node) {
            let slot = slot_of(&mut self.slots, env, symbol);
            let slots = *slot
                .typed
                .get_or_insert_with(|| self.typed.slots(&slot.reference, &slot.canonical));
            self.typed.push(slots, span, bits);
        }
    }

    /// Full mode at one scanned identifier.
    fn full(
        &mut self,
        env: &mut Env<'_>,
        node: P<Node>,
        binder_resolved: bool,
        span: Span,
        bits: u8,
    ) {
        if !binder_resolved {
            self.targets.push(Target {
                start: span.start,
                binder: None,
            });
            self.resolve_typed(env, node, span, bits);
            return;
        }
        let sensitive = self.aliases.may_answer(node.text());
        let Some(symbol) = resolve(env, node) else {
            if sensitive {
                self.targets.push(Target {
                    start: span.start,
                    binder: Some(None),
                });
            }
            return;
        };
        let slot = slot_of(&mut self.slots, env, symbol);
        if !durable_symbol_id(&slot.reference) || !durable_symbol_id(&slot.canonical) {
            // A generation-scoped id (an unresolved type name's placeholder,
            // a symbol with no provable key) means nothing next generation:
            // the identifier joins the half resolved again on every reuse.
            self.targets.push(Target {
                start: span.start,
                binder: None,
            });
            let slots = *slot
                .typed
                .get_or_insert_with(|| self.typed.slots(&slot.reference, &slot.canonical));
            self.typed.push(slots, span, bits);
            return;
        }
        let slots = *slot
            .binder
            .get_or_insert_with(|| self.binder.slots(&slot.reference, &slot.canonical));
        self.binder.push(slots, span, bits);
        if sensitive {
            self.targets.push(Target {
                start: span.start,
                binder: Some(Some((slot.reference.clone(), slot.canonical.clone()))),
            });
        }
    }

    /// Reuse mode at one scanned identifier: `false` when it contradicts the
    /// recorded walk (the reuse is abandoned).
    fn reuse(
        &mut self,
        env: &mut Env<'_>,
        node: P<Node>,
        binder_resolved: bool,
        span: Span,
        bits: u8,
    ) -> bool {
        let Some((targets, next)) = self.expected else {
            return false;
        };
        let Some(target) = targets
            .get(next)
            .filter(|target| target.start == span.start)
        else {
            // Not a target: only a binder-resolved identifier no alias can
            // answer may be skipped.
            return binder_resolved && !self.aliases.may_answer(node.text());
        };
        self.expected = Some((targets, next + 1));
        match &target.binder {
            // Type-resolved, or binder-resolved to a generation-scoped id.
            None => {
                self.resolve_typed(env, node, span, bits);
                true
            }
            Some(expected) if binder_resolved => {
                let found = resolve(env, node).map(|symbol| {
                    let slot = slot_of(&mut self.slots, env, symbol);
                    (slot.reference.clone(), slot.canonical.clone())
                });
                found == *expected
            }
            _ => false,
        }
    }

    fn visit(&mut self, env: &mut Env<'_>, text: &str, node: P<Node>) -> bool {
        if let Some((targets, next)) = self.expected {
            let Some(target) = targets.get(next) else {
                // Every target is done: nothing else to resolve.
                return true;
            };
            if node
                .end()
                .source_offset()
                .is_some_and(|end| end <= target.start)
            {
                // The subtree holds no target.
                return false;
            }
        }
        if is_identifier(node)
            && let Some(binder_resolved) = scanned_identifier(node)
        {
            let start = skip_trivia(text, node.pos().source_offset().unwrap_or(0));
            let end = node.end().source_offset().unwrap_or(start);
            let span = Span { start, end };
            let bits = if is_type_space_reference(node) { 2 } else { 1 };
            if self.expected.is_some() {
                if !self.reuse(env, node, binder_resolved, span, bits) {
                    self.failed = true;
                    return true;
                }
            } else {
                self.full(env, node, binder_resolved, span, bits);
            }
        }
        let mut stop = false;
        node.for_each_child(&mut |child| {
            stop = self.visit(env, text, child);
            stop
        });
        stop
    }

    fn run(
        env: &mut Env<'_>,
        file: P<SourceFile>,
        aliases: &'r AliasNames,
        expected: Option<&'r [Target]>,
    ) -> Self {
        let mut walk = Walk {
            slots: HashMap::new(),
            binder: Acc::new(),
            typed: Acc::new(),
            aliases,
            expected: expected.map(|targets| (targets, 0)),
            targets: Vec::new(),
            failed: false,
        };
        let text = file.text();
        file.as_node()
            .for_each_child(&mut |child| walk.visit(env, text, child));
        if let Some((targets, next)) = walk.expected
            && next != targets.len()
        {
            walk.failed = true;
        }
        walk
    }
}

impl ReferenceIndex {
    /// Contribution files held (tests).
    #[must_use]
    pub fn files(&self) -> &HashMap<String, FileReferences> {
        &self.files
    }

    /// Whether the merged index is built (tests).
    #[must_use]
    pub fn is_merged(&self) -> bool {
        self.merged.is_some()
    }

    /// Whether the changed-symbol set is an exact delta (tests).
    #[must_use]
    pub fn delta_exact(&self) -> bool {
        self.delta_exact
    }

    #[must_use]
    pub fn changed_symbol_set(&self) -> &HashSet<SymbolId> {
        &self.changed_symbols
    }

    /// The paths with retained binder-level answers (tests).
    #[must_use]
    pub fn retained_paths(&self) -> Vec<&str> {
        let mut paths: Vec<&str> = self.by_path.keys().map(String::as_str).collect();
        paths.sort_unstable();
        paths
    }

    /// Reuse counts since the last [`ReferenceIndex::take_retention`].
    pub fn take_retention(&mut self) -> BinderRetention {
        std::mem::take(&mut self.retention)
    }

    /// Go: `reset`. Drops this generation's index; the retained binder-level
    /// answers stay, guarded by their dependencies.
    pub fn reset(&mut self) {
        self.merged = None;
        self.spaces = None;
        self.refresh_paths.clear();
        self.changed_symbols.clear();
        self.delta_exact = false;
        self.files.clear();
        self.paths.clear();
        self.used.clear();
        self.complete = None;
        self.current = None;
        let epoch = self.symbol_epoch;
        self.symbol_facts
            .retain(|_, fact| fact.epoch + SYMBOL_FACT_EPOCHS >= epoch);
        // Between tables no slot index is held: compact the emptied slots.
        if self.records.len() > 2 * self.by_path.len() + 64 {
            let mut records = Vec::with_capacity(self.by_path.len());
            for slot in self.by_path.values_mut() {
                records.push(self.records[*slot].take());
                *slot = records.len() - 1;
            }
            self.records = records;
        }
    }

    /// The session accepted an update: from now on binder-level answers are
    /// recorded for reuse by later generations.
    pub fn start_retaining(&mut self) {
        self.retaining = true;
    }

    /// Whether binder-level answers are recorded for reuse.
    #[must_use]
    pub fn retaining(&self) -> bool {
        self.retaining
    }

    /// Drops everything, the retained answers included (close).
    pub fn forget(&mut self) {
        self.reset();
        self.records.clear();
        self.by_path.clear();
        self.symbol_facts.clear();
    }

    /// The retained declarations of the binder-level, non-alias symbol `id`,
    /// when every dependency they were computed under still holds; the id is
    /// then answerable in this generation (ADR 0369).
    pub fn retained_symbol_fact(
        &mut self,
        env: &mut Env<'_>,
        id: &SymbolId,
    ) -> Option<std::sync::Arc<[typefacts::Declaration]>> {
        if !self.retaining {
            return None;
        }
        self.symbol_epoch = env.inputs.epoch();
        let retained = self.symbol_facts.get_mut(id)?;
        if retained.still_valid(env, id) {
            self.retention.reused_symbol_facts += 1;
            return Some(std::sync::Arc::clone(&retained.declarations));
        }
        self.symbol_facts.remove(id);
        None
    }

    /// Keeps `declarations` of `symbol`, the binder-level root `id` names, if
    /// it is not an alias and every declaration has a source file.
    pub fn retain_symbol_fact(
        &mut self,
        env: &mut Env<'_>,
        id: &SymbolId,
        symbol: P<tsrs_ast::Symbol>,
        declarations: &std::sync::Arc<[typefacts::Declaration]>,
    ) {
        self.retention.resolved_symbol_facts += 1;
        if !self.retaining
            || !durable_symbol_id(id)
            || symbol.flags().intersects(SymbolFlags::Alias)
        {
            return;
        }
        let Some(key) = env.ids.key_of_id(id).cloned() else {
            return;
        };
        let mut paths: Vec<u32> = Vec::new();
        for node in symbol.declarations() {
            let Some(file) = tsrs_ast::get_source_file_of_node(*node) else {
                return;
            };
            if !is_typescript(file) {
                return;
            }
            paths.push(env.inputs.path_id(&clean(file.file_name())));
        }
        if let Some(reference) = key.reference() {
            paths.push(env.inputs.path_id(&reference.path));
        }
        paths.sort_unstable();
        paths.dedup();
        let mut declaring = Vec::with_capacity(paths.len());
        for path in paths {
            let (Some(text), exports) =
                env.inputs
                    .declared(env.program, env.ids.exported_by_ref(), path)
            else {
                return;
            };
            declaring.push((path, text, exports));
        }
        self.symbol_facts.insert(
            id.clone(),
            RetainedSymbol {
                epoch: env.inputs.epoch(),
                key,
                declarations: std::sync::Arc::clone(declarations),
                options: env.inputs.options(env.program),
                ambient: env.inputs.ambient(env.program),
                declaring,
            },
        );
    }

    /// Go: `releaseAnalysisState`: drops the whole index. Merging moves each
    /// file's spans into the merged buckets, so a pruned bucket cannot be
    /// restored from the per-file entries; a later query in the same generation
    /// rebuilds the index from the program (reusing the retained halves).
    pub fn release_analysis_state(&mut self) {
        self.merged = None;
        self.spaces = None;
        self.refresh_paths.clear();
        self.files.clear();
        self.paths.clear();
        self.used.clear();
        self.complete = None;
    }

    /// Go: `ensure`.
    pub fn ensure(&mut self, env: &mut Env<'_>) {
        if self.merged.is_none() {
            let started = std::time::Instant::now();
            self.merged = Some(self.build(env));
            self.retention.build += started.elapsed();
            self.refresh_paths.clear();
            self.complete = None;
            return;
        }
        if self.refresh_paths.is_empty() {
            return;
        }
        let mut paths: Vec<String> = self.refresh_paths.iter().cloned().collect();
        paths.sort();
        let mut touched: HashSet<SymbolId> = HashSet::new();
        for path in paths {
            let Some(file) = env.program.get_source_file(&path) else {
                continue;
            };
            if file.is_declaration_file() {
                continue;
            }
            let entry = self.contribution(env, file);
            self.files.insert(path.clone(), entry);
            let path_index = self.paths.partition_point(|candidate| *candidate < path);
            if self.paths.get(path_index) != Some(&path) {
                // Incremental refresh is only selected when program
                // membership is unchanged, so every refreshed path belongs to
                // the established compact path table.
                continue;
            }
            let Some(merged) = &mut self.merged else {
                continue;
            };
            let entry = self.files.get_mut(&path).expect("just inserted");
            for group in &mut entry.refs {
                touched.insert(group.id.clone());
                if self.delta_exact {
                    self.changed_symbols.insert(group.id.clone());
                }
                let references = merged.entry(group.id.clone()).or_default();
                for span in std::mem::take(&mut group.spans) {
                    references.push(Indexed {
                        path: u32::try_from(path_index).unwrap_or(u32::MAX),
                        start: span.start,
                        end: span.end,
                    });
                }
            }
            entry.merged = true;
        }
        if let Some(merged) = &mut self.merged {
            for id in touched {
                if let Some(references) = merged.get_mut(&id) {
                    references
                        .sort_by_key(|reference| (reference.path, reference.start, reference.end));
                }
            }
        }
        self.refresh_paths.clear();
    }

    /// Go: `build`: every non-declaration file's contribution, merged into
    /// the current generation's reference index. Merging in path order, with
    /// each per-file group already in byte order, preserves the References
    /// ordering contract (path, then start byte) without a global sort.
    fn build(&mut self, env: &mut Env<'_>) -> HashMap<SymbolId, Vec<Indexed>> {
        let mut paths: Vec<String> = Vec::with_capacity(env.program.source_files().len());
        for file in env.program.source_files() {
            if file.is_declaration_file() {
                continue;
            }
            let path = clean(file.file_name());
            paths.push(path.clone());
            let entry = self.contribution(env, *file);
            if self.delta_exact {
                for group in &entry.refs {
                    self.changed_symbols.insert(group.id.clone());
                }
            }
            self.files.insert(path, entry);
        }
        // A departed file's retained answers go with it.
        let records = &mut self.records;
        let files = &self.files;
        self.by_path.retain(|path, slot| {
            let present = files.contains_key(path);
            if !present {
                records[*slot] = None;
            }
            present
        });
        self.current = None;
        paths.sort();
        let mut references: HashMap<SymbolId, Vec<Indexed>> = HashMap::new();
        let mut spaces: HashMap<SymbolId, u8> = HashMap::new();
        for (path_index, path) in paths.iter().enumerate() {
            let Some(entry) = self.files.get_mut(path) else {
                continue;
            };
            for group in &mut entry.refs {
                let indexed = references.entry(group.id.clone()).or_default();
                for span in std::mem::take(&mut group.spans) {
                    indexed.push(Indexed {
                        path: u32::try_from(path_index).unwrap_or(u32::MAX),
                        start: span.start,
                        end: span.end,
                    });
                }
            }
            entry.merged = true;
            for (id, bits) in &entry.spaces {
                *spaces.entry(id.clone()).or_default() |= *bits;
            }
        }
        self.paths = paths;
        self.spaces = Some(spaces);
        references
    }

    /// The retained record of `file`, its dependencies proven at the current
    /// input epoch; a record whose dependencies changed is replaced by an
    /// empty one.
    fn binder_file(&mut self, env: &mut Env<'_>, file: P<SourceFile>) -> usize {
        let epoch = env.inputs.epoch();
        if let Some((address, at, slot)) = self.current
            && address == file.addr()
            && at == epoch
        {
            return slot;
        }
        let path = clean(file.file_name());
        let slot = match self.by_path.get(&path) {
            Some(&slot) => slot,
            None => {
                self.records.push(None);
                self.by_path.insert(path, self.records.len() - 1);
                self.records.len() - 1
            }
        };
        let valid = self.records[slot]
            .as_mut()
            .is_some_and(|record| record.still_valid(env, file));
        if !valid {
            self.records[slot] = Some(BinderFile::baseline(env, file));
        }
        self.current = Some((file.addr(), epoch, slot));
        slot
    }

    /// One file's contribution: the retained binder-resolved half with the
    /// type-resolved half resolved now, or a whole scan.
    fn contribution(&mut self, env: &mut Env<'_>, file: P<SourceFile>) -> FileReferences {
        self.next_serial += 1;
        let serial = self.next_serial;
        if !self.retaining {
            self.retention.scanned_reference_files += 1;
            let aliases = self.no_aliases.get_or_insert_with(|| AliasNames {
                file: std::sync::Arc::new(HashSet::new()),
                global: std::sync::Arc::new(HashSet::new()),
            });
            let walk = Walk::run(env, file, aliases, None);
            let (binder_groups, binder_spaces, binder_durable) = walk.binder.finish();
            let (typed_groups, typed_spaces, typed_durable) = walk.typed.finish();
            let (refs, spaces) = merge_halves(
                (&binder_groups, &binder_spaces),
                (typed_groups, typed_spaces),
            );
            return FileReferences {
                refs,
                spaces,
                durable: binder_durable && typed_durable,
                merged: false,
                serial,
            };
        }
        let slot = self.binder_file(env, file);
        let record = self.records[slot]
            .as_mut()
            .expect("binder_file fills its slot");
        let aliases = record.alias_names(env);
        if let Some(retained) = record.refs.take() {
            let walk = Walk::run(env, file, &aliases, Some(&retained.targets));
            if !walk.failed {
                let typed = walk.typed.finish();
                let typed_durable = typed.2;
                let (refs, spaces) =
                    merge_halves((&retained.groups, &retained.spaces), (typed.0, typed.1));
                record.refs = Some(retained);
                self.retention.reused_reference_files += 1;
                return FileReferences {
                    refs,
                    spaces,
                    durable: typed_durable,
                    merged: false,
                    serial,
                };
            }
        }
        self.retention.scanned_reference_files += 1;
        let walk = Walk::run(env, file, &aliases, None);
        let targets = walk.targets;
        let (binder_groups, binder_spaces, binder_durable) = walk.binder.finish();
        let (typed_groups, typed_spaces, typed_durable) = walk.typed.finish();
        let (refs, spaces) = merge_halves(
            (&binder_groups, &binder_spaces),
            (typed_groups, typed_spaces),
        );
        // Only an all-binder-id half is kept: a counter means nothing in the
        // next generation. Answers an alias can give are checked on reuse,
        // so only the ids held directly add dependencies; an alias's own id
        // is declared in the file.
        if record.retainable
            && binder_durable
            && binder_spaces
                .iter()
                .all(|(id, _)| record.depend_on(env, id))
        {
            record.refs = Some(BinderReferences {
                groups: binder_groups,
                spaces: binder_spaces,
                targets,
            });
        }
        FileReferences {
            refs,
            spaces,
            durable: binder_durable && typed_durable,
            merged: false,
            serial,
        }
    }

    /// The retained entity symbol at `[start, end)` of `file`, if one was
    /// recorded under dependencies that still hold (ADR 0369).
    pub fn retained_symbol(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
        span: (u64, u64),
    ) -> Option<(SymbolId, SymbolClaim)> {
        let slot = self.binder_file(env, file);
        let found = self.records[slot]
            .as_ref()
            .and_then(|record| record.symbols.get(&span).cloned());
        if found.is_some() {
            self.retention.reused_symbols += 1;
        } else {
            self.retention.resolved_symbols += 1;
        }
        found
    }

    /// Whether an entity symbol at `node` of `file` may be retained: a
    /// binder-resolved position no alias can answer, in a retainable file.
    pub fn may_retain_symbol(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
        node: P<Node>,
    ) -> bool {
        if !self.retaining {
            return false;
        }
        let slot = self.binder_file(env, file);
        let record = self.records[slot]
            .as_ref()
            .expect("binder_file fills its slot");
        if !record.retainable || record.aliases.contains(node.text()) {
            return false;
        }
        !env.inputs.global_aliases(env.checker).contains(node.text())
    }

    /// Keeps an entity symbol resolved at a binder-resolved position.
    pub fn retain_symbol(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
        span: (u64, u64),
        answer: (SymbolId, SymbolClaim),
    ) {
        let slot = self.binder_file(env, file);
        let record = self.records[slot]
            .as_mut()
            .expect("binder_file fills its slot");
        if record.depend_on(env, &answer.0) {
            record.symbols.insert(span, answer);
        }
    }

    /// Go: `locations`.
    #[must_use]
    pub fn locations(&self, id: &SymbolId) -> Vec<Location> {
        let Some(references) = self.merged.as_ref().and_then(|merged| merged.get(id)) else {
            return Vec::new();
        };
        references
            .iter()
            .map(|reference| Location {
                path: self.paths[reference.path as usize].as_str().into(),
                start_byte: u64::from(reference.start),
                end_byte: u64::from(reference.end),
            })
            .collect()
    }

    /// Go: `query`: completes the bucket for `id`, marks it used, and returns
    /// its locations. The merged index must already be ensured.
    pub fn query(&mut self, env: &mut Env<'_>, id: &SymbolId) -> Vec<Location> {
        if self
            .complete
            .as_ref()
            .is_some_and(|complete| !complete.contains(id))
        {
            // Pruned by `release_analysis_state`; retained contributions
            // rescan into a complete index.
            self.merged = Some(self.build(env));
            self.refresh_paths.clear();
            self.complete = None;
        }
        self.mark_used(id);
        self.locations(id)
    }

    /// Go: `markUsed`.
    fn mark_used(&mut self, id: &SymbolId) {
        self.used.insert(id.clone());
    }

    /// Go: `ChangedReferences` after `ensure`: the sorted changed symbols, or
    /// `None` when the delta is not exact.
    #[must_use]
    pub fn changed(&self) -> Option<Vec<SymbolId>> {
        if !self.delta_exact {
            return None;
        }
        let mut ids: Vec<SymbolId> = self.changed_symbols.iter().cloned().collect();
        ids.sort();
        Some(ids)
    }

    /// Go: `spaceFor`.
    pub fn space_for(&mut self, env: &mut Env<'_>, id: &SymbolId) -> ReferenceSpace {
        self.ensure(env);
        let spaces = self.spaces.get_or_insert_with(|| {
            let mut spaces: HashMap<SymbolId, u8> = HashMap::new();
            for entry in self.files.values() {
                for (id, bits) in &entry.spaces {
                    *spaces.entry(id.clone()).or_default() |= *bits;
                }
            }
            spaces
        });
        match spaces.get(id) {
            Some(bits) if *bits != 0 => space_from_bits(*bits),
            _ => ReferenceSpace::Neither,
        }
    }

    /// A whole scan of one file, retaining nothing (tests).
    #[cfg(test)]
    fn scan(&mut self, env: &mut Env<'_>, file: P<SourceFile>) -> FileReferences {
        self.next_serial += 1;
        let aliases = AliasNames {
            file: std::sync::Arc::new(file_alias_names(file)),
            global: env.inputs.global_aliases(env.checker),
        };
        let walk = Walk::run(env, file, &aliases, None);
        let (binder_groups, binder_spaces, binder_durable) = walk.binder.finish();
        let (typed_groups, typed_spaces, typed_durable) = walk.typed.finish();
        let (refs, spaces) = merge_halves(
            (&binder_groups, &binder_spaces),
            (typed_groups, typed_spaces),
        );
        FileReferences {
            refs,
            spaces,
            durable: binder_durable && typed_durable,
            merged: false,
            serial: self.next_serial,
        }
    }
}

#[cfg(test)]
impl ReferenceIndex {
    /// The scan as ported, one identity lookup per identifier; the equivalence
    /// test compares [`ReferenceIndex::scan`] against it.
    pub(crate) fn scan_reference(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
    ) -> FileReferences {
        struct Scan {
            references: HashMap<SymbolId, Vec<Span>>,
            spaces: HashMap<SymbolId, u8>,
            unordered: HashSet<SymbolId>,
            durable: bool,
        }

        fn visit(env: &mut Env<'_>, text: &str, node: P<Node>, scan: &mut Scan) {
            if is_identifier(node) {
                let symbol = if !is_declaration_name_or_import_property_name(node) {
                    env.checker.get_symbol_at_location_exported(node)
                } else if let Some(parent) = node.parent()
                    && is_shorthand_property_assignment(parent)
                    && parent.name() == Some(node)
                {
                    // `{ value }` names the property it declares and reads the
                    // value binding at once. The name is a declaration name,
                    // so without this the read of `value` is in no reference
                    // list, and a census of its references misses the object
                    // holding it.
                    env.checker
                        .get_shorthand_assignment_value_symbol(Some(parent))
                } else {
                    None
                };
                if let Some(symbol) = symbol {
                    let reference_id = env.id_for(symbol);
                    let canonical = env.canonical_symbol(symbol);
                    let id = env.id_for(canonical);
                    if !durable_symbol_id(&id) || !durable_symbol_id(&reference_id) {
                        scan.durable = false;
                    }
                    let start = skip_trivia(text, node.pos().source_offset().unwrap_or(0));
                    let end = node.end().source_offset().unwrap_or(start);
                    let spans = scan.references.entry(id.clone()).or_default();
                    // Parsed children normally arrive in source order.
                    // Preserve the ordering contract without sorting every
                    // bucket, but detect and repair any non-lexical compiler
                    // traversal defensively.
                    if spans.last().is_some_and(|last| last.start > start) {
                        scan.unordered.insert(id);
                    }
                    spans.push(Span { start, end });
                    *scan.spaces.entry(reference_id).or_default() |=
                        if is_type_space_reference(node) { 2 } else { 1 };
                }
            }
            node.for_each_child(&mut |child| {
                visit(env, text, child, scan);
                false
            });
        }

        let mut scan = Scan {
            references: HashMap::new(),
            spaces: HashMap::new(),
            unordered: HashSet::new(),
            durable: true,
        };
        let text = file.text();
        file.as_node().for_each_child(&mut |child| {
            visit(env, text, child, &mut scan);
            false
        });
        let mut refs: Vec<Group> = scan
            .references
            .into_iter()
            .map(|(id, mut spans)| {
                if scan.unordered.contains(&id) {
                    spans.sort_by_key(|span| span.start);
                }
                Group { id, spans }
            })
            .collect();
        refs.sort_by(|left, right| left.id.cmp(&right.id));
        let mut spaces: Vec<(SymbolId, u8)> = scan.spaces.into_iter().collect();
        spaces.sort_by(|left, right| left.0.cmp(&right.0));
        self.next_serial += 1;
        FileReferences {
            refs,
            spaces,
            durable: scan.durable,
            merged: false,
            serial: self.next_serial,
        }
    }

    /// Everything but the serial: the same groups, spans, spaces and
    /// durability.
    pub(crate) fn same_content(left: &FileReferences, right: &FileReferences) -> bool {
        left.durable == right.durable
            && left.spaces == right.spaces
            && left.refs.len() == right.refs.len()
            && left
                .refs
                .iter()
                .zip(&right.refs)
                .all(|(a, b)| a.id == b.id && a.spans == b.spans)
    }

    pub(crate) fn scan_pair_for_test(
        &mut self,
        env: &mut Env<'_>,
        file: P<SourceFile>,
    ) -> (FileReferences, FileReferences) {
        let fast = self.scan(env, file);
        let reference = self.scan_reference(env, file);
        (fast, reference)
    }
}

#[cfg(test)]
impl ReferenceIndex {
    /// Go sets `referenceIndex.used` directly.
    pub(crate) fn set_used_for_test(&mut self, ids: &[SymbolId]) {
        self.used = ids.iter().cloned().collect();
    }
}
