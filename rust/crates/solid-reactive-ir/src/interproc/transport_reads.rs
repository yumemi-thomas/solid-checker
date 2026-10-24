//! What a file's result reads read of the stage's transport tables (L3,
//! ADR 03XX-s6b): the call targets accessor transport found, the calls it
//! could not follow, the array-member bodies, and the resolver's source
//! origins. Each read is recorded with its answer while the file's result
//! chunk computes, and the chunk is reused only while every one answers as
//! it did. This replaces comparing the whole tables with the last build's:
//! a target that moved for one call reruns the files that asked about that
//! call, not every file.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::*;

/// A transport question a result chunk asked.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum TransportKey {
    /// `transported_calls[site]`, by path and byte range.
    Call(Arc<str>, u64, u64),
    /// `unavailable_calls ∋ site`.
    Unavailable(Arc<str>, u64, u64),
    /// Whether any transported call is an array-member target.
    ArrayBodiesPresent,
    /// `array_member_bodies ∋ (path, span)`.
    ArrayBody(String, Span),
    /// The resolver's source origin of a symbol.
    SourceOrigin(SymbolId),
    /// The resolver's runtime source origin of a symbol.
    RuntimeSourceOrigin(SymbolId),
    /// Source discovery's accessor entry of a symbol (the graph embeds its
    /// display label).
    Accessor(SymbolId),
}

/// A result chunk's transport reads, sorted, each with its answer.
pub(crate) type TransportReads = Vec<(TransportKey, u64)>;

thread_local! {
    static LOG: RefCell<Option<Vec<(TransportKey, u64)>>> = const { RefCell::new(None) };
}

fn note(key: TransportKey, answer: u64) {
    LOG.with(|log| {
        if let Some(log) = log.borrow_mut().as_mut() {
            log.push((key, answer));
        }
    });
}

/// Runs a result chunk, recording its transport reads.
pub(super) fn record<R>(f: impl FnOnce() -> R) -> (R, TransportReads) {
    let outer = LOG.with(|log| log.replace(Some(Vec::new())));
    let value = f();
    let mut reads = LOG.with(|log| log.replace(outer)).unwrap_or_default();
    reads.sort_unstable();
    reads.dedup();
    (value, reads)
}

fn callable_answer(callable: Option<&accessor_provenance::Callable>) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    match callable {
        None => 0_u8.hash(&mut hasher),
        Some(accessor_provenance::Callable::Accessor(symbol)) => {
            1_u8.hash(&mut hasher);
            symbol.hash(&mut hasher);
        }
        Some(accessor_provenance::Callable::Function { path, span }) => {
            2_u8.hash(&mut hasher);
            path.hash(&mut hasher);
            span.hash(&mut hasher);
        }
        Some(accessor_provenance::Callable::ArrayMembers(targets)) => {
            3_u8.hash(&mut hasher);
            targets.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// `transported_calls[site]`, recorded.
pub(super) fn call<'m>(
    calls: &'m HashMap<Location, accessor_provenance::Callable>,
    site: &Location,
) -> Option<&'m accessor_provenance::Callable> {
    let callable = calls.get(site);
    note(
        TransportKey::Call(site.path.clone(), site.start_byte, site.end_byte),
        callable_answer(callable),
    );
    callable
}

/// `unavailable_calls ∋ site`, recorded.
pub(super) fn unavailable(calls: &HashSet<Location>, site: &Location) -> bool {
    let unavailable = calls.contains(site);
    note(
        TransportKey::Unavailable(site.path.clone(), site.start_byte, site.end_byte),
        u64::from(unavailable),
    );
    unavailable
}

/// Whether some array-member body exists, recorded.
pub(super) fn array_bodies_present(bodies: &HashSet<(String, Span)>) -> bool {
    let present = !bodies.is_empty();
    note(TransportKey::ArrayBodiesPresent, u64::from(present));
    present
}

/// `array_member_bodies ∋ key`, recorded.
pub(super) fn array_body(bodies: &HashSet<(String, Span)>, key: &(String, Span)) -> bool {
    let found = bodies.contains(key);
    note(
        TransportKey::ArrayBody(key.0.clone(), key.1),
        u64::from(found),
    );
    found
}

/// [`accessor_provenance::Resolver::has_source_origin`], recorded.
pub(super) fn source_origin(
    provenance: &accessor_provenance::Resolver<'_, '_>,
    symbol: &SymbolId,
) -> bool {
    let found = provenance.has_source_origin(symbol);
    note(TransportKey::SourceOrigin(symbol.clone()), u64::from(found));
    found
}

/// [`accessor_provenance::Resolver::has_runtime_source_origin`], recorded.
pub(super) fn runtime_source_origin(
    provenance: &accessor_provenance::Resolver<'_, '_>,
    symbol: &SymbolId,
) -> bool {
    let found = provenance.has_runtime_source_origin(symbol);
    note(
        TransportKey::RuntimeSourceOrigin(symbol.clone()),
        u64::from(found),
    );
    found
}

fn site(path: &Arc<str>, start_byte: u64, end_byte: u64) -> Location {
    Location {
        path: path.clone(),
        start_byte,
        end_byte,
    }
}

fn accessor_answer(entry: Option<&(SymbolId, Location)>) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    entry.hash(&mut hasher);
    hasher.finish()
}

/// Source discovery's accessor entry of `symbol`, recorded.
pub(super) fn accessor<'m>(
    accessors: &'m HashMap<SymbolId, (SymbolId, Location)>,
    symbol: &SymbolId,
) -> Option<&'m (SymbolId, Location)> {
    let entry = accessors.get(symbol);
    note(
        TransportKey::Accessor(symbol.clone()),
        accessor_answer(entry),
    );
    entry
}

/// Whether a graph chunk's recorded reads (accessor entries only) answer in
/// this build as they did.
pub(super) fn hold_accessors(
    reads: &TransportReads,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> bool {
    reads.iter().all(|(key, answer)| match key {
        TransportKey::Accessor(symbol) => accessor_answer(accessors.get(symbol)) == *answer,
        _ => false,
    })
}

/// The tables a recorded transport read is asked of again.
pub(super) trait TransportTables {
    fn call(&self, site: &Location) -> Option<&accessor_provenance::Callable>;
    fn unavailable(&self, site: &Location) -> bool;
    fn array_bodies_present(&self) -> bool;
    fn array_body(&self, key: &(String, Span)) -> bool;
    fn source_origin(&self, symbol: &SymbolId) -> bool;
    fn runtime_source_origin(&self, symbol: &SymbolId) -> bool;
    fn accessor(&self, symbol: &SymbolId) -> Option<&(SymbolId, Location)>;
}

impl TransportTables for InterproceduralResultReadContext<'_, '_> {
    fn call(&self, site: &Location) -> Option<&accessor_provenance::Callable> {
        self.transported_calls.get(site)
    }
    fn unavailable(&self, site: &Location) -> bool {
        self.unavailable_calls.contains(site)
    }
    fn array_bodies_present(&self) -> bool {
        !self.array_member_bodies.is_empty()
    }
    fn array_body(&self, key: &(String, Span)) -> bool {
        self.array_member_bodies.contains(key)
    }
    fn source_origin(&self, symbol: &SymbolId) -> bool {
        self.provenance.has_source_origin(symbol)
    }
    fn runtime_source_origin(&self, symbol: &SymbolId) -> bool {
        self.provenance.has_runtime_source_origin(symbol)
    }
    fn accessor(&self, symbol: &SymbolId) -> Option<&(SymbolId, Location)> {
        self.accessors.get(symbol)
    }
}

/// Whether every recorded read answers in this build as it did.
pub(super) fn hold(reads: &TransportReads, tables: &dyn TransportTables) -> bool {
    reads.iter().all(|(key, answer)| {
        let current = match key {
            TransportKey::Call(path, start, end) => {
                callable_answer(tables.call(&site(path, *start, *end)))
            }
            TransportKey::Unavailable(path, start, end) => {
                u64::from(tables.unavailable(&site(path, *start, *end)))
            }
            TransportKey::ArrayBodiesPresent => u64::from(tables.array_bodies_present()),
            TransportKey::ArrayBody(path, span) => {
                u64::from(tables.array_body(&(path.clone(), *span)))
            }
            TransportKey::SourceOrigin(symbol) => u64::from(tables.source_origin(symbol)),
            TransportKey::RuntimeSourceOrigin(symbol) => {
                u64::from(tables.runtime_source_origin(symbol))
            }
            TransportKey::Accessor(symbol) => accessor_answer(tables.accessor(symbol)),
        };
        current == *answer
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Tables {
        calls: HashMap<Location, accessor_provenance::Callable>,
        unavailable: HashSet<Location>,
        accessors: HashMap<SymbolId, (SymbolId, Location)>,
    }

    impl TransportTables for Tables {
        fn call(&self, site: &Location) -> Option<&accessor_provenance::Callable> {
            self.calls.get(site)
        }
        fn unavailable(&self, site: &Location) -> bool {
            self.unavailable.contains(site)
        }
        fn array_bodies_present(&self) -> bool {
            false
        }
        fn array_body(&self, _: &(String, Span)) -> bool {
            false
        }
        fn source_origin(&self, _: &SymbolId) -> bool {
            false
        }
        fn runtime_source_origin(&self, _: &SymbolId) -> bool {
            false
        }
        fn accessor(&self, symbol: &SymbolId) -> Option<&(SymbolId, Location)> {
            self.accessors.get(symbol)
        }
    }

    fn at(path: &str, start: u64) -> Location {
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + 3,
        }
    }

    fn function(span: u32) -> accessor_provenance::Callable {
        accessor_provenance::Callable::Function {
            path: "H.ts".into(),
            span: Span::new(span, span + 10),
        }
    }

    /// The gate this replaces compared the whole transport tables: any
    /// moved target reran every result file. A chunk's recorded reads hold
    /// while the targets it asked about stay, whatever moves elsewhere, and
    /// fail exactly when one of them moves.
    #[test]
    fn a_result_chunk_reruns_exactly_when_a_call_it_asked_about_moves() {
        let mut tables = Tables::default();
        tables.calls.insert(at("A.ts", 10), function(100));
        tables.calls.insert(at("B.ts", 10), function(200));
        let ((), reads) = record(|| {
            assert!(call(&tables.calls, &at("A.ts", 10)).is_some());
            assert!(!unavailable(&tables.unavailable, &at("A.ts", 20)));
            assert!(accessor(&tables.accessors, &SymbolId::from("count")).is_none());
        });
        assert_eq!(reads.len(), 3);
        assert!(hold(&reads, &tables));
        // Another file's call moves: the chunk still holds.
        tables.calls.insert(at("B.ts", 10), function(300));
        tables.unavailable.insert(at("B.ts", 30));
        assert!(hold(&reads, &tables));
        // The call it asked about moves, an entry appears where it saw none,
        // or a call it asked about becomes unavailable: each fails it.
        let mut moved = Tables::default();
        moved.calls.insert(at("A.ts", 10), function(400));
        assert!(!hold(&reads, &moved));
        let mut unavailable = Tables::default();
        unavailable.calls.insert(at("A.ts", 10), function(100));
        unavailable.unavailable.insert(at("A.ts", 20));
        assert!(!hold(&reads, &unavailable));
        let mut accessor_appears = Tables::default();
        accessor_appears.calls.insert(at("A.ts", 10), function(100));
        accessor_appears.accessors.insert(
            SymbolId::from("count"),
            (SymbolId::from("count"), at("S.ts", 0)),
        );
        assert!(!hold(&reads, &accessor_appears));
        // Outside a recording nothing is kept.
        let _ = call(&tables.calls, &at("A.ts", 10));
        assert!(LOG.with(|log| log.borrow().is_none()));
    }
}
