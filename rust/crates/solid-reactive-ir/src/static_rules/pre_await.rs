//! Whether a computation's pre-await segment may already have tracked a
//! source (ADR 0195, ADR 0340).
//!
//! `checkPostAwaitRead` stays silent for a node already among the
//! computation's dependencies, so a read after the await of a source the
//! computation read before it is no defect. Two answers are proven: the same
//! source read in the straight-line flow before the first suspension (a
//! dependency: [`Tracking::Tracked`]), and a source nothing before the first
//! suspension can have read (a defect: [`Tracking::Untracked`]). Everything
//! between is [`Tracking::Possible`] and never a violation: a read through a
//! helper, a getter, an inline callback or a call whose body is not visible
//! (a package, a global, a standard-library call outside the audited inert
//! rows: it may run a callback or read a source captured earlier),
//! a conditional read, and a store key another path may alias (store path
//! text equality is not dependency identity: `{ a: item, b: item }`).
//! "Unresolved" is not evidence of anything, so a call the analysis cannot see
//! into is a possible read.

use super::native_promise::{direct_builtin, native_promise};
use super::{
    call_accessor_symbol, store_path_tracked, tracked_before_first_suspension,
    tracked_store_paths_before_first_suspension,
};
use crate::HashSet;
use crate::cleanup::callback_argument_literal;
use crate::identity::SymbolId;
use crate::owners::containing_ast_function;
use crate::pipeline::AnalysisContext;
use crate::runtime_semantics::{is_inline_standard_callback, standard_call_name};
use solid_facts::FileFacts;
use solid_facts::ast::{ArrayElementKind, CallFact, FunctionFact, IdentifierRole};
use solid_facts::core::Span;
use std::cell::OnceCell;
use std::collections::HashMap;
use typefacts::AsyncFunctionFact;

/// Helper-in-helper nesting the scan follows before it gives up (a possible
/// read).
const MAX_DEPTH: usize = 6;

/// Whether the compiler proves a value is a number and nothing else: the type
/// facts' primitive domain at the call, with no string, boolean, bigint,
/// symbol, nullish, object or unknown possibility (ADR 0350).
fn only_numbers(domain: typefacts::PrimitiveValueDomain) -> bool {
    domain.may_be_number()
        && !domain.may_be_string()
        && !domain.may_be_boolean()
        && !domain.may_be_big_int()
        && !domain.may_be_symbol()
        && !domain.may_be_null()
        && !domain.may_be_undefined()
        && !domain.may_be_object()
        && !domain.unknown()
}

/// A plain decimal numeric literal (`0`, `12`, `2.5`): a Number, never a
/// string, an identifier or a signed expression. Exponents and other forms are
/// not claimed.
fn is_numeric_literal(file: &FileFacts, span: Span) -> bool {
    let Some(text) = file.source_text(file.ast.peel_ts_sugar_span(span)) else {
        return false;
    };
    text.starts_with(|c: char| c.is_ascii_digit())
        && text.chars().all(|c| c.is_ascii_digit() || c == '.')
        && text.matches('.').count() <= 1
}

/// What the pre-await segment is known to have done to a source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tracking {
    /// A dependency already: a read after the await is silent.
    Tracked,
    /// Cannot be ruled out: the read after the await is an obligation.
    Possible,
    /// Nothing before the first suspension can have read it.
    Untracked,
}

impl Tracking {
    /// The better of two windows' answers: one window that tracked the source
    /// is enough, one that may have is the next best.
    pub(super) fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Tracked, _) | (_, Self::Tracked) => Self::Tracked,
            (Self::Possible, _) | (_, Self::Possible) => Self::Possible,
            _ => Self::Untracked,
        }
    }
}

/// How the segment touches one source: every key name a plain member chain
/// reads on it, and whether anything else (a bare reference, a computed or
/// optional member, a call that may read it) happens.
#[derive(Default)]
struct Reads {
    opaque: bool,
    names: HashSet<String>,
}

pub(super) struct PreAwait<'f, 'a> {
    ctx: &'f AnalysisContext<'a>,
    files: HashMap<&'f str, &'f FileFacts>,
    /// Project functions by the property name they are declared under: what
    /// a member read or a member call may run.
    methods: OnceCell<HashMap<String, Vec<(&'f FileFacts, &'f FunctionFact)>>>,
    /// ADR 0350: the accessors of signals that only ever hold numbers.
    number_accessors: OnceCell<HashSet<SymbolId>>,
}

impl<'f, 'a> PreAwait<'f, 'a> {
    pub(super) fn new(ctx: &'f AnalysisContext<'a>) -> Self {
        Self {
            ctx,
            files: ctx
                .facts
                .files
                .iter()
                .map(|file| (file.path.as_str(), file))
                .collect(),
            methods: OnceCell::new(),
            number_accessors: OnceCell::new(),
        }
    }

    /// Whether `symbol`, an accessor, is a dependency of the computation by
    /// the time it first suspends, in any of the windows.
    pub(super) fn accessor(
        &self,
        windows: &[std::rc::Rc<AsyncFunctionFact>],
        symbol: &SymbolId,
    ) -> Tracking {
        windows
            .iter()
            .map(|window| {
                if tracked_before_first_suspension(self.ctx, &self.files, window, symbol) {
                    Tracking::Tracked
                } else if self
                    .reads(window, symbol)
                    .is_some_and(|reads| reads.opaque || !reads.names.is_empty())
                {
                    Tracking::Possible
                } else {
                    Tracking::Untracked
                }
            })
            .fold(Tracking::Untracked, Tracking::or)
    }

    /// Whether the store key a member read after the await reads (`text`, its
    /// plain path, when it has one) is a dependency already.
    pub(super) fn store_key(
        &self,
        windows: &[std::rc::Rc<AsyncFunctionFact>],
        symbol: &SymbolId,
        text: Option<&str>,
    ) -> Tracking {
        windows
            .iter()
            .map(|window| self.store_key_in(window, symbol, text))
            .fold(Tracking::Untracked, Tracking::or)
    }

    fn store_key_in(
        &self,
        window: &AsyncFunctionFact,
        symbol: &SymbolId,
        text: Option<&str>,
    ) -> Tracking {
        let Some(reads) = self.reads(window, symbol) else {
            return Tracking::Possible;
        };
        let Some(text) = text else {
            // A computed key may be any key a read before the await used.
            return if reads.opaque || !reads.names.is_empty() {
                Tracking::Possible
            } else {
                Tracking::Untracked
            };
        };
        let tracked = tracked_store_paths_before_first_suspension(self.ctx, &self.files, window);
        let steps: Vec<&str> = text.split('.').collect();
        let mut possible = false;
        for index in 1..steps.len() {
            let prefix = steps[..=index].join(".");
            if store_path_tracked(&tracked, symbol, &prefix) {
                continue;
            }
            // Not the same path read before. Another path reaches the same
            // key node when it reads a key of that name (an object stored
            // under two paths shares its key nodes), or when anything else
            // before the await may have read the store.
            if reads.opaque || reads.names.contains(steps[index]) {
                possible = true;
            } else {
                return Tracking::Untracked;
            }
        }
        if possible {
            Tracking::Possible
        } else {
            Tracking::Tracked
        }
    }

    /// The window's own function, its file, and the code that runs before its
    /// first suspension: from the body's start to the end of the first
    /// `await` (its operand is evaluated before it suspends).
    fn region(
        &self,
        window: &AsyncFunctionFact,
    ) -> Option<(&'f FileFacts, &'f FunctionFact, Span)> {
        let file = *self.files.get(&*window.expression.path)?;
        let start = u32::try_from(window.expression.start_byte).ok()?;
        let end = u32::try_from(window.expression.end_byte).ok()?;
        let expression = file.ast.peel_ts_sugar_span(Span::new(start, end));
        let function = file.ast.function_at(expression)?;
        Some((file, function, Self::synchronous_part(file, function)))
    }

    /// The part of a function that runs on its caller's stack: up to the end
    /// of its first own suspension. What follows runs after the caller
    /// resumed, which is where this question stops.
    fn synchronous_part(file: &FileFacts, function: &FunctionFact) -> Span {
        let end = file
            .ast
            .awaits
            .iter()
            .chain(&file.ast.implicit_suspensions)
            .filter(|suspension| {
                function.body.contains(**suspension)
                    && containing_ast_function(&file.ast, **suspension)
                        .is_some_and(|owner| owner.span == function.span)
            })
            .min_by_key(|suspension| suspension.start)
            .map_or(function.body.end, |suspension| suspension.end);
        Span::new(function.body.start, end)
    }

    /// What the window's pre-await code may do to `symbol`, or `None` when
    /// the window cannot be found (nothing can then be ruled out).
    fn reads(&self, window: &AsyncFunctionFact, symbol: &SymbolId) -> Option<Reads> {
        let (file, function, region) = self.region(window)?;
        let mut reads = Reads::default();
        let mut visited = HashSet::default();
        self.scan(
            file,
            region,
            symbol,
            0,
            (&mut visited, &mut reads),
            Some(function),
        );
        Some(reads)
    }

    /// The function literals written in `region` that do not run as part of
    /// it: only an immediately invoked function expression and a callback an
    /// exact standard-library call invokes before it returns do. A handler,
    /// a timer callback, a callback of an effect, `untrack` or `runWithOwner`
    /// (tracking is off in them), or a function handed to code the analysis
    /// cannot see runs elsewhere; a call that may run one is an opaque call
    /// (see [`Self::call_reads`]).
    fn deferred_functions(&self, file: &'f FileFacts, region: Span) -> Vec<Span> {
        let ctx = self.ctx;
        let mut inline: HashSet<Span> = HashSet::default();
        for call in file.ast.calls_within(region) {
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if !call.construct && file.ast.function_at(callee).is_some() {
                inline.insert(callee);
            }
            let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, call.callee) else {
                continue;
            };
            for (index, argument) in call.arguments.iter().enumerate() {
                if !argument.spread
                    && is_inline_standard_callback(
                        resolved,
                        ctx.semantic_lookup
                            .smallest_contained_callability(file.path.as_str(), argument.span),
                        index,
                    )
                    && let Some(callback) = callback_argument_literal(file, argument.span)
                {
                    inline.insert(callback.span);
                }
            }
        }
        let mut deferred = Vec::new();
        for function in &file.ast.functions {
            if !region.contains(function.span) {
                continue;
            }
            if !inline.contains(&function.span) {
                deferred.push(function.span);
            } else if function.r#async || function.generator {
                // Called in place it runs up to its first suspension; the
                // rest is a continuation of its own.
                let end = Self::synchronous_part(file, function).end;
                deferred.push(Span::new(end, function.span.end));
            }
        }
        deferred
    }

    /// Everything `region` of `file` may do to `symbol`: references, calls
    /// into code that may read it, and member reads that may run a getter.
    fn scan(
        &self,
        file: &'f FileFacts,
        region: Span,
        symbol: &SymbolId,
        depth: usize,
        state: (&mut HashSet<(String, u32)>, &mut Reads),
        own: Option<&FunctionFact>,
    ) {
        let (visited, reads) = state;
        if depth > MAX_DEPTH {
            reads.opaque = true;
            return;
        }
        let ctx = self.ctx;
        let excluded = self.deferred_functions(file, region);
        let skipped = |span: Span| excluded.iter().any(|callback| callback.contains(span));
        // Written in the computation's own flow, not in a callback or a
        // helper: a conditional read there is a run that tracked nothing,
        // which is the feasible path the finding stands on (ADR 0195 exempts
        // only a read made on every run). A straight-line one is judged by
        // the caller (`tracked_before_first_suspension`, the store paths).
        let is_own =
            |span: Span| own.is_some_and(|own| crate::owners::owned_by_body(&file.ast, own, span));
        let identifiers = &file.ast.identifiers;
        let first = identifiers.partition_point(|identifier| identifier.span.start < region.start);
        for identifier in identifiers[first..]
            .iter()
            .take_while(|identifier| identifier.span.start < region.end)
        {
            if identifier.role != IdentifierRole::Reference
                || skipped(identifier.span)
                || ctx.entities.at(file.path.as_str(), identifier.span) != Some(symbol)
            {
                continue;
            }
            if is_own(identifier.span) {
                if file
                    .ast
                    .calls_within(region)
                    .any(|call| file.ast.peel_ts_sugar_span(call.callee) == identifier.span)
                {
                    continue;
                }
                // A key path read in straight-line flow can share a key node
                // with the one read after the await; a conditional one
                // leaves a run that read nothing.
                let mut own_reads = Reads::default();
                let outermost = self.chain(file, identifier.span, &mut own_reads);
                if outermost.is_none_or(|outermost| {
                    file.ast.completion_sites.binary_search(&outermost).is_ok()
                }) {
                    reads.opaque |= own_reads.opaque;
                    reads.names.extend(own_reads.names);
                }
                continue;
            }
            self.chain(file, identifier.span, reads);
        }
        for call in file.ast.calls_within(region) {
            if skipped(call.span)
                || (reads.opaque && depth > 0)
                || (is_own(call.span) && call_accessor_symbol(ctx, file, call) == Some(symbol))
            {
                continue;
            }
            if self.call_reads(file, call, symbol, depth, visited) {
                reads.opaque = true;
            }
        }
        if file.ast.coercing_operands.iter().any(|operand| {
            region.contains(*operand)
                && !skipped(*operand)
                && !self.primitive_value(file, *operand)
                && !self.suspends_within(file, *operand)
        }) {
            reads.opaque = true;
        }
        // A plain member read may run a getter of that name.
        for member in &file.ast.members {
            if !region.contains(member.span)
                || skipped(member.span)
                || file.ast.computed_members.contains(&member.span)
            {
                continue;
            }
            let Some(name) = file.source_text(member.property) else {
                continue;
            };
            for (method_file, method) in self.methods_named(name) {
                if self.function_reads(method_file, method, symbol, depth + 1, visited) {
                    reads.opaque = true;
                }
            }
        }
    }

    /// The member chain a reference to the source starts: the key names it
    /// reads, or an opaque use when it is not a plain chain. Returns the
    /// chain's outermost member, `None` for a bare reference.
    fn chain(&self, file: &FileFacts, reference: Span, reads: &mut Reads) -> Option<Span> {
        let mut current = reference;
        while let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| member.object == current)
        {
            if file.ast.computed_members.contains(&member.span) {
                reads.opaque = true;
                return Some(member.span);
            }
            match file.source_text(member.property) {
                Some(name) => {
                    reads.names.insert(name.to_owned());
                }
                None => reads.opaque = true,
            }
            current = member.span;
        }
        if current == reference {
            reads.opaque = true;
            return None;
        }
        Some(current)
    }

    /// Whether a call written in the segment may read `symbol`.
    fn call_reads(
        &self,
        file: &'f FileFacts,
        call: &'f CallFact,
        symbol: &SymbolId,
        depth: usize,
        visited: &mut HashSet<(String, u32)>,
    ) -> bool {
        let ctx = self.ctx;
        let callee_symbol = call_accessor_symbol(ctx, file, call);
        if callee_symbol == Some(symbol) {
            return true;
        }
        // The source passed along, or a function passed that may read it.
        for argument in &call.arguments {
            // The source itself handed over, not a read of it (`f(count())`
            // calls `count` here, which the call scan sees).
            let span = file.ast.peel_ts_sugar_span(argument.span);
            if file
                .ast
                .identifiers
                .binary_search_by_key(&span, |identifier| identifier.span)
                .is_ok()
                && ctx.entities.at(file.path.as_str(), span) == Some(symbol)
            {
                return true;
            }
            if callback_argument_literal(file, argument.span).is_none()
                && let Some((helper_file, helper)) = ctx
                    .semantic_lookup
                    .function_called_at(file.path.as_str(), argument.span)
                && self.function_reads(helper_file, helper, symbol, depth + 1, visited)
            {
                return true;
            }
        }
        // An accessor of another source, a setter, an action and a function
        // literal called in place (its body is part of the segment) read
        // nothing of this one by being called.
        if callee_symbol.is_some_and(|callee| {
            ctx.accessors.contains_key(callee)
                || ctx.setters.contains_key(callee)
                || ctx.actions.contains_key(callee)
        }) {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if !call.construct && file.ast.function_at(callee).is_some() {
            return false;
        }
        if crate::call_primitive_name_cached(
            ctx.semantic_lookup,
            file,
            call,
            ctx.entities,
            ctx.symbol_names,
            ctx.dialect,
        )
        .is_some_and(|name| name.primitive().is_some())
        {
            return false;
        }
        // A standard-library call states its own behavior, but only the
        // audited rows of [`inert_standard_call`] are known to run no code
        // registered elsewhere. Another one (`dispatchEvent`, `Function.call`,
        // `Reflect.apply`) may invoke a callback captured earlier, and a
        // package or global callee may read a source it was handed before
        // (ADR 0340): a possible read.
        if let Some(name) = ctx
            .semantic_lookup
            .resolved_callee_call(file, call.callee)
            .and_then(standard_call_name)
        {
            // A handler runs on its receiver. `then` on a native promise runs
            // no code now: its callbacks run later, and are scanned as deferred
            // (ADR 0349). Any other receiver may run its own `then`.
            if matches!(name, "Promise.then" | "Promise.catch" | "Promise.finally") {
                let receiver = file
                    .ast
                    .member_at(callee)
                    .filter(|_| !file.ast.computed_members.contains(&callee))
                    .map(|member| member.object);
                return !receiver.is_some_and(|receiver| native_promise(ctx, file, receiver, 0));
            }
            if direct_builtin(file, call, name) && self.inert_row(file, name, call) {
                return false;
            }
            // The declaration names the built-in, so a project member that
            // shares its property name (`stats.max` beside `Math.max`) is never
            // the callee. A call that is not inert may run what it holds.
            return true;
        }
        // A project function whose body is visible.
        let member = file
            .ast
            .member_at(callee)
            .filter(|member| !file.ast.computed_members.contains(&member.span));
        let target = member.map_or(callee, |member| member.property);
        if let Some((helper_file, helper)) = ctx
            .semantic_lookup
            .function_called_at(file.path.as_str(), target)
        {
            return self.function_reads(helper_file, helper, symbol, depth + 1, visited);
        }
        if let Some(member) = member {
            let name = file.source_text(member.property).unwrap_or_default();
            let methods = self.methods_named(name);
            if !methods.is_empty() {
                return methods.iter().any(|(method_file, method)| {
                    self.function_reads(method_file, method, symbol, depth + 1, visited)
                });
            }
        }
        // Not visible: nothing rules out a read.
        true
    }

    /// ADR 0350: a call of a signal accessor whose stored value is always a
    /// number. The signal is a `createSignal` array binding whose initial value
    /// is a numeric literal and whose setter is only ever called with numeric
    /// literals ([`Self::number_accessors`]). The type facts must also resolve
    /// the call to `number`. Coercing a number runs no code.
    fn number_signal_call(&self, file: &FileFacts, call: &CallFact) -> bool {
        let ctx = self.ctx;
        call.arguments.is_empty()
            && call_accessor_symbol(ctx, file, call)
                .is_some_and(|symbol| self.number_accessors().contains(symbol))
            && ctx
                .semantic_lookup
                .entity_at(file.path.as_str(), call.span)
                .and_then(|entity| entity.primitive_value_domain.present())
                .is_some_and(only_numbers)
    }

    /// The accessors of the numeric signals (ADR 0350), computed once: every
    /// `createSignal` array binding `[get, set]` with a numeric literal initial
    /// value, whose setter has no reference that is not a call with one numeric
    /// literal argument. A setter passed or read elsewhere may hold anything.
    fn number_accessors(&self) -> &HashSet<SymbolId> {
        self.number_accessors.get_or_init(|| {
            let ctx = self.ctx;
            // A signal without a setter (`const [count] = createSignal(0)`) is never written.
            let mut candidates: Vec<(SymbolId, Option<SymbolId>)> = Vec::new();
            for file in self.files.values() {
                for binding in &file.ast.bindings {
                    let (get, set) = match binding.array_slots.as_slice() {
                        [Some(get)] => (get, None),
                        [Some(get), None] => (get, None),
                        [Some(get), Some(set)] => (get, Some(set)),
                        _ => continue,
                    };
                    // A `let` pair could be reassigned to another accessor.
                    if binding.shape != solid_facts::ast::BindingShape::Array || !binding.immutable {
                        continue;
                    }
                    let Some(call) = binding
                        .call_initializer
                        .and_then(|span| file.ast.call_at(file.ast.peel_ts_sugar_span(span)))
                    else {
                        continue;
                    };
                    let creates_signal = !call.construct
                        && crate::call_primitive_name_cached(
                            ctx.semantic_lookup,
                            file,
                            call,
                            ctx.entities,
                            ctx.symbol_names,
                            ctx.dialect,
                        )
                        .and_then(|name| name.primitive())
                            == Some(solid_dialect::Primitive::CreateSignal);
                    let initial_is_number = matches!(
                        call.arguments.as_slice(),
                        [argument] if !argument.spread && is_numeric_literal(file, argument.span)
                    );
                    if !creates_signal || !initial_is_number {
                        continue;
                    }
                    let accessor = ctx.entities.at(file.path.as_str(), get.span);
                    let setter = match set {
                        Some(set) => ctx.entities.at(file.path.as_str(), set.span).map(Some),
                        None => Some(None),
                    };
                    if let (Some(accessor), Some(setter)) = (accessor, setter) {
                        candidates.push((accessor.clone(), setter.cloned()));
                    }
                }
            }
            if candidates.is_empty() {
                return HashSet::default();
            }
            let setters: HashSet<&SymbolId> = candidates
                .iter()
                .filter_map(|(_, setter)| setter.as_ref())
                .collect();
            // A setter reference is a numeric write unless it is anything else.
            let mut escaped: HashSet<SymbolId> = HashSet::default();
            for file in self.files.values() {
                let calls: HashMap<Span, &CallFact> = file
                    .ast
                    .calls
                    .iter()
                    .map(|call| (file.ast.peel_ts_sugar_span(call.callee), call))
                    .collect();
                for identifier in &file.ast.identifiers {
                    if identifier.role != IdentifierRole::Reference {
                        continue;
                    }
                    let Some(symbol) = ctx.entities.at(file.path.as_str(), identifier.span) else {
                        continue;
                    };
                    if !setters.contains(symbol) {
                        continue;
                    }
                    let numeric_write = calls.get(&identifier.span).is_some_and(|call| {
                        !call.construct
                            && matches!(
                                call.arguments.as_slice(),
                                [argument] if !argument.spread && is_numeric_literal(file, argument.span)
                            )
                    });
                    if !numeric_write {
                        escaped.insert(symbol.clone());
                    }
                }
            }
            candidates
                .into_iter()
                .filter(|(_, setter)| setter.as_ref().is_none_or(|setter| !escaped.contains(setter)))
                .map(|(accessor, _)| accessor)
                .collect()
        })
    }

    /// Whether an own suspension lies inside `operand`. Its value, and so the
    /// coercion of it, is produced only after the function resumed (ADR 0348).
    fn suspends_within(&self, file: &FileFacts, operand: Span) -> bool {
        let owner = containing_ast_function(&file.ast, operand).map(|function| function.span);
        file.ast
            .awaits
            .iter()
            .chain(&file.ast.implicit_suspensions)
            .any(|suspension| {
                operand.contains(*suspension)
                    && containing_ast_function(&file.ast, *suspension).map(|function| function.span)
                        == owner
            })
    }

    /// Whether the audited row `name` runs no code for the values `call`
    /// receives (ADR 0349). The caller has proven `call` is that built-in; each
    /// slot is judged by the value it holds, and a Promise row also needs the
    /// Promise premise, and the aggregates the array-iteration premise.
    fn inert_row(&self, file: &FileFacts, name: &str, call: &CallFact) -> bool {
        if call.arguments.iter().any(|argument| argument.spread) {
            // ArgumentListEvaluation itself drives an iterator.
            return false;
        }
        let semantic = self.ctx.semantic_lookup;
        let intrinsics = semantic.promise_intrinsics_intact();
        let all_primitive = || {
            call.arguments
                .iter()
                .all(|argument| self.primitive_value(file, argument.span))
        };
        match name {
            // These operations do not coerce or inspect argument properties.
            "ArrayConstructor.construct"
            | "ArrayConstructor.isArray"
            | "NumberConstructor.isFinite"
            | "NumberConstructor.isInteger"
            | "NumberConstructor.isNaN"
            | "NumberConstructor.isSafeInteger"
            | "DateConstructor.now" => true,
            // The reason is stored without thenable assimilation.
            "PromiseConstructor.reject" => intrinsics,
            "setTimeout"
            | "setInterval"
            | "WindowOrWorkerGlobalScope.setTimeout"
            | "WindowOrWorkerGlobalScope.setInterval" => {
                call.arguments.first().is_none_or(|argument| {
                    self.function_value(file, argument.span)
                        || self.primitive_value(file, argument.span)
                }) && call
                    .arguments
                    .get(1)
                    .is_none_or(|argument| self.primitive_value(file, argument.span))
                // Remaining arguments are retained without coercion.
            }
            "queueMicrotask"
            | "requestAnimationFrame"
            | "WindowOrWorkerGlobalScope.queueMicrotask" => call
                .arguments
                .first()
                .is_none_or(|argument| self.function_value(file, argument.span)),
            // The executor runs at once; its body is scanned as a callback.
            "PromiseConstructor.construct" => {
                intrinsics
                    && call
                        .arguments
                        .first()
                        .is_some_and(|argument| self.function_value(file, argument.span))
            }
            "PromiseConstructor.all"
            | "PromiseConstructor.allSettled"
            | "PromiseConstructor.race"
            | "PromiseConstructor.any" => {
                intrinsics && semantic.array_iteration_intact() && self.settled_elements(file, call)
            }
            "PromiseConstructor.resolve" => intrinsics && all_primitive(),
            "clearTimeout"
            | "clearInterval"
            | "cancelAnimationFrame"
            | "WindowOrWorkerGlobalScope.clearTimeout"
            | "WindowOrWorkerGlobalScope.clearInterval"
            | "fetch"
            | "WindowOrWorkerGlobalScope.fetch"
            | "parseInt"
            | "parseFloat"
            | "isNaN"
            | "isFinite"
            | "NumberConstructor.parseInt"
            | "NumberConstructor.parseFloat"
            | "encodeURIComponent"
            | "decodeURIComponent"
            | "encodeURI"
            | "decodeURI"
            | "ErrorConstructor.construct"
            | "TypeErrorConstructor.construct"
            | "RangeErrorConstructor.construct"
            | "SyntaxErrorConstructor.construct"
            | "ReferenceErrorConstructor.construct"
            | "EvalErrorConstructor.construct"
            | "URIErrorConstructor.construct"
            | "DateConstructor.construct"
            | "MapConstructor.construct"
            | "SetConstructor.construct"
            | "WeakMapConstructor.construct"
            | "WeakSetConstructor.construct"
            | "RegExpConstructor.construct"
            | "Math.abs"
            | "Math.acos"
            | "Math.acosh"
            | "Math.asin"
            | "Math.asinh"
            | "Math.atan"
            | "Math.atanh"
            | "Math.atan2"
            | "Math.cbrt"
            | "Math.ceil"
            | "Math.clz32"
            | "Math.cos"
            | "Math.cosh"
            | "Math.exp"
            | "Math.expm1"
            | "Math.floor"
            | "Math.fround"
            | "Math.hypot"
            | "Math.imul"
            | "Math.log"
            | "Math.log1p"
            | "Math.log2"
            | "Math.log10"
            | "Math.max"
            | "Math.min"
            | "Math.pow"
            | "Math.random"
            | "Math.round"
            | "Math.sign"
            | "Math.sin"
            | "Math.sinh"
            | "Math.sqrt"
            | "Math.tan"
            | "Math.tanh"
            | "Math.trunc" => all_primitive(),
            _ => false,
        }
    }

    /// `Promise.all([...])` and its siblings over an array literal whose every
    /// element is a native promise or a primitive (ADR 0349): driving the
    /// iterator and each element's `then` then runs only the intrinsics.
    fn settled_elements(&self, file: &FileFacts, call: &CallFact) -> bool {
        let [argument] = call.arguments.as_slice() else {
            return false;
        };
        if argument.spread {
            return false;
        }
        let Some(array) = file
            .ast
            .array_literal_at(file.ast.peel_ts_sugar_span(argument.span))
        else {
            return false;
        };
        array.elements.iter().all(|element| {
            element.kind == ArrayElementKind::Value && {
                let value = file.ast.peel_ts_sugar_span(element.span);
                native_promise(self.ctx, file, value, 0) || self.primitive_value(file, value)
            }
        })
    }

    /// Runtime function provenance, not a callable assertion. In addition to
    /// project functions, Promise supplies its executor's resolve/reject as
    /// native functions. This keeps `new Promise(done => setTimeout(done, 20))`
    /// inspectable without trusting an arbitrary callable-typed parameter.
    fn function_value(&self, file: &FileFacts, value: Span) -> bool {
        let value = file.ast.peel_ts_sugar_span(value);
        if callback_argument_literal(file, value).is_some()
            || self
                .ctx
                .semantic_lookup
                .function_called_at(file.path.as_str(), value)
                .is_some_and(|(f, fun)| self.ctx.semantic_lookup.function_value_is_current(f, fun))
        {
            return true;
        }
        let Some(declaration) = file.ast.reference_declaration(value) else {
            return false;
        };
        if file
            .ast
            .write_targets
            .iter()
            .any(|written| file.ast.reference_declaration(*written) == Some(declaration))
        {
            return false;
        }
        file.ast.calls.iter().any(|call| {
            call.construct
                && file.source_text(call.callee) == Some("Promise")
                && file.ast.reference_declaration(call.callee).is_none()
                && self
                    .ctx
                    .semantic_lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(standard_call_name)
                    == Some("PromiseConstructor.construct")
                && call
                    .arguments
                    .first()
                    .filter(|argument| !argument.spread)
                    .and_then(|argument| callback_argument_literal(file, argument.span))
                    .is_some_and(|executor| {
                        executor.parameters.iter().take(2).any(|parameter| {
                            parameter.shape == solid_facts::ast::BindingShape::Identifier
                                && parameter.names.len() == 1
                                && parameter.names[0].span == declaration
                        })
                    })
        })
    }

    fn primitive_value(&self, file: &FileFacts, value: Span) -> bool {
        let value = file.ast.peel_ts_sugar_span(value);
        // The result domain of an operator or member does not prove that
        // evaluating it is inert (`delete proxy.key` returns a boolean, a
        // member Get may run a getter). A call is different: the segment scan
        // judges the call's own evaluation, so only its result is asked here
        // (ADR 0350).
        if file.ast.primitive_results.binary_search(&value).is_ok()
            || file.ast.member_at(value).is_some()
            || file.ast.conditional_at(value).is_some()
            || file.ast.logical_at(value).is_some()
        {
            return false;
        }
        if let Some(call) = file.ast.call_at(value) {
            return self
                .ctx
                .semantic_lookup
                .proven_primitive(file, value, MAX_DEPTH)
                || self.number_signal_call(file, call);
        }
        file.source_text(value) == Some("null")
            || (file.source_text(value) == Some("undefined")
                && file.ast.reference_declaration(value).is_none())
            || self
                .ctx
                .semantic_lookup
                .proven_primitive(file, value, MAX_DEPTH)
    }

    /// Whether running a project function may read `symbol`.
    fn function_reads(
        &self,
        file: &'f FileFacts,
        function: &'f FunctionFact,
        symbol: &SymbolId,
        depth: usize,
        visited: &mut HashSet<(String, u32)>,
    ) -> bool {
        if !visited.insert((file.path.to_string(), function.span.start)) {
            return false;
        }
        let mut reads = Reads::default();
        self.scan(
            file,
            Self::synchronous_part(file, function),
            symbol,
            depth,
            (visited, &mut reads),
            None,
        );
        reads.opaque || !reads.names.is_empty()
    }

    fn methods_named(&self, name: &str) -> &[(&'f FileFacts, &'f FunctionFact)] {
        self.methods
            .get_or_init(|| {
                let mut methods: HashMap<String, Vec<(&FileFacts, &FunctionFact)>> = HashMap::new();
                for file in &self.ctx.facts.files {
                    for shape in &file.ast.object_get_shapes {
                        for property in &shape.properties {
                            if let Some(getter) = property
                                .getter
                                .and_then(|getter| file.ast.function_at(getter))
                            {
                                methods
                                    .entry(property.key.to_string())
                                    .or_default()
                                    .push((file, getter));
                            }
                        }
                    }
                    for function in &file.ast.functions {
                        if let Some(name) = function
                            .method_name
                            .as_ref()
                            .and_then(|name| file.source_text(name.span))
                        {
                            methods
                                .entry(name.to_owned())
                                .or_default()
                                .push((file, function));
                        }
                    }
                }
                methods
            })
            .get(name)
            .map_or(&[], Vec::as_slice)
    }
}
