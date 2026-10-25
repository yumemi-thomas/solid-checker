//! Pure-function keys for binder-level symbols (ADR 0366, STABLE-IDS slice 1).
//!
//! A binder-level symbol's id is a function of the program and nothing else:
//! not of which symbol asked first, which checker history ran, or the order the
//! files were given. The key is one of
//!
//! * `Intrinsic`: one of the checker's own declaration-less singletons,
//!   matched by pointer, never by name;
//! * `Global(name)`: an entry of the checker's `globals` table, merged by name
//!   (so `JSON` has one id whichever lib file declares it);
//! * `Export`: the span-insensitive hash of declaring path and name, for a
//!   symbol a module exports;
//! * `Local`: for a symbol a source file (not a `.d.ts`) declares and does
//!   not export, the minimum declaration by `(path, offset)` (not
//!   `declarations()[0]`, which is merge order) named by its path, its
//!   shift-stable [`LocalChains`] chain, and the symbol's name;
//! * `Decl`: the name span and name of that minimum declaration, for every
//!   other declared symbol (a `.d.ts` declaration, or a declaration the
//!   chain does not reach).
//!
//! Only a symbol proven binder-level has a key. A derived symbol (an
//! instantiation, a transient clone, a mapped or spread property) shares its
//! source's declarations, so keying it by declaration would hand it the
//! source's id (invariant 3); it keeps a counter, as does any symbol whose
//! binder-ness cannot be proven (invariant 2).
//!
//! One derived family has a key of its own (ADR 0371): a multi-origin
//! property, which the checker builds from several properties (a union or
//! intersection property, `create_union_or_intersection_property`, or a
//! spread-merged property). Its `Set` key is its combination kind, its name
//! and the sorted set of the binder keys of its declarations, hashed in a
//! domain of its own, so it never shares an id with a binder-level symbol.

use crate::durable::{
    DurableRef, durable_ref_for_declaration, hashed_symbol_id, minimum_declaration_ref, name_bytes,
    normalized_name,
};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use tsrs_ast::{
    CheckFlags, Kind, Node, SourceFile, Symbol, SymbolFlags, get_source_file_of_node,
    is_declaration, is_identifier, is_private_identifier, is_property_name_literal,
};
use tsrs_checker::types::{ObjectFlags, Type, TypeFlags};
use tsrs_compiler::Checker;
use tsrs_core::P;
use tsrs_scanner::skip_trivia;

/// A declaration-less checker singleton.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Intrinsic {
    Undefined,
    Arguments,
    Require,
    Unknown,
    GlobalThis,
}

impl Intrinsic {
    fn name(self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Arguments => "arguments",
            Self::Require => "require",
            Self::Unknown => "unknown",
            Self::GlobalThis => "globalThis",
        }
    }

    fn symbol(self, checker: &Checker) -> P<Symbol> {
        match self {
            Self::Undefined => checker.undefined_symbol,
            Self::Arguments => checker.arguments_symbol,
            Self::Require => checker.require_symbol,
            Self::Unknown => checker.unknown_symbol,
            Self::GlobalThis => checker.global_this_symbol,
        }
    }

    fn of(checker: &Checker, symbol: P<Symbol>) -> Option<Self> {
        [
            Self::Undefined,
            Self::Arguments,
            Self::Require,
            Self::Unknown,
            Self::GlobalThis,
        ]
        .into_iter()
        .find(|which| which.symbol(checker).addr() == symbol.addr())
    }
}

/// How the checker combined a multi-origin property's constituents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Combination {
    /// A property of a union type.
    Union,
    /// A property of an intersection type.
    Intersection,
    /// A property an object spread merged from its left and right sides.
    Spread,
}

impl Combination {
    fn name(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Intersection => "intersection",
            Self::Spread => "spread",
        }
    }
}

/// The pointer-free key a symbol's id is the hash of.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Key {
    Decl(DurableRef),
    /// A source file's unexported declaration, by its [`LocalChains`] chain:
    /// no span, so an edit before it keeps its id (ADR 03XX-stable-locals).
    Local {
        path: String,
        chain: Chain,
        name: String,
    },
    Export(DurableRef),
    Global(String),
    Intrinsic(Intrinsic),
    /// A multi-origin property: the combination, the normalized name, and
    /// the binder keys of its origins without duplicates (a union's ordered
    /// by id, an intersection's and a spread's in written order).
    Set {
        combination: Combination,
        name: String,
        origins: Vec<Key>,
    },
    /// A derived symbol keyed by its source and exact type context
    /// (`derived_key.rs`); `tag == "root"` is an instantiation the root
    /// answers for, whose id is the root's own.
    Derived {
        tag: &'static str,
        name: String,
        root: String,
        context: [u8; 32],
    },
}

impl Key {
    /// `symbol:h:<24 hex>`. `Decl` and `Export` hash exactly what the ids of
    /// an unmerged, non-private symbol always hashed.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Decl(reference) => reference.id(),
            Self::Export(reference) => reference.exported_id(),
            Self::Local { path, chain, name } => {
                // `\0local 0 path-length 0 path chain name`: the domain starts
                // with a NUL, which no other key's input does (a `Decl` input
                // begins with its path, never empty, never NUL), and the chain
                // has a fixed length, so the path's length makes the split exact.
                let name = name_bytes(name);
                let mut payload = Vec::with_capacity(48 + path.len() + name.len());
                payload.extend_from_slice(path.len().to_string().as_bytes());
                payload.push(0);
                payload.extend_from_slice(path.as_bytes());
                payload.extend_from_slice(chain);
                payload.extend_from_slice(&name);
                hashed(b"\0local", payload)
            }
            Self::Global(name) => hashed(b"global", name_bytes(name)),
            Self::Intrinsic(which) => hashed(b"intrinsic", which.name().as_bytes().to_vec()),
            Self::Set {
                combination,
                name,
                origins,
            } => {
                // `synth 0 combination 0 name-length 0 name (0 origin-id)*`:
                // the combination is a word where a `Decl` input has digits,
                // and the name's length makes the name/origins split exact.
                let name = name_bytes(name);
                let mut payload = Vec::with_capacity(32 + name.len() + 34 * origins.len());
                payload.extend_from_slice(combination.name().as_bytes());
                payload.push(0);
                payload.extend_from_slice(name.len().to_string().as_bytes());
                payload.push(0);
                payload.extend_from_slice(&name);
                for origin in origins {
                    payload.push(0);
                    payload.extend_from_slice(origin.id().as_bytes());
                }
                hashed(b"synth", payload)
            }
            Self::Derived {
                tag,
                name,
                root,
                context,
            } => {
                if *tag == "root" {
                    return root.clone();
                }
                let name = name_bytes(name);
                let mut payload = Vec::with_capacity(96 + name.len());
                payload.extend_from_slice(tag.as_bytes());
                payload.push(0);
                payload.extend_from_slice(name.len().to_string().as_bytes());
                payload.push(0);
                payload.extend_from_slice(&name);
                payload.push(0);
                payload.extend_from_slice(root.as_bytes());
                payload.push(0);
                payload.extend_from_slice(context);
                hashed(b"derived", payload)
            }
        }
    }

    /// The declaration reference of a `Decl` or `Export` key.
    #[must_use]
    pub fn reference(&self) -> Option<&DurableRef> {
        match self {
            Self::Decl(reference) | Self::Export(reference) => Some(reference),
            Self::Local { .. }
            | Self::Global(_)
            | Self::Intrinsic(_)
            | Self::Set { .. }
            | Self::Derived { .. } => None,
        }
    }

    /// Whether the key names a binder-level symbol. A `Set` key names a
    /// checker-derived property, a `Derived` key a derived symbol: neither is
    /// re-resolved from its key.
    #[must_use]
    pub fn is_binder(&self) -> bool {
        !matches!(self, Self::Set { .. } | Self::Derived { .. })
    }
}

fn hashed(domain: &[u8], payload: Vec<u8>) -> String {
    let mut input = Vec::with_capacity(domain.len() + payload.len() + 1);
    input.extend_from_slice(domain);
    input.push(0);
    input.extend_from_slice(&payload);
    hashed_symbol_id(&Sha256::digest(&input))
}

/// Follows the checker's merge records to the symbol a pre-merge binder
/// symbol stands for (phase 0: without this `JSON` and `Set` split in two).
fn merged(checker: &Checker, mut symbol: P<Symbol>) -> P<Symbol> {
    for _ in 0..64 {
        match checker.merged_symbols.get(&symbol) {
            Some(next) if next.addr() != symbol.addr() => symbol = *next,
            _ => break,
        }
    }
    symbol
}

/// A stand-in for `root`'s identity that lives as long as the files of its
/// declarations keep their parse: a binder symbol that owns its declarations
/// stands for itself; a merge clone whose every declaration's binder symbol
/// merges into it stands for the least of those binder symbols (the set is
/// a fact of the program, so every checker of the chain makes the same
/// clone from it). Within one checker two roots share a stand-in only when
/// they are one symbol. `None` for any other root.
pub(crate) fn lasting_root(checker: &Checker, root: P<Symbol>) -> Option<usize> {
    if root.declarations().is_empty() {
        return None;
    }
    if !root.flags().intersects(SymbolFlags::Transient) {
        let owns = root.declarations().iter().all(|declaration| {
            declaration
                .symbol()
                .is_some_and(|own| own.addr() == root.addr())
                || declaration
                    .local_symbol()
                    .is_some_and(|own| own.addr() == root.addr())
        });
        return owns.then_some(root.addr());
    }
    let mut least: Option<usize> = None;
    for declaration in root.declarations() {
        let binder = declaration.symbol()?;
        if binder.flags().intersects(SymbolFlags::Transient)
            || merged(checker, binder).addr() != root.addr()
        {
            return None;
        }
        least = Some(least.map_or(binder.addr(), |held| held.min(binder.addr())));
    }
    least
}

/// Whether `symbol` carries any derivation link or check flag. A binder-level
/// symbol, merged clones included, carries none.
fn derived(checker: &Checker, symbol: P<Symbol>) -> bool {
    if !symbol.check_flags().is_empty() {
        return true;
    }
    if let Some(links) = checker.value_symbol_links.try_get_if_id_assigned(symbol)
        && (links.target().is_some()
            || links.mapper().is_some()
            || links.containing_type().is_some())
    {
        return true;
    }
    if let Some(links) = checker.mapped_symbol_links.try_get(symbol)
        && links.synthetic_origin.get().is_some()
    {
        return true;
    }
    checker.spread_links.try_get(symbol).is_some_and(|links| {
        links.left_spread.get().is_some() || links.right_spread.get().is_some()
    })
}

/// A transient symbol made by the checker's merge (`clone_symbol`): the binder
/// symbol of one of its declarations is recorded as merged into exactly it.
fn is_merge_clone(checker: &Checker, symbol: P<Symbol>) -> bool {
    symbol.declarations().iter().any(|declaration| {
        declaration
            .symbol()
            .and_then(|binder| checker.merged_symbols.get(&binder))
            .is_some_and(|target| target.addr() == symbol.addr())
    })
}

/// The binder-level symbol `symbol` is or stands for, or `None` when that
/// cannot be proven: a non-transient symbol after the merge map, or a merge
/// clone with no derivation link. Never follows `target`/`origin` links:
/// following one would erase an instantiation into its source.
#[must_use]
pub fn binder_root(checker: &Checker, symbol: P<Symbol>) -> Option<P<Symbol>> {
    if Intrinsic::of(checker, symbol).is_some() {
        return Some(symbol);
    }
    let root = merged(checker, symbol);
    if !root.flags().intersects(SymbolFlags::Transient) {
        return Some(root);
    }
    (!derived(checker, root) && is_merge_clone(checker, root)).then_some(root)
}

/// An entry of the checker's global table (merged symbols by pointer).
fn is_global(checker: &Checker, root: P<Symbol>) -> bool {
    root.parent().is_none()
        && checker
            .globals
            .lookup(root.name())
            .is_some_and(|global| global.addr() == root.addr())
}

/// Whether every declaration of `root` is bound to `root`: its binder symbol
/// merges into `root`, or `root` is its module-local half (`localSymbol`, the
/// same logical symbol as the export). One node can declare two different
/// symbols, and the second shares the first's declaration-derived key: a
/// parameter property `constructor(public x)` declares the parameter `x` and
/// the property `x`, and the node's symbol is the property. The symbol that
/// does not own its declarations keeps a counter (fail closed), so the
/// parameter and the property never share an id.
fn owns_every_declaration(checker: &Checker, root: P<Symbol>) -> bool {
    root.declarations().iter().all(|declaration| {
        declaration
            .symbol()
            .is_some_and(|binder| merged(checker, binder).addr() == root.addr())
            || declaration
                .local_symbol()
                .is_some_and(|local| local.addr() == root.addr())
    })
}

/// The key of `symbol` and the symbol the id resolves to. `exported` answers
/// whether a declaration reference is a module export of the program (the
/// scan result, pointer-free so it survives a release); `chains` memoizes
/// the [`LocalChains`] of the generation's files.
pub fn binder_key(
    checker: &Checker,
    symbol: P<Symbol>,
    exported: impl Fn(&DurableRef) -> bool,
    chains: &LocalChains,
) -> Option<(Key, P<Symbol>)> {
    let root = binder_root(checker, symbol)?;
    if let Some(which) = Intrinsic::of(checker, root) {
        return Some((Key::Intrinsic(which), root));
    }
    if is_global(checker, root) {
        return Some((Key::Global(root.name().to_owned()), root));
    }
    let reference = minimum_declaration_ref(root)?;
    if !owns_every_declaration(checker, root) {
        return None;
    }
    let key = if exported(&reference) {
        Key::Export(reference)
    } else if let Some(chain) =
        minimum_declaration(root, &reference).and_then(|declaration| chains.chain(declaration))
    {
        Key::Local {
            path: reference.path,
            chain,
            name: reference.name,
        }
    } else {
        Key::Decl(reference)
    };
    Some((key, root))
}

/// The declaration of `root` that `reference` (its minimum declaration
/// reference) names, when exactly one does: two declaration nodes sharing a
/// name span would make the choice merge order.
fn minimum_declaration(root: P<Symbol>, reference: &DurableRef) -> Option<P<Node>> {
    let mut found: Option<P<Node>> = None;
    for declaration in root.declarations() {
        let here = durable_ref_for_declaration(root, *declaration)?;
        if here.path == reference.path
            && here.start_byte == reference.start_byte
            && here.end_byte == reference.end_byte
        {
            if found.is_some() {
                return None;
            }
            found = Some(*declaration);
        }
    }
    found
}

/// A declaration's shift-stable position or chain in its file, as a hash
/// (ADR 03XX-stable-locals).
pub type Chain = [u8; 32];

/// Shift-stable names for the declarations of source files, memoized for
/// one generation (node addresses: cleared wherever the identity maps are).
///
/// A declaration's *scope* is its nearest ancestor that is a named
/// declaration (an identifier, private name or literal names it), or the
/// file. Its *position* is the hash of its scope's position (the file's is
/// zero), its kind, its name's source text (empty when it has none) and its
/// rank among the declarations of its scope with that kind and name text, in
/// source order. A scope's declarations are those reached from it without
/// entering another named declaration, so an anonymous function or object
/// literal is ranked in its scope and its contents belong to it too.
///
/// Its *chain* is the hash of its position and its own source text (from its
/// first token to its end, leading trivia excluded). The text is what makes a
/// kept id name the same declaration: when an insert takes an earlier rank,
/// the declaration that now holds the old rank has other text, so no id
/// changes hands. An edit leaves a chain alone unless it changes the
/// declaration's own text or its position, and a position moves only when the
/// declaration or an enclosing named scope is renamed, reranked or moved.
/// Inserting a statement elsewhere shifts every later span, not these.
///
/// Two declarations of one file never share a position: by induction on the
/// nesting, the same position means the same scope, and within a scope the
/// same kind, name text and rank mean the same node. So they never share a
/// chain, which hashes the position in.
///
/// Only a non-declaration (`.d.ts`) file is chained: those are the files an
/// editor changes, and their scopes are small, while a `.d.ts` of the
/// libraries keeps its span key (`Decl`). A declaration the scan of its scope
/// does not reach (a JSDoc tag) or with a synthesized position has no chain.
#[derive(Default)]
pub struct LocalChains {
    /// Declaration address to its position, filled a scope at a time.
    positions: RefCell<HashMap<usize, Chain>>,
    /// Declaration address to its chain (its position and its own text).
    chains: RefCell<HashMap<usize, Chain>>,
    /// Scopes already scanned (a node absent from `positions` after its
    /// scope's scan has no position).
    scanned: RefCell<HashSet<usize>>,
    /// Per file address, every chained declaration by chain (re-resolution).
    files: RefCell<HashMap<usize, FileChains>>,
}

/// One file's declarations by chain; `None` where two share a chain, which
/// the scan rules out.
type FileChains = HashMap<Chain, Option<P<Node>>>;

impl LocalChains {
    /// Forgets every node address (the generation's files may be freed).
    pub fn clear(&mut self) {
        self.positions.get_mut().clear();
        self.chains.get_mut().clear();
        self.scanned.get_mut().clear();
        self.files.get_mut().clear();
    }

    /// The chain of `declaration`, or `None` when it has none.
    pub fn chain(&self, declaration: P<Node>) -> Option<Chain> {
        let file = get_source_file_of_node(declaration)?;
        if file.is_declaration_file() || declaration.kind() == Kind::SourceFile {
            return None;
        }
        if let Some(chain) = self.chains.borrow().get(&declaration.addr()) {
            return Some(*chain);
        }
        let position = self.position(file, declaration)?;
        let text = file.text();
        let start = skip_trivia(text, declaration.pos().source_offset()?) as usize;
        let end = declaration.end().source_offset()? as usize;
        let mut hasher = Sha256::new();
        hasher.update(position);
        hasher.update(text.get(start..end)?.as_bytes());
        let chain: Chain = hasher.finalize().into();
        self.chains.borrow_mut().insert(declaration.addr(), chain);
        Some(chain)
    }

    /// The position of `declaration`: its scope's position, its kind, its
    /// name and its rank, scanning the scope on first use.
    fn position(&self, file: P<SourceFile>, declaration: P<Node>) -> Option<Chain> {
        if let Some(position) = self.positions.borrow().get(&declaration.addr()) {
            return Some(*position);
        }
        let scope = scope_of(declaration)?;
        if !self.scanned.borrow().contains(&scope.addr()) {
            let parent = if scope.kind() == Kind::SourceFile {
                Chain::default()
            } else {
                self.position(file, scope)?
            };
            self.scan(file, scope, parent)?;
        }
        self.positions.borrow().get(&declaration.addr()).copied()
    }

    /// Positions the declarations of `scope`, whose own position is `parent`.
    fn scan(&self, file: P<SourceFile>, scope: P<Node>, parent: Chain) -> Option<()> {
        let text = file.text();
        let mut members: Vec<P<Node>> = Vec::new();
        fn collect(node: P<Node>, members: &mut Vec<P<Node>>) {
            node.for_each_child(&mut |child| {
                if is_declaration(child) {
                    members.push(child);
                }
                if !is_scope(child) {
                    collect(child, members);
                }
                false
            });
        }
        collect(scope, &mut members);
        let mut ranks: HashMap<(i16, &str), u32> = HashMap::new();
        let mut placed: Vec<(usize, Chain)> = Vec::with_capacity(members.len());
        for member in members {
            let name = match member.name() {
                Some(name) => {
                    let start = skip_trivia(text, name.pos().source_offset()?) as usize;
                    let end = name.end().source_offset()? as usize;
                    text.get(start..end)?
                }
                None => "",
            };
            let kind = member.kind() as i16;
            let rank = ranks.entry((kind, name)).or_insert(0);
            let mut hasher = Sha256::new();
            hasher.update(parent);
            hasher.update(kind.to_le_bytes());
            hasher.update((name.len() as u64).to_le_bytes());
            hasher.update(name.as_bytes());
            hasher.update(rank.to_le_bytes());
            *rank += 1;
            placed.push((member.addr(), hasher.finalize().into()));
        }
        self.positions.borrow_mut().extend(placed);
        self.scanned.borrow_mut().insert(scope.addr());
        Some(())
    }

    /// The declaration of `file` whose chain is `chain`, if any (and if no
    /// second declaration of the file has that chain).
    pub fn declaration(&self, file: P<SourceFile>, chain: &Chain) -> Option<P<Node>> {
        if file.is_declaration_file() {
            return None;
        }
        let address = file.as_node().addr();
        if !self.files.borrow().contains_key(&address) {
            let mut every: Vec<P<Node>> = Vec::new();
            fn collect(node: P<Node>, every: &mut Vec<P<Node>>) {
                node.for_each_child(&mut |child| {
                    if is_declaration(child) {
                        every.push(child);
                    }
                    collect(child, every);
                    false
                });
            }
            collect(file.as_node(), &mut every);
            let mut by_chain: HashMap<Chain, Option<P<Node>>> = HashMap::new();
            for node in every {
                let Some(chain) = self.chain(node) else {
                    continue;
                };
                by_chain
                    .entry(chain)
                    .and_modify(|found| *found = None)
                    .or_insert(Some(node));
            }
            self.files.borrow_mut().insert(address, by_chain);
        }
        self.files
            .borrow()
            .get(&address)?
            .get(chain)
            .copied()
            .flatten()
    }
}

/// A named declaration: a scope of [`LocalChains`].
fn is_scope(node: P<Node>) -> bool {
    is_declaration(node)
        && node.name().is_some_and(|name| {
            is_identifier(name) || is_private_identifier(name) || is_property_name_literal(name)
        })
}

/// The nearest ancestor of `node` that is a scope, or its file.
fn scope_of(node: P<Node>) -> Option<P<Node>> {
    let mut ancestor = node.parent();
    while let Some(candidate) = ancestor {
        if candidate.kind() == Kind::SourceFile || is_scope(candidate) {
            return Some(candidate);
        }
        ancestor = candidate.parent();
    }
    None
}

/// Check flags `create_union_or_intersection_property` and the spread merge
/// never set: a symbol carrying one was made by another path.
const FOREIGN_CHECK_FLAGS: CheckFlags = CheckFlags::Instantiated
    .union(CheckFlags::Late)
    .union(CheckFlags::ReverseMapped)
    .union(CheckFlags::OptionalParameter)
    .union(CheckFlags::RestParameter)
    .union(CheckFlags::Mapped)
    .union(CheckFlags::StripOptional)
    .union(CheckFlags::Unresolved)
    .union(CheckFlags::IndexSymbol);

/// How `symbol` was combined, when it is a multi-origin property exactly as
/// the checker makes one and its type is a function of its origins, or
/// `None`.
///
/// * Union or intersection property (`create_union_or_intersection_property`):
///   transient, a `Synthetic*` check flag and no `Partial` one (every
///   constituent declares it; no index signature or missing member adds to
///   its type), a containing type that is a union or an intersection whose
///   every constituent's own property keeps its declared type
///   ([`constituent_preserves`]: not mapped, inherited from a mapped type,
///   spread-merged or otherwise transformed), no `target`/`mapper`, mapped
///   origin or spread link. (Its single-constituent clone carries a `target`
///   and is not one.)
/// * Spread property (`get_spread_type`): transient, both spread links, each
///   side a declared property or an instantiation of one
///   ([`preserves_declared_type`]), no check flag the merge does not set, no
///   other derivation link.
fn combination(checker: &Checker, symbol: P<Symbol>) -> Option<Combination> {
    if !symbol.flags().intersects(SymbolFlags::Transient) {
        return None;
    }
    let check = symbol.check_flags();
    if check.intersects(FOREIGN_CHECK_FLAGS) {
        return None;
    }
    if let Some(links) = checker.mapped_symbol_links.try_get(symbol)
        && links.synthetic_origin.get().is_some()
    {
        return None;
    }
    let links = checker.value_symbol_links.try_get_if_id_assigned(symbol);
    if links.is_some_and(|links| links.target().is_some() || links.mapper().is_some()) {
        return None;
    }
    let containing = links.and_then(|links| links.containing_type());
    let spread = checker
        .spread_links
        .try_get(symbol)
        .map(|links| (links.left_spread.get(), links.right_spread.get()));
    if check.intersects(CheckFlags::Synthetic) {
        if check.intersects(CheckFlags::Partial)
            || spread.is_some_and(|(left, right)| left.is_some() || right.is_some())
        {
            return None;
        }
        let containing = containing?;
        let flags = containing.flags();
        let combination = if flags.intersects(TypeFlags::Union) {
            Combination::Union
        } else if flags.intersects(TypeFlags::Intersection) {
            Combination::Intersection
        } else {
            return None;
        };
        let name = symbol.name();
        let preserved = containing
            .types()
            .iter()
            .all(|constituent| constituent_preserves(checker, *constituent, name));
        return preserved.then_some(combination);
    }
    match spread {
        Some((Some(left), Some(right)))
            if containing.is_none()
                && preserves_declared_type(checker, left)
                && preserves_declared_type(checker, right) =>
        {
            Some(Combination::Spread)
        }
        _ => None,
    }
}

/// Whether `constituent`'s own property `name` keeps its declaration's type,
/// read without type work: a plain object type whose resolved members (as
/// `try_resolved_property` reads them) hold `name` as a symbol that
/// [`preserves_declared_type`], or a generic instantiation whose target
/// declares `name` itself. Object flags alone prove nothing: an interface
/// extending a mapped type is a plain interface whose inherited members are
/// mapped symbols, and an object literal's member may be a spread merge.
fn constituent_preserves(checker: &Checker, constituent: P<Type>, name: &str) -> bool {
    if constituent.flags() != TypeFlags::Object
        || constituent.object_flags().intersects(
            ObjectFlags::Mapped
                | ObjectFlags::ReverseMapped
                | ObjectFlags::EvolvingArray
                | ObjectFlags::InstantiationExpressionType,
        )
    {
        return false;
    }
    if constituent
        .object_flags_lazy()
        .intersects(ObjectFlags::MembersResolved)
    {
        return constituent
            .try_as_structured_type()
            .and_then(|structured| structured.members())
            .and_then(|members| members.lookup(name))
            .is_some_and(|property| preserves_declared_type(checker, property));
    }
    // tsrs resolves a generic instantiation's properties one at a time,
    // without its member table. Its property is then an instantiation of the
    // target class or interface's own member of that name when there is one
    // (an own member hides any base's); an inherited one is refused.
    if !constituent
        .object_flags()
        .intersects(ObjectFlags::Reference)
    {
        return false;
    }
    let Some(declared) = constituent.target().and_then(|target| target.symbol()) else {
        return false;
    };
    declared
        .flags()
        .intersects(SymbolFlags::Class | SymbolFlags::Interface)
        && declared
            .members()
            .and_then(|members| members.lookup(name))
            .is_some_and(|property| binder_root(checker, property).is_some())
}

/// A binder-level property, or a chain of instantiations (`Instantiated`
/// with a `target` link, nothing else) ending at one. Under
/// [`closed_declaration`] an instantiation's mapper cannot change the
/// declared type. Any other derived symbol (mapped, reverse-mapped, spread,
/// synthetic, a widened or `this` clone) may have another type: refused.
fn preserves_declared_type(checker: &Checker, mut symbol: P<Symbol>) -> bool {
    for _ in 0..16 {
        if binder_root(checker, symbol).is_some() {
            return true;
        }
        let check = symbol.check_flags();
        if !check.intersects(CheckFlags::Instantiated)
            || check.intersects(
                CheckFlags::Synthetic
                    | CheckFlags::Mapped
                    | CheckFlags::ReverseMapped
                    | CheckFlags::Late,
            )
            || checker
                .mapped_symbol_links
                .try_get(symbol)
                .is_some_and(|links| links.synthetic_origin.get().is_some())
            || checker.spread_links.try_get(symbol).is_some_and(|links| {
                links.left_spread.get().is_some() || links.right_spread.get().is_some()
            })
        {
            return false;
        }
        match checker
            .value_symbol_links
            .try_get_if_id_assigned(symbol)
            .and_then(|links| links.target())
        {
            Some(target) => symbol = target,
            None => return false,
        }
    }
    false
}

/// Whether an origin declaration's type is the same in every instantiation
/// of its containers, so the properties sharing a `Set` key have one type
/// (a consumer keyed by id reads one type's roles for all of them; ADR 0371).
/// Syntax only, and conservative:
///
/// * the declaration is a property or method of an interface, type literal
///   or class, or a parameter property, in a TypeScript file (not an object
///   literal member, whose type is contextual; not an accessor, which a
///   spread turns into a value);
/// * every enclosing node is a declaration container or a type (no function,
///   mapped or conditional type, whose parameters it could read);
/// * its type is written: a property, parameter property or method with a
///   type annotation (a method: its return and every parameter), never
///   inferred;
/// * no `this` or `super` in it (polymorphic `this` is instantiated per
///   receiver);
/// * no identifier in it spells a type parameter of an enclosing class,
///   interface or alias (a name match only ever refuses).
///
/// A written annotation with neither resolves, in the declaration's scope,
/// to one program-level type no container's mapper can reach (`typeof x`,
/// `ReturnType<...>` and defaulted references name program-level entities).
fn closed_declaration(declaration: P<Node>) -> bool {
    let Some(file) = get_source_file_of_node(declaration) else {
        return false;
    };
    let name = file.file_name();
    let typescript = [".ts", ".tsx", ".mts", ".cts"]
        .iter()
        .any(|extension| name.ends_with(extension));
    if !typescript {
        return false;
    }
    let Some(parent) = declaration.parent() else {
        return false;
    };
    let member = match declaration.kind() {
        Kind::PropertySignature | Kind::MethodSignature => {
            matches!(
                parent.kind(),
                Kind::InterfaceDeclaration | Kind::TypeLiteral
            )
        }
        Kind::PropertyDeclaration | Kind::MethodDeclaration => {
            matches!(
                parent.kind(),
                Kind::ClassDeclaration | Kind::ClassExpression
            )
        }
        Kind::Parameter => parent.kind() == Kind::Constructor,
        _ => false,
    };
    if !member {
        return false;
    }
    let mut parameters: Vec<&'static str> = Vec::new();
    let mut ancestor = Some(parent);
    while let Some(node) = ancestor {
        match node.kind() {
            Kind::SourceFile => break,
            Kind::ClassDeclaration
            | Kind::ClassExpression
            | Kind::InterfaceDeclaration
            | Kind::TypeAliasDeclaration => {
                for parameter in node.type_parameters() {
                    if let Some(name) = parameter.name() {
                        parameters.push(name.text());
                    }
                }
            }
            Kind::Constructor => {
                if declaration.kind() != Kind::Parameter || node.addr() != parent.addr() {
                    return false;
                }
            }
            Kind::TypeLiteral
            | Kind::PropertySignature
            | Kind::UnionType
            | Kind::IntersectionType
            | Kind::ParenthesizedType
            | Kind::ArrayType
            | Kind::TupleType
            | Kind::NamedTupleMember
            | Kind::OptionalType
            | Kind::RestType
            | Kind::TypeOperator
            | Kind::ModuleBlock
            | Kind::ModuleDeclaration
            | Kind::VariableDeclaration
            | Kind::VariableDeclarationList
            | Kind::VariableStatement => {}
            _ => return false,
        }
        ancestor = node.parent();
    }
    // The type must be written: an inferred one (an initializer, a method's
    // body or unannotated parameters) can read `super`, a method returning
    // `this`, or another declaration's inferred type, which no syntax check
    // bounds.
    let written = match declaration.kind() {
        Kind::MethodSignature | Kind::MethodDeclaration => {
            declaration.type_node().is_some()
                && declaration
                    .parameters()
                    .iter()
                    .all(|parameter| parameter.type_node().is_some())
        }
        _ => declaration.type_node().is_some(),
    };
    if !written {
        return false;
    }
    fn open(node: P<Node>, parameters: &[&'static str]) -> bool {
        if matches!(
            node.kind(),
            Kind::ThisType | Kind::ThisKeyword | Kind::SuperKeyword
        ) || (is_identifier(node) && parameters.contains(&node.text()))
        {
            return true;
        }
        let mut found = false;
        node.for_each_child(&mut |child| {
            found = open(child, parameters);
            found
        });
        found
    }
    !open(declaration, &parameters)
}

/// The `Set` key of a multi-origin property (ADR 0371), or `None` (a counter).
///
/// Its origins are the binder-level symbols of its declarations: each
/// declaration's binder symbol must have a binder key (ADR 0366), and its root
/// must list the declaration and bear the property's name (it is that
/// declaration's property), else the property is not keyed (fail closed).
/// The origins must also be whole: every declaration of every origin is a
/// declaration of the property, so the property's declarations are a function
/// of the key and two properties sharing an id have the same declarations.
/// Every declaration must be [`closed_declaration`], so they have the same
/// type. A union's origins are sorted by id (a union is commutative, and the
/// order of its constituents falls back to type ids, so history); an
/// intersection's and a spread's keep the order the property lists them in,
/// which is the written order of the constituents or the spread (an
/// intersection of overloaded members resolves calls in that order). Two
/// distinct keys spelling one origin id fail closed.
pub fn set_key(
    checker: &Checker,
    symbol: P<Symbol>,
    exported: impl Fn(&DurableRef) -> bool,
    chains: &LocalChains,
) -> Option<Key> {
    let combination = combination(checker, symbol)?;
    let declarations = symbol.declarations();
    if declarations.is_empty() {
        return None;
    }
    let name = normalized_name(symbol.name());
    let mut origins: Vec<(String, Key, P<Symbol>)> = Vec::new();
    for declaration in declarations {
        if !closed_declaration(*declaration) {
            return None;
        }
        let binder = declaration.symbol()?;
        let (key, root) = binder_key(checker, binder, &exported, chains)?;
        // The origin is the declaration's own property: it lists the
        // declaration and has the property's name. A late-bound member's
        // node holds the binder's `__computed` placeholder, not the member.
        if normalized_name(root.name()) != name
            || !root
                .declarations()
                .iter()
                .any(|own| own.addr() == declaration.addr())
        {
            return None;
        }
        if origins
            .iter()
            .any(|(_, _, held)| held.addr() == root.addr())
        {
            continue;
        }
        origins.push((key.id(), key, root));
    }
    let own: HashSet<usize> = declarations.iter().map(|node| node.addr()).collect();
    if origins.iter().any(|(_, _, root)| {
        root.declarations()
            .iter()
            .any(|node| !own.contains(&node.addr()))
    }) {
        return None;
    }
    if combination == Combination::Union {
        origins.sort_by(|left, right| left.0.cmp(&right.0));
    }
    let mut keys: Vec<Key> = Vec::with_capacity(origins.len());
    let mut seen: Vec<(String, usize)> = Vec::with_capacity(origins.len());
    for (id, key, _) in origins {
        if let Some((_, at)) = seen.iter().find(|(held, _)| *held == id) {
            // One id, two roots: the same key is a module export and its
            // local half; different keys are a collision.
            if keys[*at] != key {
                return None;
            }
            continue;
        }
        seen.push((id, keys.len()));
        keys.push(key);
    }
    Some(Key::Set {
        combination,
        name: name.into_owned(),
        origins: keys,
    })
}

/// The root a key names in `checker`, before verification.
#[must_use]
pub fn global_symbol(checker: &Checker, name: &str) -> Option<P<Symbol>> {
    checker.globals.lookup(name)
}

/// The singleton an `Intrinsic` key names.
#[must_use]
pub fn intrinsic_symbol(checker: &Checker, which: Intrinsic) -> P<Symbol> {
    which.symbol(checker)
}
