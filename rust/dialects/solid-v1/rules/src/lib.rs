//! The Solid 1.x rule catalog: projects the reactive IR's `Program` onto the
//! dialect's findings.
//!
//! The engine's analysis is shared with the 2.0 dialect; what is dialect-
//! specific here is which tables can fire at all under 1.x vocabulary, the
//! external rule names (`v1/<rule>`), and every sentence of message and hint —
//! a 1.x diagnostic never tells its reader to call an API their Solid version
//! does not have.

mod rules;

use solid_reactive_ir::{
    Program, StaticDefect, StaticDefectKind, direct_mutation_wording, finish_findings,
    static_violation_finding, strict_read_evidence, strict_read_message,
    strict_read_related_locations,
};
use std::time::Instant;

pub use rules::{Rule, docs_url, manifest_json};
pub use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, SolveTimings};

#[must_use]
pub fn solve_measured(program: &Program) -> (Vec<Finding>, SolveTimings) {
    let total_started = Instant::now();
    let construction_started = Instant::now();
    // Tables the 1.x vocabulary can never populate are deliberately not read:
    // `actions` and `async_reads` come from 2.0-only primitives, and the
    // cleanup-return tables from the returned-cleanup form 1.x does not have
    // (`accepts_cleanup_return` is empty for every 1.x primitive).
    let mut findings = program
        .reads
        .iter()
        .filter(|read| read.execution.reports_untracked_read())
        .map(|read| Finding {
            analysis_context: read.context.to_string(),
            subject_kind: read.kind.to_string(),
            related_locations: strict_read_related_locations(read),
            evidence: strict_read_evidence(read),
            hint: "Move the read into a tracking scope: JSX, a createMemo, or the callback of createEffect(fn). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit.".into(),
            ..Finding::new(
                Rule::StrictReadUntracked.metadata(),
                strict_read_message(read),
                read.location.clone(),
            )
        })
        .collect::<Vec<_>>();
    findings.extend(
        program
            .writes
            .iter()
            .filter(|write| !write.allowed_by_option && !write.execution.permits_write())
            .map(|write| {
                let context = if write.context.is_empty() {
                    "owned scope"
                } else {
                    &write.context
                };
                Finding {
                    analysis_context: context.into(),
                    related_locations: vec![write.declaration.clone()],
                    evidence: vec![
                        EvidenceStep {
                            message: format!(
                                "{:?} is the setter returned by createSignal or createStore",
                                write.setter
                            ),
                            location: Some(write.declaration.clone()),
                        },
                        EvidenceStep {
                            message: "this scope is owned (tracking phase); writes are only allowed in event handlers, onMount, untracked or deferred callbacks, and directive bodies"
                                .into(),
                            location: Some(write.location.clone()),
                        },
                    ],
                    hint: "Derive the value instead of writing it back: replace compute-then-set with a createMemo. If the write is genuinely imperative, move it to an event handler, onMount, or a callback that runs after the current computation.".into(),
                    ..Finding::new(
                        Rule::ReactiveWriteInOwnedScope.metadata(),
                        format!(
                            "signal setter {:?} is called inside owned scope {context}; writes during the tracking phase re-trigger the computation that made them and can loop the reactive graph",
                            write.setter
                        ),
                        write.location.clone(),
                    )
                }
            }),
    );
    findings.extend(program.leaf_operations.iter().map(|operation| {
        let (rule, message, hint) = if operation.primitive == "onCleanup" {
            (
                Rule::CleanupInForbiddenScope,
                format!(
                    "onCleanup is called inside {}, whose callback runs as a leaf with no owner to register cleanup on; the cleanup function will never run",
                    operation.owner
                ),
                format!(
                    "Register the cleanup in the computation that owns the {} instead, or create the surrounding scope with createRoot so disposal exists.",
                    operation.owner
                ),
            )
        } else {
            (
                Rule::PrimitiveInLeafOwner,
                format!(
                    "reactive primitive {} is created inside {}; {} runs its callback as a leaf owner with no children, so nested primitives are never tracked or disposed",
                    operation.primitive, operation.owner, operation.owner
                ),
                format!(
                    "Create the primitive in the component body (or another owning scope) and read its accessor inside {}.",
                    operation.owner
                ),
            )
        };
        Finding {
            evidence: vec![EvidenceStep {
                message: format!(
                    "the call is lexically contained by the {} callback",
                    operation.owner
                ),
                location: Some(operation.location.clone()),
            }],
            fixes: operation.fix.clone().into_iter().collect(),
            hint,
            ..Finding::new(rule.metadata(), message, operation.location.clone())
        }
    }));
    findings.extend(program.static_defects.iter().map(static_defect_finding));
    findings.extend(program.static_violations.iter().map(|violation| {
        static_violation_finding(violation, "the v1 rule catalog", |code, name| {
            Rule::from_identity(code, name).map(Rule::metadata)
        })
    }));
    findings.extend(program.directive_creations.iter().map(|creation| Finding {
        evidence: vec![EvidenceStep {
            message: if creation.returned_closure {
                "the primitive is created inside the callback returned to a compiler-recognized ref application".into()
            } else {
                "the primitive is created inside a compiler-recognized ref application callback".into()
            },
            location: Some(creation.location.clone()),
        }],
        hint: "Use the two-phase directive factory: create primitives and subscriptions in the setup phase (the factory body, which runs in an owned scope) and keep the returned ref callback to DOM work only.".into(),
        ..Finding::new(
            Rule::PrimitiveInDirectiveApplication.metadata(),
            format!(
                "reactive primitive {} is created in a directive application callback; the apply phase runs per element as an unowned leaf, so primitives created here are never tracked or disposed",
                creation.primitive
            ),
            creation.location.clone(),
        )
    }));
    findings.extend(program.missing_owners.iter().filter_map(|requirement| {
        if !requirement.report {
            return None;
        }
        let (rule, message, hint) = match requirement.operation.as_str() {
            "cleanup" => (
                Rule::NoOwnerCleanup,
                "onCleanup is called without a reactive owner; no scope's disposal can trigger it, so this cleanup function will never run",
                "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists.",
            ),
            "boundary" => (
                Rule::NoOwnerBoundary,
                "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
            ),
            _ => (
                Rule::NoOwnerEffect,
                "effect is created without a reactive owner; nothing will ever dispose it, so it keeps running and holding its subscriptions for the lifetime of the app",
                "Create effects inside a component or computation so their owner disposes them. For deliberate module-scope reactivity, wrap the setup in createRoot(dispose => ...) and keep the dispose handle.",
            ),
        };
        Some(Finding::for_owner_requirement(
            rule.metadata(),
            requirement,
            message,
            hint,
        ))
    }));
    finish_findings(findings, total_started, construction_started)
}

fn static_defect_finding(defect: &StaticDefect) -> Finding {
    let (rule, message, hint) = match &defect.kind {
        StaticDefectKind::ExecutionMapIncomplete => (
            Rule::ExecutionMapIncomplete,
            "the Solid compiler did not classify this JSX expression as tracked, untracked, or a callback; without an execution role, solid-checker cannot certify any reactive read inside it".into(),
            "Simplify the expression: hoist complex logic into a createMemo and interpolate the accessor. If this persists on plain JSX, re-run with fresh compiler facts and report the pattern as a solid-checker issue.".into(),
        ),
        StaticDefectKind::ComponentPropsDestructure => (
            Rule::NoDestructure,
            "destructuring props unwraps each property once at component setup; the bindings are frozen values, and the component never updates when the parent passes new props".into(),
            "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use splitProps(props, ...keys) and mergeProps(defaults, props) instead of destructuring.".into(),
        ),
        StaticDefectKind::ReactiveReadAfterAwait { accessor } => (
            Rule::ReactiveReadAfterAwait,
            format!(
                "reactive accessor {accessor:?} is read after an await; dependency tracking ends at the first await, so this read registers no dependency and the computation never re-runs when {accessor:?} changes"
            ),
            "Read reactive values before the first await and carry the results through the async work. If the value must stay live after the await, split the read into its own synchronous computation.".into(),
        ),
        StaticDefectKind::ComponentReturnsConditionally => (
            Rule::ComponentsReturnOnce,
            "this component's return value depends on a reactive condition, but a component body runs once; whichever branch is taken at setup renders forever, and the condition is never re-evaluated".into(),
            "Return a single JSX tree and move the branch into it: wrap the alternatives in <Show when={...} fallback={...}> (or <Switch>/<Match> for multiple cases), or use a ternary inside JSX where it stays tracked.".into(),
        ),
        StaticDefectKind::PackageContractExportMissing {
            module,
            export,
            reexported,
        } => (
            Rule::PackageContractExportMissing,
            format!(
                "the reactivity contract for {module} has no entrypoint/export summary for {} export {export}; solid-checker cannot tell whether it reads reactive values, takes tracked callbacks, or returns accessors, so code flowing through it cannot be certified",
                if *reexported { "re-exported" } else { "imported" }
            ),
            format!(
                "Add an export summary for {export} to the package's solid-reactivity.json (reactive reads, callbacks, return kind); an empty summary certifies explicitly that the export is not reactive. See docs/package-contracts.md for the format."
            ),
        ),
        StaticDefectKind::MissingEffectFunction => (
            Rule::MissingEffectFunction,
            "createEffect is called without an effect function; the signature is createEffect(fn, value?), where fn tracks dependencies and runs the side effect, and the optional value seeds the previous value passed to fn on its first run".into(),
            "Pass the effect function as the first argument. Reads inside it are tracked, and cleanup is registered with onCleanup rather than returned.".into(),
        ),
        StaticDefectKind::UntrackedDerivedFunction { name } => (
            Rule::UntrackedDerivedFunction,
            format!(
                "{name} derives from reactive state but every call to it is untracked, so its reads subscribe to nothing and the derivation never updates"
            ),
            format!(
                "Call {name} from a tracking scope — JSX, a createMemo, or a createEffect callback — or inline the value if a one-off read at setup is what was meant."
            ),
        ),
        StaticDefectKind::ReactiveSourceUncaptured { source, callee } => (
            Rule::ReactiveSourceUncaptured,
            format!(
                "the reactive source {source:?} is passed to {callee}, whose reactive behaviour is not described anywhere: it has no body in this project, no package contract entry, and is not a Solid primitive; whether reads through it stay tracked cannot be certified"
            ),
            format!(
                "Describe {callee} in its package's solid-reactivity.json — which arguments it tracks and what it returns — or keep the function in the project so its body is analysed. See docs/package-contracts.md."
            ),
        ),
        StaticDefectKind::ReactiveHandlerRead {
            attribute,
            expression,
        } => (
            Rule::ExpectedFunctionGotExpression,
            format!(
                "{attribute} reads {expression} once during DOM setup; later reactive updates cannot replace the installed listener"
            ),
            format!(
                "Wrap the read so it happens when the event fires: {attribute}={{event => {expression}(event)}}."
            ),
        ),
        StaticDefectKind::HandlerCallResult {
            attribute,
            callee,
            call,
        } => (
            Rule::ExpectedFunctionGotExpression,
            format!(
                "{attribute} is given the result of calling {callee}, not a function; the call runs once during setup and its value is bound as the listener"
            ),
            format!("Wrap it: {attribute}={{() => {call}}}, or pass the function itself uncalled."),
        ),
        StaticDefectKind::UncalledAccessor { name, position } => (
            Rule::UncalledAccessor,
            format!(
                "accessor {name:?} is used as a value in {position}; the expression receives the accessor function itself, not the value it holds, and never updates"
            ),
            format!(
                "Call it: {name}(). Passing {name} uncalled is only correct where the receiver calls it later."
            ),
        ),
        StaticDefectKind::DirectMutation { name, target } => {
            // Only the store hint is dialect-specific: 1.x writes through
            // path setters and `produce`.
            let (message, hint) = direct_mutation_wording(name, *target, |name| {
                format!(
                    "Write through the store's setter: setStore(\"key\", value), or produce(draft => ...) for an in-place update. Direct assignment to {name} does not notify subscribers."
                )
            });
            (Rule::NoDirectMutation, message, hint)
        }
    };
    let evidence = match &defect.kind {
        StaticDefectKind::ComponentPropsDestructure => {
            "the destructuring pattern is bound to proven component props"
        }
        StaticDefectKind::ComponentReturnsConditionally => {
            "a proven reactive read controls the component's return shape"
        }
        StaticDefectKind::PackageContractExportMissing { .. } => {
            "the imported package has a contract, but this export has no effect summary"
        }
        _ => "the invalid API shape is statically present at this call",
    };
    Finding {
        analysis_context: defect.analysis_context.clone(),
        evidence: vec![EvidenceStep {
            message: evidence.into(),
            location: Some(defect.location.clone()),
        }],
        fixes: defect.fixes.clone(),
        hint,
        ..Finding::new(rule.metadata(), message, defect.location.clone())
    }
}
