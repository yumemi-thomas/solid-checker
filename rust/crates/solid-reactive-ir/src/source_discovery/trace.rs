//! What a post-pass row read outside its own file (S7a; S7 design §2).
//!
//! A post-pass row is a function of its own file's facts (bound by the file
//! being one of the build's `unchanged_paths`), of the memo answers and
//! lookup sections it read (a [`crate::base::reads::ReadProof`]), and of the
//! answers recorded here: another file's facts, a lookup question about
//! another file or a symbol, and an earlier pass's state at a key. A row is
//! kept while each of them answers as recorded (the trace-replay lemma,
//! study §3.3): re-asking a question is exact whatever the lookup computes
//! it from.
//!
//! A computation records on its own thread ([`record`]); a note outside
//! every recording goes nowhere, so the traced lookups below serve every
//! other caller unchanged.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast::FunctionFact;
use solid_facts::core::Span;
use typefacts::Location;

use crate::HashSet;
use crate::identity::SymbolId;
use crate::indexes::SemanticLookup;

/// A function a question answered with: its file's path and its span.
pub(crate) type FunctionAt = Option<(Arc<str>, Span)>;

/// One read a post-pass row made outside its own file, with its answer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Dep {
    /// Another file's facts (its source, entity rows and the symbols they
    /// name), and whether an analyzed file had that path.
    File(Arc<str>, bool),
    /// `function_for_symbol(symbol)`.
    FunctionFor(SymbolId, FunctionAt),
    /// The symbol `binding_at_reference(path, span)` names.
    BindingAt(Arc<str>, Span, Option<SymbolId>),
    /// `symbol_references(symbol)`, fingerprinted.
    References(SymbolId, u64),
    /// `symbol_declarations(symbol)`, fingerprinted.
    Declarations(SymbolId, u64),
    /// Whether a visible mutation may change the array iteration protocol.
    TupleVeto(bool),
    /// The symbol pass 4 resolves a returned identifier's name to in a file
    /// (`declared_symbol`).
    DeclaredIn(Arc<str>, Arc<str>, Option<SymbolId>),
    /// The TypeScript entity sites of a member symbol: the paths that hold
    /// one (`LiteralMemberSlots`).
    Sites(SymbolId, Vec<Arc<str>>),
    /// An earlier pass's table at a key, fingerprinted.
    State(StateKey, u64),
}

/// A key of the source tables an earlier pass filled, at the stage a row
/// reads it.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum StateKey {
    /// Pass 0's accessors: a runtime source before the post-passes.
    Runtime0(SymbolId),
    /// Pass 0's source kind.
    Kind0(SymbolId),
    /// The accessors the component pass reads (after pass 6).
    Accessor(SymbolId),
    /// The source kinds the component pass reads.
    Kind(SymbolId),
    /// A prop source's declaration (after pass 8).
    PropSource(SymbolId),
    /// The prop sources attached to a declaration (path, start, end).
    Attached(Arc<str>, u64, u64),
    /// A runtime source no file writes (after pass 9).
    Runtime(SymbolId),
    /// The JSX elements that render a project function (path, span).
    Uses(Arc<str>, Span),
    /// Some analyzed file exports the symbol.
    Exported(SymbolId),
}

/// The fingerprint of an answer.
pub(crate) fn fingerprint(value: &impl Hash) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

/// The fingerprint of a source kind (the kind derives no `Hash`).
pub(crate) fn kind_fingerprint(kind: Option<&crate::ReactiveSourceKind>) -> u64 {
    match kind {
        None => 0,
        Some(crate::ReactiveSourceKind::Accessor) => 1,
        Some(crate::ReactiveSourceKind::Store) => 2,
    }
}

/// The fingerprint of a symbol's declarations (which derive no `Hash`): every
/// field, through its debug form.
fn declarations_fingerprint(declarations: Option<&[typefacts::Declaration]>) -> u64 {
    fingerprint(&format!("{declarations:?}"))
}

thread_local! {
    static FRAMES: RefCell<Vec<Vec<Dep>>> = const { RefCell::new(Vec::new()) };
}

/// Pops the frame [`record`] opened, even on unwind.
struct FrameGuard;

impl Drop for FrameGuard {
    fn drop(&mut self) {
        FRAMES.with(|frames| {
            frames.borrow_mut().pop();
        });
    }
}

/// Runs `f`, returning what it recorded, sorted and without duplicates: the
/// facts of `own` (the row's own file) are bound by its gate, not recorded.
pub(crate) fn record<R>(own: &str, f: impl FnOnce() -> R) -> (R, Vec<Dep>) {
    FRAMES.with(|frames| frames.borrow_mut().push(Vec::new()));
    let guard = FrameGuard;
    let value = f();
    let mut deps = FRAMES.with(|frames| {
        std::mem::take(
            frames
                .borrow_mut()
                .last_mut()
                .expect("a trace frame is open"),
        )
    });
    drop(guard);
    deps.sort_unstable();
    deps.dedup();
    // An enclosing computation read what this one did.
    replay(&deps);
    deps.retain(|dep| !matches!(dep, Dep::File(path, _) if &**path == own));
    (value, deps)
}

/// Whether some computation on this thread records.
pub(crate) fn recording() -> bool {
    FRAMES.with(|frames| !frames.borrow().is_empty())
}

/// Records `dep` in the current computation, if one records.
pub(crate) fn note(dep: Dep) {
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.push(dep);
        }
    });
}

/// Records `deps` as if the current computation had made them (a cached
/// answer another computation recorded).
pub(crate) fn replay(deps: &[Dep]) {
    FRAMES.with(|frames| {
        if let Some(frame) = frames.borrow_mut().last_mut() {
            frame.extend_from_slice(deps);
        }
    });
}

/// Records that the current computation read `file`'s facts.
pub(crate) fn read_file(file: &FileFacts) {
    if recording() {
        note(Dep::File(file.path.shared(), true));
    }
}

/// Records that the current computation read the facts at `path`, an
/// analyzed file's or none.
pub(crate) fn read_path(lookup: &SemanticLookup<'_>, path: &str) {
    if recording() {
        note(Dep::File(
            Arc::from(path),
            lookup.file_by_path(path).is_some(),
        ));
    }
}

fn function_at(found: Option<(&FileFacts, &FunctionFact)>) -> FunctionAt {
    found.map(|(file, function)| (file.path.shared(), function.span))
}

/// [`SemanticLookup::function_called_at`] for a callee of `file`, whose
/// facts the caller holds (its own file's, or another file it recorded):
/// the function of the callee's value symbol, recorded with the function's
/// file, which the caller reads.
pub(crate) fn called_at<'a>(
    lookup: &SemanticLookup<'a>,
    file: &FileFacts,
    callee: Span,
) -> Option<(&'a FileFacts, &'a FunctionFact)> {
    // `function_called_at` is exactly this: the file at the path, the
    // callee's value symbol there, and that symbol's function.
    let file = lookup.file_by_path(file.path.as_str())?;
    let symbol = lookup.entities().value_symbol(file, callee)?;
    function_for(lookup, symbol)
}

/// The function a JSX tag of `file` renders, as its file and span: recorded
/// as the tag symbol's function only, since the caller reads nothing of that
/// function's file.
pub(crate) fn render_target(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    tag: Span,
) -> FunctionAt {
    let file = lookup.file_by_path(file.path.as_str())?;
    let symbol = lookup.entities().value_symbol(file, tag)?;
    let found = function_at(lookup.function_for_symbol(symbol));
    if recording() {
        note(Dep::FunctionFor(symbol.clone(), found.clone()));
    }
    found
}

/// [`SemanticLookup::namespace_member_function`]: the member symbol is the
/// caller's file's, its function recorded with that function's file.
pub(crate) fn namespace_member<'a>(
    lookup: &SemanticLookup<'a>,
    file: &FileFacts,
    callee: Span,
) -> Option<(&'a FileFacts, &'a FunctionFact)> {
    let symbol = lookup.namespace_member_symbol(file, callee)?;
    lookup.namespace_member_target(function_for(lookup, symbol))
}

/// [`SemanticLookup::function_for_symbol`], recorded with the function's
/// file.
pub(crate) fn function_for<'a>(
    lookup: &SemanticLookup<'a>,
    symbol: &str,
) -> Option<(&'a FileFacts, &'a FunctionFact)> {
    let found = lookup.function_for_symbol(symbol);
    if recording() {
        note(Dep::FunctionFor(SymbolId::from(symbol), function_at(found)));
        if let Some((file, _)) = found {
            read_file(file);
        }
    }
    found
}

/// The symbol [`SemanticLookup::binding_at_reference`] names, recorded.
pub(crate) fn binding_symbol(
    lookup: &SemanticLookup<'_>,
    path: &str,
    span: Span,
) -> Option<SymbolId> {
    let symbol = lookup
        .binding_at_reference(path, span)
        .map(|(_, _, symbol)| symbol);
    if recording() {
        note(Dep::BindingAt(Arc::from(path), span, symbol.clone()));
    }
    symbol
}

/// [`SemanticLookup::symbol_references`], recorded.
pub(crate) fn references(lookup: &SemanticLookup<'_>, symbol: &str) -> Vec<Location> {
    let references = lookup.symbol_references(symbol);
    if recording() {
        note(Dep::References(
            SymbolId::from(symbol),
            fingerprint(&references),
        ));
    }
    references
}

/// [`SemanticLookup::symbol_declarations`], recorded.
pub(crate) fn declarations<'a>(
    lookup: &SemanticLookup<'a>,
    symbol: &str,
) -> Option<&'a [typefacts::Declaration]> {
    let declarations = lookup.symbol_declarations(symbol);
    if recording() {
        note(Dep::Declarations(
            SymbolId::from(symbol),
            declarations_fingerprint(declarations),
        ));
    }
    declarations
}

/// [`SemanticLookup::tuple_iteration_may_be_mutated`], recorded.
pub(crate) fn tuple_veto(lookup: &SemanticLookup<'_>) -> bool {
    let veto = lookup.tuple_iteration_may_be_mutated();
    if recording() {
        note(Dep::TupleVeto(veto));
    }
    veto
}

/// What the build answers a recorded read with now.
pub(crate) struct Answers<'c, 'l> {
    pub(crate) lookup: &'c SemanticLookup<'l>,
    /// The analyzed files whose own facts are the last build's.
    pub(crate) unchanged: &'c HashSet<String>,
    /// Pass 4's declared symbols, built on the first read that needs them.
    pub(crate) declared: &'c dyn Fn(&str, &str) -> Option<SymbolId>,
    /// Whether the TypeScript entity rows of a path are the last build's.
    pub(crate) entity_rows_unchanged: &'c dyn Fn(&str) -> bool,
    /// The member symbols some entity of a changed path resolves to.
    pub(crate) changed_roots: &'c dyn Fn(&SymbolId) -> bool,
    /// Whether a symbol's TypeScript row (its declarations and references)
    /// is the last build's.
    pub(crate) symbol_unchanged: &'c dyn Fn(&str) -> bool,
    /// An earlier pass's state at a key, as [`Dep::State`] fingerprints it;
    /// `None` when this check runs where that state is not read.
    pub(crate) state: &'c dyn Fn(&StateKey) -> Option<u64>,
}

impl Dep {
    /// Whether the read answers as recorded.
    pub(crate) fn holds(&self, answers: &Answers<'_, '_>) -> bool {
        let lookup = answers.lookup;
        match self {
            Self::File(path, analyzed) => {
                if *analyzed {
                    answers.unchanged.contains(&**path)
                } else {
                    lookup.file_by_path(path).is_none()
                }
            }
            Self::FunctionFor(symbol, answer) => {
                function_at(lookup.function_for_symbol(symbol)) == *answer
            }
            Self::BindingAt(path, span, answer) => {
                lookup
                    .binding_at_reference(path, *span)
                    .map(|(_, _, symbol)| symbol)
                    == *answer
            }
            Self::References(symbol, answer) => {
                (answers.symbol_unchanged)(symbol)
                    || fingerprint(&lookup.symbol_references(symbol)) == *answer
            }
            Self::Declarations(symbol, answer) => {
                (answers.symbol_unchanged)(symbol)
                    || declarations_fingerprint(lookup.symbol_declarations(symbol)) == *answer
            }
            Self::TupleVeto(veto) => lookup.tuple_iteration_may_be_mutated() == *veto,
            Self::DeclaredIn(path, name, answer) => (answers.declared)(path, name) == *answer,
            Self::Sites(symbol, paths) => {
                !(answers.changed_roots)(symbol)
                    && paths
                        .iter()
                        .all(|path| (answers.entity_rows_unchanged)(path))
            }
            Self::State(key, answer) => (answers.state)(key) == Some(*answer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_keeps_its_own_reads_sorted_and_hands_them_on() {
        let ((), outer) = record("own.ts", || {
            let ((), inner) = record("inner.ts", || {
                note(Dep::TupleVeto(true));
                note(Dep::File(Arc::from("own.ts"), true));
                note(Dep::File(Arc::from("inner.ts"), true));
            });
            // The inner row's own file is bound by its gate; to the outer
            // row it is another file.
            assert_eq!(
                inner,
                vec![Dep::File(Arc::from("own.ts"), true), Dep::TupleVeto(true)]
            );
            note(Dep::TupleVeto(true));
        });
        assert_eq!(
            outer,
            vec![Dep::File(Arc::from("inner.ts"), true), Dep::TupleVeto(true)]
        );
        assert!(!recording());
        // Outside every recording a note goes nowhere.
        note(Dep::TupleVeto(false));
    }
}
