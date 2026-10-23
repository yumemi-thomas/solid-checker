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
//! * `Decl`: the name span and name of the minimum declaration by
//!   `(path, offset)`, not `declarations()[0]`, which is merge order.
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
    DurableRef, hashed_symbol_id, minimum_declaration_ref, name_bytes, normalized_name,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tsrs_ast::{
    CheckFlags, Kind, Node, Symbol, SymbolFlags, get_source_file_of_node, is_identifier,
};
use tsrs_checker::types::{ObjectFlags, TypeFlags};
use tsrs_compiler::Checker;
use tsrs_core::P;

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
}

impl Key {
    /// `symbol:h:<24 hex>`. `Decl` and `Export` hash exactly what the ids of
    /// an unmerged, non-private symbol always hashed.
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::Decl(reference) => reference.id(),
            Self::Export(reference) => reference.exported_id(),
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
        }
    }

    /// The declaration reference of a `Decl` or `Export` key.
    #[must_use]
    pub fn reference(&self) -> Option<&DurableRef> {
        match self {
            Self::Decl(reference) | Self::Export(reference) => Some(reference),
            Self::Global(_) | Self::Intrinsic(_) | Self::Set { .. } => None,
        }
    }

    /// Whether the key names a binder-level symbol. A `Set` key names a
    /// checker-derived property: it is never re-resolved from its key, and no
    /// answer holding it is retained across updates (ADR 0369).
    #[must_use]
    pub fn is_binder(&self) -> bool {
        !matches!(self, Self::Set { .. })
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
/// scan result, pointer-free so it survives a release).
pub fn binder_key(
    checker: &Checker,
    symbol: P<Symbol>,
    exported: impl Fn(&DurableRef) -> bool,
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
    } else {
        Key::Decl(reference)
    };
    Some((key, root))
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
///   its type), a containing type that is a union or an intersection of plain
///   object types (no mapped or reverse-mapped type, which would transform the
///   declared type; no nested union, intersection, type parameter or
///   primitive), no `target`/`mapper`, mapped origin or spread link. (Its
///   single-constituent clone carries a `target` and is not one.)
/// * Spread property (`get_spread_type`): transient, both spread links, each
///   side a declared property or an instantiation of one, no check flag the
///   merge does not set, no other derivation link.
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
        let plain = containing.types().iter().all(|constituent| {
            constituent.flags() == TypeFlags::Object
                && !constituent.object_flags().intersects(
                    ObjectFlags::Mapped
                        | ObjectFlags::ReverseMapped
                        | ObjectFlags::EvolvingArray
                        | ObjectFlags::InstantiationExpressionType,
                )
        });
        return plain.then_some(combination);
    }
    match spread {
        Some((Some(left), Some(right)))
            if containing.is_none()
                && declared_or_instance(checker, left)
                && declared_or_instance(checker, right) =>
        {
            Some(Combination::Spread)
        }
        _ => None,
    }
}

/// A binder-level property, or a chain of instantiations or clones
/// (`target` links only) ending at one: its type is its declaration's under a
/// mapper, which [`closed_declaration`] proves does not change it.
fn declared_or_instance(checker: &Checker, mut symbol: P<Symbol>) -> bool {
    for _ in 0..16 {
        if binder_root(checker, symbol).is_some() {
            return true;
        }
        if symbol
            .check_flags()
            .intersects(CheckFlags::Synthetic | CheckFlags::Mapped | CheckFlags::ReverseMapped)
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
/// * no `this` in it (polymorphic `this` is instantiated per receiver);
/// * no identifier in it spells a type parameter of an enclosing class,
///   interface or alias (a name match only ever refuses), and under such a
///   parameter the declaration's type is written, not inferred.
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
    if !parameters.is_empty() && declaration.type_node().is_none() {
        return false;
    }
    fn open(node: P<Node>, parameters: &[&'static str]) -> bool {
        if matches!(node.kind(), Kind::ThisType | Kind::ThisKeyword)
            || (is_identifier(node) && parameters.contains(&node.text()))
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
        let (key, root) = binder_key(checker, binder, &exported)?;
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
