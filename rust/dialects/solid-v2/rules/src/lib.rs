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
    StaticDefectFamily, StaticDefectKind, project_finding, project_findings, strict_read_evidence,
    strict_read_message,
};

pub use rules::{Rule, docs_url, manifest_json};
pub use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, SolveTimings};

struct Catalog;

pub const CATALOG_CAPABILITIES: CatalogCapabilities = CatalogCapabilities::SOLID_2;

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

// The trailing sentence is the only clause that differs between a proven
// violation and an uncertifiable finding, so it is split out here rather than
// duplicating the whole hint body per kind. A violation's read is proven
// untracked in a proven tracking context, so Solid provably warns there — the
// flat "Solid warns ... here in dev" claim is earned. An uncertifiable read is
// not: it rests on unenumerable callers (`ReactiveRead::uncertain`), a census
// hole (`ReactiveRead::missing_jsx_census`), and in either case the flat claim
// asserts a runtime behavior the finding's own message says cannot be proven.
// The conditional phrasing stays true regardless of which uncertainty caused
// the finding.
const STRICT_READ_UNTRACKED_WARNS: &str = "Solid warns STRICT_READ_UNTRACKED here in dev.";
const STRICT_READ_UNTRACKED_MAY_WARN: &str = "If this read executes untracked in dev, Solid warns STRICT_READ_UNTRACKED for it; this finding does not establish that it does, so confirm the read's actual execution and tracking status before relying on that warning.";

fn strict_read_hint(kind: &str, uncertifiable: bool) -> String {
    let warning = if uncertifiable {
        STRICT_READ_UNTRACKED_MAY_WARN
    } else {
        STRICT_READ_UNTRACKED_WARNS
    };
    if kind == "component-props" {
        format!(
            "Move the prop read into a tracking scope: read props.<name> directly in JSX, derive it with createMemo(() => props.<name>), or read it in the compute function of createEffect(compute, apply). Do not use untrack() to make a prop reactive; untrack() only documents an intentional one-time snapshot. {warning}"
        )
    } else {
        format!(
            "Move the read into a tracking scope: JSX, a createMemo, or the compute function of createEffect(compute, apply). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit. {warning}"
        )
    }
}

impl CatalogWording for Catalog {
    fn capabilities(&self) -> CatalogCapabilities {
        CATALOG_CAPABILITIES
    }

    fn wording(&self, seed: FindingSeed<'_>) -> FindingWording {
        match seed {
            FindingSeed::StrictRead(read) => {
                let mut message = strict_read_message(read);
                if read.uncertain {
                    // Signal-backing is a caller-decided fact in 2.0 (probed:
                    // devComponent's strict-read window only warns for props
                    // whose getter reads reactive state). When the callers
                    // cannot be enumerated, this is a proof obligation.
                    message.push_str(
                        "; this component's call sites cannot be enumerated (it is exported, spread into, or referenced outside JSX), so whether the prop is signal-backed can be neither proven nor ruled out — this finding is a proof obligation, not a proven runtime warning",
                    );
                }
                // The same predicate projection.rs sets `finding.kind =
                // "uncertifiable"` from — one owner, so the hint and the kind
                // cannot disagree on which findings are proven.
                let uncertifiable = read.is_uncertifiable();
                FindingWording::new(
                    Rule::StrictReadUntracked.metadata(),
                    message,
                    strict_read_hint(&read.kind, uncertifiable),
                )
                .with_evidence(strict_read_evidence(read))
            }
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
            FindingSeed::StaticDefect(defect) => static_defect_wording(defect),
            FindingSeed::StaticViolation(violation) => static_violation_wording(violation),
            FindingSeed::DirectiveCreation(creation) => FindingWording::new(
                Rule::PrimitiveInDirectiveApplication.metadata(),
                format!(
                    "reactive primitive {} registers a computation in a directive application callback; the apply phase runs once per element with no owner, so the computation is never disposed and leaks for every element the directive is applied to (the dev runtime warns NO_OWNER for it)",
                    creation.primitive
                ),
                "Use the two-phase directive factory: create computations and subscriptions in the setup phase (the factory body, which runs in an owned scope) and keep the returned ref callback to DOM work only. Value-form state (createSignal(0), createStore({...})) needs no owner and is fine here.",
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
                let (message, hint) = match requirement.operation {
                    OwnerRequirementOperation::Cleanup => (
                        "onCleanup is called without a reactive owner; no scope's disposal can trigger it, so this cleanup function will never run",
                        "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists. For one-time setup with teardown, use onSettled with a returned cleanup in a component.",
                    ),
                    OwnerRequirementOperation::Boundary => (
                        "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                        "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
                    ),
                    OwnerRequirementOperation::SettledCleanup => (
                        "onSettled returns a cleanup function in a scope with no owner to register it on; Solid throws SETTLED_CLEANUP_UNOWNED here in dev, and in production the cleanup is silently dropped and will never run",
                        "Call onSettled where an owner is active (a component body or computation), or wrap the scope in createRoot. Inside event handlers a returned cleanup is not supported; do the teardown explicitly instead.",
                    ),
                    OwnerRequirementOperation::Effect => (
                        "effect is created without a reactive owner; nothing will ever dispose it, so it keeps running and holding its subscriptions for the lifetime of the app",
                        "Create effects inside a component or computation so their owner disposes them. For deliberate module-scope reactivity, wrap the setup in createRoot(dispose => ...) and keep the dispose handle.",
                    ),
                };
                let mut metadata = Rule::MissingOwner.metadata();
                if requirement.operation == OwnerRequirementOperation::SettledCleanup {
                    metadata.severity = "error";
                }
                FindingWording::new(metadata, message, hint)
            }
            FindingSeed::AsyncRead(read) => async_read_wording(read),
            FindingSeed::PackageContractIssue(issue) => {
                let (condition, hint) = match &issue.status {
                    PackageContractIssueKind::Stale {
                        contract_version,
                        installed_version,
                    } => (
                        format!(
                            "has a reactivity contract for version {contract_version}, but version {installed_version} is installed"
                        ),
                        format!(
                            "The accepted document is evidence about a release this project no longer installs. Generate a temporary-v2 proposal for node_modules/{package} with its exact registry integrity, prove the open claims, issue a new receipt, and update .solid-checker/accepted-contracts.json.",
                            package = issue.package
                        ),
                    ),
                    PackageContractIssueKind::StaleBundled {
                        audited_version,
                        installed_version,
                    } => (
                        format!(
                            "is audited by this checker at version {audited_version}, but version {installed_version} is installed"
                        ),
                        format!(
                            "Bundled contracts describe one exact release. Install {} {audited_version}, or upgrade solid-checker to a release that audits {installed_version}.",
                            issue.package
                        ),
                    ),
                    PackageContractIssueKind::IntegrityMismatch {
                        contract_integrity,
                        installed_integrity,
                        bundled,
                    } => (
                        format!(
                            "has a reactivity contract audited against npm integrity {contract_integrity}, but the project's lockfile installs {installed_integrity}"
                        ),
                        if *bundled {
                            format!(
                                "A version string is not a pin: the installed bytes were republished, patched, or overridden. Install the artifact this checker audited for {}, or upgrade solid-checker to a release that audits the installed one.",
                                issue.package
                            )
                        } else {
                            format!(
                                "A version string is not a pin: the installed bytes were republished, patched, or overridden, so the accepted document is evidence about a tarball this project does not have. Generate a temporary-v2 proposal for node_modules/{package} with its exact registry integrity, prove it, issue a receipt, and update .solid-checker/accepted-contracts.json.",
                                package = issue.package
                            )
                        },
                    ),
                    PackageContractIssueKind::Unverified => (
                        "has only an unaccepted temporary-v2 reactivity proposal".to_owned(),
                        "Replay the proposal's required proof plan against the exact artifact. Only the Rust verifier may finalize closed claims and issue the receipt referenced by .solid-checker/accepted-contracts.json; probes may falsify but cannot accept closure.".into(),
                    ),
                    PackageContractIssueKind::Missing => (
                        "has no reactivity contract".to_owned(),
                        format!(
                            "Generate a temporary-v2 proposal for {} at {}, verify its required proofs, and add the proof-issued receipt plus exact import identity to .solid-checker/accepted-contracts.json. Missing evidence remains uncertifiable. See docs/package-contracts.md for the workflow.",
                            issue.package, issue.contract_path
                        ),
                    ),
                };
                FindingWording::new(
                    Rule::PackageContractIncomplete.metadata(),
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
            Rule::LeafOwnerForbiddenCall,
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
            Rule::LeafOwnerForbiddenCall,
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
            Rule::LeafOwnerForbiddenCall,
            format!(
                "reactive primitive {primitive} is created inside {}; {} is a leaf owner with no children, so nested primitives are never tracked or disposed, and Solid throws in dev",
                operation.owner, operation.owner
            ),
            format!(
                "Create the primitive in the component body (or another owning scope) and read its accessor inside {}.",
                operation.owner
            ),
        ),
        LeafOwnerOperationKind::UnresolvedCallback => (
            Rule::ReactiveDispatchUnresolved,
            format!(
                "{} receives a type-correct callback whose exact synchronous body cannot be resolved; whether it performs cleanup, flush, or creates a nested primitive in this leaf scope cannot be certified",
                operation.owner
            ),
            "Pass an exact in-project function or a function literal directly, so solid-checker can inspect the callback body and certify or report its leaf-scope operations.".into(),
        ),
    };
    if let Some(via) = &operation.via
        && !matches!(operation.kind, LeafOwnerOperationKind::UnresolvedCallback)
    {
        message.push_str(&format!(
            " — reached through {via}(), which performs the operation in its synchronous extent and is called from this scope"
        ));
    }
    let mut evidence = vec![EvidenceStep {
        message: if matches!(operation.kind, LeafOwnerOperationKind::UnresolvedCallback) {
            "the leaf callback's exact synchronous target is not available in the project and is not a resolved standard-library operation".into()
        } else {
            match &operation.via {
                Some(via) => format!(
                    "the exactly resolved helper {via}() runs the operation synchronously, and this call site is inside the {} callback",
                    operation.owner
                ),
                None => format!(
                    "the call is lexically contained by the {} callback",
                    operation.owner
                ),
            }
        },
        location: Some(operation.location.clone()),
    }];
    if operation.uncertain
        && (!matches!(operation.kind, LeafOwnerOperationKind::UnresolvedCallback)
            || operation.call_site_gate.is_some())
    {
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
    // exactly like an undeclared node. SC5001 therefore stays
    // reported on declared sources with this conditional wording, while
    // SC5003 never reaches this function for them (suppressed in selection).
    let declared = read.declared_loading;
    let ssr_client_variant = matches!(read.execution, ExecutionRole::TrackedJsx)
        && (read.ssr_client_hole || read.server_rendering_unresolved)
        && !read.under_loading;
    let (rule, message, hint) = if let Some(owner) = &read.leaf_owner {
        (
            Rule::PendingAsyncUnsuspendableRead,
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
                Rule::PendingAsyncUnsuspendableRead,
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
            ExecutionRole::TrackedJsx
                if (read.ssr_client_hole || read.server_rendering_unresolved)
                    && !read.under_loading => {
                let unresolved = read.server_rendering_unresolved;
                (
                    Rule::AsyncOutsideLoadingBoundary,
                    if unresolved {
                        format!(
                            "source {:?} declares ssrSource: \"client\" with no loadingValue/seedLoadingValue and is read outside a Loading boundary, but the analyzed project cannot prove whether a server-rendering entry exists; if this application server-renders, the read throws during SSR even when the compute is fully synchronous",
                            read.accessor
                        )
                    } else {
                        format!(
                            "source {:?} declares ssrSource: \"client\" with no loadingValue/seedLoadingValue, and this project server-renders; the server never runs the compute, so rendering this read outside a Loading boundary throws `ssrSource: \"client\" read during SSR outside a <Loading> boundary` during SSR — even when the compute is fully synchronous",
                            read.accessor
                        )
                    },
                    "Wrap the reading subtree in <Loading fallback={...}> so the server can flush the fallback and hand the position to the client, or declare a loadingValue (loadingValue: undefined is valid; store-family sources use seedLoadingValue: true) so the server renders a provisional value instead.".to_owned(),
                )
            },
            ExecutionRole::TrackedJsx if !read.under_loading => (
                Rule::AsyncOutsideLoadingBoundary,
                format!(
                    "async accessor {:?} is rendered without a Loading boundary above it; while it is pending nothing renders, and the mount is deferred until all uncaught async settles (Solid dev warning ASYNC_OUTSIDE_LOADING_BOUNDARY)",
                    read.accessor
                ),
                "This is safe but shows nothing while loading. Wrap the reading subtree in <Loading fallback={...}> for visible fallback UI, or leave it as is if an empty container during load is intended. For a revalidation indicator, use isPending(() => ...) under the same boundary.".to_owned(),
            ),
            // `DiscardedRendering` is here and not above: a pending read the
            // compiler deleted cannot throw, so selection excludes it exactly
            // as it excludes an unclassified span.
            ExecutionRole::Unknown
            | ExecutionRole::TrackedJsx
            | ExecutionRole::DeferredCallback
            | ExecutionRole::UntrackedCallback
            | ExecutionRole::EffectApply
            | ExecutionRole::EventCallback
            | ExecutionRole::DirectiveApply
            | ExecutionRole::DiscardedRendering => {
                panic!("async projector received a seed that selection should have excluded")
            }
        }
    };
    let mut provenance = if ssr_client_variant {
        if read.server_rendering_unresolved {
            "the source declares ssrSource: \"client\" and no loadingValue/seedLoadingValue; no server rendering entry point is visible in the analyzed project, which does not prove the application is CSR-only".to_owned()
        } else {
            "the source declares ssrSource: \"client\" and no loadingValue/seedLoadingValue, and a server rendering entry point is imported in this project".to_owned()
        }
    } else {
        "the accessor is returned by an async computation".to_owned()
    };
    if read.options_opaque
        && rule == Rule::PendingAsyncUnsuspendableRead
        && read.leaf_owner.is_none()
    {
        provenance.push_str(
            "; the source's options argument cannot be read statically, so a loadingValue declaration (which would make the first flight safe) can be neither proven nor ruled out — this finding is a proof obligation, not a proven throw",
        );
    }
    let mut metadata = rule.metadata();
    if read.leaf_owner.is_some() {
        metadata.severity = "warning";
    } else if ssr_client_variant {
        metadata.severity = "error";
    }
    FindingWording::new(metadata, message.clone(), hint).with_evidence(vec![
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
        Rule::ResolveInReactiveScope => {
            "the resolve() call runs directly in a tracked scope, where the runtime's observer guard throws in dev"
        }
        Rule::HttpResponseAfterFlush => {
            "the call's scope renders below a Loading boundary, but request-time ordering does not prove whether the boundary settles before or after the response head commits"
        }
        Rule::ServerFunctionModuleDirective => {
            "the module's directive prologue contains \"use server\" and this export is provably not a direct function declaration"
        }
        // The nested claim is about a value the argument *holds*, not the
        // argument's own resolved type, so it cannot borrow the top-level
        // sentence: that one would assert a fact the analysis never proved.
        Rule::ServerFunctionRichArgument
            if violation.analysis_context == "nested-rich-argument" =>
        {
            "the callee carries a \"use server\" directive, a closed object literal reaching it holds a value in the JSON-unsafe set, and nothing in the project installs an argument serializer"
        }
        Rule::ServerFunctionRichArgument => {
            "the callee carries a \"use server\" directive, the argument's resolved type is in the JSON-unsafe set, and nothing in the project installs an argument serializer"
        }
        Rule::JsxNoDuplicateProps => {
            "the intrinsic element uses more than one competing source of DOM child content"
        }
        Rule::PreferFor => "an array map call is used directly in a JSX rendering position",
        Rule::PreferShow => "a conditional JSX expression matches the configured Show preference",
        Rule::StrictReadUntracked
        | Rule::ReactiveReadAfterAwait
        | Rule::UncalledAccessor
        | Rule::ExpectedFunctionGotExpression
        | Rule::NoDirectMutation
        | Rule::ReactiveSourceUncaptured
        | Rule::ReactiveDispatchUnresolved
        | Rule::ComponentPropsDestructure
        | Rule::ComponentReturnsConditionally
        | Rule::ReactiveWriteInOwnedScope
        | Rule::ActionCalledInOwnedScope
        | Rule::LeafOwnerForbiddenCall
        | Rule::MissingOwner
        | Rule::PendingAsyncUnsuspendableRead
        | Rule::AsyncOutsideLoadingBoundary
        | Rule::PrimitiveInDirectiveApplication
        | Rule::MissingEffectFunction
        | Rule::PackageContractIncomplete => panic!(
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
    // The grouping of defect kinds into finding families lives once, in
    // `StaticDefectKind::family`; this dialect only names its own rule for
    // each family. A new defect kind therefore cannot reach the catalog
    // without both the family mapping and this arm being written.
    let rule = match defect.kind.family() {
        StaticDefectFamily::ReactiveObjectDestructure => Rule::ComponentPropsDestructure,
        StaticDefectFamily::ReactiveReadAfterAwait => Rule::ReactiveReadAfterAwait,
        StaticDefectFamily::ComponentReturnsConditionally => Rule::ComponentReturnsConditionally,
        StaticDefectFamily::PackageContractIncomplete
        | StaticDefectFamily::UnknownCallbackExecution => Rule::PackageContractIncomplete,
        StaticDefectFamily::MissingEffectFunction => Rule::MissingEffectFunction,
        StaticDefectFamily::ReactiveSourceUncaptured => Rule::ReactiveSourceUncaptured,
        StaticDefectFamily::ReactiveDispatchUnresolved => Rule::ReactiveDispatchUnresolved,
        StaticDefectFamily::ExpectedFunctionGotExpression => Rule::ExpectedFunctionGotExpression,
        StaticDefectFamily::UncalledAccessor => Rule::UncalledAccessor,
        StaticDefectFamily::DirectMutation => Rule::NoDirectMutation,
    };
    let text = solid_reactive_ir::static_defect_text(defect, &V2_STATIC_TERMS);
    let mut message = text.message;
    // This suffix names *one* source of uncertainty: a component whose call
    // sites cannot be enumerated, so its props' signal backing is unprovable.
    // Kinds whose uncertainty is something else already say so in
    // `static_defect_text`, and appending this to them describes the wrong
    // proof obligation -- an unchecked handler value has no component and no
    // props to enumerate.
    if defect.uncertain
        && !matches!(
            &defect.kind,
            StaticDefectKind::MissingEffectFunction
                | StaticDefectKind::ReactiveDispatchUnresolved { .. }
                | StaticDefectKind::ReactiveCallbackUnresolved { .. }
                | StaticDefectKind::StructuredReturnUnresolved { .. }
                | StaticDefectKind::HandlerValueUnresolved { .. }
        )
    {
        if defect.analysis_context == "draggable-default-uncertain" {
            message.push_str(
                "; resolve the final href value before treating this as a violation or certifying it as safe",
            );
        } else {
            message.push_str(
                "; this component's call sites cannot be enumerated (it is exported, spread into, or referenced outside JSX), so whether the props are signal-backed can be neither proven nor ruled out — this finding is a proof obligation, not a proven runtime defect",
            );
        }
    }
    FindingWording::new(rule.metadata(), message, text.hint).with_evidence(vec![EvidenceStep {
        message: text.evidence.into(),
        location: Some(defect.location.clone()),
    }])
}

const V2_STATIC_TERMS: solid_reactive_ir::StaticDefectTerms =
    solid_reactive_ir::StaticDefectTerms {
        props_destructure_hint: "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use omit(props, ...keys) and merge(defaults, props) instead of destructuring.",
        reactive_object_destructure_hint: "Keep the reactive object intact and read object.<name> inside JSX or a tracked computation. A property access made there remains subscribed; a setup-time destructuring binding does not.",
        missing_effect_message: "createEffect is called without an effect function; the signature is createEffect(compute, apply), where compute tracks dependencies and returns a value, and apply receives that value and performs the side effect",
        missing_effect_hint: "Split the callback: reactive reads go in the compute function, the side effect in the apply function, and cleanup is returned from apply. For error handling, pass { effect, error } as the second argument.",
        store_mutation_hint: v2_store_mutation_hint,
        removed_export_hint: v2_removed_export_hint,
    };

fn v2_store_mutation_hint(name: &str) -> String {
    format!(
        "Write through the store's setter: setStore(store => {{ store.key = value; }}) mutates the draft in place, and setStore(reconcile(next)) replaces it wholesale. Direct assignment to {name} does not notify subscribers."
    )
}

/// The Solid 1.x APIs removed or renamed in 2.0 that a project migrating to
/// the v2 dialect is most likely to still import, mapped to their 2.0
/// replacement. Derived from the official 2.0 migration guide's rename and
/// removal tables; each name is verified absent from the bundled
/// `solid-v2/solid-js.json` contract by
/// `removed_exports_are_absent_from_the_bundled_contract` — anything the
/// contract still exports is not removed and must not appear here.
const V2_REMOVED_EXPORTS: &[(&str, &str)] = &[
    (
        "batch",
        "updates batch by default on the microtask queue; call flush() where you need the old synchronous application",
    ),
    (
        "on",
        "split effects make it unnecessary: put the reads in createEffect's compute function and the side effect in apply",
    ),
    (
        "onMount",
        "use onSettled, which can also return a cleanup function",
    ),
    (
        "onError",
        "use the <Errored> boundary or the { effect, error } second argument of createEffect",
    ),
    (
        "catchError",
        "use the <Errored> boundary or the { effect, error } second argument of createEffect",
    ),
    (
        "createResource",
        "use an async computation (createMemo(async ...) or createStore(fn)) read under a <Loading> boundary",
    ),
    // `createRenderEffect` and `untrack` are deliberately NOT here: both
    // still exist in 2.0 (bundled contract exports them), so a missing
    // summary for them is a real contract gap, not a removed API.
    (
        "createComputed",
        "use createEffect(compute, apply), a function-form createSignal/createStore, or createMemo",
    ),
    ("createMutable", "use createStore with draft-first setters"),
    ("modifyMutable", "use createStore with draft-first setters"),
    ("mergeProps", "renamed to merge"),
    ("splitProps", "renamed to omit"),
    ("Suspense", "renamed to Loading"),
    (
        "SuspenseList",
        "renamed to Reveal, which coordinates sibling Loading boundaries via its order prop",
    ),
    ("ErrorBoundary", "renamed to Errored"),
    (
        "Index",
        "use <For keyed={false}>, whose children receive an item accessor and a stable numeric index",
    ),
    ("indexArray", "use mapArray with keyed: false"),
    (
        "createSelector",
        "use createProjection or a function-form createStore",
    ),
    ("createDynamic", "use the dynamic(source) component factory"),
    ("unwrap", "renamed to snapshot"),
    ("equalFn", "renamed to isEqual"),
    ("getListener", "renamed to getObserver"),
    (
        "startTransition",
        "transitions are built in; use isPending/Loading and the optimistic APIs",
    ),
    (
        "useTransition",
        "transitions are built in; use isPending/Loading and the optimistic APIs",
    ),
    (
        "produce",
        "draft-first mutation is now the default store setter behavior; drop the wrapper",
    ),
    ("from", "use async iterators as computation results"),
    (
        "observable",
        "push signal changes to external subscribers with createEffect",
    ),
    ("createDeferred", "removed; handle deferral outside Solid"),
    (
        "resetErrorBoundaries",
        "no longer needed; error boundaries heal automatically",
    ),
    ("enableScheduling", "removed; scheduling is built in"),
    ("writeSignal", "removed; it was an internal API"),
];

/// Migration-oriented SC9005 hint for the removed 1.x APIs: telling the user
/// to write a contract entry for `batch` would send them to document an
/// export that no longer exists. Applies to the packages the v2 dialect
/// itself contracts (`solid-js`, `@solidjs/web`); a same-named export of a
/// third-party package keeps the generic contract hint.
fn v2_removed_export_hint(module: &str, export: &str) -> Option<String> {
    if module != "solid-js" && module != "@solidjs/web" && !module.starts_with("@solidjs/web/") {
        return None;
    }
    let (_, replacement) = V2_REMOVED_EXPORTS
        .iter()
        .find(|(name, _)| *name == export)?;
    Some(format!(
        "{export} was removed in Solid 2.0; see the migration guide — {replacement}."
    ))
}

#[cfg(test)]
mod removed_export_tests {
    use super::{V2_REMOVED_EXPORTS, v2_removed_export_hint};

    /// A name the bundled 2.0 contract still exports is not removed: hinting
    /// "migrate away" for it would be wrong, and SC9005 for it is a genuine
    /// contract gap. This holds the map to the shipped export list.
    #[test]
    fn removed_exports_are_absent_from_the_bundled_contract() {
        let contract = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/solid-dialect/contracts/solid-v2/solid-js.json");
        let contract: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&contract).unwrap()).unwrap();
        let entrypoints = contract["entrypoints"]
            .as_object()
            .expect("bundled contract has entrypoints");
        let exports = entrypoints
            .values()
            .flat_map(|entrypoint| {
                entrypoint["cases"]
                    .as_array()
                    .expect("bundled entrypoint has artifact cases")
            })
            .map(|case| {
                case["exports"]
                    .as_object()
                    .expect("bundled artifact case has exact exports")
            })
            .collect::<Vec<_>>();
        for (name, _) in V2_REMOVED_EXPORTS {
            assert!(
                exports.iter().all(|case| !case.contains_key(*name)),
                "{name} is exported by the bundled solid-v2 contract, so it is not a removed API and must leave V2_REMOVED_EXPORTS"
            );
        }
    }

    #[test]
    fn removed_export_hint_is_scoped_to_solid_packages() {
        let hint = v2_removed_export_hint("solid-js", "batch").expect("batch is removed");
        assert!(hint.contains("removed in Solid 2.0"));
        assert!(hint.contains("flush()"));
        assert_eq!(v2_removed_export_hint("some-lib", "batch"), None);
        assert_eq!(v2_removed_export_hint("solid-js", "createMemo"), None);
        // The fixture-pinned quartet all carry migration hints.
        for name in ["batch", "createComputed", "createResource", "onMount"] {
            assert!(v2_removed_export_hint("solid-js", name).is_some());
        }
    }
}

#[cfg(test)]
mod strict_read_hint_tests {
    use super::strict_read_hint;

    #[test]
    fn component_props_hint_does_not_present_untrack_as_a_reactivity_fix() {
        let hint = strict_read_hint("component-props", false);
        assert!(hint.contains("read props.<name> directly in JSX"));
        assert!(hint.contains("Do not use untrack() to make a prop reactive"));
    }

    #[test]
    fn violation_hint_asserts_solid_warns_unconditionally() {
        for kind in ["component-props", "signal"] {
            let hint = strict_read_hint(kind, false);
            assert!(hint.contains("Solid warns STRICT_READ_UNTRACKED here in dev"));
            assert!(!hint.contains("If this read executes untracked"));
        }
    }

    #[test]
    fn uncertifiable_hint_never_asserts_that_solid_will_warn() {
        for kind in ["component-props", "signal"] {
            let hint = strict_read_hint(kind, true);
            assert!(!hint.contains("Solid warns STRICT_READ_UNTRACKED here in dev"));
            assert!(hint.contains("If this read executes untracked in dev, Solid warns"));
            assert!(hint.contains("this finding does not establish that it does"));
        }
    }
}
