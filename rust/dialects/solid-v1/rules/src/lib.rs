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
    CatalogCapabilities, CatalogWording, FindingSeed, FindingWording, LeafOwnerOperationKind,
    OwnerRequirementOperation, PackageContractIssue, PackageContractIssueKind, Program,
    ReactiveSourceKind, ReactiveWriteOperation, StaticDefect, StaticDefectKind, project_finding,
    project_findings, strict_read_evidence, strict_read_message,
};

pub use rules::{Rule, docs_url, manifest_json};
pub use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, SolveTimings};

struct Catalog;

#[must_use]
pub fn solve_measured(program: &Program) -> (Vec<Finding>, SolveTimings) {
    project_findings(program, &Catalog)
}

#[must_use]
pub fn package_contract_finding(issue: &PackageContractIssue) -> Finding {
    project_finding(FindingSeed::PackageContractIssue(issue), &Catalog)
}

impl CatalogWording for Catalog {
    fn capabilities(&self) -> CatalogCapabilities {
        CatalogCapabilities::SOLID_1
    }

    fn wording(&self, seed: FindingSeed<'_>) -> FindingWording {
        match seed {
            FindingSeed::StrictRead(read) => FindingWording::new(
                Rule::StrictReadUntracked.metadata(),
                strict_read_message(read),
                "Move the read into a tracking scope: JSX, a createMemo, or the callback of createEffect(fn). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit.",
            )
            .with_evidence(strict_read_evidence(read)),
            FindingSeed::OwnedWrite(write) => owned_write_wording(write),
            FindingSeed::LeafOperation(operation) => {
                let (rule, message, hint) = match &operation.kind {
                    LeafOwnerOperationKind::Cleanup => (
                        Rule::CleanupInForbiddenScope,
                        format!(
                            "onCleanup is called inside {}, whose callback runs as a leaf with no owner to register cleanup on; the cleanup function will never run",
                            operation.owner
                        ),
                        format!(
                            "Register the cleanup in the computation that owns the {} instead, or create the surrounding scope with createRoot so disposal exists.",
                            operation.owner
                        ),
                    ),
                    LeafOwnerOperationKind::Primitive(primitive) => (
                        Rule::PrimitiveInLeafOwner,
                        format!(
                            "reactive primitive {primitive} is created inside {}; {} runs its callback as a leaf owner with no children, so nested primitives are never tracked or disposed",
                            operation.owner, operation.owner
                        ),
                        format!(
                            "Create the primitive in the component body (or another owning scope) and read its accessor inside {}.",
                            operation.owner
                        ),
                    ),
                    LeafOwnerOperationKind::Flush => {
                        panic!("Solid 1.x analysis emitted a 2.0-only flush leaf operation")
                    }
                };
                FindingWording::new(rule.metadata(), message, hint).with_evidence(vec![
                    EvidenceStep {
                        message: format!(
                            "the call is lexically contained by the {} callback",
                            operation.owner
                        ),
                        location: Some(operation.location.clone()),
                    },
                ])
            }
            FindingSeed::StaticDefect(defect) => static_defect_wording(defect),
            FindingSeed::StaticViolation(violation) => static_violation_wording(violation),
            FindingSeed::DirectiveCreation(creation) => FindingWording::new(
                Rule::PrimitiveInDirectiveApplication.metadata(),
                format!(
                    "reactive primitive {} is created in a directive application callback; the apply phase runs per element as an unowned leaf, so primitives created here are never tracked or disposed",
                    creation.primitive
                ),
                "Use the two-phase directive factory: create primitives and subscriptions in the setup phase (the factory body, which runs in an owned scope) and keep the returned ref callback to DOM work only.",
            )
            .with_evidence(vec![EvidenceStep {
                message: if creation.returned_closure {
                    "the primitive is created inside the callback returned to a compiler-recognized ref application".into()
                } else {
                    "the primitive is created inside a compiler-recognized ref application callback".into()
                },
                location: Some(creation.location.clone()),
            }]),
            FindingSeed::OwnerRequirement(requirement) => {
                let (rule, message, hint) = match requirement.operation {
                    OwnerRequirementOperation::Cleanup => (
                        Rule::NoOwnerCleanup,
                        "onCleanup is called without a reactive owner; no scope's disposal can trigger it, so this cleanup function will never run",
                        "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists.",
                    ),
                    OwnerRequirementOperation::Boundary => (
                        Rule::NoOwnerBoundary,
                        "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                        "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
                    ),
                    OwnerRequirementOperation::Effect => (
                        Rule::NoOwnerEffect,
                        "effect is created without a reactive owner; nothing will ever dispose it, so it keeps running and holding its subscriptions for the lifetime of the app",
                        "Create effects inside a component or computation so their owner disposes them. For deliberate module-scope reactivity, wrap the setup in createRoot(dispose => ...) and keep the dispose handle.",
                    ),
                    OwnerRequirementOperation::SettledCleanup => panic!(
                        "Solid 1.x analysis emitted a 2.0-only settled-cleanup requirement"
                    ),
                };
                FindingWording::new(rule.metadata(), message, hint)
            }
            FindingSeed::PackageContractIssue(issue) => {
                let (condition, hint) = match issue.status {
                    PackageContractIssueKind::Unverified => (
                        "has only an unverified generated reactivity contract",
                        "Verify the generated contract against the exact package artifacts and behavioral probes, then record verified, reviewed, or attested evidence.".into(),
                    ),
                    PackageContractIssueKind::Missing => (
                        "has no reactivity contract",
                        format!(
                            "Create a local contract at {}, or pass one explicitly with --contract <PATH>. If you maintain {}, ship solid-reactivity.json in the package root so every consumer gets it. See docs/package-contracts.md for the format.",
                            issue.contract_path, issue.package
                        ),
                    ),
                };
                FindingWording::new(
                    Rule::PackageContractMissing.metadata(),
                    format!(
                        "imported Solid package {:?} {condition}; solid-checker cannot rely on its export summaries, so every use of them is uncertifiable",
                        issue.package
                    ),
                    hint,
                )
            }
            FindingSeed::Action(_)
            | FindingSeed::AsyncRead(_)
            | FindingSeed::InvalidCleanupReturn(_)
            | FindingSeed::UnresolvedCleanupReturn(_) => {
                panic!("Solid 1.x projector received a disabled 2.0-only finding seed")
            }
        }
    }
}

fn owned_write_wording(write: &solid_reactive_ir::ReactiveWrite) -> FindingWording {
    match write.operation {
        ReactiveWriteOperation::Refresh => {
            panic!("Solid 1.x analysis emitted a 2.0-only refresh write")
        }
        ReactiveWriteOperation::Setter => {}
    }
    let context = if write.context.is_empty() {
        "owned scope"
    } else {
        &write.context
    };
    let source = match write.source_kind {
        ReactiveSourceKind::Accessor => "accessor",
        ReactiveSourceKind::Store => "store",
    };
    FindingWording::new(
        Rule::ReactiveWriteInOwnedScope.metadata(),
        format!(
            "{source} setter {:?} is called inside owned scope {context}; writes during the tracking phase re-trigger the computation that made them and can loop the reactive graph",
            write.setter
        ),
        "Derive the value instead of writing it back: replace compute-then-set with a createMemo. If the write is genuinely imperative, move it to an event handler, onMount, or a callback that runs after the current computation.",
    )
    .with_evidence(vec![
        EvidenceStep {
            message: format!(
                "{:?} is paired with a source proven to be a Solid {source}",
                write.setter
            ),
            location: Some(write.declaration.clone()),
        },
        EvidenceStep {
            message: "this scope is owned (tracking phase); writes are only allowed in event handlers, onMount, untracked or deferred callbacks, and directive bodies".into(),
            location: Some(write.location.clone()),
        },
    ])
}

fn static_violation_wording(violation: &solid_reactive_ir::StaticViolation) -> FindingWording {
    let rule = Rule::from_identity(&violation.id, &violation.rule).unwrap_or_else(|| {
        panic!(
            "diagnostic identity is missing from the v1 rule catalog: {} [{}]",
            violation.id, violation.rule
        )
    });
    let evidence = match rule {
        Rule::EventHandlers => {
            "the native JSX attribute spelling does not match Solid's event-handler contract"
        }
        Rule::Imports => "the import resolves to a module that does not own this Solid export",
        Rule::JsxNoDuplicateProps => "the same JSX property is assigned more than once",
        Rule::JsxNoScriptUrl => "the statically resolved URL uses the javascript: scheme",
        Rule::JsxNoUndef => "the JSX name has no value-space binding in lexical scope",
        Rule::NoArrayHandlers => "the native event attribute receives an array-valued handler",
        Rule::NoInnerhtml => "the JSX attribute writes markup through an HTML injection sink",
        Rule::NoProxyApis => "the import or call requires Proxy-backed Solid APIs",
        Rule::NoReactDeps => "a Solid reactive primitive received a React-style dependency array",
        Rule::NoReactSpecificProps => "the JSX attribute uses a React-specific property spelling",
        Rule::NoUnknownNamespaces => "the JSX namespace is outside Solid's known vocabulary",
        Rule::PreferClasslist => "a class helper call matches the configured classList preference",
        Rule::PreferFor => "an array map call is used directly in a JSX rendering position",
        Rule::PreferShow => "a conditional JSX expression matches the configured Show preference",
        Rule::SelfClosingComp => {
            "the element's child and closing-tag shape conflicts with the configured policy"
        }
        Rule::StyleProp => "the style attribute shape conflicts with Solid's style contract",
        Rule::NoAsyncTrackedScope => {
            "the tracked callback is syntactically async and can continue after an await"
        }
        Rule::StrictReadUntracked
        | Rule::ReactiveReadAfterAwait
        | Rule::NoDestructure
        | Rule::ComponentsReturnOnce
        | Rule::ReactiveWriteInOwnedScope
        | Rule::CleanupInForbiddenScope
        | Rule::PrimitiveInLeafOwner
        | Rule::NoOwnerEffect
        | Rule::NoOwnerCleanup
        | Rule::NoOwnerBoundary
        | Rule::PrimitiveInDirectiveApplication
        | Rule::MissingEffectFunction
        | Rule::UncalledAccessor
        | Rule::UntrackedDerivedFunction
        | Rule::ExpectedFunctionGotExpression
        | Rule::NoDirectMutation
        | Rule::ReactiveSourceUncaptured
        | Rule::PreferComponentSyntax
        | Rule::NoImplicitDraggable
        | Rule::ValidJsxNesting
        | Rule::JsxUsesVars
        | Rule::PackageContractExportMissing
        | Rule::PackageContractCallbackMissing
        | Rule::PackageContractMissing
        | Rule::ExecutionMapIncomplete => panic!(
            "v1 rule {} is not emitted through the static-violation channel",
            rule.metadata().name
        ),
    };
    FindingWording::new(
        rule.metadata(),
        violation.message.clone(),
        violation.hint.clone(),
    )
    .with_evidence(vec![EvidenceStep {
        message: evidence.into(),
        location: Some(violation.location.clone()),
    }])
}

fn static_defect_wording(defect: &StaticDefect) -> FindingWording {
    let rule = match &defect.kind {
        StaticDefectKind::ExecutionMapIncomplete => Rule::ExecutionMapIncomplete,
        StaticDefectKind::ReactiveObjectDestructure { .. } => Rule::NoDestructure,
        StaticDefectKind::ReactiveReadAfterAwait { .. } => Rule::ReactiveReadAfterAwait,
        StaticDefectKind::ComponentReturnsConditionally => Rule::ComponentsReturnOnce,
        StaticDefectKind::PreferComponentSyntax { .. } => Rule::PreferComponentSyntax,
        StaticDefectKind::ImplicitDraggableBoolean => Rule::NoImplicitDraggable,
        StaticDefectKind::InvalidJsxNesting { .. } => Rule::ValidJsxNesting,
        StaticDefectKind::PackageContractExportMissing { .. } => Rule::PackageContractExportMissing,
        StaticDefectKind::UnknownCallbackExecution { .. } => Rule::PackageContractCallbackMissing,
        StaticDefectKind::MissingEffectFunction => Rule::MissingEffectFunction,
        StaticDefectKind::UntrackedDerivedFunction { .. } => Rule::UntrackedDerivedFunction,
        StaticDefectKind::ReactiveSourceUncaptured { .. } => Rule::ReactiveSourceUncaptured,
        StaticDefectKind::ReactiveHandlerRead { .. }
        | StaticDefectKind::HandlerCallResult { .. } => Rule::ExpectedFunctionGotExpression,
        StaticDefectKind::UncalledAccessor { .. } => Rule::UncalledAccessor,
        StaticDefectKind::DirectMutation { .. } => Rule::NoDirectMutation,
    };
    let text = solid_reactive_ir::static_defect_text(defect, &V1_STATIC_TERMS);
    FindingWording::new(rule.metadata(), text.message, text.hint).with_evidence(vec![
        EvidenceStep {
            message: text.evidence.into(),
            location: Some(defect.location.clone()),
        },
    ])
}

const V1_STATIC_TERMS: solid_reactive_ir::StaticDefectTerms =
    solid_reactive_ir::StaticDefectTerms {
        props_destructure_hint: "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use splitProps(props, ...keys) and mergeProps(defaults, props) instead of destructuring.",
        reactive_object_destructure_hint: "Keep the reactive object intact and read object.<name> inside JSX or a tracked computation. A property access made there remains subscribed; a setup-time destructuring binding does not.",
        missing_effect_message: "createEffect is called without an effect function; the signature is createEffect(fn, value?), where fn tracks dependencies and runs the side effect, and the optional value seeds the previous value passed to fn on its first run",
        missing_effect_hint: "Pass the effect function as the first argument. Reads inside it are tracked, and cleanup is registered with onCleanup rather than returned.",
        tracked_derived_scope: "JSX, a createMemo, or a createEffect callback",
        store_mutation_hint: v1_store_mutation_hint,
    };

fn v1_store_mutation_hint(name: &str) -> String {
    format!(
        "Write through the store's setter: setStore(\"key\", value), or produce(draft => ...) for an in-place update. Direct assignment to {name} does not notify subscribers."
    )
}
