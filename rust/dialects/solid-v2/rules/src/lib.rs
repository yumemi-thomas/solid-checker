//! The Solid 2.0 rule catalog: projects the shared reactive IR's [`Program`]
//! onto 2.0 findings.
//!
//! Unprefixed external rule names belong to this catalog. Some defect concepts
//! are shared with Solid 1.x and retain the same `SCxxxx` code across both
//! catalogs; the messages and hints here always use 2.0 vocabulary such as
//! `Loading`, `onSettled`, `flush`, actions, and split effects. The remaining
//! rules cover 2.0-only runtime behavior: async computations, action/refresh
//! writes, returned cleanup, and scheduler restrictions.

mod rules;

use solid_reactive_ir::{
    CatalogCapabilities, CatalogWording, ExecutionRole, FindingSeed, FindingWording,
    LeafOwnerOperationKind, OwnerRequirementOperation, PackageContractIssue,
    PackageContractIssueKind, Program, ReactiveSourceKind, ReactiveWriteOperation, StaticDefect,
    StaticDefectKind, project_finding, project_findings, strict_read_evidence, strict_read_message,
};

pub use rules::{Rule, docs_url, manifest_json};
pub use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, SolveTimings};

struct Catalog;

#[must_use]
pub fn solve(program: &Program) -> Vec<Finding> {
    solve_measured(program).0
}

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
        CatalogCapabilities::SOLID_2
    }

    fn wording(&self, seed: FindingSeed<'_>) -> FindingWording {
        match seed {
            FindingSeed::StrictRead(read) => FindingWording::new(
                Rule::StrictReadUntracked.metadata(),
                strict_read_message(read),
                "Move the read into a tracking scope: JSX, a createMemo, or the compute function of createEffect(compute, apply). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit. Solid warns STRICT_READ_UNTRACKED here in dev.",
            )
            .with_evidence(strict_read_evidence(read)),
            FindingSeed::OwnedWrite(write) => owned_write_wording(write),
            FindingSeed::Action(action) => FindingWording::new(
                Rule::ActionCalledInOwnedScope.metadata(),
                format!(
                    "action {:?} is called inside owned scope {}; invoking an action starts a write transaction (optimistic writes, refresh) under a children-capable owner, which re-triggers the scope that called it",
                    action.action, action.context
                ),
                "Call the action from an event handler, onSettled, createTrackedEffect, or another imperative boundary; untrack() does not lift the restriction. To load data reactively you don't need an action: return the Promise from a computation and read it under a <Loading> boundary.",
            )
            .with_evidence(vec![EvidenceStep {
                message: "invoking an action starts a write transaction while a children-capable owner is active"
                    .into(),
                location: Some(action.location.clone()),
            }]),
            FindingSeed::LeafOperation(operation) => leaf_operation_wording(operation),
            FindingSeed::InvalidCleanupReturn(invalid) => FindingWording::new(
                Rule::InvalidCleanupReturn.metadata(),
                format!(
                    "{} callback returns a value that is not a cleanup function; Solid treats this return value as cleanup, and anything other than a function or undefined throws in dev",
                    invalid.primitive
                ),
                "Return a cleanup function or nothing at all. An async callback can never return valid cleanup because it implicitly returns a Promise; make the callback synchronous and start the async work inside it.",
            )
            .with_evidence(vec![EvidenceStep {
                message: "the callback statically returns a non-function value, including an implicit Promise from an async callback".into(),
                location: Some(invalid.location.clone()),
            }]),
            FindingSeed::UnresolvedCleanupReturn(unresolved) => FindingWording::new(
                Rule::CleanupReturnUnresolved.metadata(),
                format!(
                    "cannot prove that the {} callback returns only a cleanup function or undefined; an unresolved return value may throw at runtime",
                    unresolved.primitive
                ),
                "Make the return shape explicit at each return site: return a function literal, a named local function, or nothing. Returns of member expressions, call results, or values that cross files defeat this analysis.",
            )
            .with_evidence(vec![EvidenceStep {
                message: format!(
                    "the return value of the {} callback cannot be resolved statically",
                    unresolved.primitive
                ),
                location: Some(unresolved.location.clone()),
            }]),
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
                        "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists. For one-time setup with teardown, use onSettled with a returned cleanup in a component.",
                    ),
                    OwnerRequirementOperation::Boundary => (
                        Rule::NoOwnerBoundary,
                        "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                        "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
                    ),
                    OwnerRequirementOperation::SettledCleanup => (
                        Rule::NoOwnerSettledCleanup,
                        "onSettled returns a cleanup function in a scope with no owner to register it on; the cleanup is silently dropped and will never run",
                        "Call onSettled where an owner is active (a component body or computation), or wrap the scope in createRoot. Inside event handlers a returned cleanup is not supported; do the teardown explicitly instead.",
                    ),
                    OwnerRequirementOperation::Effect => (
                        Rule::NoOwnerEffect,
                        "effect is created without a reactive owner; nothing will ever dispose it, so it keeps running and holding its subscriptions for the lifetime of the app",
                        "Create effects inside a component or computation so their owner disposes them. For deliberate module-scope reactivity, wrap the setup in createRoot(dispose => ...) and keep the dispose handle.",
                    ),
                };
                FindingWording::new(rule.metadata(), message, hint)
            }
            FindingSeed::AsyncRead(read) => async_read_wording(read),
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
        }
    }
}

fn owned_write_wording(write: &solid_reactive_ir::ReactiveWrite) -> FindingWording {
    let context = if write.context.is_empty() {
        "owned scope"
    } else {
        &write.context
    };
    let (message, hint, provenance) = match write.operation {
        ReactiveWriteOperation::Refresh => (
            format!(
                "refresh() is called inside owned scope {context}; a write transaction cannot start while an owner is tracking, and Solid throws here in dev"
            ),
            "Move the refresh() call to an event handler, an action, or a leaf scope such as onSettled or createTrackedEffect; a recompute cannot be requested from inside a children-capable owner, and untrack() does not lift the restriction.".to_owned(),
            "the refresh target is a proven Solid source accessor or store".to_owned(),
        ),
        ReactiveWriteOperation::Setter => {
            let source = match write.source_kind {
                ReactiveSourceKind::Accessor => "accessor",
                ReactiveSourceKind::Store => "store",
            };
            (
                format!(
                    "{source} setter {:?} is called inside owned scope {context}; writes under a children-capable owner create feedback loops in the reactive graph, and Solid throws REACTIVE_WRITE_IN_OWNED_SCOPE here in dev",
                    write.setter
                ),
                "Derive the value instead of writing it back: replace compute-then-set with a createMemo. If the write is genuinely imperative, move it to an event handler, an action, the apply function of createEffect(compute, apply), or a leaf scope (onSettled, createTrackedEffect). Wrapping the write in untrack() does not help: the guard keys on the owner, not on tracking. For an internal reactive source only, opt in with { ownedWrite: true } in that source's creation options.".to_owned(),
                format!(
                    "{:?} is paired with a source proven to be a Solid {source}",
                    write.setter
                ),
            )
        }
    };
    FindingWording::new(
        Rule::ReactiveWriteInOwnedScope.metadata(),
        message,
        hint,
    )
    .with_evidence(vec![
        EvidenceStep {
            message: provenance,
            location: Some(write.declaration.clone()),
        },
        EvidenceStep {
            message: "this scope runs under a children-capable owner; writes are only legal outside one — event handlers, actions, effect apply callbacks, and the leaf scopes onSettled and createTrackedEffect — and untrack() keeps the enclosing owner".into(),
            location: Some(write.location.clone()),
        },
    ])
}

fn leaf_operation_wording(operation: &solid_reactive_ir::LeafOwnerOperation) -> FindingWording {
    let (rule, mut message, hint) = match &operation.kind {
        LeafOwnerOperationKind::Cleanup => (
            Rule::CleanupInForbiddenScope,
            format!(
                "onCleanup is called inside {}, a leaf owner that manages cleanup through its return value; Solid throws CLEANUP_IN_FORBIDDEN_SCOPE here in dev",
                operation.owner
            ),
            format!(
                "Return the cleanup function from the {} callback instead: do the setup, then return () => teardown().",
                operation.owner
            ),
        ),
        LeafOwnerOperationKind::Flush => (
            Rule::FlushInForbiddenScope,
            format!(
                "flush() is called inside {}, which runs as part of the flush cycle itself; the call would re-enter the scheduler, and Solid throws here in dev",
                operation.owner
            ),
            format!(
                "Inside {} the graph has already settled, so signal values and the DOM are current and the flush() is usually unnecessary. If you need to observe a write you just made, move both the write and the flush() to the event handler or imperative boundary that triggered this scope.",
                operation.owner
            ),
        ),
        LeafOwnerOperationKind::Primitive(primitive) => (
            Rule::PrimitiveInLeafOwner,
            format!(
                "reactive primitive {primitive} is created inside {}; {} is a leaf owner with no children, so nested primitives are never tracked or disposed, and Solid throws in dev",
                operation.owner, operation.owner
            ),
            format!(
                "Create the primitive in the component body (or another owning scope) and read its accessor inside {}.",
                operation.owner
            ),
        ),
    };
    let mut evidence = vec![EvidenceStep {
        message: format!(
            "the call is lexically contained by the {} callback",
            operation.owner
        ),
        location: Some(operation.location.clone()),
    }];
    if operation.uncertain {
        message.push_str(
            "; solid-checker cannot prove this call runs under a live children-capable owner (out-of-band the callback is a plain queued function and this does not throw), so the finding is a proof obligation",
        );
        evidence.push(EvidenceStep {
            message: format!(
                "the {} call site's owner context cannot be proven (exported helper or conditional owner)",
                operation.owner
            ),
            location: operation.call_site_gate.clone(),
        });
    }
    FindingWording::new(rule.metadata(), message, hint).with_evidence(evidence)
}

fn async_read_wording(read: &solid_reactive_ir::AsyncRead) -> FindingWording {
    // Declared first paint (probed against rc.0): a loadingValue /
    // seedLoadingValue node is born committed, so its *first flight* cannot
    // throw anywhere — but once the first real answer lands, a pending
    // re-ask (input change or refresh) throws for untracked and leaf reads
    // exactly like an undeclared node. SC5001/SC5002 therefore stay
    // reported on declared sources with this conditional wording, while
    // SC5003 never reaches this function for them (suppressed in selection).
    let declared = read.declared_loading;
    let (rule, message, hint) = if let Some(owner) = &read.leaf_owner {
        (
            Rule::PendingAsyncForbiddenScope,
            if declared {
                format!(
                    "async accessor {:?} declares a loadingValue, so its first flight serves the declared value here, but after the first real answer lands any pending re-ask read inside {} throws at runtime ({} runs after the graph settles and cannot suspend)",
                    read.accessor, owner, owner
                )
            } else {
                format!(
                    "pending async accessor {:?} is read inside {}, which runs after the graph settles and cannot suspend; a pending read here throws at runtime",
                    read.accessor, owner
                )
            },
            format!(
                "Settle the value before it reaches {owner}: read the accessor in the compute function of createEffect(compute, apply) and pass the resolved value through, or guard the scope so it only runs once the data is ready."
            ),
        )
    } else {
        match read.execution {
            ExecutionRole::ModuleInitialization | ExecutionRole::UntrackedRendering => (
                Rule::PendingAsyncUntrackedRead,
                if declared {
                    format!(
                        "async accessor {:?} declares a loadingValue, so this untracked read serves the declared value during the first flight, but after the first real answer lands a pending re-ask (input change or refresh) makes it throw PENDING_ASYNC_UNTRACKED_READ in dev",
                        read.accessor
                    )
                } else if read.options_opaque {
                    format!(
                        "async accessor {:?} may be read here while pending; its options argument cannot be read statically, so unless it declares a loadingValue this untracked read cannot suspend or retry and throws PENDING_ASYNC_UNTRACKED_READ in dev",
                        read.accessor
                    )
                } else {
                    format!(
                        "pending async accessor {:?} is read outside a tracking scope; an untracked read cannot suspend or retry, and Solid throws PENDING_ASYNC_UNTRACKED_READ in dev",
                        read.accessor
                    )
                },
                "Read async values where the graph can wait for them: JSX, a createMemo, or an effect's compute function. The read then suspends to the nearest <Loading> boundary and re-runs when the value settles.".to_owned(),
            ),
            ExecutionRole::TrackedJsx if read.ssr_client_hole && !read.under_loading => (
                Rule::SsrClientSourceOutsideLoadingBoundary,
                format!(
                    "source {:?} declares ssrSource: \"client\" with no loadingValue/seedLoadingValue, and this project server-renders; the server never runs the compute, so rendering this read outside a Loading boundary throws `ssrSource: \"client\" read during SSR outside a <Loading> boundary` during SSR — even when the compute is fully synchronous",
                    read.accessor
                ),
                "Wrap the reading subtree in <Loading fallback={...}> so the server can flush the fallback and hand the position to the client, or declare a loadingValue (loadingValue: undefined is valid; store-family sources use seedLoadingValue: true) so the server renders a provisional value instead.".to_owned(),
            ),
            ExecutionRole::TrackedJsx if !read.under_loading => (
                Rule::AsyncOutsideLoadingBoundary,
                format!(
                    "async accessor {:?} is rendered without a Loading boundary above it; while it is pending nothing renders, and the mount is deferred until all uncaught async settles (Solid dev warning ASYNC_OUTSIDE_LOADING_BOUNDARY)",
                    read.accessor
                ),
                "This is safe but shows nothing while loading. Wrap the reading subtree in <Loading fallback={...}> for visible fallback UI, or leave it as is if an empty container during load is intended. For a revalidation indicator, use isPending(() => ...) under the same boundary.".to_owned(),
            ),
            ExecutionRole::Unknown
            | ExecutionRole::TrackedJsx
            | ExecutionRole::DeferredCallback
            | ExecutionRole::UntrackedCallback
            | ExecutionRole::EffectApply
            | ExecutionRole::EventCallback
            | ExecutionRole::DirectiveApply => {
                panic!("async projector received a seed that selection should have excluded")
            }
        }
    };
    let mut provenance = if rule == Rule::SsrClientSourceOutsideLoadingBoundary {
        "the source declares ssrSource: \"client\" and no loadingValue/seedLoadingValue, and a server rendering entry point is imported in this project".to_owned()
    } else {
        "the accessor is returned by an async computation".to_owned()
    };
    if read.options_opaque && rule == Rule::PendingAsyncUntrackedRead {
        provenance.push_str(
            "; the source's options argument cannot be read statically, so a loadingValue declaration (which would make the first flight safe) can be neither proven nor ruled out — this finding is a proof obligation, not a proven throw",
        );
    }
    FindingWording::new(rule.metadata(), message.clone(), hint).with_evidence(vec![
        EvidenceStep {
            message: provenance,
            location: Some(read.declaration.clone()),
        },
        EvidenceStep {
            message,
            location: Some(read.location.clone()),
        },
    ])
}

fn static_violation_wording(violation: &solid_reactive_ir::StaticViolation) -> FindingWording {
    let rule = Rule::from_identity(&violation.id, &violation.rule).unwrap_or_else(|| {
        panic!(
            "diagnostic identity is missing from the rule catalog: {} [{}]",
            violation.id, violation.rule
        )
    });
    let evidence = match rule {
        Rule::SyncNodeReceivedAsync => {
            "a sync computation is proven capable of returning Promise or AsyncIterable"
        }
        Rule::InvalidRefreshTarget => {
            "the refresh call's arity or target shape violates the branded-source contract"
        }
        Rule::InvalidAffectsTarget => {
            "the affects call's arity or target shape violates the branded-source contract"
        }
        Rule::AffectsKeysOnAccessor => {
            "the proven target is an accessor, but the call also supplies store path keys"
        }
        Rule::RefreshTargetUnresolved => {
            "the refresh target has no provenance tying it to a branded Solid source"
        }
        Rule::AffectsTargetUnresolved => {
            "the affects target has no provenance tying it to a branded Solid source"
        }
        Rule::StrictReadUntracked
        | Rule::ReactiveReadAfterAwait
        | Rule::UncalledAccessor
        | Rule::UntrackedDerivedFunction
        | Rule::ExpectedFunctionGotExpression
        | Rule::NoDirectMutation
        | Rule::ReactiveSourceUncaptured
        | Rule::ComponentPropsDestructure
        | Rule::ComponentReturnsConditionally
        | Rule::PreferComponentSyntax
        | Rule::NoImplicitDraggable
        | Rule::ValidJsxNesting
        | Rule::ReactiveWriteInOwnedScope
        | Rule::ActionCalledInOwnedScope
        | Rule::CleanupInForbiddenScope
        | Rule::PrimitiveInLeafOwner
        | Rule::FlushInForbiddenScope
        | Rule::InvalidCleanupReturn
        | Rule::NoOwnerEffect
        | Rule::NoOwnerCleanup
        | Rule::NoOwnerBoundary
        | Rule::NoOwnerSettledCleanup
        | Rule::PendingAsyncUntrackedRead
        | Rule::PendingAsyncForbiddenScope
        | Rule::AsyncOutsideLoadingBoundary
        | Rule::SsrClientSourceOutsideLoadingBoundary
        | Rule::PrimitiveInDirectiveApplication
        | Rule::MissingEffectFunction
        | Rule::PackageContractExportMissing
        | Rule::PackageContractCallbackMissing
        | Rule::PackageContractMissing
        | Rule::CleanupReturnUnresolved
        | Rule::ExecutionMapIncomplete => panic!(
            "rule {} is not emitted through the static-violation channel",
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
        StaticDefectKind::ReactiveObjectDestructure { .. } => Rule::ComponentPropsDestructure,
        StaticDefectKind::ReactiveReadAfterAwait { .. } => Rule::ReactiveReadAfterAwait,
        StaticDefectKind::ComponentReturnsConditionally => Rule::ComponentReturnsConditionally,
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
    let text = solid_reactive_ir::static_defect_text(defect, &V2_STATIC_TERMS);
    FindingWording::new(rule.metadata(), text.message, text.hint).with_evidence(vec![
        EvidenceStep {
            message: text.evidence.into(),
            location: Some(defect.location.clone()),
        },
    ])
}

const V2_STATIC_TERMS: solid_reactive_ir::StaticDefectTerms =
    solid_reactive_ir::StaticDefectTerms {
        props_destructure_hint: "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use omit(props, ...keys) and merge(defaults, props) instead of destructuring.",
        reactive_object_destructure_hint: "Keep the reactive object intact and read object.<name> inside JSX or a tracked computation. A property access made there remains subscribed; a setup-time destructuring binding does not.",
        missing_effect_message: "createEffect is called without an effect function; the signature is createEffect(compute, apply), where compute tracks dependencies and returns a value, and apply receives that value and performs the side effect",
        missing_effect_hint: "Split the callback: reactive reads go in the compute function, the side effect in the apply function, and cleanup is returned from apply. For error handling, pass { effect, error } as the second argument.",
        tracked_derived_scope: "JSX, a createMemo, or the compute function of createEffect(compute, apply)",
        store_mutation_hint: v2_store_mutation_hint,
    };

fn v2_store_mutation_hint(name: &str) -> String {
    format!(
        "Write through the store's setter: setStore(store => {{ store.key = value; }}) mutates the draft in place, and setStore(reconcile(next)) replaces it wholesale. Direct assignment to {name} does not notify subscribers."
    )
}
