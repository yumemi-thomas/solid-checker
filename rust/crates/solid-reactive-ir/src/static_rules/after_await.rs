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
    async_helper, is_global_promise, is_global_promise_static, native_promise,
    native_promise_claim, native_promise_undecided,
};
use super::pre_await::{PreAwait, Tracking};
use super::{
    SharedAsyncComputations, call_accessor_symbol, member_read_site, member_reads_after_await,
    tracked_async_computation_context, typed_after_await_obligation, typed_unknown,
};
use crate::HashMap;
use crate::HashSet;
use crate::cleanup::callback_argument_literal;
use crate::identity::SymbolId;
use crate::indexes::mentions_word;
use crate::owners::written_directly_in;
use crate::pipeline::{AnalysisContext, ProgramDraft};
use crate::runtime_semantics::{is_inline_standard_callback, is_timer_call, standard_call_name};
use crate::{AfterAwaitObligation, StaticDefect, StaticDefectKind, location};
use solid_dialect::{Primitive, SlotObserver, SlotOwner, SlotRun};
use solid_facts::FileFacts;
use solid_facts::ast::{
    ArgumentValueKind, CallFact, FunctionFact, FunctionKind, TaggedTemplateFact,
};
use solid_facts::core::Span;
use std::sync::Arc;
use typefacts::AsyncFunctionFact;

/// Nesting limit for callbacks inside callbacks and for chained promises.
const MAX_DEPTH: usize = 6;

/// ADR 0537: the in-place regions (an IIFE, an inline callback, a function
/// handed by name to a carrier) the stack walk follows below a chain: ADR
/// 0532's climb bound of 16 layers. Past it a call is an obligation, never a
/// silent clean.
const MAX_REGION_DEPTH: usize = MAX_DEPTH + 16;

/// ADR 0871: how many distinct window sets one function is walked under. Each
/// path to a helper may hold another set; past the bound a last walk with no
/// window reports every read as an obligation, so nothing is lost.
const MAX_WALKS_PER_FUNCTION: usize = 16;

/// One function whose continuation is part of a tracked async computation:
/// the computation itself, or a helper its returned promise awaits.
#[derive(Clone, PartialEq)]
struct Member {
    function: Arc<AsyncFunctionFact>,
    context: String,
    /// The functions whose own tracking window a source read may already
    /// have entered: the computation, every helper down to this one.
    windows: Vec<Arc<AsyncFunctionFact>>,
    /// The member's continuation may run before anything holds a reaction on
    /// its promise: an `async` function returned, not awaited, whose first
    /// await is on a promise not proven still pending. Its reads, and the
    /// reads of everything it continues into, are proof obligations, for
    /// the reason held here (ADR 0595).
    conditional: Option<AfterAwaitObligation>,
    /// The blocker of this member's obligations: `search-depth-limit` past
    /// [`MAX_WALKS_PER_FUNCTION`] (ADR 0871), `function-run-unplaced` for a
    /// compute no call proves (ADR 0873).
    blocker: Option<&'static str>,
}

/// ADR 0871: what decides a member's rows: the function, the windows its
/// reads may be dependencies of, and how it is conditional.
type WalkKey = (
    (String, u64, u64),
    Vec<(String, u64, u64)>,
    Option<std::mem::Discriminant<AfterAwaitObligation>>,
);

/// A generator's call: the file and function it is written in, the call, and
/// the generator's file and function.
type GeneratorCall<'f> = (
    &'f FileFacts,
    &'f FunctionFact,
    &'f CallFact,
    &'f FileFacts,
    &'f FunctionFact,
);

/// What one member's walk has recorded so far.
struct Walk<'m> {
    member: &'m Member,
    reported: HashSet<(String, u64, u64)>,
    /// The calls this walk has filed an unresolved invocation for (ADR 0964): a set of its own, so that a row the hand-off (ADR 0669)
    /// or the inexact callee (ADR 0879) files at the same callee is neither pre-empted nor pre-empts it.
    invoked: HashSet<(String, u64, u64)>,
    /// The calls and tagged templates this walk has entered, by file and span (slice 9): a span is an offset within one file, so two
    /// files' calls at the same offsets are two calls, and the first must not hide the second.
    visited: HashSet<(Arc<str>, Span)>,
    /// Function literals queued as members, which own their obligations.
    queued: HashSet<(String, Span)>,
    /// Functions whose prefix the walk has reported as run on the
    /// continuation (ADR 0565): the unplaced-function pass skips them.
    placed: HashSet<(String, Span)>,
    /// Helpers whose member reads this walk has reported (ADR 0614).
    member_helpers: HashSet<(String, u64, u64)>,
    /// ADR 0695: helpers whose straight-line reads this walk reached: a chain
    /// is followed to its end, each helper once.
    stack_helpers: HashSet<(String, u64, u64)>,
    /// How the promise chain being followed reaches the flight.
    carried: Carried,
    /// The chain being followed runs inside `untrack`: a helper it reaches
    /// is not inside the computation's tracking window.
    untracked: bool,
    /// The chain being followed runs on an intrinsic a visible write may have
    /// changed (ADR 0349): every member it queues is conditional.
    unproven: bool,
    /// ADR 0872: the chain being followed runs past the member's first
    /// suspension, so a helper it reaches adds no tracking window.
    past_suspension: bool,
    /// ADR 0870: past one only on some path (a conditional await): every
    /// member it queues is an obligation.
    past_on_some_path: bool,
}

/// What every [`Reach`] of one build reads and none writes: built once per
/// build and shared by the driver's and the workers' walks.
struct Shared<'f, 'a> {
    files: HashMap<&'f str, &'f FileFacts>,
    facts: HashMap<(&'f str, u64, u64), &'f AsyncFunctionFact>,
    /// The producer's facts by end offset: an object method's span ends where its function does (ADR 0351).
    facts_by_end: HashMap<(&'f str, u64), Vec<&'f AsyncFunctionFact>>,
    pre: PreAwait<'f, 'a>,
    /// Whether any function of the project is a generator: only then may a
    /// computation step one (ADR 0820).
    has_generators: bool,
    /// Each file's objects of longer member paths (`member_reads_after_await`).
    longer_path_objects: super::FileOnce<'f, HashSet<Span>>,
}

struct Reach<'s, 'f, 'a> {
    ctx: &'f AnalysisContext<'a>,
    files: &'s HashMap<&'f str, &'f FileFacts>,
    facts: &'s HashMap<(&'f str, u64, u64), &'f AsyncFunctionFact>,
    facts_by_end: &'s HashMap<(&'f str, u64), Vec<&'f AsyncFunctionFact>>,
    done: HashSet<WalkKey>,
    /// ADR 0871: how many walks each function has had.
    walks: HashMap<(String, u64, u64), usize>,
    pre: &'s PreAwait<'f, 'a>,
    has_generators: bool,
    longer_path_objects: &'s super::FileOnce<'f, HashSet<Span>>,
    /// Each function's [`Self::generator_calls`], once per build: the walk
    /// over the functions it calls is a function of the build alone, and a
    /// function is asked once per member walk and once by the run's filter.
    generator_calls: std::cell::RefCell<crate::HashMap<(&'f str, Span), Vec<GeneratorCall<'f>>>>,
}

impl<'f, 'a> Shared<'f, 'a> {
    fn new(ctx: &'f AnalysisContext<'a>) -> Self {
        Shared {
            files: crate::base::reads::all_files(ctx.facts)
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
                let mut by_end: HashMap<(&str, u64), Vec<&AsyncFunctionFact>> = HashMap::default();
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
            pre: PreAwait::new(ctx),
            has_generators: crate::base::reads::all_files(ctx.facts)
                .iter()
                .any(|file| file.ast.functions.iter().any(|function| function.generator)),
            longer_path_objects: super::FileOnce::new(crate::base::reads::all_files(ctx.facts)),
        }
    }
}

impl<'s, 'f, 'a> Reach<'s, 'f, 'a> {
    fn new(ctx: &'f AnalysisContext<'a>, shared: &'s Shared<'f, 'a>) -> Self {
        Reach {
            ctx,
            files: &shared.files,
            facts: &shared.facts,
            facts_by_end: &shared.facts_by_end,
            done: HashSet::default(),
            walks: HashMap::default(),
            pre: &shared.pre,
            has_generators: shared.has_generators,
            longer_path_objects: &shared.longer_path_objects,
            generator_calls: std::cell::RefCell::default(),
        }
    }
}

pub(super) fn run<'a>(
    ctx: &AnalysisContext<'a>,
    computations: &SharedAsyncComputations<'a>,
    draft: &mut ProgramDraft,
) {
    let shared = Shared::new(ctx);
    let mut reach = Reach::new(ctx, &shared);
    // The members each function starts with, in order. Building one reads
    // the build alone, never the walks' state.
    // The members each function starts with, in order. Deciding and
    // building one reads the build alone, never the walks' state, so both run
    // on the workers (ADR 0909).
    let functions = ctx
        .facts
        .typescript
        .files()
        .flat_map(|file| file.async_functions.iter())
        .collect::<Vec<_>>();
    let candidates = crate::pipeline::parallel_balanced_with_state(
        &functions,
        || Reach::new(ctx, &shared),
        |reach, function| reach.may_report(function),
    );
    let candidates = functions
        .into_iter()
        .zip(candidates)
        .filter_map(|(function, candidate)| candidate.then_some(function))
        .collect::<Vec<_>>();
    // Built on first use, once per build: only a function with something to
    // report needs it.
    let starts = if candidates.is_empty() {
        Vec::new()
    } else {
        let index = computations.get(ctx);
        crate::pipeline::parallel_balanced_with_state(
            &candidates,
            || Reach::new(ctx, &shared),
            |reach, function| reach.start_member(index, function),
        )
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
    };
    // ADR 0963: the region of the net for reads no call edge places: a tracked async computation, and a suspension in the program
    // for a continuation to follow (an `await` or an implicit suspension, or a promise handler: `then`, `catch`, `finally`).
    computations
        .holds_suspending
        .set(!starts.is_empty() && program_suspends(ctx));
    // ADR 0909: a member's walk is a function of the member and the build;
    // only the rows' span dedup reads the draft (`ProgramDraft::spans_agree`)
    // and only the queue's bound reads the walks so far
    // (`Reach::bounded`). So each starting member is walked speculatively on
    // the workers, and the driver below, unchanged in order, files a
    // speculative walk where its span answers agree and walks it again in
    // place otherwise.
    let mut speculated = speculate_waves(ctx, &shared, draft, &starts);
    for member in starts {
        let mut queue = vec![member];
        while let Some(member) = queue.pop() {
            let ahead = speculated.take(&member);
            reach.report_member(draft, &member, &mut queue, ahead);
        }
    }
    unlisted_async_computations(ctx, &reach, draft);
}

/// ADR 0963: whether the program holds a suspension a continuation follows: an `await`, an implicit suspension (`for await`,
/// `await using`), or a call of a promise handler (`then`, `catch`, `finally`: named, as a premise only, never as proof). An async
/// function with none (`async () => 1`) has no code after a suspension.
fn program_suspends(ctx: &AnalysisContext<'_>) -> bool {
    crate::base::reads::all_files(ctx.facts).iter().any(|file| {
        let ast = &file.ast;
        !ast.awaits.is_empty()
            || !ast.implicit_suspensions.is_empty()
            || ast.calls.iter().any(|call| {
                ast.member_at(ast.peel_ts_sugar_span(call.callee))
                    .and_then(|member| file.source_text(member.property))
                    .is_some_and(|name| matches!(name, "then" | "catch" | "finally"))
            })
    })
}

/// ADR 0963: a reactive read the continuation of an async computation may run by a route no call shows is an obligation of
/// SC1002 when the program holds a tracked async computation with something after a suspension: the read is written in a class
/// element (a constructor, a method, an accessor, a field's initializer), in an object literal's accessor or method (a getter
/// or setter, a coercion or iterator-protocol method, a Proxy trap), in a tag function, in a function a `new` runs, or in a
/// helper the exact calls of which end in one of those ([`crate::static_api::unplaced_member_blocker`]); the continuation runs
/// them without a call (a property access, a `new`, a coercion, a spread or a destructuring, a tag, a trap), the walk of the
/// continuation follows calls, and the read is untracked there. Only reads the role engine could not place are asked (a read
/// in code a call edge places has its role). The same reads stand in SC1001's uncertifiable rows; the claiming rule of a read
/// after an await is SC1002, so the row is its own (a row of another rule at the site covers nothing of it). Never a
/// violation: nothing here proves a continuation runs the member. A read directly in a class field's initializer says so
/// ([`crate::blockers::FIELD_INITIALIZER_UNPLACED`]): the construction that runs it is the missing witness. A function handed
/// where nothing follows it is run by a call; its reads are not this net's.
pub(super) fn reads_run_unplaced(
    ctx: &AnalysisContext<'_>,
    computations: &SharedAsyncComputations<'_>,
    files_by_path: &std::collections::HashMap<&str, &FileFacts>,
    lookup: &crate::indexes::SemanticLookup<'_>,
    draft: &mut ProgramDraft,
) {
    if !computations.holds_suspending.get() {
        return;
    }
    // ADR 0964: a call that invokes a function value the walk cannot resolve ([`unresolved_invocation`]) written in code a member
    // route runs (a class method that iterates stored listeners, an object method that calls a stored callback) is an obligation
    // too: the continuation reaches it through the member's access, not through a call the walk follows.
    let mut invocations = Vec::new();
    let mut widened = Vec::new();
    for file in crate::base::reads::all_files(ctx.facts) {
        for call in &file.ast.calls {
            let known = unresolved_invocation_known(ctx, file, call);
            if (known || unresolved_invocation_widened(ctx, file, call))
                && crate::static_api::unplaced_member_blocker(file, call.span, lookup).is_some()
            {
                let row = unresolved_invocation_obligation(
                    file,
                    call,
                    crate::blockers::UNRESOLVED_INVOCATION,
                );
                if known {
                    invocations.push(row);
                } else {
                    widened.push(row);
                }
            }
        }
        // Batch 10c (B1): a tag the program stored, written in member-route code, is called there as a call is.
        for template in &file.ast.tagged_templates {
            if unresolved_tag(ctx, file, template)
                && crate::static_api::unplaced_member_blocker(file, template.span, lookup).is_some()
            {
                widened.push(unresolved_obligation_at(
                    file,
                    file.ast.peel_ts_sugar_span(template.tag),
                    template.tag,
                ));
            }
        }
    }
    for defect in invocations {
        draft.push_defect(defect);
    }
    let defects = draft
        .reads
        .iter()
        .filter(|read| {
            matches!(
                read.execution,
                crate::ExecutionRole::Unknown | crate::ExecutionRole::Unreported
            )
        })
        .filter_map(|read| {
            let file = files_by_path.get(read.location.path.as_ref())?;
            let span = Span::new(
                u32::try_from(read.location.start_byte).ok()?,
                u32::try_from(read.location.end_byte).ok()?,
            );
            let blocker = crate::static_api::unplaced_member_blocker(file, span, lookup)?;
            Some(StaticDefect {
                kind: StaticDefectKind::ReactiveReadAfterAwait {
                    accessor: read.accessor.to_string(),
                    obligation: Some(AfterAwaitObligation::RunUnplaced),
                },
                location: read.location.clone(),
                analysis_context: if blocker == crate::blockers::FUNCTION_RUN_UNPLACED
                    && crate::execution_role::in_field_initializer(file, span)
                {
                    crate::blockers::FIELD_INITIALIZER_UNPLACED
                } else {
                    blocker
                }
                .into(),
                fixes: vec![],
                uncertain: true,
            })
        })
        .collect::<Vec<_>>();
    for defect in defects {
        draft.push_defect(defect);
    }
    // The rows of the widened invocations file last: a row of the family already at the same start (a read's, the net's earlier
    // rows) keeps its place, and this one stands only where none does.
    for defect in widened {
        draft.push_defect(defect);
    }
}

/// The walks done ahead of the driver, by member ([`speculate_waves`]).
#[derive(Default)]
struct Ahead(crate::HashMap<(String, u64, u64), Vec<(Member, Speculated)>>);

impl Ahead {
    /// The walk of exactly `member`, if one was done ahead.
    fn take(&mut self, member: &Member) -> Option<Speculated> {
        let kept = self.0.get_mut(&Reach::key(&member.function))?;
        let at = kept.iter().position(|(ahead, _)| ahead == member)?;
        Some(kept.swap_remove(at).1)
    }

    fn holds(&self, member: &Member) -> bool {
        self.0
            .get(&Reach::key(&member.function))
            .is_some_and(|kept| kept.iter().any(|(ahead, _)| ahead == member))
    }

    fn of_function(&self, member: &Member) -> usize {
        self.0
            .get(&Reach::key(&member.function))
            .map_or(0, Vec::len)
    }
}

/// How many waves of queued members are walked ahead of the driver.
const AHEAD_WAVES: usize = 8;

/// ADR 0909: the starting members' walks, then the walks of the members
/// those queue, wave by wave, each on the workers. A member is walked ahead
/// once, and at most [`MAX_WALKS_PER_FUNCTION`] per function (the driver
/// walks no more before ADR 0871's bound rewrites a member, which then is
/// another member, walked in place). The driver uses a walk only for
/// exactly its member.
fn speculate_waves<'f, 'a>(
    ctx: &'f AnalysisContext<'a>,
    shared: &Shared<'f, 'a>,
    draft: &ProgramDraft,
    starts: &[Member],
) -> Ahead {
    let mut ahead = Ahead::default();
    let mut wave = starts.to_vec();
    for _ in 0..AHEAD_WAVES {
        if wave.is_empty() {
            break;
        }
        // A member's walk is costly (a tenth to half a millisecond): a few
        // already pay for the threads.
        let walks = crate::pipeline::parallel_balanced_with_state_from(
            4,
            &wave,
            || (Reach::new(ctx, shared), draft.speculative()),
            |(reach, template), member| reach.speculate(template, member),
        );
        let mut next = Vec::new();
        for (member, walk) in wave.into_iter().zip(walks) {
            for queued in &walk.queued {
                if !ahead.holds(queued)
                    && !next.contains(queued)
                    && ahead.of_function(queued)
                        + next
                            .iter()
                            .filter(|other: &&Member| {
                                Reach::key(&other.function) == Reach::key(&queued.function)
                            })
                            .count()
                        < MAX_WALKS_PER_FUNCTION
                {
                    next.push(queued.clone());
                }
            }
            if !ahead.holds(&member) {
                ahead
                    .0
                    .entry(Reach::key(&member.function))
                    .or_default()
                    .push((member, walk));
            }
        }
        next.retain(|member| !ahead.holds(member));
        wave = next;
    }
    ahead
}

/// A member's walk done ahead of the driver ([`Reach::speculate`]).
struct Speculated {
    rows: Vec<StaticDefect>,
    /// The members it queued, before [`Reach::bounded`].
    queued: Vec<Member>,
    spans: crate::HashSet<crate::pipeline::SpanKey>,
    span_log: Vec<(crate::pipeline::SpanKey, bool)>,
}

/// ADR 0964 as filed in batch 10b: whether `call` invokes a function value the walk cannot resolve through the name of a parameter or a
/// variable, or through the result of a call. Its rows are the ones the net has always filed, in the order it files them
/// ([`unresolved_invocation_widened`] files the rest after them, and never in their place).
fn unresolved_invocation_known(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
) -> bool {
    let lookup = ctx.semantic_lookup;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if call.construct || file.ast.function_at(callee).is_some() {
        return false;
    }
    // What the callee names, from the call's own syntax and bindings first (cheap), then what reads it elsewhere.
    let stored = if crate::owners::is_identifier_span(file, callee) {
        file.ast
            .reference_declaration(callee)
            .is_some_and(|declaration| {
                // A `function` declaration or a `const` bound to a function literal is that function, called or thrown (a call
                // before its declarator): the layer walk reads the body or no function runs. Nothing was stored there.
                super::same_file_function(file, declaration).is_none()
                    && (super::declared_as_parameter(file, declaration)
                        || file.ast.bindings.iter().any(|binding| {
                            binding.names.iter().any(|name| name.span == declaration)
                        }))
            })
    } else {
        // The callee is a call's result: the value that call returns is called (`registry.get(k)!()`), unless that call is a
        // primitive (`createMemo(f)()` reads the accessor it made).
        file.ast
            .call_at(callee)
            .is_some_and(|inner| lookup.primitive_at_call(file, inner.span).is_none())
    };
    stored
        && lookup.primitive_at_call(file, call.span).is_none()
        && !(crate::owners::is_identifier_span(file, callee)
            && lookup
                .function_called_at(file.path.as_str(), callee)
                .is_some())
        && !call_accessor_symbol(ctx, file, call).is_some_and(|symbol| {
            ctx.accessors.contains_key(symbol)
                || ctx.setters.contains_key(symbol)
                || ctx.actions.contains_key(symbol)
                // ADR 0294: a typed candidate whose implementation is not visible (a parameter typed `Accessor<T>`) has its own row, SC9012
                // `type-only-source`: its declared type says what it may be, which a stored function's is not.
                || typed_unknown(ctx, symbol)
        })
}

/// Batch 10c (B1): the invocations [`unresolved_invocation`] takes beyond [`unresolved_invocation_known`]: a slot, `.call` and `.apply`, a
/// reassigned `let`, a `new`. Added rows only: they are filed after the rows the net filed before ([`Walk::invoked`]), claim no callee
/// another row stands at, and stand only where no other row does.
fn unresolved_invocation_widened(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
) -> bool {
    unresolved_invocation(ctx, file, call) && !unresolved_invocation_known(ctx, file, call)
}

/// ADR 0964: whether `call` invokes a function value the walk cannot resolve: a callback stored in a parameter, a variable, an array, a
/// map or an object, called through that name (`callbacks.forEach((c) => c())`, `const fn = list[0]; fn()`, `registry.get(k)!()`).
/// Whatever function the program put there runs on the call's stack, so on the continuation it may read a reactive source untracked:
/// an obligation at the call, never silence. Three facts decide it, all of the call's own syntax and bindings (a resolved-call fact is
/// demanded for few calls, so its absence says nothing): the callee is an identifier declared as a parameter, or as a variable whose
/// value is not a function the walk resolves; or it is the result of a call (`get(k)!()`, `f()()`) that is no primitive's accessor.
/// A callee that is an accessor, a setter, an action, a typed candidate (ADR 0294), a primitive, a project function or a function literal is read elsewhere.
///
/// Batch 10c (B1) closes the spellings the Fable review of batch 10b found silent, each the same fact (a function the program stored is
/// run here) written another way ([`stored_function_value`]): a member the project declares as a slot and no function body defines
/// (`bus.cb()`, `holder.cb?.()`, a function-typed field), the receiver of `f.call(…)` and `f.apply(…)` (the value those run), a `let`
/// the program reassigns (its initializer is not the value any more), a `new` of a stored value, and a tagged template's tag
/// ([`unresolved_tag`]).
fn unresolved_invocation(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let lookup = ctx.semantic_lookup;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.function_at(callee).is_some() {
        return false;
    }
    // `f.call(…)` and `f.apply(…)` run the value they are read from: the value invoked is the receiver.
    let receiver = crate::local_access::dispatch_receiver(lookup, file, call)
        .map(|receiver| file.ast.peel_ts_sugar_span(receiver));
    let (invoked, how) = match receiver {
        Some(receiver) => (receiver, Invoked::Receiver),
        None if call.construct => (callee, Invoked::Construct),
        None => (callee, Invoked::Callee(call)),
    };
    stored_function_value(ctx, file, invoked, how)
        && lookup.primitive_at_call(file, call.span).is_none()
        && !invoked_symbol(ctx, file, call, receiver).is_some_and(|symbol| {
            ctx.accessors.contains_key(symbol)
                || ctx.setters.contains_key(symbol)
                || ctx.actions.contains_key(symbol)
                // ADR 0294: a typed candidate whose implementation is not visible (a parameter typed `Accessor<T>`) has its own row, SC9012
                // `type-only-source`: its declared type says what it may be, which a stored function's is not.
                || typed_unknown(ctx, symbol)
        })
}

/// What invokes the value [`stored_function_value`] asks about.
#[derive(Clone, Copy)]
enum Invoked<'c> {
    /// The callee of this call.
    Callee(&'c CallFact),
    /// The callee of a `new`.
    Construct,
    /// The receiver of `f.call(…)` or `f.apply(…)`.
    Receiver,
    /// The tag of a tagged template.
    Tag,
}

/// The symbol of the value an invocation runs, for the exclusions of an accessor, a setter, an action and a typed candidate: the
/// callee's own for a plain call ([`call_accessor_symbol`]), the receiver's for `f.call(…)`.
fn invoked_symbol<'s>(
    ctx: &'s AnalysisContext<'_>,
    file: &FileFacts,
    call: &CallFact,
    receiver: Option<Span>,
) -> Option<&'s SymbolId> {
    let Some(receiver) = receiver else {
        return call_accessor_symbol(ctx, file, call);
    };
    tag_symbol(ctx, file, receiver)
}

/// The symbol of the identifier, or of the property of the static member, at `value`.
fn tag_symbol<'s>(
    ctx: &'s AnalysisContext<'_>,
    file: &FileFacts,
    value: Span,
) -> Option<&'s SymbolId> {
    match file.ast.member_at(value) {
        Some(member) if !file.ast.computed_members.contains(&member.span) => {
            ctx.entities.at(file.path.as_str(), member.property)
        }
        Some(_) => None,
        None => ctx.entities.at(file.path.as_str(), value),
    }
}

/// Whether the value at `value` (the callee of a call or a `new`, the receiver of `.call` or `.apply`, a tag) is one the program
/// stored, whose function this walk does not enter: an identifier declared as a parameter or a variable that is not a function the
/// walk resolves (a `let` the program reassigns holds more than its initializer), a call's result that is no primitive's, an element
/// access (the value stored under an index or a key, for a receiver or a tag; for a callee the declaration the compiler picked
/// decides), or a member the project declares as a slot ([`crate::indexes::SemanticLookup::project_slot_without_body`]).
fn stored_function_value(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    value: Span,
    how: Invoked<'_>,
) -> bool {
    let lookup = ctx.semantic_lookup;
    if crate::owners::is_identifier_span(file, value) {
        return file
            .ast
            .reference_declaration(value)
            .is_some_and(|declaration| {
                // A `function` declaration or a `const` bound to a function literal is that function, called or thrown (a call
                // before its declarator): the layer walk reads the body or no function runs. Nothing was stored there.
                super::same_file_function(file, declaration).is_none()
                    && !(matches!(how, Invoked::Construct)
                        && file.ast.bindings.iter().any(|binding| {
                            binding.names.iter().any(|name| name.span == declaration)
                                && binding.initializer.is_some_and(|initializer| {
                                    let initializer = file.ast.peel_ts_sugar_span(initializer);
                                    file.ast.classes.iter().any(|class| class.span == initializer)
                                })
                        }))
                    && (super::declared_as_parameter(file, declaration)
                        || file.ast.bindings.iter().any(|binding| {
                            binding.names.iter().any(|name| name.span == declaration)
                        }))
            })
            // The function the identifier names, when it is the value the identifier holds now: a `let` the program writes again
            // does not hold its initializer.
            && !lookup
                .function_called_at(file.path.as_str(), value)
                .is_some_and(|(function_file, function)| {
                    lookup.function_value_is_current(function_file, function)
                });
    }
    // The value is a call's result: the value that call returns is called (`registry.get(k)!()`), unless that call is a primitive
    // (`createMemo(f)()` reads the accessor it made).
    if let Some(inner) = file.ast.call_at(value) {
        return lookup.primitive_at_call(file, inner.span).is_none();
    }
    let Some(member) = file.ast.member_at(value) else {
        return false;
    };
    // A `new` of a member (`new this.Impl()`, `new ns.Widget()`) is a class or a constructor the program names: not this net's.
    if matches!(how, Invoked::Construct) {
        return false;
    }
    // A method or a function the project declares has a body: a class method and an object literal's method are read where they are
    // written (ADR 0963's class elements and methods), a function declaration is placed by its exact call edges.
    if let Invoked::Callee(call) = how
        && lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                matches!(
                    declaration.kind.as_ref(),
                    "MethodDeclaration" | "FunctionDeclaration" | "Constructor"
                )
            })
    {
        return false;
    }
    if file.ast.computed_members.contains(&member.span) {
        return match how {
            Invoked::Receiver | Invoked::Tag => true,
            Invoked::Callee(call) => lookup
                .resolved_callee_call(file, call.callee)
                .and_then(|resolved| resolved.declaration.as_ref())
                .is_some_and(|declaration| lookup.resolved_slot_without_body(declaration)),
            Invoked::Construct => false,
        };
    }
    // A closed object literal's member is the function the walk follows (ADR 0351, `called_function`): it is entered, not stored.
    if lookup.closed_member_function(file, member).is_some() {
        return false;
    }
    match ctx.entities.at(file.path.as_str(), member.property) {
        Some(symbol) => lookup.project_slot_without_body(symbol.as_str()),
        // The property of a receiver or a tag has no entity fact of its own (only a callee's member is demanded): the member of a value
        // the project holds (a local, a parameter, `this`) is a slot the program may have stored a function in; a global's or an
        // import's (`Math.max.apply`) is not this net's.
        None => {
            matches!(how, Invoked::Receiver | Invoked::Tag) && project_owned_member(file, member)
        }
    }
}

/// Whether the member chain at `member` starts at a value the project holds: a name declared as a parameter or a variable in this
/// file (`bus.cb`, `props.fn`), not a global (`Math.max`) nor an import. A chain rooted at `this` names no declaration and is not
/// decided here (`docs/open-work.md`).
fn project_owned_member(file: &FileFacts, member: &solid_facts::ast::MemberFact) -> bool {
    let mut root = file.ast.peel_ts_sugar_span(member.object);
    while let Some(inner) = file.ast.member_at(root) {
        root = file.ast.peel_ts_sugar_span(inner.object);
    }
    file.ast
        .reference_declaration(root)
        .is_some_and(|declaration| {
            super::declared_as_parameter(file, declaration)
                || file
                    .ast
                    .bindings
                    .iter()
                    .any(|binding| binding.names.iter().any(|name| name.span == declaration))
        })
}

/// Batch 10c (B1): whether the tag of `template` is a stored value the walk cannot resolve ([`stored_function_value`]): a tagged
/// template calls its tag where it is written (ADR 0531), so the function the program stored there runs on this stack. A tag that is
/// a function literal or a project function the walk names is followed by [`Reach::visit_template`] instead.
fn unresolved_tag(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    template: &TaggedTemplateFact,
) -> bool {
    let tag = file.ast.peel_ts_sugar_span(template.tag);
    file.ast.function_at(tag).is_none()
        && stored_function_value(ctx, file, tag, Invoked::Tag)
        && !tag_symbol(ctx, file, tag).is_some_and(|symbol| {
            ctx.accessors.contains_key(symbol)
                || ctx.setters.contains_key(symbol)
                || ctx.actions.contains_key(symbol)
                || typed_unknown(ctx, symbol)
        })
}

/// The obligation row at a call that invokes a function value the walk cannot resolve ([`unresolved_invocation`]).
fn unresolved_invocation_obligation(
    file: &FileFacts,
    call: &CallFact,
    context: &str,
) -> StaticDefect {
    let _ = context;
    // A `new` runs its callee at the `new` keyword: V8's frame for the construction stands there, so the row starts there (a call's
    // frame stands at its callee, which is where its own span starts).
    let at = if call.construct {
        Span::new(call.span.start, call.callee.end)
    } else {
        call.callee
    };
    unresolved_obligation_at(file, file.ast.peel_ts_sugar_span(call.callee), at)
}

/// The obligation row at the value `text_of` names, for a call, a `new` or a tag ([`unresolved_invocation`], [`unresolved_tag`]).
fn unresolved_obligation_at(file: &FileFacts, text_of: Span, at: Span) -> StaticDefect {
    StaticDefect {
        kind: StaticDefectKind::ReactiveReadAfterAwait {
            accessor: file.source_text(text_of).map_or_else(
                || "a stored function".to_owned(),
                |text| text.chars().take(80).collect(),
            ),
            obligation: Some(AfterAwaitObligation::UnresolvedInvocation),
        },
        location: location(file.path.shared(), at),
        analysis_context: crate::blockers::UNRESOLVED_INVOCATION.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// ADR 0879 (B5): a callee that names no exact symbol (`fns[0]()`,
/// `get()()`, `(c ? a : b)()`, a member of an untyped value) and resolves to
/// no declaration and no project function: nothing proves the value called
/// is not an accessor. An exact standard or package declaration
/// (`res.json()`) and a `new` are not this.
fn inexact_callee(ctx: &AnalysisContext<'_>, file: &FileFacts, call: &CallFact) -> bool {
    let lookup = ctx.semantic_lookup;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    !call.construct
        && !crate::owners::is_identifier_span(file, callee)
        // A function literal called in place is exact: the layer walk reads it.
        && file.ast.function_at(callee).is_none()
        && file
            .source_text(callee)
            .is_some_and(|text| !matches!(text, "super" | "import"))
        && lookup.resolved_callee_call(file, call.callee).is_none()
        && lookup
            .function_called_at(file.path.as_str(), callee)
            .is_none()
}

fn inexact_callee_obligation(file: &FileFacts, call: &CallFact, context: &str) -> StaticDefect {
    StaticDefect {
        kind: StaticDefectKind::ReactiveReadAfterAwait {
            accessor: file
                .source_text(file.ast.peel_ts_sugar_span(call.callee))
                .unwrap_or("this callee")
                .chars()
                .take(60)
                .collect(),
            obligation: Some(AfterAwaitObligation::CalleeInexact),
        },
        location: location(file.path.shared(), call.callee),
        analysis_context: context.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// A parameter of a callback a primitive runs (`createRoot((dispose) => …)`): the
/// primitive supplies it, so no caller hands it an accessor.
fn primitive_parameter(ctx: &AnalysisContext<'_>, file: &FileFacts, declaration: Span) -> bool {
    file.ast.functions.iter().any(|function| {
        function
            .parameters
            .iter()
            .flat_map(|parameter| &parameter.names)
            .any(|name| name.span == declaration)
            && file
                .ast
                .arguments_containing(function.span)
                .any(|(call, index)| {
                    file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                        && ctx
                            .semantic_lookup
                            .primitive_at_call(file, call.span)
                            .is_some()
                })
    })
}

/// ADR 0517: an async compute the producer lists no fact for (an object
/// literal's member, `createMemo(box.compute)` over `{ compute: async () =>
/// … }` or `{ async compute() {} }`), named exactly at a tracked position
/// (ADR 0516's `exact_function_argument`). No dominance fact describes its
/// body, so an accessor read written in it after its first suspension
/// (`await`, `for await`) is an obligation, never silence.
fn unlisted_async_computations(
    ctx: &AnalysisContext<'_>,
    reach: &Reach<'_, '_, '_>,
    draft: &mut ProgramDraft,
) {
    for file in crate::base::reads::all_files(ctx.facts) {
        let path = file.path.as_str();
        for call in &file.ast.calls {
            let Some(argument) = call.arguments.first() else {
                continue;
            };
            let Some(primitive) = crate::call_primitive_name_cached(
                ctx.semantic_lookup,
                file,
                call,
                ctx.entities,
                ctx.symbol_names,
                ctx.dialect,
            )
            .and_then(|name| name.primitive()) else {
                continue;
            };
            if !ctx
                .dialect
                .callback_semantics_at(primitive, 0, call.arguments.len())
                .tracks_reads
            {
                continue;
            }
            let Some((target, span)) =
                crate::owners::exact_function_argument(file, argument.span, ctx.semantic_lookup)
            else {
                continue;
            };
            if target != path
                || reach
                    .facts_by_end
                    .contains_key(&(path, u64::from(span.end)))
            {
                continue;
            }
            let Some(function) = file.ast.function_at(span) else {
                continue;
            };
            if !function.r#async {
                continue;
            }
            let Some(boundary) = file
                .ast
                .awaits
                .iter()
                .chain(&file.ast.implicit_suspensions)
                .filter(|point| {
                    function.body.contains(**point)
                        && written_directly_in(&file.ast, function, **point)
                })
                .map(|point| point.start)
                .chain(first_yield_end(file, function))
                .min()
            else {
                continue;
            };
            // Every read after the boundary, in the body or in a function
            // written in it (an in-place layer may run it there), and a call
            // there of a project function whose own body reads an accessor
            // (a wrapper): each may read after the suspension.
            let accessor_read = |in_file: &FileFacts, call: &CallFact| {
                ctx.semantic_lookup
                    .callee_symbol(in_file, call.callee)
                    .and_then(|symbol| ctx.accessors.get(symbol))
                    .map(|(name, _)| name.to_string())
            };
            for read in &file.ast.calls {
                if read.span.start < boundary || !function.body.contains(read.span) {
                    continue;
                }
                let name = accessor_read(file, read).or_else(|| {
                    let (callee_file, callee) =
                        ctx.semantic_lookup.function_called_at(path, read.callee)?;
                    callee_file
                        .ast
                        .calls
                        .iter()
                        .filter(|inner| callee.body.contains(inner.span))
                        .find_map(|inner| accessor_read(callee_file, inner))
                });
                let Some(name) = name else {
                    continue;
                };
                draft.push_defect(StaticDefect {
                    kind: StaticDefectKind::ReactiveReadAfterAwait {
                        accessor: name,
                        obligation: Some(AfterAwaitObligation::SuspensionUnplaced),
                    },
                    location: location(file.path.shared(), read.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                    uncertain: true,
                });
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
    if let Some(found) = file.ast.functions.iter().find(|candidate| {
        candidate.span.end == end
            && candidate.span.start > start
            && method_header(file.source_text(Span::new(start, candidate.span.start)))
    }) {
        return Some((file, found));
    }
    // ADR 0517: an object literal's member (`{ cb: async () => … }`, `{ async
    // cb() {} }`) whose producer span holds more of the property: the
    // outermost function inside the span that ends where it does.
    let found = file
        .ast
        .functions
        .iter()
        .filter(|candidate| candidate.span.end == end && candidate.span.start > start)
        .min_by_key(|candidate| candidate.span.start)?;
    // Exactly one property's span: the producer named the property.
    file.ast
        .object_properties
        .iter()
        .any(|property| property.span == Span::new(start, end))
        .then_some((file, found))
}

/// ADR 0961 (PD): the constructor of the class a `new` call names, when the class is written in the same file.
fn constructed_constructor<'f>(file: &'f FileFacts, call: &CallFact) -> Option<&'f FunctionFact> {
    let declaration = file
        .ast
        .reference_declaration(file.ast.peel_ts_sugar_span(call.callee))?;
    let class = file.ast.classes.iter().find(|class| {
        class
            .name
            .as_ref()
            .is_some_and(|name| name.span == declaration)
    })?;
    let constructor = class
        .elements
        .iter()
        .find(|element| element.kind == solid_facts::ast::ClassElementKind::Constructor)?;
    file.ast.function_at(constructor.value?)
}

/// ADR 0961 (PD, batch-10 addendum 13): whether the argument keeps the parameter's default from running. Only syntax proves a
/// value defined (`argument_values.rs`: a type says nothing about runtime nullishness): a `null`, a function, a primitive, object
/// or array literal. An identifier or any other expression may be `undefined` at run time (`h(maybe)`, `h(c ? 1 : undefined)`),
/// and then the default runs; a spread may have the argument away.
fn proves_defined(argument: &solid_facts::ast::ArgumentFact) -> bool {
    use solid_facts::ast::{ArgumentValueKind, RuntimeValueKind};
    !argument.spread
        && (matches!(
            argument.value,
            ArgumentValueKind::Null
                | ArgumentValueKind::Function
                | ArgumentValueKind::AsyncFunction
        ) || matches!(
            argument.runtime_value_kind,
            RuntimeValueKind::Primitive
                | RuntimeValueKind::Function
                | RuntimeValueKind::Object
                | RuntimeValueKind::Array
        ))
}

/// ADR 0961 (PD): whether `call` runs a parameter initializer of `function` that holds code that may read a source (a call, a
/// member access, a function: an IIFE or a closure). A default is evaluated unless the call proves a defined value at its index
/// ([`proves_defined`]): no argument, `undefined`, a spread before it, and an identifier or an expression that may be
/// `undefined` all run it; a destructuring pattern's defaults are evaluated whatever the argument is (a property it lacks), so
/// such a parameter counts whenever its pattern holds that code. A constant default reads nothing.
fn parameter_initializer_runs(
    target: &FileFacts,
    function: &FunctionFact,
    call: &CallFact,
) -> bool {
    // The invocation of a function literal reads nothing itself (what the literal's body calls is counted where it is written).
    let may_read = |region: Span| {
        target.ast.calls.iter().any(|inner| {
            region.contains(inner.span)
                && target
                    .ast
                    .function_at(target.ast.peel_ts_sugar_span(inner.callee))
                    .is_none()
        }) || target
            .ast
            .members
            .iter()
            .any(|member| region.contains(member.span))
    };
    function
        .parameters
        .iter()
        .enumerate()
        .any(|(index, parameter)| {
            if parameter.shape != solid_facts::ast::BindingShape::Identifier {
                return may_read(parameter.declaration);
            }
            let Some(initializer) = parameter.initializer else {
                return false;
            };
            let keeps_default = call
                .arguments
                .iter()
                .take(index + 1)
                .all(|argument| !argument.spread)
                && call.arguments.get(index).is_some_and(proves_defined);
            !keeps_default && may_read(initializer)
        })
}

/// Whole-argument hand-ons followed from a call: past it, an obligation.
const MAX_FORWARDS: usize = 32;

/// Text a method's producer span may hold before its parameter list and no
/// other function: `async`, `*`, a key name, whitespace; and (ADR 0517) an
/// object literal property's `key:` before the function it holds
/// (`{ compute: async () => … }`), whose producer span starts at the key.
fn method_header(text: Option<&str>) -> bool {
    text.is_some_and(|text| {
        !text.is_empty()
            && text.chars().filter(|c| *c == ':').count() <= 1
            && text.chars().all(|c| {
                c.is_alphanumeric()
                    || c == '_'
                    || c == '$'
                    || c == '*'
                    || c == ':'
                    || c.is_whitespace()
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

/// ADR 0820: where a generator's body stops running on the stack that stepped
/// it first: the end of its first `yield` written in it, the operand included.
fn first_yield_end(file: &FileFacts, function: &FunctionFact) -> Option<u32> {
    if !function.generator {
        return None;
    }
    file.ast
        .yields
        .iter()
        .filter(|point| {
            function.body.contains(point.span)
                && written_directly_in(&file.ast, function, point.span)
        })
        .map(|point| point.span.end)
        .min()
}

/// A generator that may give up the stack at a `yield` before anything else
/// suspends it: its straight-line reads are no window of the computation.
fn leading_yield(file: &FileFacts, function: &FunctionFact) -> bool {
    first_yield_end(file, function).is_some_and(|end| synchronous_prefix(file, function).end >= end)
}

/// A call that is the operand of a `for await` written in the function: the
/// loop's first `next()` is on the spot, and every later step is the loop's.
fn for_await_operand(file: &FileFacts, call: &CallFact) -> bool {
    file.ast.iterated_operands.contains(&call.span)
        && !file
            .ast
            .yields
            .iter()
            .any(|point| point.delegate && point.span.contains(call.span))
        && file
            .ast
            .implicit_suspensions
            .iter()
            .any(|suspension| suspension.end == call.span.end && suspension.start < call.span.start)
}

/// A sync generator's call that is a `for…of` operand with no suspension of
/// `function` in the loop: the loop drains it on one stack.
fn drained_in_place(file: &FileFacts, function: &FunctionFact, call: &CallFact) -> bool {
    file.ast.iterated_operands.contains(&call.span)
        && file.ast.loop_statements.iter().any(|lp| {
            lp.contains(call.span)
                && written_directly_in(&file.ast, function, *lp)
                && !file
                    .ast
                    .awaits
                    .iter()
                    .chain(&file.ast.implicit_suspensions)
                    .any(|point| {
                        lp.contains(*point) && written_directly_in(&file.ast, function, *point)
                    })
        })
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

/// Turns the reads reported since `mark` into proof obligations for `reason`;
/// a read that is one already keeps its own (inner) reason (ADR 0595).
fn mark_uncertain(draft: &mut ProgramDraft, mark: usize, reason: &AfterAwaitObligation) {
    for defect in &mut draft.static_defects[mark..] {
        if let StaticDefectKind::ReactiveReadAfterAwait { obligation, .. } = &mut defect.kind {
            defect.uncertain = true;
            obligation.get_or_insert_with(|| reason.clone());
            if *reason == AfterAwaitObligation::GeneratorStepped
                && !crate::blockers::named(&defect.analysis_context)
            {
                defect.analysis_context = crate::blockers::with_detail(
                    crate::blockers::GENERATOR_STEPPED,
                    &defect.analysis_context,
                );
            }
        }
    }
}

/// A callee's source text, as a finding quotes it.
fn callee_text(file: &FileFacts, callee: Span) -> String {
    file.source_text(file.ast.peel_ts_sugar_span(callee))
        .map_or_else(
            || "a call".to_owned(),
            |text| text.chars().take(80).collect(),
        )
}

/// The obligation of an accessor read past an await: [`tracking_obligation`]'s,
/// or, for an async source (ADR 0614), whether it is pending at the read.
fn read_obligation(
    ctx: &AnalysisContext<'_>,
    symbol: Option<&SymbolId>,
    tracking: Tracking,
) -> Option<AfterAwaitObligation> {
    tracking_obligation(tracking).or_else(|| {
        symbol
            .is_some_and(|symbol| ctx.async_sources.contains(symbol))
            .then_some(AfterAwaitObligation::AsyncSourceMayBePending)
    })
}

/// The reason a read's pre-await tracking leaves it an obligation, if any.
fn tracking_obligation(tracking: Tracking) -> Option<AfterAwaitObligation> {
    (tracking == Tracking::Possible).then_some(AfterAwaitObligation::MayBeTracked)
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

impl<'s, 'f, 'a> Reach<'s, 'f, 'a> {
    fn key(function: &AsyncFunctionFact) -> (String, u64, u64) {
        (
            function.expression.path.to_string(),
            function.expression.start_byte,
            function.expression.end_byte,
        )
    }

    /// ADR 0871: the windows as a set, so a recursive helper ends.
    fn walk_key(member: &Member) -> WalkKey {
        let mut windows = member
            .windows
            .iter()
            .map(|window| Self::key(window))
            .collect::<Vec<_>>();
        windows.sort_unstable();
        windows.dedup();
        (
            Self::key(&member.function),
            windows,
            member.conditional.as_ref().map(std::mem::discriminant),
        )
    }

    /// Whether the function returns or awaits a call that could put a
    /// promise on a computation's flight. Cheap and syntactic-plus-exact: the
    /// computation proof, which scans the project, runs only when it holds.
    fn has_promise_seed(&self, function: &'f AsyncFunctionFact) -> bool {
        let Some((file, ast_function)) = ast_function(self.files, function) else {
            return false;
        };
        chain_calls(file, ast_function)
            .into_iter()
            .any(|(call, _)| {
                async_helper(self.ctx, file, call).is_some()
                    || self
                        .ctx
                        .semantic_lookup
                        .function_called_at(
                            file.path.as_str(),
                            file.ast.peel_ts_sugar_span(call.callee),
                        )
                        .is_some()
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
        ast_function(self.files, function).is_some_and(|(file, function)| {
            file.ast
                .try_head_awaits
                .iter()
                .any(|head| written_directly_in(&file.ast, function, head.awaited))
        })
    }

    /// Whether the function itself suspends in a way the producer's facts do
    /// not model (`for await`, `await using`).
    /// ADR 0870: whether the function holds an `await` of its own: one a
    /// branch may skip dominates nothing, so the producer lists no call for it.
    fn has_own_await(&self, function: &'f AsyncFunctionFact) -> bool {
        ast_function(self.files, function).is_some_and(|(file, ast_function)| {
            file.ast.awaits.iter().any(|awaited| {
                ast_function.body.contains(*awaited)
                    && written_directly_in(&file.ast, ast_function, *awaited)
            })
        })
    }

    fn has_implicit_suspension(&self, function: &'f AsyncFunctionFact) -> bool {
        ast_function(self.files, function).is_some_and(|(file, ast_function)| {
            file.ast.implicit_suspensions.iter().any(|suspension| {
                ast_function.body.contains(*suspension)
                    && written_directly_in(&file.ast, ast_function, *suspension)
            }) || loop_suspension_start(file, ast_function).is_some()
        })
    }

    /// Whether the windows' tracking already made `symbol` a dependency
    /// (ADR 0195), may have (a read through a helper, a getter, an inline
    /// callback or a call the analysis cannot see into), or cannot have.
    fn tracking(&self, member: &Member, symbol: &SymbolId) -> Tracking {
        self.pre.accessor(&member.windows, symbol)
    }

    /// ADR 0871: [`Self::report`] on rows of its own, merged once its marks
    /// are done: a read another walk reported, under other windows, is decided
    /// by the stronger row, and marking this walk's rows touches no other's.
    fn report_member(
        &mut self,
        draft: &mut ProgramDraft,
        member: &Member,
        queue: &mut Vec<Member>,
        speculated: Option<Speculated>,
    ) {
        if !self.done.insert(Self::walk_key(member)) {
            return;
        }
        *self.walks.entry(Self::key(&member.function)).or_default() += 1;
        let (rows, next) = match speculated.filter(|walk| draft.spans_agree(&walk.span_log)) {
            Some(walk) => {
                draft.add_seen_spans(walk.spans);
                (walk.rows, walk.queued)
            }
            None => {
                let earlier = std::mem::take(&mut draft.static_defects);
                let seen = std::mem::take(&mut draft.seen_diagnostics);
                let mut next = Vec::new();
                self.report(draft, member, &mut next);
                draft.seen_diagnostics = seen;
                let rows = std::mem::replace(&mut draft.static_defects, earlier);
                (rows, next)
            }
        };
        for mut row in rows {
            if let Some(blocker) = member.blocker.filter(|_| row.uncertain) {
                row.analysis_context = blocker.into();
            }
            draft.push_defect_or_strengthen(row);
        }
        // The bound reads the walks so far, which the walk itself never
        // changes: applied here, it sees what it saw inside the walk.
        queue.extend(next.into_iter().map(|member| self.bounded(member)));
    }

    /// Whether `function` has anything after an await to walk.
    fn may_report(&self, function: &'f AsyncFunctionFact) -> bool {
        let member_site = self
            .ctx
            .dialect
            .reports_member_reads_after_await()
            .then(|| member_read_site(self.files, function))
            .flatten();
        !(function.calls_after_await.is_empty()
            && member_site.is_none()
            && !self.has_promise_seed(function)
            && !self.has_implicit_suspension(function)
            && !self.has_own_await(function)
            && !self.has_try_handler(function)
            && !self.steps_generator(function))
    }

    /// The member a function the walks report on starts with, or `None`
    /// for one no computation runs.
    fn start_member(
        &self,
        index: &super::AsyncComputations,
        function: &'f AsyncFunctionFact,
    ) -> Option<Member> {
        let ctx = self.ctx;
        // ADR 0873: a compute a project wrapper's parameter hands on.
        let (context, handed) = match tracked_async_computation_context(ctx, function, index) {
            Some(context) => (context, None),
            None => match ast_function(self.files, function).and_then(|(file, function)| {
                super::handed_compute::handed_compute(ctx, &index.exact_arguments, file, function)
            })? {
                super::handed_compute::HandedCompute::Tracked(context) => (context, None),
                super::handed_compute::HandedCompute::Unplaced { via } => (
                    "handed async function".to_owned(),
                    Some(AfterAwaitObligation::HandOffNotFollowed { via }),
                ),
            },
        };
        let blocker = handed
            .is_some()
            .then_some(crate::blockers::FUNCTION_RUN_UNPLACED);
        let function = Arc::new(function.clone());
        // An owner a `dispose` function can reach may be gone when the
        // continuation resumes: the runtime then reads in a disposed
        // computation and stays silent, so every read is an obligation.
        let conditional = owner_may_be_disposed(ctx, self.files, &function)
            .then_some(AfterAwaitObligation::OwnerMayBeDisposed)
            .or(handed);
        // ADR 0820: past a generator's leading `yield` nothing is tracked.
        let windows = if ast_function(self.files, &function)
            .is_some_and(|(file, function)| leading_yield(file, function))
        {
            Vec::new()
        } else {
            vec![Arc::clone(&function)]
        };
        Some(Member {
            function,
            context,
            windows,
            conditional,
            blocker,
        })
    }

    /// ADR 0909: `member`'s walk on a draft of its own
    /// ([`ProgramDraft::speculative`]), for [`Self::report_member`].
    fn speculate(&mut self, template: &ProgramDraft, member: &Member) -> Speculated {
        let mut draft = template.speculative();
        let mut queued = Vec::new();
        self.report(&mut draft, member, &mut queued);
        let (spans, span_log) = draft.take_spans();
        Speculated {
            rows: draft.static_defects,
            queued,
            spans,
            span_log,
        }
    }

    /// ADR 0871: an unseen window set past the bound: one last walk with no
    /// window and every row an obligation.
    fn bounded(&self, mut member: Member) -> Member {
        if !self.done.contains(&Self::walk_key(&member))
            && self
                .walks
                .get(&Self::key(&member.function))
                .copied()
                .unwrap_or(0)
                >= MAX_WALKS_PER_FUNCTION
        {
            member.windows = Vec::new();
            member.conditional = Some(AfterAwaitObligation::SuspensionUnplaced);
            member.blocker = Some(crate::blockers::SEARCH_DEPTH_LIMIT);
        }
        member
    }

    fn report(&mut self, draft: &mut ProgramDraft, member: &Member, queue: &mut Vec<Member>) {
        let ctx = self.ctx;
        let function = &*member.function;
        let member_site = ctx
            .dialect
            .reports_member_reads_after_await()
            .then(|| member_read_site(self.files, function))
            .flatten();
        let mut walk = Walk {
            member,
            reported: HashSet::default(),
            invoked: HashSet::default(),
            visited: HashSet::default(),
            queued: HashSet::default(),
            placed: HashSet::default(),
            member_helpers: HashSet::default(),
            stack_helpers: HashSet::default(),
            carried: Carried::Held,
            untracked: false,
            unproven: false,
            past_suspension: false,
            past_on_some_path: false,
        };
        let mark = draft.static_defects.len();
        // The producer's dominance facts: accessor calls after the first
        // await that dominates them.
        let mut after_await_calls: Vec<(&'f FileFacts, &'f CallFact)> = Vec::new();
        let mut after_await_templates: Vec<(&'f FileFacts, &'f TaggedTemplateFact)> = Vec::new();
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
                } else if let Some((file, candidate)) = ast_call
                    && symbol.is_none_or(|symbol| {
                        !ctx.setters.contains_key(symbol) && !ctx.actions.contains_key(symbol)
                    })
                    && crate::local_access::handed_value(
                        &crate::local_access::HandedTables {
                            lookup: ctx.semantic_lookup,
                            entities: ctx.entities,
                            prop_sources: crate::late_chunks::Noted::new(
                                crate::late_chunks::TableId::PropSources,
                                ctx.prop_sources,
                            ),
                            source_kinds: crate::late_chunks::Noted::new(
                                crate::late_chunks::TableId::SourceKinds,
                                ctx.source_kinds,
                            ),
                            accessors: crate::late_chunks::Noted::new(
                                crate::late_chunks::TableId::Accessors,
                                ctx.accessors,
                            ),
                            setters: crate::late_chunks::Noted::new(
                                crate::late_chunks::TableId::Setters,
                                ctx.setters,
                            ),
                            actions: crate::late_chunks::Noted::new(
                                crate::late_chunks::TableId::Actions,
                                ctx.actions,
                            ),
                            props_reactivity: crate::late_chunks::NotedProps::new(
                                ctx.props_reactivity,
                            ),
                            accessors_that_count: None,
                        },
                        file,
                        candidate.callee,
                        crate::local_access::HANDED_DEPTH,
                    )
                    && walk
                        .reported
                        .insert((call.path.to_string(), call.start_byte, call.end_byte))
                {
                    // ADR 0509: the callee is a value a caller supplied (a
                    // prop, a context or hook member); it may be an accessor.
                    let mut obligation = typed_after_await_obligation(file, candidate.callee);
                    obligation.analysis_context = "handed-read-after-await".to_owned();
                    draft.push_defect(obligation);
                } else if let Some((file, candidate)) = ast_call
                    && symbol.is_none_or(|symbol| {
                        !ctx.setters.contains_key(symbol) && !ctx.actions.contains_key(symbol)
                    })
                    && file
                        .ast
                        .reference_declaration(file.ast.peel_ts_sugar_span(candidate.callee))
                        .is_some_and(|declaration| {
                            super::declared_as_parameter(file, declaration)
                                && !primitive_parameter(ctx, file, declaration)
                        })
                    && walk
                        .reported
                        .insert((call.path.to_string(), call.start_byte, call.end_byte))
                {
                    // ADR 0669: a parameter is a value a caller supplied too.
                    draft.push_defect(StaticDefect {
                        kind: StaticDefectKind::ReactiveReadAfterAwait {
                            accessor: file
                                .source_text(candidate.callee)
                                .unwrap_or("this parameter")
                                .to_owned(),
                            obligation: Some(AfterAwaitObligation::HandedParameter),
                        },
                        location: location(file.path.shared(), candidate.callee),
                        analysis_context: crate::blockers::HANDED_READ_AFTER_AWAIT.to_owned(),
                        fixes: vec![],
                        uncertain: true,
                    });
                } else if let Some((file, candidate)) = ast_call
                    && symbol.is_none()
                    && inexact_callee(ctx, file, candidate)
                    && walk
                        .reported
                        .insert((call.path.to_string(), call.start_byte, call.end_byte))
                {
                    draft.push_defect(inexact_callee_obligation(file, candidate, &member.context));
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
            let obligation = read_obligation(ctx, symbol, tracking);
            let uncertain = obligation.is_some();
            draft.push_defect(StaticDefect {
                kind: StaticDefectKind::ReactiveReadAfterAwait {
                    accessor: display.to_string(),
                    obligation,
                },
                location: typefacts::Location {
                    path: call.path.clone(),
                    start_byte: call.start_byte,
                    end_byte: call.end_byte,
                },
                analysis_context: member.context.clone(),
                fixes: vec![],
                uncertain,
            });
        }
        // ADR 0517: a comma, conditional, logical or assignment callee
        // (`(slot = r)()`) names no symbol, so the producer's after-await
        // calls never list it. Written in the function's own body after one of
        // its awaits and able to yield an accessor, the call may read it there.
        // A total callee every operand of which names one accessor, after an
        // await that dominates it, is that accessor's read; any other is an
        // obligation, never silence.
        if let Some((file, ast_function)) = ast_function(self.files, function) {
            for candidate in &file.ast.calls {
                if !ast_function.body.contains(candidate.span)
                    || crate::owners::containing_ast_function(&file.ast, candidate.span)
                        .is_none_or(|owner| owner.span != ast_function.span)
                    || ctx
                        .semantic_lookup
                        .callee_symbol(file, candidate.callee)
                        .is_some()
                {
                    continue;
                }
                let references = file.ast.composite_callee_references(candidate.callee);
                let accessors = references
                    .iter()
                    .filter_map(|operand| ctx.entities.at(file.path.as_str(), *operand))
                    .filter(|symbol| ctx.accessors.contains_key(*symbol))
                    .collect::<Vec<_>>();
                let Some(&symbol) = accessors.first() else {
                    continue;
                };
                let dominated =
                    crate::execution_role::follows_unconditional_await(file, candidate.span)
                        || crate::execution_role::follows_dominating_await(file, candidate.span);
                // A later pass of a loop that awaits, and a `finally` or
                // `catch` after an awaited `try` head, may run after a
                // suspension too (ADR 0516's loop and try-head facts).
                let may_follow =
                    crate::execution_role::follows_any_suspension(file, candidate.span)
                        || crate::execution_role::follows_try_head_await(
                            file,
                            candidate.span,
                            ctx.semantic_lookup,
                        )
                        || loop_suspension_start(file, ast_function)
                            .is_some_and(|start| candidate.span.start >= start)
                        || first_yield_end(file, ast_function)
                            .is_some_and(|end| candidate.span.start >= end)
                        || file.ast.awaits.iter().any(|awaited| {
                            awaited.end <= candidate.span.start
                                && written_directly_in(&file.ast, ast_function, *awaited)
                        });
                if !dominated && !may_follow {
                    continue;
                }
                // ADR 0897 (review D31): silent only when every operand is a
                // reference to the one tracked accessor and the callee total;
                // a tracked first operand beside another is an obligation.
                let same = accessors.len() == references.len()
                    && references.len()
                        == file.ast.composite_callee_operands(candidate.callee).len()
                    && accessors.iter().all(|other| *other == symbol)
                    && file.ast.composite_callee_total(candidate.callee);
                let tracking = self.tracking(member, symbol);
                if same && tracking == Tracking::Tracked {
                    continue;
                }
                let exact = dominated && same;
                let callee = location(file.path.shared(), candidate.callee);
                if !walk.reported.insert((
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                )) {
                    continue;
                }
                let name = ctx
                    .accessors
                    .get(symbol)
                    .map_or_else(|| symbol.to_string(), |(name, _)| name.to_string());
                draft.push_defect(StaticDefect {
                    kind: StaticDefectKind::ReactiveReadAfterAwait {
                        accessor: name,
                        obligation: (!exact)
                            .then_some(AfterAwaitObligation::CalleeInexact)
                            .or_else(|| tracking_obligation(tracking)),
                    },
                    // ADR 0897: one accessor read is at its reference; V8's
                    // frame for the call names the callee's last token.
                    location: if same {
                        location(file.path.shared(), references[references.len() - 1])
                    } else {
                        callee
                    },
                    analysis_context: member.context.clone(),
                    fixes: vec![],
                    uncertain: !exact || tracking == Tracking::Possible,
                });
            }
        }
        // ADR 0964: a call written in the function's own body after one of its awaits that invokes a function value the walk cannot
        // resolve (`fn()` for a `const fn = list[0]`, a stored callback) runs whatever function the program stored there on the
        // continuation: an obligation at the call, where no row stands yet. The rows wait for the end of the walk (`deferred`): a row the
        // hand-off, the inexact callee or the parameter files at the same callee is the cover that stands, and this one neither
        // pre-empts it nor repeats it.
        let mut deferred = Vec::new();
        if let Some((file, ast_function)) = ast_function(self.files, function) {
            for candidate in &file.ast.calls {
                let written_here = ast_function.body.contains(candidate.span)
                    && crate::owners::containing_ast_function(&file.ast, candidate.span)
                        .is_some_and(|owner| owner.span == ast_function.span);
                if !written_here
                    || !(crate::execution_role::follows_unconditional_await(file, candidate.span)
                        || crate::execution_role::follows_dominating_await(file, candidate.span)
                        || crate::execution_role::follows_any_suspension(file, candidate.span)
                        || crate::execution_role::may_follow_own_await(file, candidate.span))
                {
                    continue;
                }
                let callee = location(file.path.shared(), candidate.callee);
                let key = (callee.path.to_string(), callee.start_byte, callee.end_byte);
                if unresolved_invocation_known(ctx, file, candidate) {
                    if walk.reported.insert(key) {
                        draft.push_defect(unresolved_invocation_obligation(
                            file,
                            candidate,
                            &member.context,
                        ));
                    }
                } else if unresolved_invocation_widened(ctx, file, candidate) {
                    deferred.push((
                        key,
                        unresolved_invocation_obligation(file, candidate, &member.context),
                    ));
                }
            }
        }
        // ADR 0961 (PD): a call written in the function's own body after one of its awaits runs the parameter initializers of the
        // function it calls (or of the constructor it constructs) when it omits the argument, on the continuation. The walk reads
        // an accessor call in a default, and no IIFE, closure, destructuring default or constructor's: where such an initializer
        // holds code that may read a source and no row stands at the call yet, the call is an obligation, never silence.
        if let Some((file, ast_function)) = ast_function(self.files, function) {
            for candidate in &file.ast.calls {
                // A call this function's own body writes: the innermost function around it is this one (a nested function's calls
                // are its own member's).
                let written_here = ast_function.body.contains(candidate.span)
                    && crate::owners::containing_ast_function(&file.ast, candidate.span)
                        .is_some_and(|owner| owner.span == ast_function.span);
                if !written_here {
                    continue;
                }
                let after_await =
                    crate::execution_role::follows_unconditional_await(file, candidate.span)
                        || crate::execution_role::follows_dominating_await(file, candidate.span)
                        || crate::execution_role::follows_any_suspension(file, candidate.span)
                        || crate::execution_role::may_follow_own_await(file, candidate.span);
                if !after_await {
                    continue;
                }
                let callee_span = file.ast.peel_ts_sugar_span(candidate.callee);
                let target = if candidate.construct {
                    constructed_constructor(file, candidate).map(|constructor| (file, constructor))
                } else {
                    ctx.semantic_lookup
                        .function_called_at(file.path.as_str(), callee_span)
                };
                let Some((target_file, target)) = target else {
                    continue;
                };
                if !parameter_initializer_runs(target_file, target, candidate) {
                    continue;
                }
                let callee = location(file.path.shared(), candidate.callee);
                if !walk.reported.insert((
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                )) {
                    continue;
                }
                draft.push_defect(StaticDefect {
                    kind: StaticDefectKind::ReactiveReadAfterAwait {
                        accessor: callee_text(file, candidate.callee),
                        obligation: Some(AfterAwaitObligation::ParameterInitializerUnplaced),
                    },
                    // The call expression's start: a `new` call's frame is at its `new` token (V8 places the frame at the
                    // callee, the judge reads it as the expression's start), a plain call's at its callee.
                    location: location(
                        file.path.shared(),
                        Span::new(candidate.span.start, candidate.callee.end),
                    ),
                    analysis_context: crate::blockers::with_detail(
                        crate::blockers::PARAMETER_INITIALIZER_UNPLACED,
                        &member.context,
                    ),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
        // The promise chain first: it records which function literals become
        // members, so the callback walk below leaves their obligations to them.
        if let Some((file, ast_function)) = ast_function(self.files, function) {
            for (call, carried) in chain_calls(file, ast_function) {
                walk.carried = carried;
                // ADR 0872: a helper called past the member's first suspension
                // ran its prefix with no observer.
                let dominated = crate::execution_role::follows_unconditional_await(file, call.span)
                    || crate::execution_role::follows_dominating_await(file, call.span);
                let may_follow = crate::execution_role::may_follow_own_await(file, call.span)
                    || crate::execution_role::follows_any_suspension(file, call.span);
                walk.past_suspension = dominated || may_follow;
                walk.past_on_some_path = may_follow && !dominated;
                self.expand(draft, &mut walk, queue, file, call, 0);
            }
            walk.past_suspension = false;
            walk.past_on_some_path = false;
        }
        if let Some(site) = member_site {
            member_reads_after_await(
                ctx,
                draft,
                site,
                &member.context,
                &|symbol, text| self.pre.store_key(&member.windows, symbol, text),
                self.longer_path_objects,
            );
            let (file, ast_function, boundary) = site;
            for call in file.ast.calls_within(ast_function.body) {
                if call.span.start >= boundary
                    && crate::owners::owned_by_body(&file.ast, ast_function, call.span)
                {
                    after_await_calls.push((file, call));
                }
            }
            for template in &file.ast.tagged_templates {
                if template.span.start >= boundary
                    && crate::owners::owned_by_body(&file.ast, ast_function, template.span)
                {
                    after_await_templates.push((file, template));
                }
            }
        }
        for (file, call) in after_await_calls {
            self.visit_call(draft, &mut walk, file, call, 0);
        }
        for (file, template) in after_await_templates {
            self.visit_template(draft, &mut walk, file, template, 0);
        }
        if let Some((file, ast_function)) = ast_function(self.files, function) {
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
                        mark_uncertain(draft, before, &AfterAwaitObligation::SuspensionUnplaced);
                    }
                }
            }
            self.after_implicit_suspension(draft, &mut walk, mark, file, ast_function);
            self.after_conditional_await(draft, &mut walk, file, ast_function);
            self.report_unplaced_functions(draft, &mut walk, file, ast_function);
            self.follow_generators(draft, &mut walk, queue, file, ast_function);
        }
        for (key, defect) in deferred {
            if !walk.reported.contains(&key) && walk.invoked.insert(key) {
                draft.push_defect(defect);
            }
        }
        if let Some(reason) = &member.conditional {
            mark_uncertain(draft, mark, reason);
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
        let implicit = file
            .ast
            .implicit_suspensions
            .iter()
            .filter(|suspension| {
                function.body.contains(**suspension)
                    && written_directly_in(&file.ast, function, **suspension)
            })
            .map(|suspension| suspension.end)
            .min();
        // ADR 0516: a loop holding an own `await` suspends on its back edge,
        // so code written anywhere in it (before the `await` too) may run after
        // a suspension on a later pass, as may everything after the loop: from
        // the loop's start every read is an obligation (the first pass's reads
        // before the `await` are dependencies, ADR 0195, never violations).
        // ADR 0820: a generator's body past its first `yield` runs when its
        // iterator is stepped again: off this stack, on whoever steps it.
        let Some(boundary) = implicit
            .into_iter()
            .chain(loop_suspension_start(file, function))
            .chain(first_yield_end(file, function))
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
                self.longer_path_objects,
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
        for template in &file.ast.tagged_templates {
            if function.body.contains(template.span)
                && template.span.start >= boundary
                && written_directly_in(&file.ast, function, template.span)
            {
                self.visit_template(draft, walk, file, template, 0);
            }
        }
        let reason = if function.generator {
            AfterAwaitObligation::GeneratorStepped
        } else {
            AfterAwaitObligation::SuspensionUnplaced
        };
        mark_uncertain(draft, mark, &reason);
    }

    /// ADR 0870: an `await` a branch may skip dominates nothing, so no fact
    /// lists what follows it (`if (f) await x; n();`, `f && (await x)`, a
    /// `switch` case, a destructuring default). What may follow an own await
    /// on some path and has no row yet is an obligation: position cannot tell
    /// an arm that leaves from one that falls through, and two conditions may
    /// correlate, so a run past the await is not proven.
    fn after_conditional_await(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        function: &'f FunctionFact,
    ) {
        let Some(boundary) = file
            .ast
            .awaits
            .iter()
            .filter(|awaited| {
                function.body.contains(**awaited)
                    && written_directly_in(&file.ast, function, **awaited)
            })
            .map(|awaited| awaited.end)
            .min()
        else {
            return;
        };
        let mark = draft.static_defects.len();
        walk.past_suspension = true;
        walk.past_on_some_path = true;
        if self.ctx.dialect.reports_member_reads_after_await() {
            member_reads_after_await(
                self.ctx,
                draft,
                (file, function, boundary),
                &walk.member.context,
                &|symbol, text| self.pre.store_key(&walk.member.windows, symbol, text),
                self.longer_path_objects,
            );
            // Past the boundary by position only: the other arm of the await's
            // `if` runs with no await before it.
            let mut index = mark;
            while index < draft.static_defects.len() {
                let site = &draft.static_defects[index].location;
                let may_follow = u32::try_from(site.start_byte)
                    .ok()
                    .zip(u32::try_from(site.end_byte).ok())
                    .is_none_or(|(start, end)| {
                        self.site_may_follow(file, function, Span::new(start, end))
                    });
                if may_follow {
                    index += 1;
                } else {
                    draft.static_defects.remove(index);
                }
            }
        }
        for call in file.ast.calls_within(function.body) {
            if written_directly_in(&file.ast, function, call.span)
                && crate::execution_role::may_follow_own_await(file, call.span)
            {
                self.report_call_read(draft, walk, file, call);
                self.visit_call(draft, walk, file, call, 0);
            }
        }
        for template in &file.ast.tagged_templates {
            if function.body.contains(template.span)
                && written_directly_in(&file.ast, function, template.span)
                && crate::execution_role::may_follow_own_await(file, template.span)
            {
                self.visit_template(draft, walk, file, template, 0);
            }
        }
        walk.past_suspension = false;
        walk.past_on_some_path = false;
        mark_uncertain(draft, mark, &AfterAwaitObligation::SuspensionUnplaced);
    }

    /// Whether a member read reported in `function`'s body may run after one
    /// of its own awaits. A read in a layer run in place (a closure, an IIFE,
    /// a helper) is placed by the call in `function`'s body that runs the
    /// layer; a site whose call cannot be named stays (fail closed).
    fn site_may_follow(&self, file: &FileFacts, function: &FunctionFact, site: Span) -> bool {
        if crate::execution_role::may_follow_own_await(file, site) {
            return true;
        }
        if crate::owners::containing_ast_function(&file.ast, site)
            .is_some_and(|owner| owner.span == function.span)
        {
            return false;
        }
        crate::execution_role::climb_in_place_layers_within(
            file,
            site,
            self.ctx.semantic_lookup,
            Some(function.span),
        )
        .is_none_or(|climb| crate::execution_role::may_follow_own_await(file, climb.at))
    }

    /// ADR 0820: the project function a call runs, exactly: a literal callee,
    /// an exact function, or a closed object's member.
    fn called_function(
        &self,
        file: &'f FileFacts,
        call: &CallFact,
    ) -> Option<(&'f FileFacts, &'f FunctionFact)> {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if let Some(function) = file.ast.function_at(callee) {
            return Some((file, function));
        }
        let lookup = self.ctx.semantic_lookup;
        let (target, function) = if let Some(member) = file.ast.member_at(callee) {
            if file.ast.computed_members.contains(&member.span) {
                return None;
            }
            lookup.closed_member_function(file, member)?
        } else {
            lookup.function_called_at(file.path.as_str(), callee)?
        };
        let target = *self.files.get(target.path.as_str())?;
        target
            .ast
            .functions
            .iter()
            .find(|candidate| candidate.span == function.span)
            .map(|function| (target, function))
    }

    /// ADR 0820: a generator a member calls, there or in a function it calls.
    /// `for await` steps its iterator from the member's continuation, so an
    /// async generator is a member of the flight: reads after its first
    /// `await` follow the proof that holds for an `async` helper (a `for
    /// await` written in the member) and reads after its first `yield` are
    /// obligations. Any other stepping (`it.next()`, a value handed on, a loop
    /// in a helper) makes the whole member an obligation. A sync generator
    /// stepped across one of the member's suspensions runs past its first
    /// `yield` on the continuation; a loop that drains it on one stack does not.
    fn follow_generators(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &'f FileFacts,
        function: &'f FunctionFact,
    ) {
        if !self.has_generators {
            return;
        }
        walk.carried = Carried::Held;
        let calls = self.generator_calls(file, function);
        let suspends = file
            .ast
            .awaits
            .iter()
            .chain(&file.ast.implicit_suspensions)
            .any(|point| {
                function.body.contains(*point) && written_directly_in(&file.ast, function, *point)
            });
        for (in_file, holder, call, target_file, target) in calls {
            let direct = holder.span == function.span
                && written_directly_in(&in_file.ast, holder, call.span);
            if target.r#async {
                let leading = leading_yield(target_file, target);
                let exact = direct
                    && !leading
                    && for_await_operand(in_file, call)
                    && self
                        .ctx
                        .semantic_lookup
                        .function_value_is_current(target_file, target);
                let tracking = std::mem::replace(&mut walk.untracked, leading);
                self.enqueue(
                    walk,
                    queue,
                    target_file,
                    target,
                    (!exact).then_some(AfterAwaitObligation::GeneratorStepped),
                );
                walk.untracked = tracking;
            } else if suspends
                && !(direct && drained_in_place(in_file, holder, call))
                && let Some(boundary) = first_yield_end(target_file, target)
            {
                let mark = draft.static_defects.len();
                let mut past = target.clone();
                past.body = Span::new(boundary, target.body.end);
                self.report_region(draft, walk, target_file, &past, 0);
                // ADR 0850: a function the body hands on (a member's value, a
                // slot, a forwarded callback) may run past the `yield` too.
                self.report_unplaced_functions(draft, walk, target_file, target);
                mark_uncertain(draft, mark, &AfterAwaitObligation::GeneratorStepped);
            }
        }
    }

    /// The generators `function` calls and keeps the iterator of, there or in
    /// a function it calls exactly, each call with the function it is in.
    fn generator_calls(
        &self,
        file: &'f FileFacts,
        function: &'f FunctionFact,
    ) -> Vec<GeneratorCall<'f>> {
        let key = (file.path.as_str(), function.span);
        if let Some(calls) = self.generator_calls.borrow().get(&key) {
            return calls.clone();
        }
        let calls = self.generator_calls_uncached(file, function);
        self.generator_calls.borrow_mut().insert(key, calls.clone());
        calls
    }

    fn generator_calls_uncached(
        &self,
        file: &'f FileFacts,
        function: &'f FunctionFact,
    ) -> Vec<GeneratorCall<'f>> {
        let mut calls = Vec::new();
        let mut seen = HashSet::default();
        let mut stack = vec![(file, function)];
        while let Some((in_file, holder)) = stack.pop() {
            for call in in_file.ast.calls_within(holder.body) {
                if call.construct {
                    continue;
                }
                let Some((target_file, target)) = self.called_function(in_file, call) else {
                    continue;
                };
                if !target.generator {
                    if seen.insert((target_file.path.as_str(), target.span)) {
                        stack.push((target_file, target));
                    }
                } else if !call.result_discarded {
                    calls.push((in_file, holder, call, target_file, target));
                }
            }
        }
        calls
    }

    /// Whether a member's walk has a generator to follow: it is one with a
    /// `yield`, or it reaches a call of one.
    fn steps_generator(&self, function: &AsyncFunctionFact) -> bool {
        self.has_generators
            && ast_function(self.files, function).is_some_and(|(file, function)| {
                first_yield_end(file, function).is_some()
                    || !self.generator_calls(file, function).is_empty()
            })
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
        if !walk.visited.insert((file.path.shared(), call.span)) {
            return;
        }
        if depth > MAX_REGION_DEPTH {
            // ADR 0750: past the depth the walk follows.
            let mark = draft.static_defects.len();
            self.push_unfollowed(draft, walk, file, call, call.callee);
            for defect in &mut draft.static_defects[mark..] {
                defect.analysis_context = crate::blockers::with_detail(
                    crate::blockers::SEARCH_DEPTH_LIMIT,
                    &defect.analysis_context,
                );
            }
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
            && (!function.generator
                || crate::owners::generator_iterator_used(file, function)
                    .is_some_and(|created| created.span == call.span))
        {
            // ADR 0516: calling a generator runs none of its body; stepping
            // the iterator it returns runs it on this stack, which nothing
            // follows: its reads are obligations.
            let mark = draft.static_defects.len();
            self.report_prefix(draft, walk, file, function, depth);
            if function.generator {
                mark_uncertain(draft, mark, &AfterAwaitObligation::GeneratorStepped);
            }
            return;
        }
        // ADR 0944: a callback of a primitive the dialect says runs in place runs on this stack, past the
        // suspension, as an IIFE's does: asked of the dialect, not of the primitive's name.
        if !call.construct
            && let Some(primitive) = self.primitive(file, call)
            && let Some((index, callback_file, callback)) = call
                .arguments
                .iter()
                .enumerate()
                .take_while(|(_, argument)| !argument.spread)
                .find_map(|(index, argument)| {
                    matches!(
                        ctx.dialect.slot_run(primitive, index, call.arguments.len()),
                        SlotRun::InPlace(_)
                    )
                    .then(|| {
                        callback_argument_literal(file, argument.span)
                            .map(|callback| (file, callback))
                            .or_else(|| {
                                ctx.semantic_lookup
                                    .function_called_at(
                                        file.path.as_str(),
                                        file.ast.peel_ts_sugar_span(argument.span),
                                    )
                                    .filter(|(f, fun)| {
                                        ctx.semantic_lookup.function_value_is_current(f, fun)
                                    })
                            })
                            .map(|(callback_file, callback)| (index, callback_file, callback))
                    })
                    .flatten()
                })
            && !callback.generator
            && let SlotRun::InPlace(run) =
                ctx.dialect.slot_run(primitive, index, call.arguments.len())
        {
            let via = callee_text(file, call.callee);
            match run.owner {
                // `runWithOwner(owner, fn)` runs `fn` at once with `owner` as the context and tracking off
                // (`dev-shared.js` `runWithOwner`), and `checkPostAwaitRead` returns only for a context that
                // is not `null`: a literal `null` owner (the detached-root spelling) keeps the read a
                // warning, `undefined` silences it, and an owner the analysis cannot see through may be
                // either.
                SlotOwner::Supplied { argument } => {
                    let owner = call.arguments.get(argument).map(|owner| owner.value);
                    let mark = draft.static_defects.len();
                    if owner != Some(ArgumentValueKind::Undefined) {
                        self.report_prefix(draft, walk, callback_file, callback, depth);
                    }
                    if !matches!(
                        owner,
                        Some(ArgumentValueKind::Null | ArgumentValueKind::Undefined)
                    ) {
                        mark_uncertain(draft, mark, &AfterAwaitObligation::OwnerUnresolved);
                    }
                }
                // The callback adds no owner and the continuation has none: the read is as untracked as one
                // written here, and a thrown error needs no word (a read is no throw). A callback that keeps
                // the observer (`latest`, `isPending`, `flush(fn)`) runs on this stack under none, so its
                // reads are the continuation's; one that clears tracking (`untrack`) is the documented way to
                // read here on purpose, so what it reads is no defect; an observer the dialect does not
                // state is nothing claimed either way, an obligation.
                SlotOwner::Inherits => match run.observer {
                    SlotObserver::Keeps => {
                        self.report_prefix(draft, walk, callback_file, callback, depth);
                    }
                    SlotObserver::Clears => {}
                    SlotObserver::Unstated => self.report_conditional_region(
                        draft,
                        walk,
                        callback_file,
                        callback,
                        depth,
                        &AfterAwaitObligation::HandOffNotFollowed { via: Some(via) },
                    ),
                },
                // A root's callback runs under an owner the call creates, and the guard returns for a context
                // that is not `null`: the runtime stays silent (`createRoot` after an await, measured).
                SlotOwner::Creates => {}
                // No owner, whatever the continuation's: whether the read warns is not stated here.
                SlotOwner::Detached => self.report_conditional_region(
                    draft,
                    walk,
                    callback_file,
                    callback,
                    depth,
                    &AfterAwaitObligation::HandOffNotFollowed { via: Some(via) },
                ),
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
                continue;
            }
            // A literal handed on through wrappers (`hc1(() => read())` over
            // `const hc1 = (g) => { hc0(g); }`): an exact chain that ends in a
            // direct call of the parameter runs it here. Anything else may run
            // it: an obligation, at its reads, never silence and never a name.
            if self.handed_to_project_code(file, call) {
                if self.forward_runs(
                    file,
                    call,
                    argument.span,
                    MAX_FORWARDS,
                    &mut HashSet::default(),
                ) {
                    self.report_prefix(draft, walk, file, callback, depth);
                } else {
                    self.report_conditional_region(
                        draft,
                        walk,
                        file,
                        callback,
                        depth,
                        &AfterAwaitObligation::HandOffNotFollowed { via: None },
                    );
                }
                in_place = true;
            }
        }
        // ADR 0565: a function named whole at an argument that an exact
        // same-file project function, called by its plain name, calls on every
        // completion of its own synchronous body (`run(hf)` over `const run =
        // (g) => { g(); }`) runs on this call's stack, as a literal there does
        // (ADR 0485). A wrapper in another module or reached through a member,
        // or one that calls it on some paths only, is ADR 0565's obligation.
        if file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .is_none()
        {
            for (index, argument) in call.arguments.iter().enumerate() {
                if argument.spread || argument.value != ArgumentValueKind::Identifier {
                    continue;
                }
                let Some((function_file, function)) = self.named_function(file, argument.span)
                else {
                    continue;
                };
                if function.generator {
                    continue;
                }
                let same_file_wrapper = crate::execution_role::project_callee_at_parameter(
                    file,
                    call,
                    index,
                    ctx.semantic_lookup,
                )
                .is_some_and(|(callee_file, callee, exact)| {
                    exact && callee_file.path == file.path && callee.method_name.is_none()
                });
                let forwarded = self.forward_runs(
                    file,
                    call,
                    argument.span,
                    MAX_FORWARDS,
                    &mut HashSet::default(),
                );
                if forwarded
                    || (same_file_wrapper
                        && crate::execution_role::helper_argument_run(
                            file,
                            call,
                            index,
                            ctx.semantic_lookup,
                        )
                        .is_some_and(|run| run.every_completion))
                {
                    self.report_prefix(draft, walk, function_file, function, depth);
                    in_place = true;
                }
            }
        }
        if in_place {
            return;
        }
        let Some(resolved) = ctx.semantic_lookup.resolved_callee_call(file, call.callee) else {
            return;
        };
        // ADR 0537: `f.call(…)` and `f.apply(…)` run `f` before they return
        // (ADR 0531's carrier), on this stack: an exact project function's
        // prefix is the continuation's, as an IIFE's is.
        if let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee))
            && crate::indexes::standard_library_declaration_of(
                Some(resolved),
                typefacts::CallKind::Call,
            )
            .is_some_and(|declaration| {
                matches!(
                    declaration.qualified_name.as_ref(),
                    "Function.call"
                        | "Function.apply"
                        | "CallableFunction.call"
                        | "CallableFunction.apply"
                        | "NewableFunction.call"
                        | "NewableFunction.apply"
                )
            })
            && crate::local_access::dispatch_invokes(file, call)
            && let Some((function_file, function)) = self.named_function(file, member.object)
        {
            if !function.generator {
                self.report_prefix(draft, walk, function_file, function, depth);
            }
            return;
        }
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
            // ADR 0537: a literal, or an exact project function named here
            // (`[1].forEach(hf)`, `Reflect.apply(hf, …)`), whose prefix runs
            // on this stack as the literal's does.
            let named = callback_argument_literal(file, argument.span)
                .map(|callback| (file, callback))
                .or_else(|| {
                    (argument.value == ArgumentValueKind::Identifier)
                        .then(|| self.named_function(file, argument.span))
                        .flatten()
                });
            match named {
                // Whether these run depends on the contents: a comparator, and
                // a reducer without an initial value, are never called for
                // fewer than two elements, a replacer when nothing matches. A
                // read in one is a proof obligation.
                Some((file, callback))
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
                    let carrier = AfterAwaitObligation::CarrierMayNotRun {
                        carrier: callee_text(file, call.callee),
                    };
                    self.report_conditional_region(draft, walk, file, callback, depth, &carrier);
                }
                // ADR 0566 (ADR 0563's predicate): an inline row is a timing
                // claim, not a count. Where the carrier may run the callback
                // zero times (`[].forEach(hf)`, `props.xs.forEach(hf)`, a
                // `Set`), a read in it is an obligation, never a violation.
                Some((callback_file, callback))
                    if !callback.generator
                        && !crate::carrier_runs::carrier_provably_runs(
                            file,
                            call.span,
                            ctx.semantic_lookup,
                        ) =>
                {
                    let carrier = AfterAwaitObligation::CarrierMayNotRun {
                        carrier: callee_text(file, call.callee),
                    };
                    self.report_conditional_region(
                        draft,
                        walk,
                        callback_file,
                        callback,
                        depth,
                        &carrier,
                    );
                }
                Some((file, callback)) if !callback.generator => {
                    self.report_prefix(draft, walk, file, callback, depth);
                }
                // A literal queued as a member of the promise chain reports
                // for itself; anything else the body of which cannot be
                // inspected stays an obligation, never a silent clean.
                Some((file, callback))
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
                    // A callee whose body cannot be read.
                    analysis_context: crate::blockers::with_detail(
                        crate::blockers::FUNCTION_CALL_TARGET_UNRESOLVED,
                        &walk.member.context,
                    ),
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
        if call.construct {
            return;
        }
        // ADR 0695: past the bound the chain is unread, and no silence.
        if depth > MAX_REGION_DEPTH {
            self.push_search_limit(draft, walk, file, call);
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
            let Some((helper_file, helper)) = self.closed_member(file, member) else {
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
        // ADR 0820: an iterator stepped past this suspension runs the
        // generator's first segment here, and nothing places the stepping.
        if helper.generator {
            if !call.result_discarded
                && let Some(helper_file) = self.files.get(helper_file.path.as_str()).copied()
                && walk.stack_helpers.insert((
                    helper_file.path.to_string(),
                    u64::from(helper.span.start),
                    u64::from(helper.span.end),
                ))
            {
                let mark = draft.static_defects.len();
                self.report_prefix(draft, walk, helper_file, helper, depth);
                mark_uncertain(draft, mark, &AfterAwaitObligation::GeneratorStepped);
            }
            return;
        }
        if !member
            && (helper.method_name.is_some()
                || !ctx
                    .semantic_lookup
                    .function_value_is_current(helper_file, helper))
        {
            return;
        }
        if !walk.stack_helpers.insert((
            helper_file.path.to_string(),
            u64::from(helper.span.start),
            u64::from(helper.span.end),
        )) {
            return;
        }
        // ADR 0614: the helper's store and prop reads run on this stack too.
        // Its body is past the suspension, up to its own first await.
        if ctx.dialect.reports_member_reads_after_await()
            && synchronous_prefix(helper_file, helper).end == helper.body.end
            && walk.member_helpers.insert((
                helper_file.path.to_string(),
                u64::from(helper.span.start),
                u64::from(helper.span.end),
            ))
        {
            member_reads_after_await(
                ctx,
                draft,
                (helper_file, helper, helper.body.start),
                &walk.member.context,
                &|symbol, text| self.pre.store_key(&walk.member.windows, symbol, text),
                self.longer_path_objects,
            );
        }
        // A read in the helper's prefix, behind a branch or a loop too, runs on
        // this stack, past the suspension, if it runs at all: "wrong if it
        // runs", as a read written in the member's own continuation is.
        let Some(helper_file) = self.files.get(helper_file.path.as_str()).copied() else {
            return;
        };
        self.report_prefix(draft, walk, helper_file, helper, depth + 1);
    }

    /// Async functions execute their prefix immediately too. Their tail is
    /// handled by the promise-flight walk, never by this stack walk.
    fn report_prefix(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        function: &FunctionFact,
        depth: usize,
    ) {
        walk.placed.insert((file.path.to_string(), function.span));
        let mut prefix = function.clone();
        prefix.body = synchronous_prefix(file, function);
        self.report_region(draft, walk, file, &prefix, depth);
    }

    /// ADR 0565: a function the walk did not place (an argument it follows,
    /// a carrier, a helper call) and that is handed where nothing here follows
    /// it -- a project or unresolved call's whole argument, a member's value,
    /// an object literal's slot, a method, a copy (`run(hf)`, `ho.m = () =>
    /// …`, `{ m: hf }`) -- may run synchronously on the continuation. It may run there
    /// when it is written in the member after one of its suspensions (it exists
    /// only on the continuation), when a reference hands it on after one, or,
    /// as a member's value, a slot or a method, when a suspension follows it. Its reads
    /// are then proof obligations, never a silent clean. A function only
    /// called directly, scheduled on a fresh stack, kept by a host, returned
    /// by the member or never referenced is left to the arms that place it.
    /// Outer functions go first, so a function an obligated one runs is placed
    /// through it.
    fn report_unplaced_functions(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        function: &'f FunctionFact,
    ) {
        use crate::local_access::HandOff;
        let lookup = self.ctx.semantic_lookup;
        let ast = &file.ast;
        let loop_start = loop_suspension_start(file, function);
        let suspensions = ast
            .awaits
            .iter()
            .chain(ast.implicit_suspensions.iter())
            .filter(|suspension| written_directly_in(ast, function, **suspension))
            .copied()
            .collect::<Vec<_>>();
        // ADR 0850: a generator gives up the stack at its first `yield`, and
        // its body past it runs on whoever steps the iterator next, as an
        // `async` function's tail runs past an `await`.
        let yield_end = first_yield_end(file, function);
        if suspensions.is_empty() && loop_start.is_none() && yield_end.is_none() {
            return;
        }
        let suspended_before = |at: u32| {
            loop_start.is_some_and(|start| at >= start)
                || yield_end.is_some_and(|end| end <= at)
                || suspensions.iter().any(|suspension| suspension.end <= at)
        };
        let suspends_after = |at: u32| {
            loop_start.is_some()
                || yield_end.is_some_and(|end| end > at)
                || suspensions.iter().any(|suspension| suspension.end > at)
        };
        // A binding the member returns (`return later`) resolves the
        // computation: it runs, if ever, after the continuation has ended.
        let returned = |at: Span| {
            ast.returns.iter().any(|returned| {
                returned
                    .argument
                    .is_some_and(|argument| ast.peel_ts_sugar_span(argument) == at)
                    && written_directly_in(ast, function, returned.span)
            })
        };
        let mut nested = ast
            .functions_within(function.body)
            .filter(|nested| nested.span != function.span)
            .collect::<Vec<_>>();
        nested.sort_by_key(|nested| (nested.span.start, std::cmp::Reverse(nested.span.end)));
        for candidate in nested {
            let key = (file.path.to_string(), candidate.span);
            if walk.placed.contains(&key) || walk.queued.contains(&key) {
                continue;
            }
            // The tail of an `async` function (or a generator's body) between
            // the member and the candidate runs off this continuation: the
            // promise-flight walk owns it.
            if ast.functions_within(function.body).any(|outer| {
                outer.span != function.span
                    && outer.span != candidate.span
                    && outer.span.contains(candidate.span)
                    && (outer.r#async || outer.generator)
            }) {
                continue;
            }
            let created_after = suspended_before(candidate.span.start);
            // A method (a class's, an object literal's, an accessor, a
            // constructor) runs wherever its member is reached, as a member's
            // value does.
            // The hand-off, when there is one: `Some(via)`, `via` the callee text of
            // the call that takes the function (ADR 0595), none for a member, an
            // object slot, a method or a copy.
            let taking_call = |at: Span| {
                ast.arguments_containing(at)
                    .find(|(call, index)| ast.peel_ts_sugar_span(call.arguments[*index].span) == at)
                    .map(|(call, _)| callee_text(file, call.callee))
            };
            let unplaced = if candidate.method_name.is_some()
                || crate::local_access::member_assigned(file, candidate)
                || crate::local_access::object_slot_value(file, candidate)
            {
                (created_after || suspends_after(candidate.span.end)).then_some(None)
            } else if let Some(references) =
                crate::local_access::function_reference_hand_offs(file, candidate, lookup)
            {
                // A copy or a whole argument of a call nothing follows.
                references
                    .into_iter()
                    .find(|(at, hand_off)| {
                        *hand_off == HandOff::Unfollowed
                            && !returned(*at)
                            && (created_after || suspended_before(at.start))
                    })
                    .map(|(at, _)| taking_call(at))
            } else {
                // A literal that is the whole argument of such a call. A
                // primitive's callback is the primitive's (its own owner and
                // tracking).
                (created_after
                    && ast
                        .arguments_containing(candidate.span)
                        .any(|(call, index)| {
                            !call.arguments[index].spread
                                && ast.peel_ts_sugar_span(call.arguments[index].span)
                                    == candidate.span
                                && lookup.primitive_at_call(file, call.span).is_none()
                        })
                    && crate::local_access::hand_off_at(file, candidate.span, lookup)
                        == HandOff::Unfollowed)
                    .then(|| taking_call(candidate.span))
            };
            if let Some(via) = unplaced {
                self.report_conditional_region(
                    draft,
                    walk,
                    file,
                    candidate,
                    0,
                    &AfterAwaitObligation::HandOffNotFollowed { via },
                );
            }
        }
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
                self.longer_path_objects,
            );
        }
        for call in file.ast.calls_within(callback.body) {
            if !written_directly_in(&file.ast, callback, call.span) {
                continue;
            }
            self.report_call_read(draft, walk, file, call);
            self.visit_call(draft, walk, file, call, depth + 1);
        }
        for template in &file.ast.tagged_templates {
            if callback.body.contains(template.span)
                && written_directly_in(&file.ast, callback, template.span)
            {
                self.visit_template(draft, walk, file, template, depth + 1);
            }
        }
    }

    /// ADR 0537: a tagged template calls its tag where it is written (ADR
    /// 0531): an exact project function's (or a literal tag's) prefix runs
    /// on this stack, as a call's does. Any other tag is not followed, as a
    /// call of it is not.
    fn visit_template(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        file: &'f FileFacts,
        template: &TaggedTemplateFact,
        depth: usize,
    ) {
        if !walk.visited.insert((file.path.shared(), template.span)) {
            return;
        }
        let tag = file.ast.peel_ts_sugar_span(template.tag);
        let Some((tag_file, function)) = callback_argument_literal(file, tag)
            .map(|function| (file, function))
            .or_else(|| self.named_function(file, tag))
        else {
            // Batch 10c (B1): a tag the walk does not enter is a value the program stored (`const t = tags[0]; t\`x\``), called on this
            // stack: an obligation at the tag, never silence.
            if unresolved_tag(self.ctx, file, template)
                && walk.invoked.insert((
                    file.path.to_string(),
                    u64::from(tag.start),
                    u64::from(tag.end),
                ))
            {
                draft.push_defect(unresolved_obligation_at(file, tag, template.tag));
            }
            return;
        };
        if function.generator {
            return;
        }
        if depth > MAX_REGION_DEPTH {
            draft.push_defect(StaticDefect {
                kind: StaticDefectKind::ReactiveCallbackUnresolved {
                    callee: file.source_text(tag).unwrap_or("this tag").to_owned(),
                },
                location: location(file.path.shared(), tag),
                // ADR 0750: past the depth the walk follows.
                analysis_context: crate::blockers::with_detail(
                    crate::blockers::SEARCH_DEPTH_LIMIT,
                    &walk.member.context,
                ),
                fixes: vec![],
                uncertain: false,
            });
            return;
        }
        self.report_prefix(draft, walk, tag_file, function, depth);
    }

    /// ADR 0537: the exact, current project function the identifier at
    /// `span` names (a carrier's argument, a `call`/`apply` receiver, a
    /// template's tag), through the exact reference resolution.
    /// A call whose callee is no primitive and no standard declaration: what
    /// it does with a function is the project's, or a dependency's.
    fn handed_to_project_code(&self, file: &FileFacts, call: &CallFact) -> bool {
        let lookup = self.ctx.semantic_lookup;
        lookup.primitive_at_call(file, call.span).is_none()
            && crate::indexes::standard_library_declaration_of(
                lookup.resolved_callee_call(file, call.callee),
                typefacts::CallKind::Call,
            )
            .is_none()
    }

    /// The whole argument at `argument` of an exact project function ends, at
    /// the end of a chain of whole-argument hand-ons each on every completion,
    /// in a direct call of the parameter holding it, on every completion too. A wrapper's parameter is asked once: a chain that
    /// fans out or recurses (`hop(g); hop(g);`) would otherwise double per hop.
    fn forward_runs(
        &self,
        file: &FileFacts,
        call: &CallFact,
        argument: Span,
        depth: usize,
        seen: &mut HashSet<(String, Span, usize)>,
    ) -> bool {
        let lookup = self.ctx.semantic_lookup;
        let Some(depth) = depth.checked_sub(1) else {
            return false;
        };
        let Some(index) = call.arguments.iter().position(|candidate| {
            !candidate.spread
                && file.ast.peel_ts_sugar_span(candidate.span)
                    == file.ast.peel_ts_sugar_span(argument)
        }) else {
            return false;
        };
        let Some((wrapper_file, wrapper, true)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, lookup)
        else {
            return false;
        };
        if !seen.insert((wrapper_file.path.to_string(), wrapper.span, index)) {
            return false;
        }
        let Some(parameter) = wrapper.parameters.get(index) else {
            return false;
        };
        let (solid_facts::ast::BindingShape::Identifier, Some(name)) =
            (parameter.shape, parameter.names.first())
        else {
            return false;
        };
        if crate::indexes::binding_written(wrapper_file, name.span) {
            return false;
        }
        // On the wrapper's own stack: not in a function written in it (a timer's,
        // a handler's, one it returns), not past a suspension of its own.
        let prefix = synchronous_prefix(wrapper_file, wrapper);
        wrapper_file
            .ast
            .reference_identifiers_to(name.span)
            .any(|reference| {
                let at = reference.span;
                if !prefix.contains(at) || !written_directly_in(&wrapper_file.ast, wrapper, at) {
                    return false;
                }
                // On every completion of the wrapper: `if (on) g()` under
                // `run(hf, false)` is a call no run reaches.
                let always = |next: &CallFact| {
                    crate::interproc::site_on_every_completion(
                        lookup,
                        wrapper_file,
                        wrapper.span,
                        next.span,
                    )
                };
                wrapper_file.ast.calls.iter().any(|next| {
                    !next.construct
                        && wrapper_file.ast.peel_ts_sugar_span(next.callee) == at
                        && always(next)
                }) || wrapper_file
                    .ast
                    .arguments_containing(at)
                    .find(|(next, index)| {
                        wrapper_file
                            .ast
                            .peel_ts_sugar_span(next.arguments[*index].span)
                            == at
                    })
                    .is_some_and(|(next, _)| {
                        always(next) && self.forward_runs(wrapper_file, next, at, depth, seen)
                    })
            })
    }

    /// The function a closed literal's member holds: written there, or named.
    fn closed_member(
        &self,
        file: &FileFacts,
        member: &solid_facts::ast::MemberFact,
    ) -> Option<(&'f FileFacts, &'f FunctionFact)> {
        let lookup = self.ctx.semantic_lookup;
        lookup
            .closed_member_function(file, member)
            .or_else(|| self.named_function(file, lookup.closed_member_value(file, member)?))
    }

    fn named_function(
        &self,
        file: &FileFacts,
        span: Span,
    ) -> Option<(&'f FileFacts, &'f FunctionFact)> {
        let lookup = self.ctx.semantic_lookup;
        let span = file.ast.peel_ts_sugar_span(span);
        if !crate::owners::is_identifier_span(file, span) {
            return None;
        }
        lookup
            .function_called_at(file.path.as_str(), span)
            .or_else(|| {
                let (_, _, symbol) = lookup.binding_at_reference(file.path.as_str(), span)?;
                lookup.function_for_symbol(&symbol)
            })
            .filter(|(function_file, function)| {
                lookup.function_value_is_current(function_file, function)
            })
    }

    /// ADR 0537: a call past the region bound: what it runs is not followed,
    /// so it is an obligation.
    /// ADR 0750: `call` continues a chain past [`MAX_DEPTH`]: an obligation
    /// at its callee, named by the bound.
    fn push_search_limit(
        &self,
        draft: &mut ProgramDraft,
        walk: &Walk<'_>,
        file: &FileFacts,
        call: &CallFact,
    ) {
        let mark = draft.static_defects.len();
        self.push_unfollowed(draft, walk, file, call, call.callee);
        for defect in &mut draft.static_defects[mark..] {
            defect.analysis_context = crate::blockers::SEARCH_DEPTH_LIMIT.into();
        }
    }

    fn push_unfollowed(
        &self,
        draft: &mut ProgramDraft,
        walk: &Walk<'_>,
        file: &FileFacts,
        call: &CallFact,
        at: Span,
    ) {
        draft.push_defect(StaticDefect {
            kind: StaticDefectKind::ReactiveCallbackUnresolved {
                callee: call
                    .static_callee(&file.source)
                    .unwrap_or("this callee")
                    .to_owned(),
            },
            location: location(file.path.shared(), at),
            analysis_context: walk.member.context.clone(),
            fixes: vec![],
            uncertain: false,
        });
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
        reason: &AfterAwaitObligation,
    ) {
        let mark = draft.static_defects.len();
        self.report_prefix(draft, walk, file, callback, depth);
        mark_uncertain(draft, mark, reason);
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
        let key = (
            file.path.to_string(),
            u64::from(call.callee.start),
            u64::from(call.callee.end),
        );
        // A composite callee names no symbol (ADR 0898): it is read through its
        // operands below (ADR 0897).
        let symbol = call_accessor_symbol(ctx, file, call);
        // ADR 0516: as on the dominance path (ADR 0509), a callee that is a
        // value a caller supplied (a prop, a context or hook member) may be an
        // accessor: after an implicit suspension its read is an obligation too.
        let handed = |walk: &mut Walk<'_>, draft: &mut ProgramDraft| {
            if symbol.is_none_or(|symbol| {
                !ctx.setters.contains_key(symbol) && !ctx.actions.contains_key(symbol)
            }) && crate::local_access::handed_value(
                &crate::local_access::HandedTables {
                    lookup: ctx.semantic_lookup,
                    entities: ctx.entities,
                    prop_sources: crate::late_chunks::Noted::new(
                        crate::late_chunks::TableId::PropSources,
                        ctx.prop_sources,
                    ),
                    source_kinds: crate::late_chunks::Noted::new(
                        crate::late_chunks::TableId::SourceKinds,
                        ctx.source_kinds,
                    ),
                    accessors: crate::late_chunks::Noted::new(
                        crate::late_chunks::TableId::Accessors,
                        ctx.accessors,
                    ),
                    setters: crate::late_chunks::Noted::new(
                        crate::late_chunks::TableId::Setters,
                        ctx.setters,
                    ),
                    actions: crate::late_chunks::Noted::new(
                        crate::late_chunks::TableId::Actions,
                        ctx.actions,
                    ),
                    props_reactivity: crate::late_chunks::NotedProps::new(ctx.props_reactivity),
                    accessors_that_count: None,
                },
                file,
                call.callee,
                crate::local_access::HANDED_DEPTH,
            ) && walk.reported.insert(key.clone())
            {
                let mut obligation = typed_after_await_obligation(file, call.callee);
                obligation.analysis_context = "handed-read-after-await".to_owned();
                draft.push_defect(obligation);
            }
        };
        // ADR 0897: a comma, conditional, logical or assignment callee
        // (`(slot = r)()`) names no symbol. On this placed stack it reads the
        // accessor its operands name, as the member's continuation does
        // (ADR 0517's arm): exact when total and every operand is a reference
        // to one accessor, which then answers as the symbol path does
        // (`read_obligation` unchanged; the region's placement marks the row as
        // it marks that path's). Any other mix with an accessor operand is an
        // obligation at the read (review D31: never a tracked first operand's
        // silence).
        if symbol.is_none() {
            let references = file.ast.composite_callee_references(call.callee);
            let accessors = references
                .iter()
                .filter_map(|operand| ctx.entities.at(file.path.as_str(), *operand))
                .filter(|symbol| ctx.accessors.contains_key(*symbol))
                .collect::<Vec<_>>();
            if let Some(&accessor) = accessors.first() {
                let same = accessors.len() == references.len()
                    && references.len() == file.ast.composite_callee_operands(call.callee).len()
                    && accessors.iter().all(|other| *other == accessor)
                    && file.ast.composite_callee_total(call.callee);
                let tracking = self.tracking(walk.member, accessor);
                if same && tracking == Tracking::Tracked {
                    return;
                }
                if walk.reported.insert(key.clone()) {
                    let obligation = if same {
                        read_obligation(ctx, Some(accessor), tracking)
                    } else {
                        Some(AfterAwaitObligation::CalleeInexact)
                    };
                    let name = ctx
                        .accessors
                        .get(accessor)
                        .map_or_else(|| accessor.to_string(), |(name, _)| name.to_string());
                    let uncertain = obligation.is_some();
                    draft.push_defect(StaticDefect {
                        kind: StaticDefectKind::ReactiveReadAfterAwait {
                            accessor: name,
                            obligation,
                        },
                        location: location(
                            file.path.shared(),
                            if same {
                                references[references.len() - 1]
                            } else {
                                call.callee
                            },
                        ),
                        analysis_context: walk.member.context.clone(),
                        fixes: vec![],
                        uncertain,
                    });
                }
                return;
            }
        }
        let Some(symbol) = symbol else {
            handed(walk, draft);
            if inexact_callee(ctx, file, call) && walk.reported.insert(key.clone()) {
                draft.push_defect(inexact_callee_obligation(file, call, &walk.member.context));
            } else if (unresolved_invocation_known(ctx, file, call)
                && walk.reported.insert(key.clone()))
                || (unresolved_invocation_widened(ctx, file, call)
                    && !walk.reported.contains(&key)
                    && walk.invoked.insert(key))
            {
                // The rows of the known classes claim the callee, as they always did; the widened ones only add.
                draft.push_defect(unresolved_invocation_obligation(
                    file,
                    call,
                    &walk.member.context,
                ));
            }
            return;
        };
        let Some((name, _)) = ctx.accessors.get(symbol) else {
            if typed_unknown(ctx, symbol) && walk.reported.insert(key.clone()) {
                draft.push_defect(typed_after_await_obligation(file, call.callee));
            } else {
                handed(walk, draft);
                if (unresolved_invocation_known(ctx, file, call)
                    && walk.reported.insert(key.clone()))
                    || (unresolved_invocation_widened(ctx, file, call)
                        && !walk.reported.contains(&key)
                        && walk.invoked.insert(key))
                {
                    draft.push_defect(unresolved_invocation_obligation(
                        file,
                        call,
                        &walk.member.context,
                    ));
                }
            }
            return;
        };
        let tracking = self.tracking(walk.member, symbol);
        if tracking == Tracking::Tracked || !walk.reported.insert(key) {
            return;
        }
        let display = call.static_callee(&file.source).unwrap_or(name);
        let obligation = read_obligation(ctx, Some(symbol), tracking);
        let uncertain = obligation.is_some();
        draft.push_defect(StaticDefect {
            kind: StaticDefectKind::ReactiveReadAfterAwait {
                accessor: display.to_string(),
                obligation,
            },
            location: location(file.path.shared(), call.callee),
            analysis_context: walk.member.context.clone(),
            fixes: vec![],
            uncertain,
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
        if call.construct {
            return;
        }
        if depth > MAX_DEPTH {
            // ADR 0750: the chain goes on past the nesting limit; what its
            // handlers read after the await is unknown, never clean.
            self.push_search_limit(draft, walk, file, call);
            return;
        }
        // A computed key (`p["then"](h)`, `p[k](h)`): whatever the type says,
        // no arm below reads its handlers. They are obligations, never dropped.
        if file
            .ast
            .computed_members
            .contains(&file.ast.peel_ts_sugar_span(call.callee))
        {
            self.unresolved_chain_handlers(draft, walk, queue, file, call, depth);
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
                    self.enqueue(
                        walk,
                        queue,
                        function_file,
                        function,
                        uncertain.then_some(AfterAwaitObligation::OwnerUnresolved),
                    );
                }
                Some((function_file, function)) if !function.generator => {
                    let mark = draft.static_defects.len();
                    for (inner, _) in chain_calls(function_file, function) {
                        self.expand(draft, walk, queue, function_file, inner, depth + 1);
                    }
                    if uncertain {
                        mark_uncertain(draft, mark, &AfterAwaitObligation::OwnerUnresolved);
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
                    // A callee whose body cannot be read.
                    analysis_context: crate::blockers::with_detail(
                        crate::blockers::FUNCTION_CALL_TARGET_UNRESOLVED,
                        &walk.member.context,
                    ),
                    fixes: vec![],
                    uncertain: false,
                }),
            }
            walk.untracked = previous;
            return;
        }
        if let Some((helper_file, helper)) = async_helper(ctx, file, call) {
            self.enqueue(walk, queue, helper_file, helper, None);
            return;
        }
        // ADR 0695: a sync helper hands on the flight its chain returns
        // (`const h1 = () => h0();`), as `untrack`'s callback does above.
        if let Some((helper_file, helper)) = ctx
            .semantic_lookup
            .function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
            .filter(|(_, helper)| !helper.r#async && !helper.generator)
        {
            for (inner, _) in chain_calls(helper_file, helper) {
                self.expand(draft, walk, queue, helper_file, inner, depth + 1);
            }
            return;
        }
        // ADR 0874: a call the census withholds is followed as the standard
        // one would run its handlers, so their rows stand at their sites; its
        // flight is unproven (as an aggregate's whose premise is withdrawn),
        // so every handler there is an obligation, never a proof.
        let (resolved, withheld) = match ctx.semantic_lookup.resolved_callee_call(file, call.callee)
        {
            Some(resolved) => (resolved, false),
            None => {
                if !ctx
                    .semantic_lookup
                    .standard_call_withheld(file, call.callee)
                {
                    self.unresolved_chain_handlers(draft, walk, queue, file, call, depth);
                    return;
                }
                let Some(resolved) = ctx.semantic_lookup.placing_callee_call(file, call.callee)
                else {
                    self.unresolved_chain_handlers(draft, walk, queue, file, call, depth);
                    return;
                };
                (resolved, true)
            }
        };
        let next = walk.unproven || withheld;
        let previous = std::mem::replace(&mut walk.unproven, next);
        self.expand_standard(draft, walk, queue, file, call, depth, resolved, withheld);
        walk.unproven = previous;
    }

    /// [`Self::expand`]'s standard-library calls: a promise's handlers, and
    /// the aggregates.
    #[allow(clippy::too_many_arguments)]
    fn expand_standard(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &'f FileFacts,
        call: &'f CallFact,
        depth: usize,
        resolved: &typefacts::ResolvedCall,
        withheld: bool,
    ) {
        let Some(name) = standard_call_name(resolved) else {
            self.unresolved_chain_handlers(draft, walk, queue, file, call, depth);
            return;
        };
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        match name {
            "Promise.then" | "Promise.catch" | "Promise.finally" => {
                let Some(receiver) = file.ast.member_at(callee).map(|member| member.object) else {
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
                    // ADR 0750: a receiver whose claim the nesting limit cut
                    // may be native too: its handlers are obligations.
                    let undecided = !native_promise_claim(self.ctx, file, receiver, 0)
                        && native_promise_undecided(self.ctx, file, receiver, 0);
                    // Review R39: a withheld call has no claim to read: its
                    // handlers and its chain stay obligations.
                    if withheld && !undecided && !native_promise_claim(self.ctx, file, receiver, 0)
                    {
                        self.unresolved_chain_handlers(draft, walk, queue, file, call, depth);
                        return;
                    }
                    let mark = draft.static_defects.len();
                    if undecided || native_promise_claim(self.ctx, file, receiver, 0) {
                        for argument in call
                            .arguments
                            .iter()
                            .take(handlers)
                            .filter(|argument| !argument.spread)
                        {
                            if let Some(callback) = callback_argument_literal(file, argument.span)
                                && !callback.generator
                            {
                                self.report_conditional_region(
                                    draft,
                                    walk,
                                    file,
                                    callback,
                                    depth,
                                    &if undecided {
                                        AfterAwaitObligation::HandlerMayNotRun
                                    } else {
                                        AfterAwaitObligation::IntrinsicMayBeReplaced
                                    },
                                );
                            }
                        }
                    }
                    if undecided {
                        for defect in &mut draft.static_defects[mark..] {
                            defect.analysis_context = crate::blockers::SEARCH_DEPTH_LIMIT.into();
                        }
                        let receiver = file.ast.peel_ts_sugar_span(receiver);
                        if let Some(inner) = file.ast.call_at(receiver) {
                            self.expand(draft, walk, queue, file, inner, depth + 1);
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
                    // ADR 0669: a project function handed by name runs as a literal does.
                    let member = file
                        .ast
                        .member_at(file.ast.peel_ts_sugar_span(argument.span))
                        .filter(|member| !file.ast.computed_members.contains(&member.span));
                    let Some((handler_file, callback)) =
                        callback_argument_literal(file, argument.span)
                            .map(|callback| (file, callback))
                            .or_else(|| {
                                (argument.value == ArgumentValueKind::Identifier)
                                    .then(|| self.named_function(file, argument.span))
                                    .flatten()
                            })
                            .or_else(|| member.and_then(|member| self.closed_member(file, member)))
                    else {
                        // A member handler the slot does not resolve exactly
                        // reads on the flight if it runs: an obligation of the
                        // rule, at the call (R41), never a name match.
                        if member.is_some() {
                            let mut obligation = typed_after_await_obligation(file, call.callee);
                            obligation.analysis_context = "handed-read-after-await".to_owned();
                            draft.push_defect(obligation);
                            continue;
                        }
                        // ADR 0880: a handler the walk cannot read (a call's
                        // result, a parameter) may read on the flight. A plain
                        // value is no function: no handler.
                        if !plain_value(file, argument.span) {
                            self.push_unfollowed(draft, walk, file, call, argument.span);
                        }
                        continue;
                    };
                    // ADR 0881: an `async` handler's body runs on the flight
                    // too, its prefix in the handler's microtask; a generator
                    // handler's iterator is the chain's value, and a later
                    // handler may step it on the flight (review D14). Each is
                    // an obligation until that run is placed.
                    if callback.r#async || callback.generator {
                        self.report_conditional_region(
                            draft,
                            walk,
                            handler_file,
                            callback,
                            depth,
                            &AfterAwaitObligation::HandOffNotFollowed {
                                via: Some(
                                    if callback.generator {
                                        "a generator handler"
                                    } else {
                                        "an async handler"
                                    }
                                    .to_owned(),
                                ),
                            },
                        );
                        continue;
                    }
                    // Review (D-R3d-2): an async function the handler calls
                    // returns a promise the handler's joins, so its body past
                    // its own await runs on the flight; its prefix runs in
                    // the handler's microtask. No queue placed it: walk it as
                    // an obligation, with no window of its own.
                    self.enqueue_handler_calls(walk, queue, handler_file, callback);
                    // A rejection handler runs only if the receiver rejects;
                    // a fulfillment handler only if it fulfills. `finally`
                    // runs either way.
                    let rejection_only =
                        name == "Promise.catch" || (name == "Promise.then" && index == 1);
                    // Inside an aggregate whose premise is withdrawn (ADR 0349),
                    // the element's flight is unproven as well.
                    if rejection_only || !fulfills || !on_flight || walk.unproven {
                        let reason = if rejection_only || !fulfills || !on_flight {
                            AfterAwaitObligation::HandlerMayNotRun
                        } else {
                            AfterAwaitObligation::IntrinsicMayBeReplaced
                        };
                        self.report_conditional_region(
                            draft,
                            walk,
                            handler_file,
                            callback,
                            depth,
                            &reason,
                        );
                    } else {
                        self.report_region(draft, walk, handler_file, callback, depth);
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
            // Review R39: `PromiseLike.then` and the rest: a withheld call has
            // handlers the walk must not lose.
            _ if withheld => {
                self.unresolved_chain_handlers(draft, walk, queue, file, call, depth);
            }
            _ => {}
        }
    }

    /// ADR 0882: `.then`/`.catch`/`.finally` on the flight with no exact
    /// standard declaration (a cast, an `any` receiver, a thenable): the
    /// handlers may run on the flight, inline, or never. A function the walk
    /// can read is an obligation region; any other handler is unresolved.
    fn unresolved_chain_handlers(
        &self,
        draft: &mut ProgramDraft,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &'f FileFacts,
        call: &CallFact,
        depth: usize,
    ) {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let Some(member) = file.ast.member_at(callee) else {
            return;
        };
        // A literal key names the member (`p["then"](h)`); any other computed
        // key may name a chain call, and nothing here says which.
        let computed = file.ast.computed_members.contains(&callee);
        let name = if computed {
            file.ast
                .literal_computed_members
                .iter()
                .find(|literal| literal.span == callee)
                .map(|literal| literal.key.as_ref())
        } else {
            file.source_text(member.property)
        };
        let handlers = match name {
            Some("then") => 2,
            Some("catch" | "finally") => 1,
            None if computed => {
                for argument in &call.arguments {
                    if argument.spread || !plain_value(file, argument.span) {
                        self.push_unfollowed(draft, walk, file, call, argument.span);
                    }
                }
                // The receiver's chain is its own, whatever this link is.
                let receiver = file.ast.peel_ts_sugar_span(member.object);
                if let Some(inner) = file.ast.call_at(receiver) {
                    self.expand(draft, walk, queue, file, inner, depth + 1);
                }
                return;
            }
            _ => return,
        };
        for argument in call.arguments.iter().take(handlers) {
            let readable = (!argument.spread)
                .then(|| {
                    callback_argument_literal(file, argument.span)
                        .map(|callback| (file, callback))
                        .or_else(|| {
                            (argument.value == ArgumentValueKind::Identifier)
                                .then(|| self.named_function(file, argument.span))
                                .flatten()
                        })
                })
                .flatten();
            match readable {
                Some((handler_file, callback)) => {
                    self.enqueue_handler_calls(walk, queue, handler_file, callback);
                    self.report_conditional_region(
                        draft,
                        walk,
                        handler_file,
                        callback,
                        depth,
                        &AfterAwaitObligation::HandlerMayNotRun,
                    );
                }
                None if !argument.spread && plain_value(file, argument.span) => {}
                None => self.push_unfollowed(draft, walk, file, call, argument.span),
            }
        }
        let receiver = file.ast.peel_ts_sugar_span(member.object);
        if let Some(inner) = file.ast.call_at(receiver) {
            self.expand(draft, walk, queue, file, inner, depth + 1);
        }
    }

    /// Review (D-R3d-2): each async project function a handler calls, walked
    /// as an obligation with no window of its own (the handler runs off the
    /// computation's stack).
    fn enqueue_handler_calls(
        &self,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &FileFacts,
        callback: &FunctionFact,
    ) {
        for inner in file.ast.calls_within(callback.body) {
            let Some((helper_file, helper)) = async_helper(self.ctx, file, inner) else {
                continue;
            };
            let past = walk.past_suspension;
            walk.past_suspension = true;
            self.enqueue(
                walk,
                queue,
                helper_file,
                helper,
                Some(AfterAwaitObligation::HandOffNotFollowed {
                    via: Some("a promise handler's call".to_owned()),
                }),
            );
            walk.past_suspension = past;
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
        // ADR 0566: a receiver that may be empty runs no helper, so the members
        // `map` adds are conditional (their reads are obligations).
        let unproven_run =
            (!crate::carrier_runs::carrier_provably_runs(file, mapped.span, ctx.semantic_lookup))
                .then(|| AfterAwaitObligation::CarrierMayNotRun {
                    carrier: callee_text(file, mapped.callee),
                });
        if let Some(callback) = callback_argument_literal(file, argument.span) {
            if callback.r#async && !callback.generator {
                self.enqueue(walk, queue, file, callback, unproven_run.clone());
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
            self.enqueue(walk, queue, helper_file, helper, unproven_run);
        }
    }

    /// Adds an `async` function whose promise the computation awaits.
    fn enqueue(
        &self,
        walk: &mut Walk<'_>,
        queue: &mut Vec<Member>,
        file: &FileFacts,
        function: &FunctionFact,
        uncertain: Option<AfterAwaitObligation>,
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
                may_answer_sync: false,
                return_open: false,
                calls_after_await: Vec::new(),
            },
            |fact| (*fact).clone(),
        );
        walk.queued.insert((file.path.to_string(), function.span));
        let fact = Arc::new(fact);
        let mut windows = walk.member.windows.clone();
        // ADR 0872: a helper's own window tracks only while the computation
        // has not suspended: past that its prefix runs with no observer.
        if !walk.untracked && !walk.past_suspension {
            windows.push(Arc::clone(&fact));
        }
        // An `async` function returned without `await` is adopted by its
        // caller one job after the call returns. Its own continuation, queued
        // by an await on a settled value, runs before that: only a first
        // await on a promise proven still pending leaves a reaction in place.
        let conditional = walk
            .member
            .conditional
            .clone()
            .or(uncertain)
            .or_else(|| {
                walk.unproven
                    .then_some(AfterAwaitObligation::IntrinsicMayBeReplaced)
            })
            .or_else(|| {
                (walk.carried == Carried::Adopted && !self.awaits_pending(file, function))
                    .then_some(AfterAwaitObligation::ContinuationMayRunFirst)
            })
            .or_else(|| {
                walk.past_on_some_path
                    .then_some(AfterAwaitObligation::SuspensionUnplaced)
            });
        // ADR 0871's bound is the driver's ([`Self::bounded`]).
        queue.push(Member {
            function: fact,
            context: walk.member.context.clone(),
            windows,
            conditional,
            blocker: walk.member.blocker,
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

/// ADR 0516: the start of the earliest loop written directly in `function`
/// that holds an `await` written directly in `function`: a back edge that
/// suspends (the dominance facts model an `await` by what follows it, not
/// what runs again after it).
fn loop_suspension_start(file: &FileFacts, function: &FunctionFact) -> Option<u32> {
    file.ast
        .loop_statements
        .iter()
        .filter(|lp| function.body.contains(**lp) && written_directly_in(&file.ast, function, **lp))
        .filter(|lp| {
            file.ast.awaits.iter().any(|awaited| {
                lp.contains(*awaited) && written_directly_in(&file.ast, function, *awaited)
            })
        })
        .map(|lp| lp.start)
        .min()
}
