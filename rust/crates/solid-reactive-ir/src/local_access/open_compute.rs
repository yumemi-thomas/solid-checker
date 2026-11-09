//! ADR 0802: what a source's compute may read. `async_sources` says what a
//! source's own computation returns. A memo over a value a caller handed it, or
//! a function-form source over anything, is pending while that value is, and its
//! return type says nothing of that (`createMemo(() => props.get())`).
//!
//! Asked once every table is built, from `reactive_analysis`: a call transport
//! resolves is decided by what it resolved to, and no result chunk holds an
//! answer that depends on another file.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, CallFact, FunctionFact};
use solid_facts::core::Span;
use typefacts::Location;

use super::{LocalAccessContext, handed_value};
use crate::identity::SymbolId;
use crate::interproc::accessor_provenance::Callable;
use crate::{HashMap, HashSet, location};

/// How many functions and sources one question visits. Past it the source is
/// open: nothing proved it settled.
const OPEN_WALK_BUDGET: usize = 512;

/// The answers of one build: each source's own walk, so an answer does not
/// depend on the order sources were asked in.
pub(crate) struct OpenComputes<'a> {
    targets: &'a HashMap<Location, Callable>,
    answers: RefCell<HashMap<SymbolId, bool>>,
}

impl<'a> OpenComputes<'a> {
    pub(crate) fn new(targets: &'a HashMap<Location, Callable>) -> Self {
        Self {
            targets,
            answers: RefCell::new(HashMap::default()),
        }
    }
}

/// One question's walk: what it has visited, and how much it may still visit.
struct OpenWalk<'a> {
    targets: &'a HashMap<Location, Callable>,
    sources: RefCell<HashSet<SymbolId>>,
    functions: RefCell<HashSet<(Arc<str>, Span)>>,
    left: Cell<usize>,
}

impl OpenWalk<'_> {
    /// False when the budget is gone, which answers open.
    fn spend(&self) -> bool {
        let left = self.left.get();
        self.left.set(left.saturating_sub(1));
        left > 0
    }
}

impl LocalAccessContext<'_, '_> {
    /// Answers [`Self::source_compute_open`] for each of `symbols` not yet
    /// answered, on the workers. Each answer is its own walk with its own
    /// budget ([`OpenComputes`]), so it is the one the first asker would get.
    pub(crate) fn prefill_source_compute_open(&self, symbols: &[&SymbolId], open: &OpenComputes<'_>)
    where
        Self: Sync,
    {
        let asked = {
            let answers = open.answers.borrow();
            symbols
                .iter()
                .copied()
                .filter(|symbol| !answers.contains_key(symbol.as_str()))
                .collect::<Vec<_>>()
        };
        let targets = open.targets;
        let answers = crate::pipeline::parallel_balanced_results(&asked, |symbol| {
            let walk = OpenWalk {
                targets,
                sources: RefCell::new(HashSet::default()),
                functions: RefCell::new(HashSet::default()),
                left: Cell::new(OPEN_WALK_BUDGET),
            };
            self.source_open(symbol, &walk)
        });
        let mut held = open.answers.borrow_mut();
        for (symbol, answer) in asked.into_iter().zip(answers) {
            held.insert((*symbol).clone(), answer);
        }
    }

    /// Whether the source `symbol` may be pending by what its compute reads,
    /// though no fact says its computation is async.
    pub(crate) fn source_compute_open(&self, symbol: &str, open: &OpenComputes<'_>) -> bool {
        if let Some(answer) = open.answers.borrow().get(symbol) {
            return *answer;
        }
        let walk = OpenWalk {
            targets: open.targets,
            sources: RefCell::new(HashSet::default()),
            functions: RefCell::new(HashSet::default()),
            left: Cell::new(OPEN_WALK_BUDGET),
        };
        let answer = self.source_open(symbol, &walk);
        open.answers.borrow_mut().insert(symbol.into(), answer);
        answer
    }

    /// [`handed_value`] with an accessor counted only if it may be pending, a
    /// source [`Self::source_compute_open`] takes included.
    pub(crate) fn value_holds_open_source(
        &self,
        file: &FileFacts,
        value: Span,
        open: &OpenComputes<'_>,
    ) -> bool {
        handed_value(
            &self.handed_tables(Some(&|symbol| self.source_compute_open(symbol, open))),
            file,
            value,
            super::HANDED_DEPTH - 1,
        )
    }

    fn source_open(&self, symbol: &str, walk: &OpenWalk<'_>) -> bool {
        // ADR 0803: a withdrawn slot may be any function, a pending read
        // included.
        if self.async_sources.contains(symbol) || self.withdrawn_array_slots.contains(symbol) {
            return true;
        }
        if !walk.sources.borrow_mut().insert(symbol.into()) {
            return false;
        }
        if !walk.spend() {
            return true;
        }
        self.source_compute(symbol)
            .is_some_and(|(file, function)| self.function_open(file, function, walk))
    }

    /// The function a source's creating call computes it with, through the
    /// hook that returns it.
    fn source_compute(&self, symbol: &str) -> Option<(&FileFacts, &FunctionFact)> {
        let (_, declared) = self.accessors.get(symbol)?;
        let file = self.lookup.file_by_path(declared.path.as_ref())?;
        let span = Span::new(
            u32::try_from(declared.start_byte).ok()?,
            u32::try_from(declared.end_byte).ok()?,
        );
        let binding = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == span))?;
        let call = file.ast.call_at(binding.call_initializer?)?;
        let (origin, created) = if self.lookup.primitive_at_call(file, call.span).is_some() {
            (file, call)
        } else {
            let (origin, created, _) = self.lookup.returned_source_at(file, call.span)?;
            (origin, created)
        };
        let argument = created
            .arguments
            .first()
            .filter(|argument| !argument.spread)?;
        let value = origin.ast.peel_ts_sugar_span(argument.span);
        match argument.value {
            ArgumentValueKind::Function => Some((origin, origin.ast.function_at(value)?)),
            ArgumentValueKind::Identifier => {
                self.lookup.function_called_at(origin.path.as_str(), value)
            }
            _ => None,
        }
    }

    fn function_open(
        &self,
        file: &FileFacts,
        function: &FunctionFact,
        walk: &OpenWalk<'_>,
    ) -> bool {
        if !walk
            .functions
            .borrow_mut()
            .insert((file.path.shared(), function.span))
        {
            return false;
        }
        if !walk.spend() {
            return true;
        }
        self.value_reaches_async_source(file, function.span).may()
            || file
                .ast
                .calls
                .iter()
                .filter(|call| function.span.contains(call.span))
                .any(|call| self.call_open(file, function, call, walk))
    }

    fn call_open(
        &self,
        file: &FileFacts,
        function: &FunctionFact,
        call: &CallFact,
        walk: &OpenWalk<'_>,
    ) -> bool {
        if self.lookup.primitive_at_call(file, call.span).is_some() {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        // A call transport resolves is what it resolved to.
        match walk.targets.get(&location(file.path.shared(), call.callee)) {
            Some(Callable::Accessor(symbol)) => return self.source_open(symbol, walk),
            Some(Callable::Function { path, span }) => {
                return self.target_open(path, *span, walk);
            }
            Some(Callable::ArrayMembers(members)) => {
                return members
                    .iter()
                    .any(|(path, span)| self.target_open(path, *span, walk));
            }
            Some(Callable::Setter(symbol) | Callable::Action(symbol)) => {
                return self.withdrawn_array_slots.contains(symbol.as_str());
            }
            None => {}
        }
        if let Some(symbol) = self.lookup.callee_symbol(file, callee) {
            if self.accessors.contains_key(symbol) {
                return self.source_open(symbol, walk);
            }
            if self.setters.contains_key(symbol) || self.actions.contains_key(symbol) {
                return self.withdrawn_array_slots.contains(symbol);
            }
        }
        if let Some((target_file, target)) =
            self.lookup.function_called_at(file.path.as_str(), callee)
        {
            return self.function_open(target_file, target, walk);
        }
        if super::function_prototype_dispatch(self.lookup, file, call) {
            let Some(member) = file.ast.member_at(callee) else {
                return true;
            };
            return self.value_open(file, function, member.object);
        }
        // A method of the standard library runs what its arguments are, and
        // those are this same text.
        if self
            .lookup
            .standard_library_declaration(file, call, typefacts::CallKind::Call)
            .is_some()
        {
            return false;
        }
        self.value_open(file, function, callee)
    }

    fn target_open(&self, path: &str, span: Span, walk: &OpenWalk<'_>) -> bool {
        let Some(file) = self.lookup.file_by_path(path) else {
            return true;
        };
        let Some(function) = file.ast.function_at(span) else {
            return true;
        };
        self.function_open(file, function, walk)
    }

    /// Whether what `value` holds was handed over: a parameter of a function
    /// around the compute, or a value the handed net follows.
    fn value_open(&self, file: &FileFacts, function: &FunctionFact, value: Span) -> bool {
        let mut root = file.ast.peel_ts_sugar_span(value);
        while let Some(member) = file.ast.member_at(root) {
            root = file.ast.peel_ts_sugar_span(member.object);
        }
        let parameter = file
            .ast
            .reference_declaration(root)
            .is_some_and(|declaration| {
                file.ast.functions.iter().any(|holder| {
                    !function.span.contains(holder.span)
                        && holder.parameters.iter().any(|parameter| {
                            parameter.names.iter().any(|name| name.span == declaration)
                        })
                })
            });
        parameter
            || handed_value(
                &self.handed_tables(Some(&|symbol| {
                    self.async_sources.contains(symbol)
                        || self.withdrawn_array_slots.contains(symbol)
                })),
                file,
                value,
                super::HANDED_DEPTH - 1,
            )
    }
}
