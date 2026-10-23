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

use crate::durable::{DurableRef, hashed_symbol_id, minimum_declaration_ref, name_bytes};
use sha2::{Digest, Sha256};
use tsrs_ast::{Symbol, SymbolFlags};
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

/// The pointer-free key a binder-level symbol's id is the hash of.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Key {
    Decl(DurableRef),
    Export(DurableRef),
    Global(String),
    Intrinsic(Intrinsic),
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
        }
    }

    /// The declaration reference of a `Decl` or `Export` key.
    #[must_use]
    pub fn reference(&self) -> Option<&DurableRef> {
        match self {
            Self::Decl(reference) | Self::Export(reference) => Some(reference),
            Self::Global(_) | Self::Intrinsic(_) => None,
        }
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
    let key = if exported(&reference) {
        Key::Export(reference)
    } else {
        Key::Decl(reference)
    };
    Some((key, root))
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
