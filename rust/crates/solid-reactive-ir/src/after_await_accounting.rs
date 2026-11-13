//! Batch 11, slice 9B (ADR 0973): every read or project call written after an async function's own suspension is accounted for.
//!
//! UNTRACKED_READ_AFTER_AWAIT (SC1002) is raised for a reactive read a continuation runs after one of its async function's own suspensions. Its producers
//! (`static_rules/after_await.rs`) walk from the tracked async computations they find and file a row where they prove or cannot decide; a read or a call they
//! never reach had no row and no record of why, a clean nobody accounted for. This pass lists the sites of the claim from facts alone:
//!
//! - every read of [`crate::proof_universe`]'s `read` table (an accessor call, a store's or props' first `Get`), and
//! - every call of a project function by an identifier: one the walk's own resolver ([`crate::execution_role::project_callee_at_parameter`]) resolves exactly (`call`: an
//!   identifier whose name still holds the function) and one it resolves without being exact (`inexact`: a `let` the program reassigns, a `function` written again), and
//!   every call of a function value the walk cannot resolve (`invocation`: a parameter, a variable, a call's result, a member slot no function body defines; ADR 0964's shape),
//!
//! written directly in a function that is `async`, after one of that function's own suspensions ([`crate::execution_role::may_follow_suspension`]: an `await`, an
//! implicit suspension, a loop that repeats it), outside dependency source. Not sites: a `.then` continuation or a callback a later microtask runs (the reads in
//! it are the function's own sites), a read in a nested function (that function's own site), a tagged template's tag. A function a reviewed test runner schedules
//! is a function like any other (owner ruling 9b): its sites are sites.
//!
//! Like [`crate::flush_accounting`] (and ADRs 0838 and 0886) this pass keeps its own typed endings and its own universe; it names no
//! [`crate::site_accounting::SiteSilence`] and no premise kind. A site ends in
//!
//! - a row of the SC1002 family that stands at exactly the site's span or one of its alternates (a row of another rule, SC1001's for the same read included, ends
//!   nothing: it claims another code), never a row that covers it from another position;
//! - an SC9012 at the site (a withholding covers any code);
//! - the compiler's discard of the code;
//! - a typed reason ([`AfterAwaitSafe`]): a fact of the program or of the site that rules the claim out, each asked of the facts and nothing else;
//!
//! or it gets an uncertifiable row of SC1002, blocker `read-after-await-unaccounted`: never a violation, and it yields to a row that stands. The reasons:
//!
//! - **the callee reads nothing**: a call whose callee is a project function the lookup resolves exactly, whose whole transitive closure (every call in it resolves
//!   exactly to a project function again, with no read of the universe, no generator, no JSX, no tagged template, no construct and no primitive in it) reads no
//!   reactive source (the callee summary, owner ruling), **in a program that holds a tracked async computation** (`holds_suspending`). A run no call shows (a getter, a
//!   coercion, an iterator or a Proxy trap) is the member-route net's (ADR 0963): the read written in such code has a row of its own, and the net runs only where a
//!   computation is recognized; where none is (and a start the producers' index does not find may still be there: the oracle cases `after-await-universe-census-*`) the
//!   call keeps its row, so the getter's read is covered by a frame of its stack;
//! - **never runs**: no execution of the program runs the site (`SiteReach`).
//!
//! Left out on purpose and named in `docs/open-work.md` (owner ruling, decision 5): a role silence (a handler, a deferred callback, a test callback), a read the
//! walk's pre-await window already tracked, and a read inside an `untrack` callback: each needs a premise and a Chrome twin of its own.
//!
//! **No census reason.** "The program holds no tracked async computation, so nothing starts a walk" is no ending: the census (`holds_suspending`, ADR 0963) is only as
//! complete as the producers' index of computations, and Chrome shows starts the index does not find (the oracle cases `after-await-universe-census-*`: a function
//! declaration handed by identifier to a project wrapper that gives it to `createMemo`, `createMemo.call`, `Reflect.apply`, an array element). A program the census calls
//! computation-free keeps the row of every site (the fixture `after-await-universe-census`); a reason there needs a proof that no computation runs the function (no
//! value use of it, callers only module-level code or such functions), not the absence of a recognized start.
//!
//! `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>[,<id>...]` (or `all`) drops a reason, so the sites it ends get their row: the mutation test of each. With the proof log
//! on, each reason is a `silent` line of SC1002 (`AfterAwaitSafe::<reason>`) at the site. With `SOLID_CHECKER_IR_PROOF_CANDIDATES` the pass also writes one `candidate`
//! line per site (`read-after-await-universe`) with how the site ends and the facts that tell the sites apart: `via` (`read`, `call`, `invocation`, `inexact`), `overlap` (a row
//! of the family overlaps the site without standing at one of its spans), `test` (the function is a literal a reviewed runner schedules), `tracked` (the census above), `never` (`SiteReach`), `callee`, `closure` and
//! `blocker` (the callee summary and the first call that stops it), `host` (the callee that takes the function as an argument), `reason` (the typed reason that ended
//! the site) and `filed` (whether the site got its row).

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
    /// A call of a project function whose whole closure reads no reactive source ([`Summary::Nothing`]), in a program whose member-route net runs (it holds a tracked
    /// async computation: `holds_suspending`): a getter, a coercion, an iterator or a Proxy trap the closure does not follow has a row of its own there.
    CalleeReadsNothing,
    /// No execution of the program runs the site (`SiteReach`).
    NeverRuns,
}

impl AfterAwaitSafe {
    /// In the order they are asked, the cheapest first.
    const ALL: [Self; 2] = [Self::CalleeReadsNothing, Self::NeverRuns];

    /// The id the proof log writes, and `SOLID_CHECKER_IR_ABLATE_SILENCE` names (`<module>.<function>.<end>`).
    const fn id(self) -> &'static str {
        match self {
            Self::CalleeReadsNothing => "after_await_accounting.account.callee-reads-nothing",
            Self::NeverRuns => "after_await_accounting.account.never-runs",
        }
    }

    /// Whether the reason holds at `site`.
    fn holds<'a>(self, site: &AwaitSite<'a>, asked: &Asked<'_, 'a>) -> bool {
        match self {
            Self::CalleeReadsNothing => {
                asked.tracked_program && matches!(site.summary, Some(Summary::Nothing))
            }
            Self::NeverRuns => matches!(
                asked.reach.site(site.file, site.at),
                crate::execution_role::Reach::Unreached(_)
            ),
        }
    }
}

/// What the reasons ask of the program, built once per build.
struct Asked<'l, 'a> {
    /// The census of ADR 0963: the program holds a tracked async computation with something after a suspension (the region of the member-route net).
    tracked_program: bool,
    reach: crate::execution_role::SiteReach<'l, 'a>,
}

/// `SOLID_CHECKER_IR_ABLATE_SILENCE`: whether the reason is dropped (a test and measurement hook, shared with the site accounting's silences).
fn ablated(reason: AfterAwaitSafe) -> bool {
    crate::site_accounting::ablated_id(reason.id())
}

/// One site: a read, or a call of a project function or of a function value the walk cannot resolve, written after its async function's own suspension.
struct AwaitSite<'a> {
    file: &'a FileFacts,
    at: Span,
    alternates: Vec<Span>,
    /// `read`, `call` or `invocation`.
    via: &'static str,
    /// What the callee's own body holds, for a call of a project function: `leaf` (no call and no read of the universe), `calls` (calls, no read of the
    /// universe: needs the closure over them), `reads` (a read of the universe); `unknown` for a stored function, `none` for a read.
    callee: &'static str,
    /// The callee summary of a call of a project function; `None` for a site that is no such call.
    summary: Option<Summary>,
    /// The function holding the site is a literal a reviewed runner schedules.
    test: bool,
    /// The callee that takes the function holding the site as an argument, when one does.
    host: String,
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

/// The project function a call's callee names, and whether the call exactly runs it: the walk's own resolver ([`crate::execution_role::project_callee_at_parameter`],
/// ADRs 0278, 0279, 0299, 0314).
enum Callee<'a> {
    /// A project function the call exactly runs: an identifier whose name still holds it (`function_value_is_current`).
    Current(&'a FileFacts, &'a solid_facts::ast::FunctionFact),
    /// A project function resolves and the call may not run it: a name the program rewrote (a `let` it reassigns), `f.call(..)`, a spread in the first argument. What runs
    /// is not decided.
    Replaced,
    /// No project function: a standard-library or package declaration, a function value.
    Unknown,
}

fn callee_of<'a>(
    lookup: &crate::indexes::SemanticLookup<'a>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Callee<'a> {
    match crate::execution_role::project_callee_at_parameter(file, call, 0, lookup) {
        Some((callee_file, function, true)) => Callee::Current(callee_file, function),
        Some((_, _, false)) => Callee::Replaced,
        None => Callee::Unknown,
    }
}

/// Whether the callee of `call` is a member (`a.b()`, `a[b]()`).
fn is_member_call(file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    file.ast.member_at(callee).is_some() || file.ast.computed_members.binary_search(&callee).is_ok()
}

/// What a project function does for a call written after an await (the callee summary).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Summary {
    /// The function and every function it calls (through calls the lookup resolves to a project function exactly) hold no read of the universe, no other call,
    /// no JSX element and no generator: whatever the call runs reads no reactive source.
    Nothing,
    /// A read of the universe lies in the function, or in one it calls.
    Reads,
    /// A function that calls itself, through any chain of exactly resolved calls (the search is cut, nothing is concluded).
    Cycle,
    /// A call or construct the summary cannot follow, or a function it does not read: why, and the text of the callee that stops it.
    Blocked(&'static str, String),
}

impl Summary {
    fn describe(&self) -> (String, String) {
        match self {
            Self::Nothing => ("nothing".to_owned(), String::new()),
            Self::Reads => ("reads".to_owned(), String::new()),
            Self::Cycle => ("cycle".to_owned(), String::new()),
            Self::Blocked(why, callee) => (format!("blocked:{why}"), callee.clone()),
        }
    }
}

/// How many functions deep the closure follows a chain of exactly resolved calls before it stops with `Blocked("depth")`: a bound on the search's stack (a chain this long is not
/// concluded about, and a stopped answer is never memoized), never a conclusion.
const MAX_CLOSURE_DEPTH: usize = 64;

/// The callee summaries of the program, memoized per function; a function met again while its own summary is open answers `Cycle`.
struct Closure<'c, 'a> {
    ctx: &'c AnalysisContext<'a>,
    reads: &'c HashMap<&'a str, Vec<Span>>,
    memo: HashMap<(&'a str, Span), Summary>,
    active: HashSet<(&'a str, Span)>,
}

impl<'a> Closure<'_, 'a> {
    fn of(&mut self, file: &'a FileFacts, function: &'a solid_facts::ast::FunctionFact) -> Summary {
        let key = (file.path.as_str(), function.span);
        if let Some(known) = self.memo.get(&key) {
            return known.clone();
        }
        if !self.active.insert(key) {
            return Summary::Cycle;
        }
        if self.active.len() > MAX_CLOSURE_DEPTH {
            self.active.remove(&key);
            return Summary::Blocked("depth", String::new());
        }
        let summary = self.compute(file, function);
        self.active.remove(&key);
        if summary != Summary::Cycle && !matches!(&summary, Summary::Blocked("depth", _)) {
            self.memo.insert(key, summary.clone());
        }
        summary
    }

    fn compute(
        &mut self,
        file: &'a FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
    ) -> Summary {
        let lookup = self.ctx.semantic_lookup;
        let blocked =
            |why: &'static str, text: &str| Summary::Blocked(why, text.chars().take(24).collect());
        if lookup.in_dependency_source(file.path.as_str()) {
            return blocked("package", "");
        }
        if function.generator {
            return blocked("generator", "");
        }
        // The whole function, nested functions and the parameter list included: what a nested function does may run when this one is called.
        if self
            .reads
            .get(file.path.as_str())
            .is_some_and(|spans| spans.iter().any(|at| function.span.contains(*at)))
        {
            return Summary::Reads;
        }
        if file
            .ast
            .jsx_elements
            .iter()
            .any(|element| function.span.contains(element.span))
        {
            return blocked("jsx", "");
        }
        if file
            .ast
            .tagged_templates
            .iter()
            .any(|template| function.span.contains(template.span))
        {
            return blocked("tagged-template", "");
        }
        for call in file
            .ast
            .calls
            .iter()
            .filter(|call| function.span.contains(call.span))
        {
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let text = file.source_text(call.callee).unwrap_or_default();
            if call.construct {
                return blocked("construct", text);
            }
            if lookup.primitive_at_call(file, call.span).is_some() {
                return blocked("primitive", text);
            }
            let (callee_file, callee_function) = match callee_of(lookup, file, call) {
                Callee::Current(callee_file, callee_function) => (callee_file, callee_function),
                Callee::Replaced => return blocked("not-exact", text),
                Callee::Unknown => {
                    let why = match lookup
                        .resolved_callee_call(file, callee)
                        .and_then(|resolved| resolved.declaration.as_ref())
                    {
                        Some(declaration) if declaration.standard_library => "standard-library",
                        Some(declaration) if lookup.resolved_slot_without_body(declaration) => {
                            "stored-function"
                        }
                        Some(_) => "package",
                        None => "unresolved",
                    };
                    return blocked(why, text);
                }
            };
            match self.of(callee_file, callee_function) {
                Summary::Nothing => {}
                other => return other,
            }
        }
        Summary::Nothing
    }
}

/// The sites of the program, outside dependency source: the reads of the universe and the calls written after their async function's own suspension.
fn sites<'a>(
    ctx: &AnalysisContext<'a>,
    universe: &[(&'a FileFacts, UniverseSite)],
) -> Vec<AwaitSite<'a>> {
    let lookup = ctx.semantic_lookup;
    // Per file, the function literals handed directly to a call a reviewed runner schedules (`it("…", async () => { … })`): measurement facts only.
    let mut scheduled: HashMap<&str, HashSet<Span>> = HashMap::default();
    let mut classify = |file: &'a FileFacts, at: Span| -> (bool, String) {
        let set = scheduled.entry(file.path.as_str()).or_insert_with(|| {
            let mut runners = HashSet::default();
            for call in &file.ast.calls {
                if lookup.test_runner_callback(file, call) != Some(TestRunnerCallback::Scheduled) {
                    continue;
                }
                for argument in &call.arguments {
                    if !argument.spread
                        && matches!(
                            argument.value,
                            ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
                        )
                    {
                        runners.insert(file.ast.peel_ts_sugar_span(argument.span));
                    }
                }
            }
            runners
        });
        let Some(function) = crate::owners::containing_ast_function(&file.ast, at) else {
            return (false, "none".to_owned());
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
        (set.contains(&function.span), host)
    };
    // A file with no async function holds no site: a site needs an async function's own suspension.
    let mut async_files: HashMap<&str, bool> = HashMap::default();
    let mut has_async = |file: &'a FileFacts| {
        *async_files
            .entry(file.path.as_str())
            .or_insert_with(|| file.ast.functions.iter().any(|function| function.r#async))
    };
    let mut found = Vec::new();
    // The reads of the universe by file, for what a callee's body holds.
    let mut reads_by_file: HashMap<&str, Vec<Span>> = HashMap::default();
    for &(file, ref site) in universe {
        if site.table == "read" {
            reads_by_file
                .entry(file.path.as_str())
                .or_default()
                .push(site.at);
        }
    }
    let mut closure = Closure {
        ctx,
        reads: &reads_by_file,
        memo: HashMap::default(),
        active: HashSet::default(),
    };
    for &(file, ref site) in universe {
        if site.table != "read"
            || lookup.in_dependency_source(file.path.as_str())
            || !has_async(file)
            || !crate::execution_role::may_follow_suspension(file, site.at)
        {
            continue;
        }
        let (test, host) = classify(file, site.at);
        found.push(AwaitSite {
            file,
            at: site.at,
            alternates: site.alternates.clone(),
            via: "read",
            callee: "none",
            summary: None,
            test,
            host,
        });
    }
    for file in lookup.files() {
        if lookup.in_dependency_source(file.path.as_str()) || !has_async(file) {
            continue;
        }
        for call in &file.ast.calls {
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            if lookup.primitive_at_call(file, call.span).is_some()
                || !crate::execution_role::may_follow_suspension(file, call.span)
            {
                continue;
            }
            // A project function the callee exactly names (and the name still holds), or a function value the walk cannot resolve (ADR 0964): a call the net takes for an
            // unresolved invocation is one whatever the lookup resolves (a `let` the program reassigns names its initializer, which is not the value), never summarized.
            let named = callee_of(lookup, file, call);
            let invoked = crate::static_rules::unresolved_invocation_shaped(ctx, file, call);
            let (via, target) = match (named, invoked) {
                (_, true) => ("invocation", None),
                // A call through a member is no site.
                (_, false) if is_member_call(file, call) => continue,
                (Callee::Current(callee_file, function), false) => {
                    ("call", Some((callee_file, function)))
                }
                (Callee::Replaced, false) => ("inexact", None),
                (Callee::Unknown, false) => continue,
            };
            let (test, host) = classify(file, call.span);
            let held = target.map_or("unknown", |(callee_file, function)| {
                let calls = callee_file
                    .ast
                    .calls
                    .iter()
                    .filter(|inner| function.body.contains(inner.span))
                    .count();
                let reads = reads_by_file
                    .get(callee_file.path.as_str())
                    .map_or(0, |spans| {
                        spans
                            .iter()
                            .filter(|at| function.body.contains(**at))
                            .count()
                    });
                match (calls, reads) {
                    (0, 0) => "leaf",
                    (_, 0) => "calls",
                    _ => "reads",
                }
            });
            let summary = target.map(|(callee_file, function)| closure.of(callee_file, function));
            found.push(AwaitSite {
                file,
                at: callee,
                alternates: vec![call.span],
                via,
                callee: held,
                summary,
                test,
                host,
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
/// flush accounting, over the final rows. `tracked_program` holds the census of ADR 0963 (`holds_suspending`, set by the prepass every build).
pub(crate) fn account<'a>(
    ctx: &AnalysisContext<'a>,
    tracked_program: bool,
    universe: &[(&'a FileFacts, UniverseSite)],
    program: &mut Program,
) {
    let found = sites(ctx, universe);
    if found.is_empty() {
        return;
    }
    let rows = row_index(program);
    let asked = Asked {
        tracked_program,
        reach: crate::execution_role::SiteReach::new(ctx.semantic_lookup),
    };
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
                .find(|reason| reason.holds(site, &asked))
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
                asked.reach.site(site.file, site.at),
                crate::execution_role::Reach::Unreached(_)
            );
            let (closure, blocker) = site
                .summary
                .as_ref()
                .map_or(("na".to_owned(), String::new()), Summary::describe);
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
                    ("tracked", yes(tracked_program)),
                    ("end", end),
                    ("overlap", yes(overlap)),
                    ("never", yes(never)),
                    ("callee", site.callee),
                    ("closure", closure.as_str()),
                    ("blocker", blocker.as_str()),
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
    fn the_reasons_are_the_two_that_remain_and_no_census_reason() {
        assert_eq!(AfterAwaitSafe::ALL.len(), 2);
        assert_eq!(
            AfterAwaitSafe::ALL.map(AfterAwaitSafe::id),
            [
                "after_await_accounting.account.callee-reads-nothing",
                "after_await_accounting.account.never-runs",
            ]
        );
        // ADR 0973: the absence of a recognized computation is no ending (the oracle cases `after-await-universe-census-*`).
        assert!(
            AfterAwaitSafe::ALL
                .iter()
                .all(|reason| !reason.id().contains("tracked-async-computation"))
        );
    }

    #[test]
    fn a_summary_describes_itself_for_the_candidate_line() {
        assert_eq!(Summary::Nothing.describe().0, "nothing");
        assert_eq!(Summary::Reads.describe().0, "reads");
        assert_eq!(Summary::Cycle.describe().0, "cycle");
        assert_eq!(
            Summary::Blocked("package", "page.getByRole".to_owned()).describe(),
            ("blocked:package".to_owned(), "page.getByRole".to_owned())
        );
    }
}
