//! SC1002 reach: which code an async computation's continuation runs.
//!
//! The TypeScript producer proves accessor calls written *in* the awaiting
//! function after its dominating await, and the member-read proof covers the
//! function's own straight-line flow. Both stop at a nested function. This
//! module extends the region by exactly the code the execution model proves
//! runs on the continuation, never by what a name suggests:
//!
//! - a function literal an exact standard-library declaration invokes before
//!   it returns (`Array#map`/`forEach`/`filter`/..., the audited
//!   [`InlineCallback`](crate::runtime_semantics) rows), and an immediately
//!   invoked function expression, written in a region, runs inside that
//!   region (ADR 0318, the standard library as documented);
//! - the callback of `.then`/`.catch`/`.finally` on a proven native
//!   `Promise`, when the call's result is the promise the computation
//!   returns or awaits, runs from a microtask with no owner: rc.13 attributes
//!   every read on the async stack below the computation's returned promise
//!   (`dist/dev-shared.js` `checkPostAwaitRead`, `attributedFlight`);
//! - an `async` project function the computation calls, exactly resolved, and
//!   whose promise the computation returns or awaits (directly, or through
//!   `Promise.all`/`allSettled`), continues the computation after its own
//!   first await.
//!
//! Every shape needs the promise to reach the computation's flight. An async
//! helper whose promise is ignored, or called from an event handler, is not
//! in a computation's continuation and stays silent. Reads of a source the
//! computation already tracked before its first suspension stay exempt
//! (ADR 0195).

use super::{
    AsyncComputations, call_accessor_symbol, member_read_site, member_reads_after_await,
    tracked_async_computation_context, tracked_before_first_suspension,
    typed_after_await_obligation, typed_unknown,
};
use crate::HashSet;
use crate::cleanup::callback_argument_literal;
use crate::identity::SymbolId;
use crate::owners::{containing_ast_function, written_directly_in};
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::runtime_semantics::{is_inline_standard_callback, standard_call_name};
use crate::{StaticDefect, StaticDefectKind, location};
use solid_facts::FileFacts;
use solid_facts::ast::{CallFact, FunctionFact, FunctionKind};
use solid_facts::core::Span;
use std::collections::HashMap;
use std::rc::Rc;
use typefacts::AsyncFunctionFact;

/// Nesting limit for callbacks inside callbacks and for chained promises.
const MAX_DEPTH: usize = 6;

/// One function whose continuation is part of a tracked async computation:
/// the computation itself, or a helper its returned promise awaits.
#[derive(Clone)]
struct Member {
    function: Rc<AsyncFunctionFact>,
    context: String,
    /// The functions whose own tracking window a source read may already
    /// have entered: the computation, every helper down to this one.
    windows: Vec<Rc<AsyncFunctionFact>>,
}

/// What one member's walk has recorded so far.
struct Walk<'m> {
    member: &'m Member,
    reported: HashSet<(String, u64, u64)>,
    visited: HashSet<Span>,
    /// Function literals queued as members, which own their obligations.
    queued: HashSet<(String, Span)>,
}

struct Reach<'f, 'a> {
    ctx: &'f AnalysisContext<'a>,
    files: HashMap<&'f str, &'f FileFacts>,
    facts: HashMap<(&'f str, u64, u64), &'f AsyncFunctionFact>,
    done: HashSet<(String, u64, u64)>,
}

pub(super) fn run(ctx: &AnalysisContext<'_>, draft: &mut ProgramDraft) {
    let mut reach = Reach {
        ctx,
        files: ctx
            .facts
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect(),
        facts: ctx
            .facts
            .typescript
            .files()
            .flat_map(|file| file.async_functions.iter())
            .map(|function| {
                (
                    (
                        function.expression.path.as_ref(),
                        function.expression.start_byte,
                        function.expression.end_byte,
                    ),
                    function,
                )
            })
            .collect(),
        done: HashSet::default(),
    };
    // Built on first use: only a function with something to report needs it.
    let mut computations: Option<AsyncComputations> = None;
    for typescript_file in ctx.facts.typescript.files() {
        for function in typescript_file.async_functions.iter() {
            let member_site = ctx
                .dialect
                .reports_member_reads_after_await()
                .then(|| member_read_site(&reach.files, function))
                .flatten();
            if function.calls_after_await.is_empty()
                && member_site.is_none()
                && !reach.has_promise_seed(function)
            {
                continue;
            }
            let index = computations
                .get_or_insert_with(|| AsyncComputations::new(ctx, &ctx.facts.typescript));
            let Some(context) = tracked_async_computation_context(ctx, function, index) else {
                continue;
            };
            let function = Rc::new(function.clone());
            let mut queue = vec![Member {
                function: Rc::clone(&function),
                context,
                windows: vec![function],
            }];
            while let Some(member) = queue.pop() {
                reach.report(draft, &member, &mut queue);
            }
        }
    }
}

/// The AST function an async-function fact describes.
fn ast_function<'f>(
    files: &HashMap<&'f str, &'f FileFacts>,
    function: &AsyncFunctionFact,
) -> Option<(&'f FileFacts, &'f FunctionFact)> {
    let file = *files.get(&*function.expression.path)?;
    let start = u32::try_from(function.expression.start_byte).ok()?;
    let end = u32::try_from(function.expression.end_byte).ok()?;
    let expression = file.ast.peel_ts_sugar_span(Span::new(start, end));
    Some((file, file.ast.function_at(expression)?))
}

/// The promise-producing operand of an `await`: the call that is the whole
/// operand, behind parentheses at most. Of nested awaits that share an end
/// (`await await call()`), the operand that starts first is the outer one's.
fn awaited_call(file: &FileFacts, awaited: Span) -> Option<&CallFact> {
    let operand = file
        .ast
        .await_operands
        .iter()
        .copied()
        .filter(|operand| operand.start > awaited.start && operand.end == awaited.end)
        .min_by_key(|operand| operand.start)?;
    file.ast
        .call_at(file.ast.peel_ts_sugar_span(operand))
        .filter(|call| !call.construct)
}

/// The calls whose promise a function hands to whoever awaits it: the
/// operands of its own `await`s (an `async` function only) and the calls it
/// returns. An `async` function that returns a promise adopts it: its own
/// promise settles only after the returned one does, and the runtime
/// attributes reads on the returned promise's continuation to the same
/// flight (measured on rc.13: `fixtures/runtime-oracle/after-await-*`).
fn chain_calls<'f>(file: &'f FileFacts, function: &'f FunctionFact) -> Vec<&'f CallFact> {
    if function.generator {
        return Vec::new();
    }
    let awaited = function
        .r#async
        .then(|| {
            file.ast
                .awaits
                .iter()
                .copied()
                .filter(|awaited| {
                    function.body.contains(*awaited)
                        && written_directly_in(&file.ast, function, *awaited)
                })
                .filter_map(|awaited| awaited_call(file, awaited))
        })
        .into_iter()
        .flatten();
    let returned = file
        .ast
        .own_returns(function)
        .chain(function.expression_return.iter())
        .filter_map(|returned| {
            let span = file.ast.peel_ts_sugar_span(returned.argument?);
            file.ast.call_at(span).filter(|call| !call.construct)
        });
    awaited.chain(returned).collect()
}

/// Whether the function the callee names is an `async` project function
/// whose value is the one the call runs.
fn async_helper<'x>(
    ctx: &AnalysisContext<'x>,
    file: &FileFacts,
    call: &CallFact,
) -> Option<(&'x FileFacts, &'x FunctionFact)> {
    if call.construct {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.member_at(callee).is_some() {
        return None;
    }
    let (helper_file, helper) = ctx
        .semantic_lookup
        .function_called_at(file.path.as_str(), callee)?;
    (helper.r#async
        && !helper.generator
        && helper.method_name.is_none()
        && ctx
            .semantic_lookup
            .function_value_is_current(helper_file, helper))
    .then_some((helper_file, helper))
}

/// The global `Promise`: the identifier `Promise`, bound by no declaration the
/// parser saw.
fn is_global_promise(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.source_text(span) == Some("Promise") && file.ast.reference_declaration(span).is_none()
}

/// A call to a static member of the global `Promise` (`Promise.all(...)`).
fn is_global_promise_static(file: &FileFacts, callee: Span) -> bool {
    let callee = file.ast.peel_ts_sugar_span(callee);
    !file.ast.computed_members.contains(&callee)
        && file
            .ast
            .member_at(callee)
            .is_some_and(|member| is_global_promise(file, member.object))
}

impl<'f, 'a> Reach<'f, 'a> {
    fn key(function: &AsyncFunctionFact) -> (String, u64, u64) {
        (
            function.expression.path.to_string(),
            function.expression.start_byte,
            function.expression.end_byte,
        )
    }

    /// Whether the function returns or awaits a call that could put a
    /// promise on a computation's flight. Cheap and syntactic-plus-exact: the
    /// computation proof, which scans the project, runs only when it holds.
    fn has_promise_seed(&self, function: &'f AsyncFunctionFact) -> bool {
        let Some((file, ast_function)) = ast_function(&self.files, function) else {
            return false;
        };
        chain_calls(file, ast_function).into_iter().any(|call| {
            async_helper(self.ctx, file, call).is_some()
                || self
                    .ctx
                    .semantic_lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(standard_call_name)
                    .is_some_and(|name| {
                        matches!(
                            name,
                            "Promise.then"
                                | "Promise.catch"
                                | "Promise.finally"
                                | "PromiseConstructor.all"
                                | "PromiseConstructor.allSettled"
                        )
                    })
        })
    }

    /// Whether the window's tracking already made `symbol` a dependency.
    fn exempt(&self, member: &Member, symbol: &SymbolId) -> bool {
        member
            .windows
            .iter()
            .any(|window| tracked_before_first_suspension(self.ctx, &self.files, window, symbol))
    }

    fn report(&mut self, draft: &mut ProgramDraft, member: &Member, queue: &mut Vec<Member>) {
        if !self.done.insert(Self::key(&member.function)) {
            return;
        }
        let ctx = self.ctx;
        let function = &*member.function;
        let member_site = ctx
            .dialect
            .reports_member_reads_after_await()
            .then(|| member_read_site(&self.files, function))
            .flatten();
        let mut walk = Walk {
            member,
            reported: HashSet::default(),
            visited: HashSet::default(),
            queued: HashSet::default(),
        };
        // The producer's dominance facts: accessor calls after the first
        // await that dominates them.
        let mut after_await_calls: Vec<(&'f FileFacts, &'f CallFact)> = Vec::new();
        for call in &function.calls_after_await {
            let ast_call = self.files.get(&*call.path).and_then(|file| {
                file.ast
                    .calls
                    .iter()
                    .find(|candidate| {
                        u64::from(candidate.callee.start) == call.start_byte
                            && u64::from(candidate.callee.end) == call.end_byte
                    })
                    .map(|candidate| (*file, candidate))
            });
            if let Some(found) = ast_call {
                after_await_calls.push(found);
            }
            // Whole member spans may retain their receiver's symbol. The
            // exact property demand carries the callable member's identity;
            // a reactive receiver does not prove its method invokes that
            // receiver or reads an accessor.
            let symbol = ast_call.and_then(|(file, candidate)| {
                if let Some(member) = file
                    .ast
                    .members
                    .iter()
                    .find(|member| member.span == file.ast.peel_ts_sugar_span(candidate.callee))
                {
                    (!file.ast.computed_members.contains(&member.span))
                        .then(|| ctx.entities.at(file.path.as_str(), member.property))
                        .flatten()
                } else {
                    ctx.entities.get(call)
                }
            });
            let Some((name, _)) = symbol.and_then(|symbol| ctx.accessors.get(symbol)) else {
                // ADR 0294: a typed candidate's read after the await is an
                // obligation; its type is the only reactive evidence.
                if let (Some(symbol), Some((file, candidate))) = (symbol, ast_call)
                    && typed_unknown(ctx, symbol)
                    && walk
                        .reported
                        .insert((call.path.to_string(), call.start_byte, call.end_byte))
                {
                    draft.push_defect(typed_after_await_obligation(file, candidate.callee));
                }
                continue;
            };
            // The same source read on every run before the first suspension
            // is a dependency already (ADR 0195), and rc.13's own
            // `UNTRACKED_READ_AFTER_AWAIT` stays silent for it.
            if symbol.is_some_and(|symbol| self.exempt(member, symbol)) {
                continue;
            }
            let display = ast_call
                .and_then(|(file, candidate)| candidate.static_callee(&file.source))
                .unwrap_or(name);
            walk.reported
                .insert((call.path.to_string(), call.start_byte, call.end_byte));
            draft.push_defect(StaticDefect {
                kind: StaticDefectKind::ReactiveReadAfterAwait {
                    accessor: display.to_string(),
                },
                location: typefacts::Location {
                    path: call.path.clone(),
                    start_byte: call.start_byte,
                    end_byte: call.end_byte,
                },
                analysis_context: member.context.clone(),
                fixes: vec![],
                uncertain: false,
            });
        }
        // The promise chain first: it records which function literals become
        // members, so the callback walk below leaves their obligations to them.
        if let Some((file, ast_function)) = ast_function(&self.files, function) {
            for call in chain_calls(file, ast_function) {
                self.expand(draft, &mut walk, queue, file, call, 0);
            }
        }
        if let Some(site) = member_site {
            member_reads_after_await(ctx, draft, site, &member.context);
            let (file, ast_function, boundary) = site;
            for call in file.ast.calls_within(ast_function.body) {
                if call.span.start >= boundary
                    && containing_ast_function(&file.ast, call.span)
                        .is_some_and(|owner| owner.span == ast_function.span)
                {
                    after_await_calls.push((file, call));
                }
            }
        }
        for (file, call) in after_await_calls {
            self.visit_call(draft, &mut walk, file, call, 0);
        }
    }

    /// The callbacks a call runs inline: an immediately invoked function
    /// expression, and a function literal in a slot an exact
    /// standard-library declaration invokes before it returns.
    fn visit_call(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        call: &'f CallFact,
        depth: usize,
    ) {
        if depth > MAX_DEPTH || !walk.visited.insert(call.span) {
            return;
        }
        let ctx = self.ctx;
        self.report_helper_reads(draft, walk, file, call, 0);
        if !call.construct
            && let Some(function) = file
                .ast
                .function_at(file.ast.peel_ts_sugar_span(call.callee))
            && function.kind != FunctionKind::Declaration
            && function.method_name.is_none()
            && !function.r#async
            && !function.generator
        {
            self.report_region(draft, walk, file, function, depth);
            return;
        }
        let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, call.callee) else {
            return;
        };
        for (index, argument) in call.arguments.iter().enumerate() {
            if argument.spread {
                continue;
            }
            let callability = ctx
                .semantic_lookup
                .smallest_contained_callability(file.path.as_str(), argument.span);
            if !is_inline_standard_callback(resolved, callability, index) {
                continue;
            }
            match callback_argument_literal(file, argument.span) {
                Some(callback) if !callback.r#async && !callback.generator => {
                    self.report_region(draft, walk, file, callback, depth);
                }
                // A literal queued as a member of the promise chain reports
                // for itself; anything else the body of which cannot be
                // inspected stays an obligation, never a silent clean.
                Some(callback)
                    if walk
                        .queued
                        .contains(&(file.path.to_string(), callback.span)) => {}
                _ => draft.push_defect(StaticDefect {
                    kind: StaticDefectKind::ReactiveCallbackUnresolved {
                        callee: call
                            .static_callee(&file.source)
                            .unwrap_or("this callback")
                            .to_owned(),
                    },
                    location: location(file.path.shared(), argument.span),
                    analysis_context: walk.member.context.clone(),
                    fixes: vec![],
                    uncertain: false,
                }),
            }
        }
    }

    /// The reads a synchronous project helper performs on every call: the
    /// call runs the helper's body on the continuation's stack, so a read the
    /// body makes on every normal completion (ADR 0276, the same completion
    /// fact the leaf-scope walk relies on) is an untracked read after the
    /// await. A read on some paths only, or after a helper's own suspension,
    /// is not proven and stays with the interprocedural obligation.
    fn report_helper_reads(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &FileFacts,
        call: &CallFact,
        depth: usize,
    ) {
        let ctx = self.ctx;
        if call.construct || depth > 3 {
            return;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if file.ast.member_at(callee).is_some() {
            return;
        }
        // A callee that is itself an accessor, or a typed candidate, is read
        // as the access it is, not as a helper.
        if call_accessor_symbol(ctx, file, call)
            .is_some_and(|symbol| ctx.accessors.contains_key(symbol) || typed_unknown(ctx, symbol))
        {
            return;
        }
        let Some((helper_file, helper)) = ctx
            .semantic_lookup
            .function_called_at(file.path.as_str(), callee)
        else {
            return;
        };
        if helper.r#async
            || helper.generator
            || helper.method_name.is_some()
            || !ctx
                .semantic_lookup
                .function_value_is_current(helper_file, helper)
        {
            return;
        }
        for read in helper_file.ast.calls_within(helper.body) {
            if !written_directly_in(&helper_file.ast, helper, read.span)
                || !crate::interproc::site_on_every_completion(
                    ctx.semantic_lookup,
                    helper_file,
                    helper.span,
                    read.span,
                )
            {
                continue;
            }
            self.report_call_read(draft, walk, helper_file, read);
            // A helper the helper calls on every completion runs on the same
            // stack, so its straight-line reads are reached the same way.
            self.report_helper_reads(draft, walk, helper_file, read, depth + 1);
        }
    }

    /// A function literal's body as part of the continuation: its member
    /// reads and accessor calls, and the callbacks its own calls run.
    fn report_region(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        callback: &'f FunctionFact,
        depth: usize,
    ) {
        let ctx = self.ctx;
        if ctx.dialect.reports_member_reads_after_await() {
            // The whole body is past the suspension: the boundary is its start.
            member_reads_after_await(
                ctx,
                draft,
                (file, callback, callback.span.start),
                &walk.member.context,
            );
        }
        for call in file.ast.calls_within(callback.body) {
            if !written_directly_in(&file.ast, callback, call.span) {
                continue;
            }
            self.report_call_read(draft, walk, file, call);
            self.visit_call(draft, walk, file, call, depth + 1);
        }
    }

    /// The accessor read a call written in a region performs.
    fn report_call_read(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        call: &CallFact,
    ) {
        let ctx = self.ctx;
        let Some(symbol) = call_accessor_symbol(ctx, file, call) else {
            return;
        };
        let key = (
            file.path.to_string(),
            u64::from(call.callee.start),
            u64::from(call.callee.end),
        );
        let Some((name, _)) = ctx.accessors.get(symbol) else {
            if typed_unknown(ctx, symbol) && walk.reported.insert(key) {
                draft.push_defect(typed_after_await_obligation(file, call.callee));
            }
            return;
        };
        if self.exempt(walk.member, symbol) || !walk.reported.insert(key) {
            return;
        }
        let display = call.static_callee(&file.source).unwrap_or(name);
        draft.push_defect(StaticDefect {
            kind: StaticDefectKind::ReactiveReadAfterAwait {
                accessor: display.to_string(),
            },
            location: location(file.path.shared(), call.callee),
            analysis_context: walk.member.context.clone(),
            fixes: vec![],
            uncertain: false,
        });
    }

    /// Follows a call whose promise reaches the computation's flight.
    fn expand(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &'f FileFacts,
        call: &'f CallFact,
        depth: usize,
    ) {
        if depth > MAX_DEPTH || call.construct {
            return;
        }
        let ctx = self.ctx;
        if let Some((helper_file, helper)) = async_helper(ctx, file, call) {
            self.enqueue(walk, queue, helper_file, helper);
            return;
        }
        let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, call.callee) else {
            return;
        };
        let Some(name) = standard_call_name(resolved) else {
            return;
        };
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        match name {
            "Promise.then" | "Promise.catch" | "Promise.finally" => {
                let Some(receiver) = file
                    .ast
                    .member_at(callee)
                    .filter(|_| !file.ast.computed_members.contains(&callee))
                    .map(|member| member.object)
                else {
                    return;
                };
                // Only a native promise runs the handler from a microtask on
                // the stack the flight is attributed to. A thenable, a
                // subclass or an unknown receiver may call back inline.
                if !self.native_promise(file, receiver, 0) {
                    return;
                }
                let handlers = if name == "Promise.then" { 2 } else { 1 };
                for argument in call.arguments.iter().take(handlers) {
                    if argument.spread {
                        continue;
                    }
                    if let Some(callback) = callback_argument_literal(file, argument.span)
                        && !callback.r#async
                        && !callback.generator
                    {
                        self.report_region(draft, walk, file, callback, depth);
                    }
                }
                let receiver = file.ast.peel_ts_sugar_span(receiver);
                if let Some(inner) = file.ast.call_at(receiver) {
                    self.expand(draft, walk, queue, file, inner, depth + 1);
                }
            }
            "PromiseConstructor.all" | "PromiseConstructor.allSettled"
                if is_global_promise_static(file, callee) =>
            {
                let Some(argument) = call.arguments.first().filter(|argument| !argument.spread)
                else {
                    return;
                };
                let span = file.ast.peel_ts_sugar_span(argument.span);
                if let Some(array) = file.ast.array_literal_at(span) {
                    for element in &array.elements {
                        if element.kind != solid_facts::ast::ArrayElementKind::Value {
                            continue;
                        }
                        let element = file.ast.peel_ts_sugar_span(element.span);
                        if let Some(inner) = file.ast.call_at(element) {
                            self.expand(draft, walk, queue, file, inner, depth + 1);
                        }
                    }
                } else if let Some(mapped) = file.ast.call_at(span) {
                    self.expand_mapped(walk, queue, file, mapped);
                }
            }
            _ => {}
        }
    }

    /// `Promise.all(items.map(async (item) => ...))`: each element is the
    /// promise an `async` callback (or an `async` project function) returns.
    fn expand_mapped(
        &self,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &'f FileFacts,
        mapped: &'f CallFact,
    ) {
        let ctx = self.ctx;
        if mapped.construct {
            return;
        }
        let Some(resolved) = ctx
            .semantic_lookup
            .resolved_callee_call(file, mapped.callee)
        else {
            return;
        };
        if !matches!(
            standard_call_name(resolved),
            Some("Array.map" | "ReadonlyArray.map")
        ) {
            return;
        }
        let Some(argument) = mapped.arguments.first().filter(|argument| !argument.spread) else {
            return;
        };
        if let Some(callback) = callback_argument_literal(file, argument.span) {
            if callback.r#async && !callback.generator {
                self.enqueue(walk, queue, file, callback);
            }
        } else if let Some((helper_file, helper)) = ctx.semantic_lookup.function_called_at(
            file.path.as_str(),
            file.ast.peel_ts_sugar_span(argument.span),
        ) && helper.r#async
            && !helper.generator
            && helper.method_name.is_none()
            && ctx
                .semantic_lookup
                .function_value_is_current(helper_file, helper)
        {
            self.enqueue(walk, queue, helper_file, helper);
        }
    }

    /// Adds an `async` function whose promise the computation awaits.
    fn enqueue(
        &self,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &FileFacts,
        function: &FunctionFact,
    ) {
        // A function with no `await` carries no producer fact: nothing is
        // dominated by a suspension in it, but its returned promises still
        // continue the chain.
        let fact = self
            .facts
            .get(&(
                file.path.as_str(),
                u64::from(function.span.start),
                u64::from(function.span.end),
            ))
            .map_or_else(
                || AsyncFunctionFact {
                    expression: location(file.path.shared(), function.span),
                    symbol: "".into(),
                    target: "".into(),
                    can_return_async: true,
                    calls_after_await: Vec::new(),
                },
                |fact| (*fact).clone(),
            );
        if self.done.contains(&Self::key(&fact)) {
            return;
        }
        walk.queued.insert((file.path.to_string(), function.span));
        let fact = Rc::new(fact);
        let mut windows = walk.member.windows.clone();
        windows.push(Rc::clone(&fact));
        queue.push(Member {
            function: fact,
            context: walk.member.context.clone(),
            windows,
        });
    }

    /// Whether `span` is provably a native `Promise` (constructor `Promise`):
    /// `new Promise(...)` and `Promise.resolve`/`reject`/combinators of the
    /// global `Promise`, a call to an `async` project function (never a
    /// subclass), a project function whose every return is one of these, and
    /// `.then`/`.catch`/`.finally` of one. A typed `Promise<T>` or
    /// `PromiseLike<T>` proves nothing: a subclass or a thenable has that
    /// type too.
    fn native_promise(&self, file: &FileFacts, span: Span, depth: usize) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        let ctx = self.ctx;
        let span = file.ast.peel_ts_sugar_span(span);
        let Some(call) = file.ast.call_at(span) else {
            return false;
        };
        if async_helper(ctx, file, call).is_some() {
            return true;
        }
        let standard = ctx
            .semantic_lookup
            .resolved_callee_call(file, call.callee)
            .and_then(standard_call_name);
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        match (call.construct, standard) {
            (true, Some("PromiseConstructor.construct")) => is_global_promise(file, call.callee),
            (
                false,
                Some(
                    "PromiseConstructor.resolve"
                    | "PromiseConstructor.reject"
                    | "PromiseConstructor.all"
                    | "PromiseConstructor.allSettled"
                    | "PromiseConstructor.race"
                    | "PromiseConstructor.any",
                ),
            ) => is_global_promise_static(file, callee),
            (false, Some("Promise.then" | "Promise.catch" | "Promise.finally")) => file
                .ast
                .member_at(callee)
                .filter(|_| !file.ast.computed_members.contains(&callee))
                .is_some_and(|member| self.native_promise(file, member.object, depth + 1)),
            (false, None) => self.returns_native_promise(file, call, depth),
            _ => false,
        }
    }

    /// A call to a project function that, on every path, returns a native
    /// promise (`const pause = () => new Promise(...)`).
    fn returns_native_promise(&self, file: &FileFacts, call: &CallFact, depth: usize) -> bool {
        let ctx = self.ctx;
        if call.construct {
            return false;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if file.ast.member_at(callee).is_some() {
            return false;
        }
        let Some((helper_file, helper)) = ctx
            .semantic_lookup
            .function_called_at(file.path.as_str(), callee)
        else {
            return false;
        };
        if helper.generator
            || helper.method_name.is_some()
            || !ctx
                .semantic_lookup
                .function_value_is_current(helper_file, helper)
        {
            return false;
        }
        let mut returned = helper_file
            .ast
            .own_returns(helper)
            .chain(helper.expression_return.iter())
            .peekable();
        if returned.peek().is_none() {
            return false;
        }
        returned.all(|returned| {
            returned
                .argument
                .is_some_and(|argument| self.native_promise(helper_file, argument, depth + 1))
        })
    }
}
