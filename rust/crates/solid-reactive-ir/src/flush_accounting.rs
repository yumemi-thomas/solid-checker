//! Batch 11, slice 5: every flush site is accounted for, per code. A flush site is a reference to the flush primitive that the flush census lists
//! (`indexes/flush_census.rs`): a call, or a value handed to code the census does not follow. The three codes a flush can raise are
//! FLUSH_IN_ACTION (SC2006, an action's step), FLUSH_IN_EFFECT_CALLBACK (SC2008, an effect's apply) and FLUSH_IN_TRACKED_QUEUE_CALLBACK (SC3001, a
//! leaf owner's callback). For each code a site must end in
//!
//! - a row of that code's rule at the site (a violation, an uncertifiable row, a leaf operation), never another rule's;
//! - an SC9012 at the site (a withholding covers any code);
//! - the compiler's discard of the code;
//! - a typed reason ([`FlushSafe`]) the proof log names: a fact of the program or of the site's census class that rules the code out, each the
//!   producers' own silent exit written down (`static_api.rs` returns from SC2006's arm with no row when the program steps no action callback, and so on);
//!
//! or it gets an uncertifiable row of the code's own rule, blocker `flush-in-action-unaccounted`, `flush-in-effect-unaccounted` or
//! `leaf-operation-unaccounted`: never a violation, and it yields to any row that stands. A give-up is not a reason: the walks that place a flush
//! (`unplaced_blocker`, the effect-phase origin) answer "placed off" for a function no call edge reaches, a body that may be a component and an owner set
//! around the call as well as for a proof, so none of their silent exits is named here (owner ruling 5a): the site stays uncertifiable.
//!
//! Like ADR 0838's and ADR 0886's passes this one keeps its own typed endings and its own universe; it names no [`crate::site_accounting::SiteSilence`].
//! `SOLID_CHECKER_IR_ABLATE_SILENCE=<id>[,<id>...]` (or `all`) drops a reason, so the sites it ends get their row: the mutation test of each.

use std::collections::{HashMap, HashSet};

use solid_facts::FileFacts;
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;
use crate::{Program, StaticDefectKind, StaticViolation};

/// The codes a flush site can raise, in the order their rows are filed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum FlushCode {
    /// SC2006: the flush runs inside an action's step.
    Action,
    /// SC2008: the flush runs inside an effect's apply.
    Effect,
    /// SC3001: the flush runs in a leaf owner's callback (a tracked queue callback).
    Leaf,
}

impl FlushCode {
    const ALL: [Self; 3] = [Self::Action, Self::Effect, Self::Leaf];

    const fn id(self) -> &'static str {
        match self {
            Self::Action => "SC2006",
            Self::Effect => "SC2008",
            Self::Leaf => "SC3001",
        }
    }

    /// The rule that answers the code.
    const fn rule(self) -> &'static str {
        match self {
            Self::Action => "flush-in-action",
            Self::Effect => "flush-in-effect-callback",
            Self::Leaf => "leaf-owner-forbidden-call",
        }
    }

    const fn blocker(self) -> &'static str {
        match self {
            Self::Action => crate::blockers::FLUSH_IN_ACTION_UNACCOUNTED,
            Self::Effect => crate::blockers::FLUSH_IN_EFFECT_UNACCOUNTED,
            Self::Leaf => crate::blockers::LEAF_FLUSH_UNACCOUNTED,
        }
    }
}

/// A typed reason a code cannot arise at a flush site. Closed: each variant rules out exactly the codes [`Self::rules_out`] names, on the fact its
/// documentation says, which the pass asks of the program or of the site's census class and nothing else.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlushSafe {
    /// The program names no primitive that runs a callback as an action's steps (`project_steps_action_callbacks`): no step is on any stack.
    NoSteppedAction,
    /// The program names no primitive that defers a callback to a later flush (`project_defers_callbacks`): no effect's apply runs.
    NoDeferringPrimitive,
    /// The program names no primitive that runs a callback under a leaf owner (`project_opens_leaf_scope`): no tracked queue callback runs.
    NoLeafScope,
    /// A callback a reviewed host scheduler runs from an empty stack, written directly in it (the census's `FreshStack`): no step, apply or leaf
    /// callback is below it. Only for a call in a function's body (not a parameter default) and when no function around it is a named function
    /// expression that its own code calls again by name.
    FreshStack,
    /// Code an `await` of its own async function dominates (the census's `AwaitContinuation`): it resumes from the microtask queue on an empty stack.
    /// Only for a call in a function's body: a parameter default runs wherever its function is called.
    AwaitContinuation,
    /// Module top level, outside every function, class and JSX element (the census's `ModuleInitialization`): evaluated before any callback runs.
    ModuleInitialization,
    /// A callback a reviewed test runner schedules, the flush written directly in it (the census's `TestRunner`: `direct_callback_contains`): the
    /// runner's own stack, on the same two conditions as [`Self::FreshStack`]. A flush in a helper the callback calls is a `TestHelper` to the census,
    /// a class no reason below names.
    TestRunner,
    /// No execution of the program runs the site (`SiteReach`).
    NeverRuns,
}

impl FlushSafe {
    /// The id the proof log writes, and `SOLID_CHECKER_IR_ABLATE_SILENCE` names (`<module>.<function>.<end>`).
    const fn id(self) -> &'static str {
        match self {
            Self::NoSteppedAction => "flush_accounting.account.no-stepped-action",
            Self::NoDeferringPrimitive => "flush_accounting.account.no-deferring-primitive",
            Self::NoLeafScope => "flush_accounting.account.no-leaf-scope",
            Self::FreshStack => "flush_accounting.account.fresh-stack",
            Self::AwaitContinuation => "flush_accounting.account.await-continuation",
            Self::ModuleInitialization => "flush_accounting.account.module-initialization",
            Self::TestRunner => "flush_accounting.account.test-runner-callback",
            Self::NeverRuns => "flush_accounting.account.never-runs",
        }
    }

    #[cfg(test)]
    const ALL: [Self; 8] = [
        Self::NoSteppedAction,
        Self::NoDeferringPrimitive,
        Self::NoLeafScope,
        Self::FreshStack,
        Self::AwaitContinuation,
        Self::ModuleInitialization,
        Self::TestRunner,
        Self::NeverRuns,
    ];

    /// Whether the reason rules `code` out.
    const fn rules_out(self, code: FlushCode) -> bool {
        match self {
            Self::NoSteppedAction => matches!(code, FlushCode::Action),
            Self::NoDeferringPrimitive => matches!(code, FlushCode::Effect),
            Self::NoLeafScope => matches!(code, FlushCode::Leaf),
            Self::FreshStack
            | Self::AwaitContinuation
            | Self::ModuleInitialization
            | Self::TestRunner
            | Self::NeverRuns => true,
        }
    }
}

/// The reasons the program and the site hold, before any row is asked: the three program censuses, the census class of the caller, and reach.
fn reasons(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    site: &crate::indexes::FlushSite,
    census: (bool, bool, bool),
) -> Vec<FlushSafe> {
    use crate::indexes::DispatchCaller;
    let lookup = ctx.semantic_lookup;
    let (steps, defers, leaf) = census;
    let mut reasons = Vec::new();
    if !steps {
        reasons.push(FlushSafe::NoSteppedAction);
    }
    if !defers {
        reasons.push(FlushSafe::NoDeferringPrimitive);
    }
    if !leaf {
        reasons.push(FlushSafe::NoLeafScope);
    }
    // The census class says where the function around the call runs; it is a statement about this call's stack only when two facts of the call's own
    // syntax hold. The call is written in the body of its function: a default value in a parameter list runs wherever that function is called, which the
    // class of the code around it does not say (the review of #213, B2). And, for the classes that name a scheduler's or a runner's own call, no function around
    // the call can be entered again by its own name: `setTimeout(function tick() { flush(); … tick(true) })` runs `tick` a second time on the stack of whatever
    // calls the name, an action's step or an effect's apply (B1). A site that fails either gets no reason, so its uncertifiable row.
    let in_body = !crate::owners::in_parameter_list(&file.ast, site.span);
    let named_again = file
        .ast
        .functions_owning(site.span)
        .any(|function| reentered_by_name(file, function));
    let class = match site.caller {
        Some(DispatchCaller::FreshStack) => {
            (in_body && !named_again).then_some(FlushSafe::FreshStack)
        }
        Some(DispatchCaller::AwaitContinuation) => in_body.then_some(FlushSafe::AwaitContinuation),
        Some(DispatchCaller::ModuleInitialization) => Some(FlushSafe::ModuleInitialization),
        Some(DispatchCaller::TestRunner) => {
            (in_body && !named_again).then_some(FlushSafe::TestRunner)
        }
        // No reason, so the site gets its uncertifiable row: `DetachedOwner` keeps the stack's action depth, `TestHelper` may be called from a step
        // inside a test, an event handler's delivery may be dispatched from a step, `EffectApply` is the apply itself, and no census class (`None`)
        // is a site the census could not place (owner ruling 5a).
        Some(
            DispatchCaller::DetachedOwner
            | DispatchCaller::TestHelper
            | DispatchCaller::EventHandler
            | DispatchCaller::EffectApply,
        )
        | None => None,
    };
    reasons.extend(class);
    if matches!(
        crate::execution_role::SiteReach::new(lookup).site(file, site.span),
        crate::execution_role::Reach::Unreached(_)
    ) {
        reasons.push(FlushSafe::NeverRuns);
    }
    reasons
}

/// A named function expression whose name is used: `setTimeout(function tick() { … tick(true) })`. The name is bound inside the function only, so the code that
/// uses it is written in it, and calls it from the stack that code runs on (an action's step, an effect's apply, a leaf callback), never the one the scheduler
/// or the runner gave the first run. A name nothing uses cannot be called again: the function runs only as often as its host calls it.
fn reentered_by_name(file: &FileFacts, function: &solid_facts::ast::FunctionFact) -> bool {
    function.kind == solid_facts::ast::FunctionKind::Expression
        && function
            .name
            .as_ref()
            .is_some_and(|name| !file.ast.references_to(name.span).is_empty())
}

/// `SOLID_CHECKER_IR_ABLATE_SILENCE`: whether the reason is dropped (a test and measurement hook, shared with the site accounting's silences).
fn ablated(reason: FlushSafe) -> bool {
    crate::site_accounting::ablated_id(reason.id())
}

/// The rows of the final program, by file, as `(start, end, label)`: a rule's own rows by the rule's name, an SC9012 as `sc9012`, a leaf operation as
/// `leaf-op`.
type RowIndex = HashMap<String, Vec<(u64, u64, &'static str)>>;

fn row_index(program: &Program) -> RowIndex {
    let mut rows: RowIndex = HashMap::new();
    for violation in &program.static_violations {
        let label = FlushCode::ALL
            .iter()
            .map(|code| code.rule())
            .find(|rule| *rule == violation.rule);
        if let Some(label) = label {
            rows.entry(violation.location.path.to_string())
                .or_default()
                .push((
                    violation.location.start_byte,
                    violation.location.end_byte,
                    label,
                ));
        }
    }
    for defect in &program.static_defects {
        if matches!(
            defect.kind,
            StaticDefectKind::ReactiveDispatchUnresolved { .. }
        ) {
            rows.entry(defect.location.path.to_string())
                .or_default()
                .push((
                    defect.location.start_byte,
                    defect.location.end_byte,
                    "sc9012",
                ));
        }
    }
    for operation in &program.leaf_operations {
        for location in std::iter::once(&operation.location).chain(operation.sites.iter()) {
            rows.entry(location.path.to_string()).or_default().push((
                location.start_byte,
                location.end_byte,
                "leaf-op",
            ));
        }
    }
    rows
}

/// A row of the code's own rule (or a leaf operation, for SC3001), or an SC9012, at the site.
fn row_at(rows: &RowIndex, path: &str, site: Span, code: FlushCode) -> bool {
    let (start, end) = (u64::from(site.start), u64::from(site.end));
    rows.get(path).is_some_and(|here| {
        here.iter().any(|(row_start, row_end, label)| {
            *row_start < end
                && start < *row_end
                && (*label == code.rule()
                    || *label == "sc9012"
                    || (code == FlushCode::Leaf && *label == "leaf-op"))
        })
    })
}

/// The uncertifiable row of `code` at a site no ending holds.
fn unaccounted_row(file: &FileFacts, site: Span, code: FlushCode) -> StaticViolation {
    let text = file.source_text(site).unwrap_or("this site");
    let text = text.get(..text.len().min(48)).unwrap_or(text);
    StaticViolation {
        id: code.id().into(),
        rule: code.rule().into(),
        message: format!(
            "this flush (`{}`) has no row and no recorded proof that places it, so whether this rule applies here is not decided",
            text.trim()
        ),
        hint: "No analysis produced a row for this flush and none named a fact that rules this rule out; it is reported as an obligation until one does.".into(),
        location: site.location(file.path.shared()),
        analysis_context: code.blocker().to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// Ends every flush site, per code, or gives it the code's uncertifiable row. Run once per build, after the site accounting, over the final rows.
pub(crate) fn account(ctx: &AnalysisContext<'_>, program: &mut Program) {
    let lookup = ctx.semantic_lookup;
    let dialect = ctx.dialect;
    let census = (
        lookup.project_steps_action_callbacks(),
        lookup.project_defers_callbacks(),
        lookup.project_opens_leaf_scope(),
    );
    // The pass reads the census of every file and the final rows: a whole-program read, recorded as one (`SemanticLookup::files`), so the
    // files are found by path in a table of it rather than one by one.
    let files: HashMap<&str, &FileFacts> = lookup
        .files()
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let rows = row_index(program);
    let logging = crate::proof_log::enabled();
    let mut filed: Vec<StaticViolation> = Vec::new();
    let mut seen: HashSet<(String, u32, u32, FlushCode)> = HashSet::new();
    for site in lookup.flush_sites() {
        let Some(file) = files.get(site.path.as_str()).copied() else {
            continue;
        };
        let primitive = if site.called {
            lookup.primitive_at_call(file, site.span)
        } else {
            crate::referenced_primitive(file, site.span, ctx.entities, ctx.symbol_names, dialect)
                .and_then(|name| name.primitive())
        };
        let Some(primitive) = primitive else {
            continue;
        };
        // Each code asks its own question of the primitive, the producers' own: the dialect's for SC2006 and SC2008, the leaf walk's kind for SC3001.
        // `render` and `hydrate` drain the queue on the caller's stack too (the census lists them for its veto) and none of the three is theirs.
        let applies = [
            (
                FlushCode::Action,
                dialect.throws_inside_action_step(primitive),
            ),
            (
                FlushCode::Effect,
                dialect.warns_inside_effect_phase(primitive),
            ),
            (
                FlushCode::Leaf,
                crate::cleanup::files_as_flush_operation(dialect, primitive),
            ),
        ];
        if applies.iter().all(|(_, asked)| !asked) {
            continue;
        }
        let discarded = crate::execution_role::discarded_region_contains(file, site.span);
        let mut held: Option<Vec<FlushSafe>> = None;
        for (code, asked) in applies {
            if !asked || discarded || row_at(&rows, &site.path, site.span, code) {
                continue;
            }
            let reasons = held.get_or_insert_with(|| reasons(ctx, file, site, census));
            let ending = reasons
                .iter()
                .copied()
                .find(|reason| reason.rules_out(code) && !ablated(*reason));
            if let Some(reason) = ending {
                if logging {
                    crate::proof_log::silent(
                        code.id(),
                        &site.span.location(file.path.shared()),
                        &format!("FlushSafe::{reason:?}"),
                        &[],
                    );
                }
                continue;
            }
            if seen.insert((site.path.clone(), site.span.start, site.span.end, code)) {
                filed.push(unaccounted_row(file, site.span, code));
            }
        }
    }
    program.static_violations.extend(filed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_a_unique_id_in_the_form_of_the_proof_log() {
        let mut seen = std::collections::BTreeSet::new();
        for reason in FlushSafe::ALL {
            let id = reason.id();
            assert!(seen.insert(id), "{reason:?} repeats the id {id}");
            let parts = id.split('.').collect::<Vec<_>>();
            assert!(
                parts.len() == 3
                    && parts[0] == "flush_accounting"
                    && parts.iter().all(|part| !part.is_empty()),
                "{reason:?}: `{id}` is not flush_accounting.<function>.<end>"
            );
        }
    }

    #[test]
    fn every_reason_is_named_by_the_function_that_records_it() {
        let source = include_str!("flush_accounting.rs");
        let body = source
            .split("fn reasons(")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .expect("the function `reasons`");
        for reason in FlushSafe::ALL {
            // A reason nobody records is a dead end: `reasons` names each once.
            assert!(
                body.contains(&format!("FlushSafe::{reason:?}")),
                "{reason:?} is named by no arm of `reasons`"
            );
        }
    }

    #[test]
    fn a_program_census_reason_rules_out_its_own_code_only() {
        for code in FlushCode::ALL {
            assert_eq!(
                FlushSafe::NoSteppedAction.rules_out(code),
                code == FlushCode::Action
            );
            assert_eq!(
                FlushSafe::NoDeferringPrimitive.rules_out(code),
                code == FlushCode::Effect
            );
            assert_eq!(
                FlushSafe::NoLeafScope.rules_out(code),
                code == FlushCode::Leaf
            );
            for reason in [
                FlushSafe::FreshStack,
                FlushSafe::AwaitContinuation,
                FlushSafe::ModuleInitialization,
                FlushSafe::TestRunner,
                FlushSafe::NeverRuns,
            ] {
                assert!(
                    reason.rules_out(code),
                    "{reason:?} is the stack's, which no code survives"
                );
            }
        }
    }

    #[test]
    fn the_rows_are_the_rules_of_the_three_codes() {
        assert_eq!(FlushCode::Action.rule(), "flush-in-action");
        assert_eq!(FlushCode::Effect.rule(), "flush-in-effect-callback");
        assert_eq!(FlushCode::Leaf.rule(), "leaf-owner-forbidden-call");
    }
}
