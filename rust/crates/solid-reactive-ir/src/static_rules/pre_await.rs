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
use solid_facts::ast::{CallFact, FunctionFact, IdentifierRole};
use solid_facts::core::Span;
use std::cell::OnceCell;
use std::collections::HashMap;
use typefacts::AsyncFunctionFact;

/// Helper-in-helper nesting the scan follows before it gives up (a possible
/// read).
const MAX_DEPTH: usize = 6;

/// An inert declaration is only inert for the supplied runtime values. In
/// particular, a primitive *type* is insufficient: assertions do not change
/// objects and coercion/iteration/thenable assimilation can call project code.
fn inert_standard_call(
    name: &str,
    call: &CallFact,
    primitive: impl Fn(&solid_facts::ast::ArgumentFact) -> bool,
    function: impl Fn(&solid_facts::ast::ArgumentFact) -> bool,
) -> bool {
    if call.arguments.iter().any(|argument| argument.spread) {
        return false; // ArgumentListEvaluation itself drives an iterator.
    }
    let all_primitive = || call.arguments.iter().all(&primitive);
    match name {
        // These operations do not coerce or inspect argument properties.
        "ArrayConstructor.construct"
        | "ArrayConstructor.isArray"
        | "NumberConstructor.isFinite"
        | "NumberConstructor.isInteger"
        | "NumberConstructor.isNaN"
        | "NumberConstructor.isSafeInteger"
        | "PromiseConstructor.reject"
        | "DateConstructor.now" => true,
        "setTimeout"
        | "setInterval"
        | "WindowOrWorkerGlobalScope.setTimeout"
        | "WindowOrWorkerGlobalScope.setInterval" => {
            call.arguments
                .first()
                .is_none_or(|arg| function(arg) || primitive(arg))
                && call.arguments.get(1).is_none_or(&primitive)
            // Remaining arguments are retained without coercion.
        }
        "queueMicrotask" | "requestAnimationFrame" | "WindowOrWorkerGlobalScope.queueMicrotask" => {
            call.arguments.first().is_none_or(&function)
        }
        "PromiseConstructor.construct" => {
            // Its executor is scanned as an inline callback, including calls
            // of resolve/reject. An uninspectable executor is opaque.
            call.arguments.first().is_some_and(&function)
        }
        // A handler receiver may run a constructor/species getter or have an
        // overridden then. Its declaration alone does not prove inertness.
        "Promise.then" | "Promise.catch" | "Promise.finally" => false,
        "clearTimeout"
        | "clearInterval"
        | "cancelAnimationFrame"
        | "WindowOrWorkerGlobalScope.clearTimeout"
        | "WindowOrWorkerGlobalScope.clearInterval"
        | "fetch"
        | "WindowOrWorkerGlobalScope.fetch"
        | "PromiseConstructor.resolve"
        | "PromiseConstructor.all"
        | "PromiseConstructor.allSettled"
        | "PromiseConstructor.race"
        | "PromiseConstructor.any"
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
        let is_own = |span: Span| {
            own.is_some_and(|own| {
                containing_ast_function(&file.ast, span).is_some_and(|owner| owner.span == own.span)
            })
        };
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
            region.contains(*operand) && !skipped(*operand) && !self.primitive_value(file, *operand)
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
            // A lib declaration describes a signature, not the runtime
            // receiver: a Proxy or subclass may inherit that signature. Keep
            // only a direct global built-in after resolving its declaration.
            let global = |span: Span, expected: &str| {
                file.source_text(span) == Some(expected)
                    && file.ast.reference_declaration(span).is_none()
            };
            let direct_builtin = if let Some(class) = name.strip_suffix("Constructor.construct") {
                call.construct && global(callee, class)
            } else if let Some((receiver, _)) = name.split_once('.') {
                file.ast.member_at(callee).is_some_and(|member| {
                    if receiver == "WindowOrWorkerGlobalScope" {
                        ["window", "globalThis", "self"]
                            .iter()
                            .any(|name| global(member.object, name))
                    } else {
                        global(
                            member.object,
                            receiver.strip_suffix("Constructor").unwrap_or(receiver),
                        )
                    }
                })
            } else {
                global(callee, name)
            };
            if direct_builtin
                && inert_standard_call(
                    name,
                    call,
                    |argument| self.primitive_value(file, argument.span),
                    |argument| self.function_value(file, argument.span),
                )
            {
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
        // The result domain of an arbitrary expression does not prove that
        // evaluating it is inert (`delete proxy.key` returns a boolean).
        // Keep literals/templates and sourced primitive bindings; calls,
        // member Gets and operators need their own execution proof.
        if file.ast.primitive_results.binary_search(&value).is_ok()
            || file.ast.call_at(value).is_some()
            || file.ast.member_at(value).is_some()
            || file.ast.conditional_at(value).is_some()
            || file.ast.logical_at(value).is_some()
        {
            return false;
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
