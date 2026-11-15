//! Batch 11, slice 9B (ADR 0973): every read or project call written after an async function's own suspension is accounted for.
//!
//! UNTRACKED_READ_AFTER_AWAIT (SC1002) is raised for a reactive read a continuation runs after one of its async function's own suspensions. Its producers
//! (`static_rules/after_await.rs`) walk from the tracked async computations they find and file a row where they prove or cannot decide; a read or a call they
//! never reach had no row and no record of why, a clean nobody accounted for. This pass lists the sites of the claim from facts alone:
//!
//! - every read of [`crate::proof_universe`]'s `read` table (an accessor call, a store's or props' first `Get`), and
//! - every call that may run a project function, written directly in a function that is `async`, after one of that function's own suspensions
//!   ([`crate::execution_role::may_follow_suspension`]: an `await`, an implicit suspension, a loop that repeats it), outside dependency source:
//!   - `call` and `member`: a project function the walk's own resolver ([`crate::execution_role::project_callee_at_parameter`]) resolves exactly (an identifier
//!     whose name still holds it; a module namespace's member, or a method of a `const` object literal or of an instance of the exact class, ADR 0279);
//!   - `inexact` and `inexact-member`: one it resolves without being exact (a `let` the program reassigns, `this.helper()`, a parameter typed as a class, an
//!     override or a replacement may run) or one it withholds (a method of a literal the receiver census does not prove: ADR 0299);
//!   - `invocation`: a function value the walk cannot resolve (a parameter, a variable, a call's result, a member slot no function body defines; ADR 0964);
//!   - `iife`: a function literal called where it is written (`(() => …)()`, `(async () => …)()`): it runs on this stack;
//!   - `apply`: a standard call that runs a function value its receiver or arguments name (`f.call(…)`, `f.apply(…)`, `Reflect.apply(f, …)`, `Reflect.construct`);
//!   - `project` and `unresolved`: any other call whose callee is no declaration outside the project: a project declaration the resolver does not run exactly
//!     (a computed or optional member of a literal, a method the receiver census withholds), or no declaration at all (no resolved-call fact, an `any`-typed value).
//!
//! Not sites, each named in `docs/open-work.md`: a `.then` continuation or a callback a later microtask runs (the reads in it are the function's own sites), a read
//! in a nested function (that function's own site), a tagged template's tag, and a call of an accessor (a read of the universe, listed as a site of its own). A call into code
//! outside the project is a site (ADR 0976): the argument that a library "runs project code only through what it is handed or a protocol the project's objects implement" was
//! the hole the reviews of #233 found (a callback the library runs here, a listener a synchronous dispatch delivers to, a receiver that is the project's own object under a library
//! type, a vetoed call the symbol fallback let through, a setter's updater, a constructor), so such a call is a site unless the typed fact [`library_call`] holds. A function a
//! reviewed test runner schedules is a function like any other (owner ruling 9b): its sites are sites.
//!
//! Like [`crate::flush_accounting`] (and ADRs 0838 and 0886) this pass keeps its own typed endings and its own universe; it names no
//! [`crate::site_accounting::SiteSilence`] and no premise kind. A site ends in
//!
//! - a row of the SC1002 family that stands at exactly the site's span or one of its alternates (a row of another rule, SC1001's for the same read included, ends
//!   nothing: it claims another code), never a row that covers it from another position;
//! - an SC9012 at the site (a withholding covers any code);
//! - the compiler's discard of the code;
//! - a typed reason ([`AfterAwaitSafe`]): a fact of the program or of the site that rules the claim out, asked of the facts and nothing else;
//!
//! or it gets an uncertifiable row of SC1002, blocker `read-after-await-unaccounted`: never a violation, and it yields to a row that stands. The reasons:
//!
//! - **never runs**: no execution of the program runs the site (`SiteReach`);
//! - **library runs nothing** (ADR 0976, [`library_call`]): a call into the standard library, a setter or an action that, by exact facts alone, runs no project code;
//! - **owner captured** (ADR 0976, R5): a `runWithOwner` call whose owner the function captured, as an unreassigned `const` of `getOwner()`, before its first suspension.
//! - **runner scheduled** (ADR 1004): the site is written directly in a function literal a reviewed test runner schedules (ADR 0438's premise, read from the runners'
//!   published bytes: a registered test or hook, and a suite body the runner defers, is called from the runner's own loop, and a registered `async` callback is wholly
//!   in the premise, its prefix and every resumption). The dialect's runtime raises the code only for a read with an async computation's flight on its async stack, and
//!   the stack of such a callback's continuations is the runner's. The facts asked are the flush accounting's for the same class
//!   ([`crate::flush_accounting::runs_when_the_call_runs`]). A helper the tests call, a callback handed by name or through a wrapper, a suite body a runner runs inside
//!   the registration call and a function written inside the callback are not covered.
//!
//! **No callee reason.** A call of a project function whose closure reads nothing the universe lists is no ending. The first build of this pass had one, and the
//! review of #233 found it unsound: the closure is a list of reads and calls, and a read reaches a continuation through values no call shows (a store read through
//! an alias, a stored generator stepped by a spread, a trap or a getter installed by reference, a props alias). Making it exact asks for facts the checker does not
//! have (which values are plain data), so the call keeps its row; `docs/open-work.md` names the cost and what a sound reason needs.
//!
//! Left out on purpose and named in `docs/open-work.md` (owner ruling, decision 5): a role silence (a handler, a deferred callback), a read the
//! walk's pre-await window already tracked, and a read inside an `untrack` callback: each needs a premise and a Chrome twin of its own. The test callback has both
//! (ADR 1004: ADR 0438's premise, the oracle case `after-await-runner-callback` and the vitest probe).
//!
//! **No census reason.** "The program holds no tracked async computation, so nothing starts a walk" is no ending: the census (`holds_suspending`, ADR 0963) is only as
//! complete as the producers' index of computations, and Chrome shows starts the index does not find (the oracle cases `after-await-universe-census-*`: a function
//! declaration handed by identifier to a project wrapper that gives it to `createMemo`, `createMemo.call`, `Reflect.apply`, an array element). A program the census calls
//! computation-free keeps the row of every site (the fixture `after-await-universe-census`); a reason there needs a proof that no computation runs the function (no
//! value use of it, callers only module-level code or such functions), not the absence of a recognized start.
//!
//! `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>[,<id>...]` (or `all`) drops a reason, so the sites it ends get their row: the mutation test of each. With the proof log
//! on, each reason is a `silent` line of SC1002 (`AfterAwaitSafe::<reason>`) at the site. With `SOLID_CHECKER_IR_PROOF_CANDIDATES` the pass also writes one `candidate`
//! line per site (`read-after-await-universe`) with how the site ends and the facts that tell the sites apart: `via` (the kinds above, and `read`), `overlap` (a row
//! of the family overlaps the site without standing at one of its spans), `test` (the function is a literal a reviewed runner schedules), `never` (`SiteReach`), `host`
//! (the callee that takes the function as an argument), `reason` (the typed reason that ended the site) and `filed` (whether the site got its row).

mod handlers;
mod library_call;
mod member_reads;
mod timer_handle;

use solid_facts::FileFacts;
use solid_facts::ast::ArgumentValueKind;
use solid_facts::core::Span;

use crate::indexes::TestRunnerCallback;
use crate::pipeline::AnalysisContext;
use crate::proof_log::{self, CandidateRecord};
use crate::proof_universe::Site as UniverseSite;
use crate::{AfterAwaitObligation, Program, StaticDefect, StaticDefectFamily, StaticDefectKind};
use crate::{HashMap, HashSet};

const fn yes(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

/// A typed reason a site written after an await needs no row. Closed: each variant is a fact the pass asks of the program or of the site and nothing else.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AfterAwaitSafe {
    /// No execution of the program runs the site (`SiteReach`).
    NeverRuns,
    /// A call into the standard library, a setter or an action that runs no project code ([`library_call`], ADR 0976).
    LibraryRunsNothing,
    /// A `runWithOwner` call whose owner the async function captured, as an unreassigned `const` of `getOwner()`, ahead of every suspension of it (ADR 0976, R5): the callback runs
    /// in the computation's own owner, and Chrome rc.21 raises nothing for a read there.
    OwnerCaptured,
    /// A member read whose object is proven plain data: a primitive from its own expression, a fresh built-in value, an unpatched ECMAScript global (ADR 0976, S4,
    /// [`member_reads::root_is_plain`]).
    PlainReceiver,
    /// The site is written directly in a function literal a reviewed test runner schedules (ADR 1004, on ADR 0438's premise: the runner calls a registered test or hook,
    /// and a suite body it defers, from its own loop, and a registered `async` callback is wholly in the premise, its prefix and every resumption). The dialect's runtime
    /// raises the code only for a read with an async computation's flight on its async stack, and the stack of such a callback and of its continuations is the runner's.
    /// The facts asked are the flush accounting's for the same class ([`crate::flush_accounting::runs_when_the_call_runs`]): the site is in the callback's body and nothing
    /// written between runs it elsewhere, the callback is no generator, and its own name (a named function expression) is used nowhere.
    RunnerScheduled,
}

impl AfterAwaitSafe {
    /// In the order they are asked, the cheapest first.
    const ALL: [Self; 5] = [
        Self::NeverRuns,
        Self::RunnerScheduled,
        Self::LibraryRunsNothing,
        Self::OwnerCaptured,
        Self::PlainReceiver,
    ];

    /// The id the proof log writes, and `SOLID_CHECKER_IR_ABLATE_SILENCE` names (`<module>.<function>.<end>`).
    const fn id(self) -> &'static str {
        match self {
            Self::NeverRuns => "after_await_accounting.account.never-runs",
            Self::LibraryRunsNothing => "after_await_accounting.account.library-runs-nothing",
            Self::OwnerCaptured => "after_await_accounting.account.owner-captured",
            Self::PlainReceiver => "after_await_accounting.account.plain-receiver",
            Self::RunnerScheduled => "after_await_accounting.account.runner-scheduled",
        }
    }

    /// Whether the reason holds at `site`.
    fn holds(
        self,
        ctx: &AnalysisContext<'_>,
        site: &AwaitSite<'_>,
        reach: &crate::execution_role::SiteReach<'_, '_>,
        object_prototype_touched: &dyn Fn() -> bool,
    ) -> bool {
        match self {
            Self::NeverRuns => matches!(
                reach.site(site.file, site.at),
                crate::execution_role::Reach::Unreached(_)
            ),
            Self::LibraryRunsNothing => site.call.is_some_and(|call| {
                library_call::runs_nothing(ctx, site.file, call, site.via, object_prototype_touched)
            }),
            Self::OwnerCaptured => {
                site.via == "primitive"
                    && site.call.is_some_and(|call| {
                        owner_captured_before_the_suspension(ctx, site.file, call)
                    })
            }
            Self::PlainReceiver => {
                site.via == "member-read"
                    && site
                        .root
                        .is_some_and(|root| member_reads::root_is_plain(ctx, site.file, root))
            }
            Self::RunnerScheduled => site.registration.is_some_and(|(registration, read_bytes)| {
                read_bytes
                    && crate::flush_accounting::runs_when_the_call_runs(
                        site.file,
                        site.at,
                        registration,
                    )
                    && site.alternates.iter().all(|alternate| {
                        crate::flush_accounting::runs_when_the_call_runs(
                            site.file,
                            *alternate,
                            registration,
                        )
                    })
            }),
        }
    }
}

/// `SOLID_CHECKER_IR_ABLATE_SILENCE`: whether the reason is dropped (a test and measurement hook, shared with the site accounting's silences).
fn ablated(reason: AfterAwaitSafe) -> bool {
    crate::site_accounting::ablated_id(reason.id())
}

/// Whether the owner a `runWithOwner` call hands its callback is one the enclosing function captured before it first suspended: an identifier bound by an unreassigned `const` of a
/// `getOwner()` call written in the same function and not after any suspension of it (the program Chrome rc.21 is quiet on: `const owner = getOwner(); await …; runWithOwner(owner,
/// () => read())`). A nullish literal, a `getOwner()` call written after the suspension (a continuation has no owner), a parameter, an owner captured in another function and any
/// value no fact proves are not.
fn owner_captured_before_the_suspension(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    use solid_facts::ast::BindingShape;
    let lookup = ctx.semantic_lookup;
    let Some(owner) = call.arguments.first().filter(|argument| !argument.spread) else {
        return false;
    };
    let Some((binding_file, binding, _)) =
        lookup.binding_at_reference(file.path.as_str(), owner.span)
    else {
        return false;
    };
    if binding_file.path != file.path
        || !binding.immutable
        || binding.shape != BindingShape::Identifier
        || binding.names.len() != 1
    {
        return false;
    }
    let Some(initializer) = binding
        .initializer
        .map(|initializer| file.ast.peel_ts_sugar_span(initializer))
    else {
        return false;
    };
    let Some(captured) = file.ast.call_at(initializer) else {
        return false;
    };
    let same_function = crate::owners::containing_ast_function(&file.ast, call.span)
        .map(|function| function.span)
        == crate::owners::containing_ast_function(&file.ast, captured.span)
            .map(|function| function.span);
    !captured.construct
        && lookup
            .primitive_at_call(file, captured.span)
            .is_some_and(|primitive| ctx.dialect.returns_current_owner(primitive))
        && same_function
        && !crate::execution_role::may_follow_suspension(file, captured.span)
}

/// One site: a read, or a call that may run a project function, written after its async function's own suspension.
struct AwaitSite<'a> {
    file: &'a FileFacts,
    at: Span,
    alternates: Vec<Span>,
    /// How the site is listed: `read`, `call`, `member`, `inexact`, `inexact-member`, `invocation`, `iife`, `apply`, `project`, `unresolved`, and (ADR 0976) `library`, `package`,
    /// `symbol`, `construct`, `setter`, `action`, `typed`, `primitive` (R5: a `runWithOwner` call), `package` or `package-symbol`.
    via: &'static str,
    /// The function holding the site is a literal a reviewed runner schedules.
    test: bool,
    /// The registration call that hands that literal to the runner (`it("…", async () => { … })`), when `test` holds, and whether the runner is one whose published
    /// JavaScript shows how it calls a registered callback ([`crate::indexes::SemanticLookup::test_runner_schedules_by_read_bytes`]).
    registration: Option<(Span, bool)>,
    /// The callee that takes the function holding the site as an argument, when one does.
    host: String,
    /// The call, for a site that is one (the typed reasons ask its facts).
    call: Option<&'a solid_facts::ast::CallFact>,
    /// The root expression of a member read (`via: member-read`): the object whose plainness a typed reason asks.
    root: Option<Span>,
}

/// The rows of the final program by file, as `(start, end, label)`: an SC1002-family row as `row`, an SC9012 as `sc9012`.
type RowIndex = HashMap<String, Vec<(u64, u64, &'static str)>>;

fn row_index(program: &Program) -> RowIndex {
    let mut by_file: RowIndex = HashMap::default();
    for defect in &program.static_defects {
        let family = defect.kind.family();
        let label = if family == StaticDefectFamily::ReactiveReadAfterAwait {
            "row"
        } else if family == StaticDefectFamily::ReactiveDispatchUnresolved {
            "sc9012"
        } else {
            continue;
        };
        by_file
            .entry(defect.location.path.to_string())
            .or_default()
            .push((defect.location.start_byte, defect.location.end_byte, label));
    }
    for violation in &program.static_violations {
        if violation.rule == "reactive-read-after-await" {
            by_file
                .entry(violation.location.path.to_string())
                .or_default()
                .push((
                    violation.location.start_byte,
                    violation.location.end_byte,
                    "row",
                ));
        }
    }
    by_file
}

/// How the site ends before any reason is asked: a row of the family or an SC9012 at exactly the site's span or one of its alternates (the exact span: a row
/// that covers the site from another position is another site's, the lesson of ADR 0971's F1), the compiler's discard, or `none`; and whether a row of the
/// family overlaps the site without standing at one of its spans (the count the exact rule loses).
fn end_of(rows: &RowIndex, site: &AwaitSite<'_>) -> (&'static str, bool) {
    let path = site.file.path.as_str();
    let mut ends = "none";
    let mut overlap = false;
    for span in std::iter::once(site.at).chain(site.alternates.iter().copied()) {
        let (start, end) = (u64::from(span.start), u64::from(span.end));
        for (row_start, row_end, label) in rows.get(path).into_iter().flatten() {
            if (*row_start, *row_end) == (start, end) {
                // A row of the family is the stronger ending.
                if *label == "row" {
                    return ("row", overlap);
                }
                ends = "sc9012";
            } else if *row_start < end && start < *row_end && *label == "row" {
                overlap = true;
            }
        }
    }
    if ends == "none" && crate::execution_role::discarded_region_contains(site.file, site.at) {
        return ("discard", overlap);
    }
    (ends, overlap)
}

/// What the walk's own resolver ([`crate::execution_role::project_callee_at_parameter`], ADRs 0278, 0279, 0299, 0314) makes of a call's callee, for an identifier
/// and for a member alike.
enum Callee {
    /// A project function the call exactly runs: an identifier whose name still holds it (`function_value_is_current`), a module namespace's member, a member of a `const`
    /// object literal or of a `new C()` of the exact class that declares the method, no write of a member of that name anywhere, the receiver not escaping (ADR 0279).
    Current,
    /// A project function resolves and the call may not run it: a name the program rewrote (a `let` it reassigns), a method an override or a replacement may stand in for
    /// (`this.helper()`, a parameter typed as a class, an instance not exactly the class), a spread in the first argument. What runs is not decided.
    Replaced,
    /// No project function: a standard-library or package declaration, a function value, a member the lookup resolves to nothing, a member of a literal the receiver
    /// census withholds ([`unnamed_call`] decides which of those is a site).
    Unknown,
}

fn callee_of(
    lookup: &crate::indexes::SemanticLookup<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Callee {
    match crate::execution_role::project_callee_at_parameter(file, call, 0, lookup) {
        Some((_, _, true)) => Callee::Current,
        Some((_, _, false)) => Callee::Replaced,
        None => Callee::Unknown,
    }
}

/// Whether the callee of `call` is a member (`a.b()`, `a[b]()`).
fn is_member_call(file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    file.ast.member_at(callee).is_some() || file.ast.computed_members.binary_search(&callee).is_ok()
}

/// The standard calls that run a function value their receiver or arguments name: `f.call(…)` and `f.apply(…)` (the receiver), `Reflect.apply(f, …)` and
/// `Reflect.construct(f, …)` (the first argument). `Function.prototype.bind` runs nothing here: the function it returns runs where that value is called.
const RUNS_A_FUNCTION_VALUE: [&str; 6] = [
    "Function.call",
    "Function.apply",
    "CallableFunction.call",
    "CallableFunction.apply",
    "Reflect.apply",
    "Reflect.construct",
];

/// What a call the lookup resolves to no project function, and that is no stored function value ([`crate::static_rules::unresolved_invocation_shaped`]), is: a site, or an
/// accessor's call. A call is a site unless it is shown to be no more than a read of the universe: each way to be no site is a named fact, never a slot that matched nothing.
///
/// This was an arm that skipped every call whose callee is outside the project or Solid's own (a standard-library or package declaration, a setter, an action, a construct), on
/// the argument that it runs project code only through what it is handed or a protocol the project's objects implement. The reviews of #233 found that each of those holds
/// (S1, S2, R1 to R4 of the follow-up): a callback the library runs on this stack, a listener a synchronous dispatch delivers to, a receiver that is the project's own object under
/// a library type, a vetoed standard call the symbol fallback let through, a setter's updater, a constructor. So such a call is a site (ADR 0976), and [`library_call`] says when
/// one runs no project code.
enum Unnamed {
    /// The `via` of the site: the call may run project code.
    Site(&'static str),
    /// An accessor's call: a read of the universe, listed as a site of its own (`read`), so the call is no second site.
    Accessor,
}

fn unnamed_call(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    callee: Span,
) -> Unnamed {
    use crate::static_rules::{CalleeHome, SolidCall};
    if call.construct {
        return Unnamed::Site("construct");
    }
    // A function literal called where it is written runs on this stack.
    if file.ast.function_at(callee).is_some() {
        return Unnamed::Site("iife");
    }
    if let Some(solid) = crate::static_rules::solid_owned_call(ctx, file, call) {
        return match solid {
            SolidCall::Accessor => Unnamed::Accessor,
            SolidCall::Setter => Unnamed::Site("setter"),
            SolidCall::Action => Unnamed::Site("action"),
            SolidCall::Typed => Unnamed::Site("typed"),
        };
    }
    let declaration = ctx
        .semantic_lookup
        .resolved_callee_call(file, callee)
        .and_then(|resolved| resolved.declaration.as_ref());
    match declaration {
        // `f.call(…)`, `f.apply(…)`, `Reflect.apply(f, …)`, `Reflect.construct(f, …)` run a function value their receiver or arguments name.
        Some(declaration) if declaration.standard_library => Unnamed::Site(
            if RUNS_A_FUNCTION_VALUE.contains(&declaration.qualified_name.as_ref()) {
                "apply"
            } else {
                "library"
            },
        ),
        Some(declaration)
            if crate::effect_phase::package_root(declaration.location.path.as_ref()).is_some() =>
        {
            Unnamed::Site("package")
        }
        Some(_) => Unnamed::Site("project"),
        // No resolved-call fact (an argumentless call demands none, a veto withholds a standard call): the compiler's declarations of the callee's own symbol decide only
        // where it is declared, and a symbol found only outside the project says nothing of what runs.
        None => match crate::static_rules::callee_home(ctx, file, call) {
            Some(CalleeHome::DefaultLibrary) => Unnamed::Site("symbol"),
            Some(CalleeHome::Package) => Unnamed::Site("package-symbol"),
            Some(CalleeHome::Project) => Unnamed::Site("project"),
            None => Unnamed::Site("unresolved"),
        },
    }
}

/// The sites of the program, outside dependency source: the reads of the universe and the calls written after their async function's own suspension.
fn sites<'a>(
    ctx: &AnalysisContext<'a>,
    universe: &[(&'a FileFacts, UniverseSite)],
) -> Vec<AwaitSite<'a>> {
    let lookup = ctx.semantic_lookup;
    // Per file, the function literals handed directly to a call a reviewed runner schedules (`it("…", async () => { … })`): measurement facts only.
    let mut scheduled: HashMap<&str, HashMap<Span, (Span, bool)>> = HashMap::default();
    let mut classify = |file: &'a FileFacts, at: Span| -> (Option<(Span, bool)>, String) {
        let set = scheduled.entry(file.path.as_str()).or_insert_with(|| {
            let mut runners = HashMap::default();
            for call in &file.ast.calls {
                if lookup.test_runner_callback(file, call) != Some(TestRunnerCallback::Scheduled) {
                    continue;
                }
                let read_bytes = lookup.test_runner_schedules_by_read_bytes(file, call);
                for argument in &call.arguments {
                    if !argument.spread
                        && matches!(
                            argument.value,
                            ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
                        )
                    {
                        runners.insert(
                            file.ast.peel_ts_sugar_span(argument.span),
                            (call.span, read_bytes),
                        );
                    }
                }
            }
            runners
        });
        let Some(function) = crate::owners::containing_ast_function(&file.ast, at) else {
            return (None, "none".to_owned());
        };
        let host = file
            .ast
            .arguments_containing(function.span)
            .find(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })
            .and_then(|(call, _)| file.source_text(call.callee))
            .map_or_else(
                || "none".to_owned(),
                |text| text.chars().take(24).collect::<String>(),
            );
        (set.get(&function.span).copied(), host)
    };
    // ADR 0976, S3: the functions a promise handler call runs after a suspension are after one, async or not.
    let handlers = handlers::handler_functions(ctx);
    let after = |file: &FileFacts, span: Span| {
        crate::execution_role::may_follow_suspension(file, span)
            || handlers::in_handler(&handlers, file, span)
    };
    // A file with no async function and no promise handler holds no site: a site needs a suspension of its own function.
    let mut async_files: HashMap<&str, bool> = HashMap::default();
    let mut holds_suspension = |file: &'a FileFacts| {
        *async_files.entry(file.path.as_str()).or_insert_with(|| {
            file.ast.functions.iter().any(|function| function.r#async)
                || handlers.contains_key(file.path.as_str())
        })
    };
    let mut found = Vec::new();
    for &(file, ref site) in universe {
        if site.table != "read"
            || lookup.in_dependency_source(file.path.as_str())
            || !holds_suspension(file)
            || !after(file, site.at)
        {
            continue;
        }
        let (registration, host) = classify(file, site.at);
        found.push(AwaitSite {
            file,
            at: site.at,
            alternates: site.alternates.clone(),
            via: "read",
            test: registration.is_some(),
            registration,
            host,
            call: None,
            root: None,
        });
    }
    for file in lookup.files() {
        if lookup.in_dependency_source(file.path.as_str()) || !holds_suspension(file) {
            continue;
        }
        for call in &file.ast.calls {
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if !after(file, call.span) {
                continue;
            }
            // A call of a primitive is the dialect's own, a site only where its callback keeps the after-await flag (ADR 0976, R5: `runWithOwner`): the other primitives clear it
            // or run the callback in a pass of their own.
            if let Some(primitive) = lookup.primitive_at_call(file, call.span) {
                if (0..call.arguments.len()).any(|index| {
                    ctx.dialect
                        .callback_keeps_after_await_flag(primitive, index)
                }) {
                    let (registration, host) = classify(file, call.span);
                    found.push(AwaitSite {
                        file,
                        at: callee,
                        alternates: vec![call.span],
                        via: "primitive",
                        test: registration.is_some(),
                        registration,
                        host,
                        call: Some(call),
                        root: None,
                    });
                }
                continue;
            }
            // A project function the callee exactly names (and the name still holds), or a function value the walk cannot resolve (ADR 0964): a call the net takes for an
            // unresolved invocation is one whatever the lookup resolves (a `let` the program reassigns names its initializer, which is not the value).
            let named = callee_of(lookup, file, call);
            let invoked = crate::static_rules::unresolved_invocation_shaped(ctx, file, call);
            let member = is_member_call(file, call);
            let via = match (named, invoked) {
                (_, true) => "invocation",
                (Callee::Current, false) => {
                    if member {
                        "member"
                    } else {
                        "call"
                    }
                }
                (Callee::Replaced, false) => {
                    if member {
                        "inexact-member"
                    } else {
                        "inexact"
                    }
                }
                (Callee::Unknown, false) => match unnamed_call(ctx, file, call, callee) {
                    Unnamed::Site(via) => via,
                    // A read of the universe: its own site.
                    Unnamed::Accessor => continue,
                },
            };
            let (registration, host) = classify(file, call.span);
            // A construct is reported from its `new` token (the position a stack frame of `new K(…)` has), not from the callee after it.
            let at = if call.construct && call.span.start <= callee.start {
                Span::new(call.span.start, callee.end)
            } else {
                callee
            };
            found.push(AwaitSite {
                file,
                at,
                alternates: vec![call.span],
                via,
                test: registration.is_some(),
                registration,
                host,
                call: Some(call),
                root: None,
            });
        }
    }
    // ADR 0976, S4: the member reads of values the universe does not list (a store or props value reached through a binding).
    for file in lookup.files() {
        if lookup.in_dependency_source(file.path.as_str()) || !holds_suspension(file) {
            continue;
        }
        for chain in member_reads::chains(file) {
            if !after(file, chain.at) || member_reads::root_is_a_source(ctx, file, chain.root) {
                continue;
            }
            let (registration, host) = classify(file, chain.at);
            found.push(AwaitSite {
                file,
                at: chain.at,
                alternates: chain.alternates,
                via: "member-read",
                test: registration.is_some(),
                registration,
                host,
                call: None,
                root: Some(chain.root),
            });
        }
    }
    found
}

/// The uncertifiable row of SC1002 at a site no ending holds: a defect of the rule's own kind, `unaccounted` (the dialect words it), never a violation.
fn unaccounted_row(site: &AwaitSite<'_>) -> StaticDefect {
    StaticDefect {
        kind: StaticDefectKind::ReactiveReadAfterAwait {
            accessor: site.file.source_text(site.at).map_or_else(
                || "this site".to_owned(),
                |text| text.chars().take(80).collect(),
            ),
            obligation: Some(AfterAwaitObligation::Unaccounted),
        },
        location: site.at.location(site.file.path.shared()),
        analysis_context: crate::blockers::READ_AFTER_AWAIT_UNACCOUNTED.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// Ends every site written after an await, or gives it the uncertifiable row of SC1002. Run once per build, after the site accounting (whose universe it reads) and the
/// flush accounting, over the final rows.
pub(crate) fn account<'a>(
    ctx: &AnalysisContext<'a>,
    universe: &[(&'a FileFacts, UniverseSite)],
    program: &mut Program,
) {
    let found = sites(ctx, universe);
    if found.is_empty() {
        return;
    }
    let rows = row_index(program);
    let reach = crate::execution_role::SiteReach::new(ctx.semantic_lookup);
    // Whether a file of the program can name `Object.prototype` (#232's fact), asked once and only by a site whose argument is an object literal.
    let by_path: std::collections::HashMap<&str, &FileFacts> = ctx
        .semantic_lookup
        .files()
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let hand_off =
        crate::package_hand_off::UnreadPackageHandOff::new(ctx.semantic_lookup, &by_path);
    let prototype_touched = std::cell::OnceCell::new();
    let object_prototype_touched =
        || *prototype_touched.get_or_init(|| hand_off.object_prototype_touched());
    let logging = proof_log::enabled();
    let candidates = crate::proof_candidates::enabled();
    let mut filed: Vec<StaticDefect> = Vec::new();
    let mut seen: HashSet<(String, u32, u32)> = HashSet::default();
    for site in &found {
        let (end, overlap) = end_of(&rows, site);
        let reason = if end == "none" {
            AfterAwaitSafe::ALL
                .into_iter()
                .filter(|reason| !ablated(*reason))
                .find(|reason| reason.holds(ctx, site, &reach, &object_prototype_touched))
        } else {
            None
        };
        if let Some(reason) = reason
            && logging
        {
            proof_log::silent(
                "SC1002",
                &site.at.location(site.file.path.shared()),
                &format!("AfterAwaitSafe::{reason:?}"),
                &[],
            );
        }
        let unaccounted = end == "none" && reason.is_none();
        let new_row =
            unaccounted && seen.insert((site.file.path.to_string(), site.at.start, site.at.end));
        if new_row {
            filed.push(unaccounted_row(site));
        }
        if candidates {
            let never = matches!(
                reach.site(site.file, site.at),
                crate::execution_role::Reach::Unreached(_)
            );
            let reason_id = reason.map_or("none", AfterAwaitSafe::id);
            proof_log::candidate(&CandidateRecord {
                rule: "SC1002",
                table: "read-after-await-universe",
                path: site.file.path.as_str(),
                at: site.at,
                alternates: &site.alternates,
                facts: &[
                    ("via", site.via),
                    ("test", yes(site.test)),
                    ("end", end),
                    ("overlap", yes(overlap)),
                    ("never", yes(never)),
                    ("host", site.host.as_str()),
                    ("reason", reason_id),
                    ("filed", yes(new_row)),
                ],
            });
        }
    }
    program.static_defects.extend(filed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_unique_id_in_the_form_of_the_proof_log() {
        let mut seen = std::collections::BTreeSet::new();
        for reason in AfterAwaitSafe::ALL {
            let id = reason.id();
            assert!(seen.insert(id), "{reason:?} repeats the id {id}");
            let parts = id.split('.').collect::<Vec<_>>();
            assert!(
                parts.len() == 3
                    && parts[0] == "after_await_accounting"
                    && parts[1] == "account"
                    && parts.iter().all(|part| !part.is_empty()),
                "{reason:?}: `{id}` is not after_await_accounting.account.<end>"
            );
        }
    }

    #[test]
    fn the_reasons_are_never_runs_library_runs_nothing_and_owner_captured_and_there_is_no_callee_or_census_reason()
     {
        assert_eq!(
            AfterAwaitSafe::ALL.map(AfterAwaitSafe::id),
            [
                "after_await_accounting.account.never-runs",
                "after_await_accounting.account.runner-scheduled",
                "after_await_accounting.account.library-runs-nothing",
                "after_await_accounting.account.owner-captured",
                "after_await_accounting.account.plain-receiver"
            ]
        );
        // ADR 0973: a closure that reads nothing the universe lists (the review of #233: a store read through an alias, a stored generator stepped by a spread, a trap or a
        // getter installed by reference) and the absence of a recognized computation (the oracle cases `after-await-universe-census-*`) are no endings.
        assert!(AfterAwaitSafe::ALL.iter().all(|reason| {
            let id = reason.id();
            !id.contains("callee") && !id.contains("tracked-async-computation")
        }));
    }

    #[test]
    fn the_standard_calls_that_run_a_function_value_are_named_by_their_declarations() {
        for name in [
            "Function.call",
            "Function.apply",
            "Reflect.apply",
            "Reflect.construct",
        ] {
            assert!(RUNS_A_FUNCTION_VALUE.contains(&name), "{name}");
        }
        // `bind` runs nothing where it is called.
        assert!(!RUNS_A_FUNCTION_VALUE.contains(&"Function.bind"));
    }
}
