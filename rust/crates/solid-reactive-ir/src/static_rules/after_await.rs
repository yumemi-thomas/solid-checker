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
//!   (`dist/dev-shared.js` `checkPostAwaitRead`, `attributedFlight`). An
//!   `async` function that returns the chain without awaiting it adopts the
//!   promise one job later, so a handler on a receiver that may be settled
//!   has no frame of the computation below it (see [`Carried`]); a rejection
//!   handler, a fulfillment handler on a receiver not proven to fulfill, a
//!   `sort` comparator, a `reduce` without an initial value and a `replace`
//!   replacer run only conditionally. Those reads are proof obligations,
//!   never violations;
//! - an `async` project function the computation calls, exactly resolved, and
//!   whose promise the computation returns or awaits (directly, or through
//!   `Promise.all`/`allSettled`), continues the computation after its own
//!   first await. Returned without `await` it is adopted a job later, so it
//!   counts only when its awaits are on proven pending promises.
//!
//! Every shape needs the promise to reach the computation's flight. An async
//! helper whose promise is ignored, or called from an event handler, is not
//! in a computation's continuation and stays silent. Reads of a source the
//! computation already tracked before its first suspension stay exempt
//! (ADR 0195).

use super::native_promise::{
    async_helper, is_global_promise, is_global_promise_static, native_promise, native_promise_claim,
};
use super::pre_await::{PreAwait, Tracking};
use super::{
    SharedAsyncComputations, call_accessor_symbol, member_read_site, member_reads_after_await,
    tracked_async_computation_context, typed_after_await_obligation, typed_unknown,
};
use crate::HashSet;
use crate::cleanup::callback_argument_literal;
use crate::identity::SymbolId;
use crate::indexes::mentions_word;
use crate::owners::written_directly_in;
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::runtime_semantics::{is_inline_standard_callback, is_timer_call, standard_call_name};
use crate::{StaticDefect, StaticDefectKind, location};
use solid_dialect::Primitive;
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentValueKind, CallFact, FunctionFact, FunctionKind};
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
    /// The member's continuation may run before anything holds a reaction on
    /// its promise: an `async` function returned, not awaited, whose first
    /// await is on a promise not proven still pending. Its reads, and the
    /// reads of everything it continues into, are proof obligations.
    conditional: bool,
}

/// What one member's walk has recorded so far.
struct Walk<'m> {
    member: &'m Member,
    reported: HashSet<(String, u64, u64)>,
    visited: HashSet<Span>,
    /// Function literals queued as members, which own their obligations.
    queued: HashSet<(String, Span)>,
    /// How the promise chain being followed reaches the flight.
    carried: Carried,
    /// The chain being followed runs inside `untrack`: a helper it reaches
    /// is not inside the computation's tracking window.
    untracked: bool,
    /// The chain being followed runs on an intrinsic a visible write may have
    /// changed (ADR 0349): every member it queues is conditional.
    unproven: bool,
}

struct Reach<'f, 'a> {
    ctx: &'f AnalysisContext<'a>,
    files: HashMap<&'f str, &'f FileFacts>,
    facts: HashMap<(&'f str, u64, u64), &'f AsyncFunctionFact>,
    /// The producer's facts by end offset: an object method's span ends where its function does (ADR 0351).
    facts_by_end: HashMap<(&'f str, u64), Vec<&'f AsyncFunctionFact>>,
    done: HashSet<(String, u64, u64)>,
    pre: PreAwait<'f, 'a>,
}

pub(super) fn run<'a>(
    ctx: &AnalysisContext<'a>,
    computations: &SharedAsyncComputations<'a>,
    draft: &mut ProgramDraft,
) {
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
        facts_by_end: {
            let mut by_end: HashMap<(&str, u64), Vec<&AsyncFunctionFact>> = HashMap::new();
            for function in ctx
                .facts
                .typescript
                .files()
                .flat_map(|file| file.async_functions.iter())
            {
                by_end
                    .entry((
                        function.expression.path.as_ref(),
                        function.expression.end_byte,
                    ))
                    .or_default()
                    .push(function);
            }
            by_end
        },
        done: HashSet::default(),
        pre: PreAwait::new(ctx),
    };
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
                && !reach.has_implicit_suspension(function)
                && !reach.has_try_handler(function)
            {
                continue;
            }
            // Built on first use, once per build: only a function with
            // something to report needs it.
            let Some(context) =
                tracked_async_computation_context(ctx, function, computations.get(ctx))
            else {
                continue;
            };
            let function = Rc::new(function.clone());
            // An owner a `dispose` function can reach may be gone when the
            // continuation resumes: the runtime then reads in a disposed
            // computation and stays silent, so every read is an obligation.
            let conditional = owner_may_be_disposed(ctx, &reach.files, &function);
            let mut queue = vec![Member {
                function: Rc::clone(&function),
                context,
                windows: vec![function],
                conditional,
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
    if let Some(found) = file.ast.function_at(expression) {
        return Some((file, found));
    }
    // ADR 0351: an object method's producer span starts at its key or `async`,
    // its function at the parameter list: the same end, with method-header text between.
    let found = file.ast.functions.iter().find(|candidate| {
        candidate.span.end == end
            && candidate.span.start > start
            && method_header(file.source_text(Span::new(start, candidate.span.start)))
    })?;
    Some((file, found))
}

/// Text a method's producer span may hold before its parameter list and no
/// other function: `async`, `*`, a key name, whitespace.
fn method_header(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        !text.is_empty()
            && text.chars().all(|c| {
                c.is_alphanumeric() || c == '_' || c == '$' || c == '*' || c.is_whitespace()
            })
    })
}

/// The immediate stack segment stops at the first own suspension, including
/// its operand. Nothing in the async tail executes on this caller's stack.
fn synchronous_prefix(file: &FileFacts, function: &FunctionFact) -> Span {
    let end = file
        .ast
        .awaits
        .iter()
        .chain(&file.ast.implicit_suspensions)
        .filter(|awaited| {
            function.body.contains(**awaited) && written_directly_in(&file.ast, function, **awaited)
        })
        .map(|awaited| awaited.end)
        .min()
        .unwrap_or(function.body.end);
    Span::new(function.body.start, end)
}

/// The operand span of an `await`. Of nested awaits that share an end
/// (`await await call()`), the operand that starts first is the outer one's.
fn await_operand(file: &FileFacts, awaited: Span) -> Option<Span> {
    file.ast
        .await_operands
        .iter()
        .copied()
        .filter(|operand| operand.start > awaited.start && operand.end == awaited.end)
        .min_by_key(|operand| operand.start)
}

/// The promise-producing operand of an `await`: the call that is the whole
/// operand, behind parentheses at most.
fn awaited_call(file: &FileFacts, awaited: Span) -> Option<&CallFact> {
    let operand = await_operand(file, awaited)?;
    file.ast
        .call_at(file.ast.peel_ts_sugar_span(operand))
        .filter(|call| !call.construct)
}

/// How a promise is proven to settle.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Settlement {
    Fulfills,
    Rejects,
    Unknown,
}

/// A literal that is not a thenable: a number, a string without
/// substitutions, `true`, `false`, `null`, or the global `undefined`, `NaN` or
/// `Infinity`. The three globals are resolved, not read by spelling: a
/// binding of that name (`const undefined = Promise.reject()`) is a value.
fn plain_value(file: &FileFacts, span: Span) -> bool {
    let Some(text) = file
        .source_text(file.ast.peel_ts_sugar_span(span))
        .map(str::trim)
    else {
        return false;
    };
    matches!(text, "true" | "false" | "null")
        || (matches!(text, "undefined" | "NaN" | "Infinity")
            && file
                .ast
                .reference_declaration(file.ast.peel_ts_sugar_span(span))
                .is_none()
            && file
                .ast
                .identifiers
                .binary_search_by_key(&file.ast.peel_ts_sugar_span(span), |identifier| {
                    identifier.span
                })
                .is_ok())
        || (!text.is_empty() && text.chars().all(|c| c.is_ascii_digit() || c == '.'))
        || (text.len() >= 2
            && matches!(text.as_bytes()[0], b'"' | b'\'')
            && text.as_bytes()[text.len() - 1] == text.as_bytes()[0]
            && !text[1..text.len() - 1].contains(text.as_bytes()[0] as char))
}

/// Whether a `createRoot` whose `dispose` function the code mentions encloses
/// the computation: the root, and the computation with it, may be disposed
/// before the continuation resumes (an explicit `dispose` call, one passed to
/// a timer, returned to a caller). The runtime then reads in a disposed
/// computation and does not warn, so no read after its await is proven.
fn owner_may_be_disposed(
    ctx: &AnalysisContext<'_>,
    files: &HashMap<&str, &FileFacts>,
    function: &AsyncFunctionFact,
) -> bool {
    let Some((file, ast_function)) = ast_function(files, function) else {
        return false;
    };
    // The outermost function around the computation: a root argument that
    // cannot be resolved might be it, or a function that contains it.
    let outermost = file
        .ast
        .functions
        .iter()
        .filter(|candidate| candidate.span.contains(ast_function.span))
        .max_by_key(|candidate| candidate.span.end - candidate.span.start);
    file.ast.calls.iter().any(|call| {
        if call.construct
            || crate::call_primitive_name_cached(
                ctx.semantic_lookup,
                file,
                call,
                ctx.entities,
                ctx.symbol_names,
                ctx.dialect,
            )
            .and_then(|name| name.primitive())
                != Some(Primitive::CreateRoot)
        {
            return false;
        }
        let Some(argument) = call.arguments.first() else {
            return false;
        };
        // The root's function: a literal written in the argument, or the
        // project function the argument exactly resolves to.
        let root = if argument.spread {
            None
        } else if let Some(literal) = callback_argument_literal(file, argument.span) {
            Some((file, literal))
        } else {
            ctx.semantic_lookup
                .function_called_at(
                    file.path.as_str(),
                    file.ast.peel_ts_sugar_span(argument.span),
                )
                .filter(|(helper_file, helper)| {
                    helper.method_name.is_none()
                        && ctx
                            .semantic_lookup
                            .function_value_is_current(helper_file, helper)
                })
        };
        let Some((root_file, root)) = root else {
            // Not resolved: the callback may be a function around the
            // computation, and then may be disposed. Only a call written in
            // the computation's own component can be one.
            return outermost.is_some_and(|outermost| outermost.span.contains(call.span))
                || call.span.contains(ast_function.span);
        };
        if root_file.path != file.path
            || !root.span.contains(ast_function.span)
            || root.span == ast_function.span
        {
            return false;
        }
        let Some(dispose) = root
            .parameters
            .first()
            .filter(|binding| binding.shape == solid_facts::ast::BindingShape::Identifier)
            .and_then(|binding| binding.names.first())
            .and_then(|name| root_file.source_text(name.span))
        else {
            return false;
        };
        root_file
            .source_text(root.body)
            .is_some_and(|body| mentions_word(body, dispose))
    })
}

/// A promise handler argument: absent (or `undefined`/`null`, which passes the
/// outcome through), a function literal proven to complete normally with a
/// value that is not a thenable, or anything else.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Handler {
    PassThrough,
    Fulfilling,
    Unproven,
}

/// Turns the reads reported since `mark` into proof obligations.
fn mark_uncertain(draft: &mut ProgramDraft, mark: usize) {
    for defect in &mut draft.static_defects[mark..] {
        if matches!(defect.kind, StaticDefectKind::ReactiveReadAfterAwait { .. }) {
            defect.uncertain = true;
        }
    }
}

/// How a call's promise reaches the computation's flight.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Carried {
    /// The operand of an `await`, or the promise a synchronous function
    /// returns: the computation's runtime holds a reaction on it from the
    /// moment it exists, so every handler below it runs on the flight.
    Held,
    /// Returned, not awaited, by an `async` function. The function adopts the
    /// promise in a later microtask (a resolve-thenable job): a handler
    /// queued before that job runs, on a receiver that was already settled,
    /// has no `async` frame of the computation below it, and rc.13's
    /// `attributedFlight` finds none. Only a receiver that stays pending past
    /// the job is on the flight.
    Adopted,
}

/// The calls whose promise a function hands to whoever awaits it: the
/// operands of its own `await`s (an `async` function only) and the calls it
/// returns. An `async` function that returns a promise adopts it: its own
/// promise settles only after the returned one does (measured on rc.13:
/// `fixtures/runtime-oracle/after-await-*`).
fn chain_calls<'f>(
    file: &'f FileFacts,
    function: &'f FunctionFact,
) -> Vec<(&'f CallFact, Carried)> {
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
                .map(|call| (call, Carried::Held))
        })
        .into_iter()
        .flatten();
    let carried = if function.r#async {
        Carried::Adopted
    } else {
        Carried::Held
    };
    let returned = file
        .ast
        .own_returns(function)
        .chain(function.expression_return.iter())
        .filter_map(move |returned| {
            let span = file.ast.peel_ts_sugar_span(returned.argument?);
            file.ast
                .call_at(span)
                .filter(|call| !call.construct)
                .map(|call| (call, carried))
        });
    awaited.chain(returned).collect()
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
        chain_calls(file, ast_function)
            .into_iter()
            .any(|(call, _)| {
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

    fn has_try_handler(&self, function: &AsyncFunctionFact) -> bool {
        ast_function(&self.files, function).is_some_and(|(file, function)| {
            file.ast
                .try_head_awaits
                .iter()
                .any(|head| written_directly_in(&file.ast, function, head.awaited))
        })
    }

    /// Whether the function itself suspends in a way the producer's facts do
    /// not model (`for await`, `await using`).
    fn has_implicit_suspension(&self, function: &'f AsyncFunctionFact) -> bool {
        ast_function(&self.files, function).is_some_and(|(file, ast_function)| {
            file.ast.implicit_suspensions.iter().any(|suspension| {
                ast_function.body.contains(*suspension)
                    && written_directly_in(&file.ast, ast_function, *suspension)
            })
        })
    }

    /// Whether the windows' tracking already made `symbol` a dependency
    /// (ADR 0195), may have (a read through a helper, a getter, an inline
    /// callback or a call the analysis cannot see into), or cannot have.
    fn tracking(&self, member: &Member, symbol: &SymbolId) -> Tracking {
        self.pre.accessor(&member.windows, symbol)
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
            carried: Carried::Held,
            untracked: false,
            unproven: false,
        };
        let mark = draft.static_defects.len();
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
                // ADR 0481: a member read that is exactly a `const` names it.
                if let Some(alias) = file
                    .ast
                    .local_alias_member(file.ast.peel_ts_sugar_span(candidate.callee))
                {
                    return ctx.entities.at(file.path.as_str(), alias.source);
                }
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
            let tracking =
                symbol.map_or(Tracking::Untracked, |symbol| self.tracking(member, symbol));
            if tracking == Tracking::Tracked {
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
                uncertain: tracking == Tracking::Possible,
            });
        }
        // The promise chain first: it records which function literals become
        // members, so the callback walk below leaves their obligations to them.
        if let Some((file, ast_function)) = ast_function(&self.files, function) {
            for (call, carried) in chain_calls(file, ast_function) {
                walk.carried = carried;
                self.expand(draft, &mut walk, queue, file, call, 0);
            }
        }
        if let Some(site) = member_site {
            member_reads_after_await(ctx, draft, site, &member.context, &|symbol, text| {
                self.pre.store_key(&member.windows, symbol, text)
            });
            let (file, ast_function, boundary) = site;
            for call in file.ast.calls_within(ast_function.body) {
                if call.span.start >= boundary
                    && crate::owners::owned_by_body(&file.ast, ast_function, call.span)
                {
                    after_await_calls.push((file, call));
                }
            }
        }
        for (file, call) in after_await_calls {
            self.visit_call(draft, &mut walk, file, call, 0);
        }
        if let Some((file, ast_function)) = ast_function(&self.files, function) {
            // A catch/finally may follow a rejected await even though the
            // producer has no normal-flow dominance fact for its reads.
            for head in &file.ast.try_head_awaits {
                if !written_directly_in(&file.ast, ast_function, head.awaited) {
                    continue;
                }
                // The catch can complete and continue after the try too.
                // This suffix remains an obligation: a synchronous operand
                // failure may have reached the same join without suspension.
                let suffix = head
                    .regions
                    .iter()
                    .map(|region| region.end)
                    .max()
                    .map(|end| Span::new(end, ast_function.body.end));
                for region in head.regions.iter().copied().chain(suffix) {
                    let before = draft.static_defects.len();
                    let mut handler = ast_function.clone();
                    handler.body = region;
                    self.report_region(draft, &mut walk, file, &handler, 0);
                    // Reuse the operand proof only while the resolved
                    // declaration is still the current function value.
                    let current = file
                        .ast
                        .call_at(file.ast.peel_ts_sugar_span(head.operand))
                        .and_then(|call| {
                            ctx.semantic_lookup.function_called_at(
                                file.path.as_str(),
                                file.ast.peel_ts_sugar_span(call.callee),
                            )
                        })
                        .is_some_and(|(f, fun)| {
                            ctx.semantic_lookup.function_value_is_current(f, fun)
                        });
                    if !current
                        || !crate::execution_role::follows_try_head_await(
                            file,
                            Span::new(region.start + 1, region.start + 1),
                            ctx.semantic_lookup,
                        )
                    {
                        mark_uncertain(draft, before);
                    }
                }
            }
            self.after_implicit_suspension(draft, &mut walk, mark, file, ast_function);
        }
        if member.conditional {
            mark_uncertain(draft, mark);
        }
    }

    /// The dialect primitive the call resolves to.
    fn primitive(&self, file: &FileFacts, call: &CallFact) -> Option<Primitive> {
        crate::call_primitive_name_cached(
            self.ctx.semantic_lookup,
            file,
            call,
            self.ctx.entities,
            self.ctx.symbol_names,
            self.ctx.dialect,
        )
        .and_then(|name| name.primitive())
    }

    /// A suspension the dominance facts do not model (`for await`, `await
    /// using`) always suspends the function at least once, but where it sits
    /// in the flow is not proven the way an `await`'s is: an accessor read
    /// written after it is a proof obligation, never a silent clean (`for await`
    /// over an empty iterable still awaits, measured on rc.13,
    /// `after-await-for-await`).
    fn after_implicit_suspension(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        base: usize,
        file: &'f FileFacts,
        function: &'f FunctionFact,
    ) {
        let Some(boundary) = file
            .ast
            .implicit_suspensions
            .iter()
            .filter(|suspension| {
                function.body.contains(**suspension)
                    && written_directly_in(&file.ast, function, **suspension)
            })
            .map(|suspension| suspension.end)
            .min()
        else {
            return;
        };
        let mark = draft.static_defects.len();
        // The suspension's span ends with the iterable, so the loop's body is
        // past it as well as what follows the loop. Whether the body runs, and
        // where an `await` the facts do model sits relative to it, is not
        // proven: every read reached is an obligation.
        if self.ctx.dialect.reports_member_reads_after_await() {
            let before = draft.static_defects.len();
            member_reads_after_await(
                self.ctx,
                draft,
                (file, function, boundary),
                &walk.member.context,
                &|symbol, text| self.pre.store_key(&walk.member.windows, symbol, text),
            );
            // A member read the `await` proof already reported stays as it
            // was reported there (a violation proven by the await wins).
            let mut index = before;
            while index < draft.static_defects.len() {
                let duplicate = draft.static_defects[..before]
                    .iter()
                    .skip(base)
                    .any(|earlier| earlier.location == draft.static_defects[index].location);
                if duplicate {
                    draft.static_defects.remove(index);
                } else {
                    index += 1;
                }
            }
        }
        for call in file.ast.calls_within(function.body) {
            if call.span.start >= boundary && written_directly_in(&file.ast, function, call.span) {
                self.report_call_read(draft, walk, file, call);
                self.visit_call(draft, walk, file, call, 0);
            }
        }
        mark_uncertain(draft, mark);
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
            && !function.generator
        {
            self.report_prefix(draft, walk, file, function, depth);
            return;
        }
        // `runWithOwner(owner, fn)` runs `fn` at once with `owner` as the
        // context and tracking off (`dev-shared.js` `runWithOwner`), and
        // `checkPostAwaitRead` returns only for a context that is not `null`:
        // a literal `null` owner (the detached-root spelling) keeps the read
        // a warning, `undefined` silences it, and an owner the analysis
        // cannot see through may be either.
        if self.primitive(file, call) == Some(Primitive::RunWithOwner)
            && let [owner, callback] = call.arguments.as_slice()
            && !owner.spread
            && !callback.spread
            && let Some((callback_file, callback)) = callback_argument_literal(file, callback.span)
                .map(|callback| (file, callback))
                .or_else(|| {
                    ctx.semantic_lookup
                        .function_called_at(
                            file.path.as_str(),
                            file.ast.peel_ts_sugar_span(callback.span),
                        )
                        .filter(|(f, fun)| ctx.semantic_lookup.function_value_is_current(f, fun))
                })
            && !callback.generator
        {
            let mark = draft.static_defects.len();
            if owner.value != ArgumentValueKind::Undefined {
                self.report_prefix(draft, walk, callback_file, callback, depth);
            }
            if !matches!(
                owner.value,
                ArgumentValueKind::Null | ArgumentValueKind::Undefined
            ) {
                mark_uncertain(draft, mark);
            }
            return;
        }
        // ADR 0485: a literal a project helper calls in its own synchronous
        // body on every completion (`layer_run`'s in-place answers at this
        // call) runs on this call's stack, on the continuation: its reads are
        // the continuation's. One such run makes the read; other runs the
        // helper may make do not unmake it.
        let mut in_place = false;
        for argument in &call.arguments {
            if argument.spread {
                continue;
            }
            let Some(callback) = callback_argument_literal(file, argument.span) else {
                continue;
            };
            if callback.generator {
                continue;
            }
            if let crate::execution_role::LayerRun::InPlace(at)
            | crate::execution_role::LayerRun::InPlaceAmongOthers(at) =
                crate::execution_role::layer_run(file, callback, ctx.semantic_lookup)
                && at == call.span
                && ctx
                    .semantic_lookup
                    .resolved_callee_call(file, call.callee)
                    .is_none_or(|resolved| {
                        !resolved
                            .declaration
                            .as_ref()
                            .is_some_and(|declaration| declaration.standard_library)
                    })
            {
                self.report_prefix(draft, walk, file, callback, depth);
                in_place = true;
            }
        }
        if in_place {
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
                // Whether these run depends on the contents: a comparator, and
                // a reducer without an initial value, are never called for
                // fewer than two elements, a replacer when nothing matches. A
                // read in one is a proof obligation.
                Some(callback)
                    if !callback.generator
                        && (matches!(
                            standard_call_name(resolved),
                            Some("Array.sort" | "String.replace" | "String.replaceAll")
                        ) || (call.arguments.len() < 2
                            && matches!(
                                standard_call_name(resolved),
                                Some(
                                    "Array.reduce"
                                        | "ReadonlyArray.reduce"
                                        | "Array.reduceRight"
                                        | "ReadonlyArray.reduceRight"
                                )
                            ))) =>
                {
                    self.report_conditional_region(draft, walk, file, callback, depth);
                }
                Some(callback) if !callback.generator => {
                    self.report_prefix(draft, walk, file, callback, depth);
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
        // A callee that is itself an accessor, or a typed candidate, is read
        // as the access it is, not as a helper.
        if call_accessor_symbol(ctx, file, call)
            .is_some_and(|symbol| ctx.accessors.contains_key(symbol) || typed_unknown(ctx, symbol))
        {
            return;
        }
        // ADR 0351: an exact function of a closed object literal is a helper
        // (its method name is no dispatch here); any other member is not.
        let (helper_file, helper, member) = if let Some(member) = file.ast.member_at(callee) {
            if file.ast.computed_members.contains(&member.span) {
                return;
            }
            let Some((helper_file, helper)) =
                ctx.semantic_lookup.closed_member_function(file, member)
            else {
                return;
            };
            (helper_file, helper, true)
        } else {
            let Some((helper_file, helper)) = ctx
                .semantic_lookup
                .function_called_at(file.path.as_str(), callee)
            else {
                return;
            };
            (helper_file, helper, false)
        };
        if helper.generator
            || (!member
                && (helper.method_name.is_some()
                    || !ctx
                        .semantic_lookup
                        .function_value_is_current(helper_file, helper)))
        {
            return;
        }
        for read in helper_file
            .ast
            .calls_within(synchronous_prefix(helper_file, helper))
        {
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

    /// Async functions execute their prefix immediately too. Their tail is
    /// handled by the promise-flight walk, never by this stack walk.
    fn report_prefix(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        function: &'f FunctionFact,
        depth: usize,
    ) {
        let mut prefix = function.clone();
        prefix.body = synchronous_prefix(file, function);
        self.report_region(draft, walk, file, &prefix, depth);
    }

    /// A function literal's body as part of the continuation: its member
    /// reads and accessor calls, and the callbacks its own calls run.
    fn report_region(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        callback: &FunctionFact,
        depth: usize,
    ) {
        let ctx = self.ctx;
        if ctx.dialect.reports_member_reads_after_await() {
            // The whole body is past the suspension: the boundary is its start.
            member_reads_after_await(
                ctx,
                draft,
                (file, callback, callback.body.start),
                &walk.member.context,
                &|symbol, text| self.pre.store_key(&walk.member.windows, symbol, text),
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

    /// A function literal's body that runs only on some executions or some
    /// receivers (a comparator, a replacer, a rejection handler, a handler
    /// whose receiver may already be settled): the reads in it are past the
    /// await when it runs, and whether it runs is not proven, so they are
    /// proof obligations and never violations.
    fn report_conditional_region(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        callback: &'f FunctionFact,
        depth: usize,
    ) {
        let mark = draft.static_defects.len();
        self.report_prefix(draft, walk, file, callback, depth);
        mark_uncertain(draft, mark);
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
        let tracking = self.tracking(walk.member, symbol);
        if tracking == Tracking::Tracked || !walk.reported.insert(key) {
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
            uncertain: tracking == Tracking::Possible,
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
        // `untrack(() => load())` returns what its callback returns: the
        // promise is the same one, but the helper's own reads before its first
        // await are not dependencies of the computation (tracking is off for
        // them) and the read after it warns all the same (measured on rc.13,
        // `after-await-untracked-helper`).
        // `untrack(fn)` and `runWithOwner(owner, fn)` return what `fn`
        // returns, and run it with tracking off. The function is a literal or
        // exactly the project function the argument names; anything else is
        // an obligation, never a silent clean.
        let primitive = self.primitive(file, call);
        let wrapped = match (primitive, call.arguments.as_slice()) {
            (Some(Primitive::Untrack), [argument]) => Some((argument, None)),
            (Some(Primitive::RunWithOwner), [owner, argument]) if !owner.spread => {
                Some((argument, Some(owner.value)))
            }
            _ => None,
        };
        if let Some((argument, owner)) = wrapped {
            let function = if argument.spread {
                None
            } else if let Some(callback) = callback_argument_literal(file, argument.span) {
                Some((file, callback))
            } else {
                ctx.semantic_lookup
                    .function_called_at(
                        file.path.as_str(),
                        file.ast.peel_ts_sugar_span(argument.span),
                    )
                    .filter(|(helper_file, helper)| {
                        helper.method_name.is_none()
                            && ctx
                                .semantic_lookup
                                .function_value_is_current(helper_file, helper)
                    })
            };
            // A detached owner (`null`) is not the computation's; the read
            // after the await then warns whatever the owner. Any other owner
            // may be one the runtime stays silent for.
            let uncertain = !matches!(owner, None | Some(ArgumentValueKind::Null));
            let previous = std::mem::replace(&mut walk.untracked, true);
            match function {
                Some((function_file, function)) if function.r#async && !function.generator => {
                    self.enqueue(walk, queue, function_file, function, uncertain);
                }
                Some((function_file, function)) if !function.generator => {
                    let mark = draft.static_defects.len();
                    for (inner, _) in chain_calls(function_file, function) {
                        self.expand(draft, walk, queue, function_file, inner, depth + 1);
                    }
                    if uncertain {
                        mark_uncertain(draft, mark);
                    }
                }
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
            walk.untracked = previous;
            return;
        }
        if let Some((helper_file, helper)) = async_helper(ctx, file, call) {
            self.enqueue(walk, queue, helper_file, helper, false);
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
                let handlers = if name == "Promise.then" { 2 } else { 1 };
                if !native_promise(self.ctx, file, receiver, 0) {
                    // ADR 0349: a receiver the premise would prove native, which
                    // a visible write withdraws, is unproven, not clean: its
                    // handlers are proof obligations.
                    if native_promise_claim(self.ctx, file, receiver, 0) {
                        for argument in call
                            .arguments
                            .iter()
                            .take(handlers)
                            .filter(|argument| !argument.spread)
                        {
                            if let Some(callback) = callback_argument_literal(file, argument.span)
                                && !callback.generator
                            {
                                self.report_conditional_region(draft, walk, file, callback, depth);
                            }
                        }
                    }
                    return;
                }
                // A handler on a receiver that may already be settled is
                // queued before an `async` function adopts the promise it
                // returns, and no frame of the computation is below it.
                let on_flight =
                    walk.carried == Carried::Held || self.pending_promise(file, receiver, 0);
                // A fulfillment handler runs only if the receiver fulfills and
                // `finally` only if it settles: proven for it, or the handler
                // is a proof obligation.
                let outcome = self.settlement(file, receiver, 0);
                let fulfills = match name {
                    "Promise.then" => outcome == Settlement::Fulfills,
                    "Promise.finally" => outcome != Settlement::Unknown,
                    _ => true,
                };
                for (index, argument) in call.arguments.iter().take(handlers).enumerate() {
                    if argument.spread {
                        continue;
                    }
                    let Some(callback) = callback_argument_literal(file, argument.span) else {
                        continue;
                    };
                    if callback.r#async || callback.generator {
                        continue;
                    }
                    // A rejection handler runs only if the receiver rejects;
                    // a fulfillment handler only if it fulfills. `finally`
                    // runs either way.
                    let rejection_only =
                        name == "Promise.catch" || (name == "Promise.then" && index == 1);
                    // Inside an aggregate whose premise is withdrawn (ADR 0349),
                    // the element's flight is unproven as well.
                    if rejection_only || !fulfills || !on_flight || walk.unproven {
                        self.report_conditional_region(draft, walk, file, callback, depth);
                    } else {
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
                self.expand_aggregate(draft, walk, queue, file, call, depth);
            }
            _ => {}
        }
    }

    /// `Promise.all([...])` and `allSettled`: each element's promise continues
    /// the flight. Driving the iterator and each element's `then` is intrinsic
    /// only under the Promise and array-iteration premises (ADR 0349); without
    /// them the elements' reads are proof obligations.
    fn expand_aggregate(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &'f FileFacts,
        call: &'f CallFact,
        depth: usize,
    ) {
        let semantic = self.ctx.semantic_lookup;
        let premise = semantic.promise_intrinsics_intact() && semantic.array_iteration_intact();
        let next = walk.unproven || !premise;
        let previous = std::mem::replace(&mut walk.unproven, next);
        if let Some(argument) = call.arguments.first().filter(|argument| !argument.spread) {
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
        walk.unproven = previous;
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
                self.enqueue(walk, queue, file, callback, false);
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
            self.enqueue(walk, queue, helper_file, helper, false);
        }
    }

    /// Adds an `async` function whose promise the computation awaits.
    fn enqueue(
        &self,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &FileFacts,
        function: &FunctionFact,
        uncertain: bool,
    ) {
        // A function with no `await` carries no producer fact: nothing is
        // dominated by a suspension in it, but its returned promises still
        // continue the chain.
        let fact = self.fact_for(file, function).map_or_else(
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
        if !walk.untracked {
            windows.push(Rc::clone(&fact));
        }
        // An `async` function returned without `await` is adopted by its
        // caller one job after the call returns. Its own continuation, queued
        // by an await on a settled value, runs before that: only a first
        // await on a promise proven still pending leaves a reaction in place.
        let conditional = walk.member.conditional
            || uncertain
            || walk.unproven
            || (walk.carried == Carried::Adopted && !self.awaits_pending(file, function));
        queue.push(Member {
            function: fact,
            context: walk.member.context.clone(),
            windows,
            conditional,
        });
    }

    /// The producer's fact of `function` (ADR 0351): the fact at its span, or an
    /// object method's, whose producer span starts at its key or `async`.
    fn fact_for(&self, file: &FileFacts, function: &FunctionFact) -> Option<&'f AsyncFunctionFact> {
        let file = *self.files.get(file.path.as_str())?;
        let path = file.path.as_str();
        if let Some(fact) = self.facts.get(&(
            path,
            u64::from(function.span.start),
            u64::from(function.span.end),
        )) {
            return Some(*fact);
        }
        self.facts_by_end
            .get(&(path, u64::from(function.span.end)))?
            .iter()
            .copied()
            .find(|fact| {
                u32::try_from(fact.expression.start_byte).is_ok_and(|start| {
                    start < function.span.start
                        && method_header(file.source_text(Span::new(start, function.span.start)))
                })
            })
    }

    /// How `span` settles, where the analysis can tell. A handler is assumed
    /// to return normally (the feasible-path premise): `then(h)` of a
    /// fulfilling receiver fulfills, a rejection handler on a rejecting one
    /// recovers it, and `finally` passes the receiver's outcome on.
    fn settlement(&self, file: &FileFacts, span: Span, depth: usize) -> Settlement {
        // ADR 0349: outcomes of the global Promise are unproven once a visible
        // write may have changed them.
        if depth > MAX_DEPTH || !self.ctx.semantic_lookup.promise_intrinsics_intact() {
            return Settlement::Unknown;
        }
        let ctx = self.ctx;
        let span = file.ast.peel_ts_sugar_span(span);
        let Some(call) = file.ast.call_at(span) else {
            return Settlement::Unknown;
        };
        let standard = ctx
            .semantic_lookup
            .resolved_callee_call(file, call.callee)
            .and_then(standard_call_name);
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let element = |element: Span| -> Settlement {
            if plain_value(file, element) {
                Settlement::Fulfills
            } else {
                self.settlement(file, element, depth + 1)
            }
        };
        let elements = |call: &CallFact| -> Option<Vec<Settlement>> {
            let argument = call.arguments.first().filter(|argument| !argument.spread)?;
            let array = file
                .ast
                .array_literal_at(file.ast.peel_ts_sugar_span(argument.span))?;
            if array.elements.is_empty()
                || array
                    .elements
                    .iter()
                    .any(|item| item.kind != solid_facts::ast::ArrayElementKind::Value)
            {
                return None;
            }
            Some(
                array
                    .elements
                    .iter()
                    .map(|item| element(item.span))
                    .collect(),
            )
        };
        // ADR 0349: an aggregate's outcome is driven by the array iterator, which
        // a visible write may have replaced.
        if matches!(
            standard,
            Some(
                "PromiseConstructor.all"
                    | "PromiseConstructor.allSettled"
                    | "PromiseConstructor.race"
                    | "PromiseConstructor.any"
            )
        ) && !ctx.semantic_lookup.array_iteration_intact()
        {
            return Settlement::Unknown;
        }
        match (call.construct, standard) {
            (true, Some("PromiseConstructor.construct")) => {
                if !is_global_promise(file, call.callee) {
                    return Settlement::Unknown;
                }
                self.timer_executor_settlement(file, call)
            }
            (false, Some("PromiseConstructor.resolve"))
                if is_global_promise_static(file, callee) =>
            {
                match call.arguments.as_slice() {
                    [] => Settlement::Fulfills,
                    [argument] if !argument.spread && plain_value(file, argument.span) => {
                        Settlement::Fulfills
                    }
                    _ => Settlement::Unknown,
                }
            }
            (false, Some("PromiseConstructor.reject"))
                if is_global_promise_static(file, callee) =>
            {
                Settlement::Rejects
            }
            (false, Some("PromiseConstructor.allSettled"))
                if is_global_promise_static(file, callee) =>
            {
                // It fulfills once every element settles, either way: an
                // element that may never settle holds it open.
                match elements(call) {
                    Some(all) if all.iter().all(|item| *item != Settlement::Unknown) => {
                        Settlement::Fulfills
                    }
                    _ => Settlement::Unknown,
                }
            }
            (false, Some("PromiseConstructor.all" | "PromiseConstructor.race"))
                if is_global_promise_static(file, callee) =>
            {
                // Every element fulfills, so the combinator does; one proven
                // rejection with the rest unknown settles nothing.
                match elements(call) {
                    Some(all) if all.iter().all(|item| *item == Settlement::Fulfills) => {
                        Settlement::Fulfills
                    }
                    _ => Settlement::Unknown,
                }
            }
            (false, Some("PromiseConstructor.any")) if is_global_promise_static(file, callee) => {
                match elements(call) {
                    Some(all) if all.contains(&Settlement::Fulfills) => Settlement::Fulfills,
                    _ => Settlement::Unknown,
                }
            }
            (
                false,
                Some(handler_name @ ("Promise.then" | "Promise.catch" | "Promise.finally")),
            ) => {
                let Some(receiver) = file
                    .ast
                    .member_at(callee)
                    .filter(|_| !file.ast.computed_members.contains(&callee))
                    .map(|member| self.settlement(file, member.object, depth + 1))
                else {
                    return Settlement::Unknown;
                };
                let handler = |index: usize| match call.arguments.get(index) {
                    None => Handler::PassThrough,
                    Some(argument) if argument.spread => Handler::Unproven,
                    Some(argument)
                        if matches!(
                            argument.value,
                            ArgumentValueKind::Undefined | ArgumentValueKind::Null
                        ) =>
                    {
                        Handler::PassThrough
                    }
                    Some(argument) => callback_argument_literal(file, argument.span)
                        .filter(|callback| self.handler_fulfills(file, callback, depth))
                        .map_or(Handler::Unproven, |_| Handler::Fulfilling),
                };
                // What a handler turns an outcome into: the result of its
                // call decides, so a handler that returns a rejected or
                // never-settling promise, or throws, leaves the downstream
                // outcome unproven.
                let recovered = |index: usize, otherwise: Settlement| match handler(index) {
                    Handler::PassThrough => otherwise,
                    Handler::Fulfilling => Settlement::Fulfills,
                    Handler::Unproven => Settlement::Unknown,
                };
                match (handler_name, receiver) {
                    (_, Settlement::Unknown) => Settlement::Unknown,
                    ("Promise.finally", outcome) => match handler(0) {
                        Handler::Unproven => Settlement::Unknown,
                        _ => outcome,
                    },
                    ("Promise.catch", Settlement::Fulfills) => Settlement::Fulfills,
                    ("Promise.catch", outcome) => recovered(0, outcome),
                    ("Promise.then", Settlement::Fulfills) => recovered(0, Settlement::Fulfills),
                    ("Promise.then", outcome) => recovered(1, outcome),
                    _ => Settlement::Unknown,
                }
            }
            (false, None) => self.returns_settlement(file, call, depth),
            _ => Settlement::Unknown,
        }
    }

    /// Whether calling the handler is proven to complete with a value that is
    /// not a thenable: no `throw` in it, no `async` or generator, and every
    /// returned value a plain literal or a promise proven to fulfill. Calls the
    /// handler makes are assumed to return (the feasible-path premise) unless
    /// they are proven never to; what it returns is not.
    fn handler_fulfills(&self, file: &FileFacts, callback: &FunctionFact, depth: usize) -> bool {
        if callback.r#async || callback.generator || depth > MAX_DEPTH {
            return false;
        }
        if file
            .source_text(callback.body)
            .is_none_or(|body| mentions_word(body, "throw"))
        {
            return false;
        }
        // A call that never returns (a `never` signature, an assertion
        // function) throws as a `throw` does. Any other call is assumed to
        // return, the feasible-path premise.
        if !self
            .ctx
            .semantic_lookup
            .never_returning_calls_within(file, callback)
            .is_empty()
        {
            return false;
        }
        file.ast
            .own_returns(callback)
            .chain(callback.expression_return.iter())
            .all(|returned| {
                returned.argument.is_none_or(|argument| {
                    plain_value(file, argument)
                        || self.settlement(file, argument, depth + 1) == Settlement::Fulfills
                })
            })
    }

    /// A call to a synchronous project function whose every return settles
    /// the same way (`const pause = () => new Promise(...)`).
    fn returns_settlement(&self, file: &FileFacts, call: &CallFact, depth: usize) -> Settlement {
        let ctx = self.ctx;
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if call.construct || file.ast.member_at(callee).is_some() {
            return Settlement::Unknown;
        }
        let Some((helper_file, helper)) = ctx
            .semantic_lookup
            .function_called_at(file.path.as_str(), callee)
        else {
            return Settlement::Unknown;
        };
        if helper.r#async
            || helper.generator
            || helper.method_name.is_some()
            || !ctx
                .semantic_lookup
                .function_value_is_current(helper_file, helper)
        {
            return Settlement::Unknown;
        }
        let mut outcomes = helper_file
            .ast
            .own_returns(helper)
            .chain(helper.expression_return.iter())
            .map(|returned| {
                returned.argument.map_or(Settlement::Unknown, |argument| {
                    self.settlement(helper_file, argument, depth + 1)
                })
            });
        let Some(first) = outcomes.next() else {
            return Settlement::Unknown;
        };
        if outcomes.all(|outcome| outcome == first) {
            first
        } else {
            Settlement::Unknown
        }
    }

    /// `new Promise((done, fail) => setTimeout(done, 20))`: the outcome is the
    /// parameter the timer passes as its callback. Any other executor is
    /// not proven to settle either way.
    fn timer_executor_settlement(&self, file: &FileFacts, construct: &CallFact) -> Settlement {
        if !self.timer_executor(file, construct) {
            return Settlement::Unknown;
        }
        let Some(function) = construct
            .arguments
            .first()
            .and_then(|argument| callback_argument_literal(file, argument.span))
        else {
            return Settlement::Unknown;
        };
        let Some(timer) = file
            .ast
            .calls_within(function.body)
            .find(|call| written_directly_in(&file.ast, function, call.span))
        else {
            return Settlement::Unknown;
        };
        let Some(callback) = timer
            .arguments
            .first()
            .and_then(|argument| file.source_text(argument.span))
            .map(str::trim)
        else {
            return Settlement::Unknown;
        };
        let parameter = |index: usize| {
            function
                .parameters
                .get(index)
                .filter(|binding| binding.shape == solid_facts::ast::BindingShape::Identifier)
                .and_then(|binding| binding.names.first())
                .and_then(|name| file.source_text(name.span))
        };
        match (parameter(0), parameter(1)) {
            // `setTimeout(done, ms, value)` resolves with `value`: only a
            // value that is not a thenable fulfills (a promise there is
            // adopted, with its outcome).
            (Some(resolve), _) if resolve == callback => {
                if timer
                    .arguments
                    .get(2)
                    .is_none_or(|value| !value.spread && plain_value(file, value.span))
                {
                    Settlement::Fulfills
                } else {
                    Settlement::Unknown
                }
            }
            (_, Some(reject)) if reject == callback => Settlement::Rejects,
            _ => Settlement::Unknown,
        }
    }

    /// Whether every suspension the function makes directly is on a promise
    /// proven still pending, so its first one cannot resume before whoever
    /// adopted its promise has attached a reaction. A function that never
    /// suspends has no continuation to lose.
    fn awaits_pending(&self, file: &FileFacts, function: &FunctionFact) -> bool {
        if file
            .ast
            .implicit_suspensions
            .iter()
            .any(|suspension| function.body.contains(*suspension))
        {
            return false;
        }
        file.ast
            .awaits
            .iter()
            .copied()
            .filter(|awaited| {
                function.body.contains(*awaited)
                    && written_directly_in(&file.ast, function, *awaited)
            })
            .all(|awaited| {
                await_operand(file, awaited)
                    .is_some_and(|operand| self.pending_promise(file, operand, 0))
            })
    }

    /// Whether `span` is a promise proven still pending after the microtasks
    /// queued by its creation and by the `return` of an `async` function have
    /// run: a `new Promise` whose executor only starts a timer, a combinator
    /// over at least one element (each element is settled through a
    /// reaction job of its own), a project function that returns one of
    /// these on every path, and a handler call on one. An `async` function's
    /// promise, a `Promise.resolve`, a `Promise.reject` and a typed value are
    /// not proof: each may be settled already.
    fn pending_promise(&self, file: &FileFacts, span: Span, depth: usize) -> bool {
        if depth > MAX_DEPTH || !self.ctx.semantic_lookup.promise_intrinsics_intact() {
            return false;
        }
        let ctx = self.ctx;
        let span = file.ast.peel_ts_sugar_span(span);
        let Some(call) = file.ast.call_at(span) else {
            return false;
        };
        let standard = ctx
            .semantic_lookup
            .resolved_callee_call(file, call.callee)
            .and_then(standard_call_name);
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        match (call.construct, standard) {
            (true, Some("PromiseConstructor.construct")) => {
                is_global_promise(file, call.callee) && self.timer_executor(file, call)
            }
            (
                false,
                Some(
                    "PromiseConstructor.all"
                    | "PromiseConstructor.allSettled"
                    | "PromiseConstructor.race"
                    | "PromiseConstructor.any",
                ),
            ) => {
                is_global_promise_static(file, callee)
                    && self.ctx.semantic_lookup.array_iteration_intact()
                    && call
                        .arguments
                        .first()
                        .filter(|argument| !argument.spread)
                        .and_then(|argument| {
                            file.ast
                                .array_literal_at(file.ast.peel_ts_sugar_span(argument.span))
                        })
                        .is_some_and(|array| {
                            !array.elements.is_empty()
                                && array.elements.iter().all(|element| {
                                    element.kind == solid_facts::ast::ArrayElementKind::Value
                                })
                        })
            }
            (false, Some("Promise.then" | "Promise.catch" | "Promise.finally")) => file
                .ast
                .member_at(callee)
                .filter(|_| !file.ast.computed_members.contains(&callee))
                .is_some_and(|member| self.pending_promise(file, member.object, depth + 1)),
            (false, None) => self.returns_pending_promise(file, call, depth),
            _ => false,
        }
    }

    /// `new Promise((done) => setTimeout(done, 20))`: the executor is one
    /// call of a standard timer and nothing else, so it cannot settle the
    /// promise or throw before the timer's task runs. The other arguments
    /// are plain identifiers or numbers, whose evaluation cannot throw.
    fn timer_executor(&self, file: &FileFacts, construct: &CallFact) -> bool {
        let [executor] = construct.arguments.as_slice() else {
            return false;
        };
        if executor.spread {
            return false;
        }
        let Some(function) = callback_argument_literal(file, executor.span) else {
            return false;
        };
        if function.r#async || function.generator {
            return false;
        }
        let mut own = file
            .ast
            .calls_within(function.body)
            .filter(|call| written_directly_in(&file.ast, function, call.span));
        let (Some(timer), None) = (own.next(), own.next()) else {
            return false;
        };
        let Some(body) = file.source_text(function.body) else {
            return false;
        };
        let body = body.trim();
        let body = body
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
            .unwrap_or(body);
        let body = body.trim().trim_end_matches(';').trim();
        if file.source_text(timer.span).map(str::trim) != Some(body) || timer.construct {
            return false;
        }
        if !self
            .ctx
            .semantic_lookup
            .resolved_callee_call(file, timer.callee)
            .is_some_and(is_timer_call)
        {
            return false;
        }
        timer.arguments.iter().enumerate().all(|(index, argument)| {
            if argument.spread {
                return false;
            }
            if index == 0 && callback_argument_literal(file, argument.span).is_some() {
                return true;
            }
            let Some(text) = file.source_text(argument.span).map(str::trim) else {
                return false;
            };
            let numeric = text.starts_with(|c: char| c.is_ascii_digit());
            !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '$' || (numeric && c == '.'))
        })
    }

    /// A call to a synchronous project function that returns a pending
    /// promise on every path (`const pause = () => new Promise(...)`).
    fn returns_pending_promise(&self, file: &FileFacts, call: &CallFact, depth: usize) -> bool {
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
        if helper.r#async
            || helper.generator
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
                .is_some_and(|argument| self.pending_promise(helper_file, argument, depth + 1))
        })
    }
}
