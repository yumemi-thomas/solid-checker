//! Package-contract resolution and export-summary construction.
//!
//! Resolves imported contract bindings to local symbols (`resolve_contract_imports`)
//! and turns the interprocedural summaries into the per-export contract artifacts
//! that a downstream package sees. Owns both the full and incremental summary
//! builds; the public contract data types stay in the crate root.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use typefacts::{Callability, Constructability, Location, ReferenceSpace, RuntimeBindingKind};

use super::{
    CallbackSchedule, ContractCallback, ContractClaim, ContractExport, ContractOwnerRequirement,
    ContractReactiveRead, ContractReturn, EntitySymbols, OwnerRequirementOperation,
    PackageContract, ReactiveSourceKind, StaticDefect, StaticDefectKind, SummaryNode, SummaryRead,
    SummaryReads, SymbolId, location, location_order,
};
use crate::cache::{CachedContractExports, ContractExportFragment, ContractNodeKey};
use crate::contract_semantics::{
    AcceptedContractIndex, AcceptedContractUse, ClaimDomain, KnowledgeSet, OperationKind,
    OwnerSource, Requirement, Schedule, Tracking, UncertifiableImportReason, ValueShape,
    ValueSource,
};
use crate::interproc::ParameterMemberInvocation;
use crate::pipeline::parallel_slice_results;

/// Whether a value-kind export's shape leaves open the possibility that it is
/// callable, in which case its open call-path domains must NOT be closed to
/// empty. `Unknown` (an `any`/error shape) and a `Choice` union whose
/// membership is not exhaustively known, or that contains a callable member,
/// are possibly-callable and stay open (fail closed). Every other value shape
/// is proven non-callable, so its vacuous call-path domains may be closed.
fn shape_may_be_callable(shape: &ValueShape) -> bool {
    match shape {
        ValueShape::Callable | ValueShape::Component | ValueShape::Unknown => true,
        ValueShape::Choice(members) => {
            !members.is_closed() || members.items().iter().any(shape_may_be_callable)
        }
        _ => false,
    }
}

/// Projects an already receipt-validated normalized export into the compact
/// indexes used by the current interprocedural solver. This is deliberately
/// downstream of exact import/artifact selection: no schema version, summary
/// ID, condition label, evidence spelling, or closure-array mechanic is inspected.
pub fn project_accepted_export(accepted: &AcceptedContractUse<'_>) -> ContractExport {
    let export = accepted.export();
    ContractExport {
        // The exact contract and export this projection came from. Re-emission
        // reads the *presence* of this to know the summary is inherited rather
        // than inferred; the strings themselves are attribution for the emit
        // boundary's record. The certifier rebinds the re-export from its own
        // snapshot-verified evidence and never reads them.
        inherited_from: Some(crate::InheritedExportOrigin {
            package_name: accepted.contract().package().name.clone(),
            package_version: accepted.contract().package().version.clone(),
            artifact_case: accepted.contract().artifact_case().id.clone(),
            semantic_digest: accepted
                .contract()
                .receipt()
                .semantic_digest
                .as_str()
                .to_owned(),
            entrypoint: export.identity.entrypoint.clone(),
            export: export.identity.public_name.clone(),
        }),
        ..project_export_semantics(export)
    }
}

/// [`project_accepted_export`] without the acceptance identity: the projection
/// itself, over one normalized export's semantics.
///
/// Split out because the certifier has to re-derive exactly this, from the
/// dependency node's own certified export, to decide whether a parent's
/// inherited closure *is* the projection of the dependency's or merely
/// resembles it. Two derivations of "the projection" would be two answers, and
/// the certifier's is the one that would silently admit a claim the generator
/// never made.
#[must_use]
pub fn project_export_semantics(
    export: &crate::contract_semantics::ExportSemantics,
) -> ContractExport {
    let mut open_claims = BTreeSet::new();
    let kind = match export.shape {
        ValueShape::Callable | ValueShape::Component => "function",
        ValueShape::Unknown => "unknown",
        _ => "value",
    };

    let mut callbacks = project_callbacks(export, &mut open_claims);
    let mut reactive_reads = project_reactive_reads(export, &mut open_claims);
    let mut returns = project_return(export, &mut open_claims);
    let mut owner_requirements = project_owner_requirements(export, &mut open_claims);
    let async_behavior = project_async_behavior(export, &mut open_claims);

    if kind == "value" && !shape_may_be_callable(&export.shape) {
        // A *proven* non-callable value export is never invoked, so every
        // call-path function-effect domain is vacuously empty. A composed
        // *proposal* dependency can still carry that export's call-path
        // knowledge as open (unresolved) -- e.g. a namespace/constant
        // re-exported past forwarders it never exposes as call targets.
        // Projecting that as open would manufacture function effects on a value
        // export, the exact inconsistency `validate_export` refuses with "value
        // export cannot have function effects". Standalone generation already
        // certifies such an export effect-free, so composing its proposal must
        // agree rather than refuse the importing package. Only the *open*
        // (vacuous) domains are closed here; a genuinely known effect is left
        // intact so a real value-with-effects defect still refuses.
        //
        // The `shape_may_be_callable` guard is load-bearing: a shape whose
        // callability is *not* proven (an `Unknown`/`any` shape, or a `Choice`
        // union whose membership is not exhaustively non-callable) must keep its
        // open domains and continue to fail closed. Closing them would assert
        // "invokes no callback / performs no read" about something that may in
        // fact be callable -- manufacturing a negative claim from missing
        // knowledge, which the precision contract forbids.
        if callbacks.is_open() {
            callbacks = ContractClaim::Known(Vec::new());
            open_claims.remove(&ClaimDomain::Callbacks);
        }
        if reactive_reads.is_open() {
            reactive_reads = ContractClaim::Known(Vec::new());
            open_claims.remove(&ClaimDomain::Reads);
        }
        if returns.is_open() {
            returns = ContractClaim::Known(None);
            open_claims.remove(&ClaimDomain::Returns);
        }
        if owner_requirements.is_open() {
            owner_requirements = ContractClaim::Known(Vec::new());
            open_claims.remove(&ClaimDomain::Creates);
        }
    }

    // Read from the accepted document itself rather than from the projection:
    // `project_owner_requirements` keeps only the operations that impose an
    // owner obligation, so a `create` this export publishes need not survive
    // it. The generator's `creates` proposal walk needs the domain's own
    // closure — closed *and* empty, which is the only shape that is not a
    // counterexample to a caller proposing `creates: []`.
    let creates = export
        .operation_claim(ClaimDomain::Creates)
        .expect("creates is an operation domain");
    let creates_closed_empty = creates.is_closed() && creates.items().is_empty();
    // ADR 0143: the same reading for `returns`. `project_return` answers
    // `Known(None)` both for `returns: []` and for a closed claim over outputs
    // that name no reactive leaf, and only the first is a closure a
    // re-exporting package may restate as empty.
    let returns_claim = export
        .operation_claim(ClaimDomain::Returns)
        .expect("returns is an operation domain");
    let returns_closed_empty = returns_claim.is_closed() && returns_claim.items().is_empty();

    ContractExport {
        kind: kind.into(),
        reactive_reads,
        returns,
        callbacks,
        owner_requirements,
        async_behavior,
        open_claims,
        creates_closed_empty,
        returns_closed_empty,
        creates_walk_clean: false,
        creates_walk_declines: Vec::new(),
        returns_walk_clean: false,
        returns_value_completion: false,
        returns_described_callables: Vec::new(),
        returns_reading_callables: Vec::new(),
        member_alias_initializer: false,
        member_alias_spelling: None,
        returns_argument_containers: Vec::new(),
        direct_callback_parameters: BTreeSet::new(),
        direct_accessor_parameters: BTreeSet::new(),
        direct_coerced_parameters: BTreeSet::new(),
        direct_member_callback_parameters: BTreeSet::new(),
        iterated_parameters: BTreeSet::new(),
        result_access_parameters: BTreeSet::new(),
        returned_invocations: project_returned_invocations(export),
        // A projected dependency export has no body here to walk.
        merged_props_return: None,
        // The projection alone states no acceptance identity;
        // `project_accepted_export` attaches it.
        inherited_from: None,
        context_premises: export
            .call
            .context_premises()
            .iter()
            .map(|premise| premise.export.clone())
            .collect(),
    }
}

/// ADR 0152: the export argument slots the returned value invokes on its own
/// invoker's stack, exactly once per invocation, and that the export invokes
/// nowhere else.
///
/// Three things must hold together, each read from the accepted document: the
/// `returns` domain is closed and every one of its items is a described
/// callable naming the slot (a union in which one alternative does not invoke
/// it says only "may run"); the `callbacks` domain is closed; and every
/// top-level item from the slot is ADR 0139's `result-access` item, so the
/// export's own call never runs it and keeps it only in the value it returns.
/// Anything else answers nothing for the slot, which leaves the consumer's
/// existing reading -- a callback of unproven timing -- in place.
fn project_returned_invocations(
    export: &crate::contract_semantics::ExportSemantics,
) -> BTreeSet<usize> {
    let returns = export
        .operation_claim(ClaimDomain::Returns)
        .expect("returns is an operation domain");
    if !returns.is_closed() || returns.items().is_empty() || !export.callbacks().is_closed() {
        return BTreeSet::new();
    }
    let mut invoked: Option<BTreeSet<usize>> = None;
    for id in returns.items() {
        let Some(ValueShape::DescribedCallable(call)) = export
            .operation(&id.0)
            .and_then(|operation| operation.output.as_ref())
        else {
            return BTreeSet::new();
        };
        let slots = call
            .callbacks
            .iter()
            .filter_map(crate::contract_semantics::DescribedCallback::parameter)
            .map(usize::from)
            .collect::<BTreeSet<_>>();
        invoked = Some(match invoked {
            None => slots,
            Some(previous) => previous.intersection(&slots).copied().collect(),
        });
    }
    let mut invoked = invoked.unwrap_or_default();
    invoked.retain(|slot| {
        let items = export
            .callbacks()
            .items()
            .iter()
            .filter(|callback| {
                matches!(&callback.from, ValueSource::Parameter { index, .. } if usize::from(*index) == *slot)
            })
            .collect::<Vec<_>>();
        !items.is_empty()
            && items.iter().all(|callback| {
                matches!(&callback.from, ValueSource::Parameter { path, .. } if path.is_empty())
                    && export
                        .operation(&callback.operation.0)
                        .is_some_and(crate::contract_semantics::Operation::is_result_access)
            })
    });
    invoked
}

fn project_callbacks(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Vec<ContractCallback>> {
    let knowledge = export.callbacks();
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Callbacks);
    }
    let mut callbacks = Vec::new();
    for callback in knowledge.items() {
        let ValueSource::Parameter { index, path } = &callback.from else {
            open.insert(ClaimDomain::Callbacks);
            continue;
        };
        let index = *index;
        let Some(operation) = export.operation(&callback.operation.0) else {
            open.insert(ClaimDomain::Callbacks);
            continue;
        };
        let Some(execution) = projected_execution(operation) else {
            open.insert(ClaimDomain::Callbacks);
            continue;
        };
        callbacks.push(ContractCallback {
            parameter: usize::from(index),
            execution: execution.into(),
            // `projected_execution` collapses a tracked operation onto one
            // attribution word, which has no schedule column. Carry the
            // operation's own schedule beside it so re-emitting an ingested row
            // republishes what the contract said rather than a default: a
            // tracked row with no execution point means the producer
            // established none, which is a different fact from `queued`.
            schedule: if operation.is_result_access() {
                // ADR 0139: a deferred row, whose one distinguishing fact is
                // the event it runs at -- kept so re-emission republishes the
                // item it was read from rather than a queued deferral.
                Some(CallbackSchedule::ResultAccess)
            } else {
                (execution == "tracked").then_some(match operation.schedule {
                    Some(Schedule::SameStack) => CallbackSchedule::SameStack,
                    Some(Schedule::Queued) => CallbackSchedule::Queued,
                    Some(Schedule::External) => CallbackSchedule::External,
                    None => CallbackSchedule::Unestablished,
                })
            },
            // The inverse of the producer's mapping, word for word
            // (`ContractCallback::clears_tracking`): `untracked` on an `inline`
            // row is a proven clearing wrapper, on a `deferred` row a deferral
            // proven to run with no caller's listener, and neither word reads
            // back from `ambient-at-execution`, which leaves the listener to
            // whoever runs the callback. A `tracked` row never carries the bit
            // -- its word is the claim.
            clears_tracking: ContractCallback::clears_tracking_from(execution, operation.tracking),
            // A non-call item (a property read, iteration, coercion or
            // `hasInstance` of the argument) is projected with its protocol and
            // kept: the domain's closure is a statement about *every* use of
            // caller-supplied code, and dropping the item would either reopen
            // the domain or, re-emitted, silently delete a claim. Every pass
            // that models invocations filters it out
            // (`ContractCallback::is_invocation`), which is sound because such
            // a use runs the caller's own traps, at the call, on the caller's
            // stack, in the caller's tracking context -- what the value's author
            // wrote -- and today raises no obligation for a non-callable
            // argument at all.
            protocol: operation.invoke_protocol(),
            // The member of the argument the item invokes (item B of
            // ways-to-improve § 3.3), carried whole. Dropping it read
            // `handler[0](…)` as "argument 1 itself is invoked inline": a
            // consumer would then fold `callHandler(e, handlerProp)` as a call
            // of `handlerProp`, and re-emission would republish the item as
            // a call of the argument. Every pass that reads a row as a call of
            // the argument asks `ContractCallback::invokes_argument`.
            path: path.clone(),
            arguments: operation.inputs.iter().map(project_return_shape).collect(),
            owner: match operation.owner.source {
                OwnerSource::None => Some("none".into()),
                OwnerSource::Created(_)
                    if operation.owner.requirements.child_owners == Requirement::Forbidden
                        && operation.owner.requirements.cleanup == Requirement::Forbidden =>
                {
                    Some("leaf".into())
                }
                OwnerSource::Created(_) => Some("created".into()),
                OwnerSource::Captured(_)
                | OwnerSource::AmbientAtCall
                | OwnerSource::AmbientAtExecution => Some("inherited".into()),
                OwnerSource::Unknown => None,
            },
        });
    }
    callbacks.sort_by_key(|callback| callback.parameter);
    match knowledge {
        KnowledgeSet::Unknown if callbacks.is_empty() => ContractClaim::Open,
        _ => ContractClaim::Known(callbacks),
    }
}

fn projected_execution(operation: &crate::contract_semantics::Operation) -> Option<&'static str> {
    if operation.tracking == Tracking::Tracked {
        return Some("tracked");
    }
    match operation.schedule {
        Some(Schedule::SameStack) => Some("inline"),
        Some(Schedule::Queued | Schedule::External) => Some("deferred"),
        None => None,
    }
}

fn project_reactive_reads(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Vec<ContractReactiveRead>> {
    let knowledge = export
        .operation_claim(ClaimDomain::Reads)
        .expect("reads is an operation domain");
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Reads);
    }
    let mut reads = Vec::new();
    for id in knowledge.items() {
        let Some(operation) = export.operation(&id.0) else {
            open.insert(ClaimDomain::Reads);
            continue;
        };
        match operation.inputs.first() {
            // Carry the whole path back. Keeping only `path.last()` would
            // round-trip an accepted `["modifiers", "includes"]` down into a
            // claim about a `includes` property of the parameter itself.
            Some(ValueShape::Parameter { index, path }) => reads.push(ContractReactiveRead {
                kind: "parameter-member".into(),
                label: String::new(),
                parameter: Some(usize::from(*index)),
                path: Some(path.clone()),
                // An *accepted* contract's operation is projected back with no
                // provenance, deliberately. Composition is intra-package: the
                // claim it discharges is "this export performs the read
                // through its call to that export of the same artifact case",
                // and an accepted dependency's export is in neither this
                // artifact case nor this census. Carrying it here would make a
                // cross-package composition the consumer has no premise for.
                composed_owner: None,
                composed_from: None,
            }),
            Some(ValueShape::Reactive { .. }) => reads.push(ContractReactiveRead {
                kind: "accessor".into(),
                label: "normalized reactive read".into(),
                parameter: None,
                path: None,
                composed_owner: None,
                composed_from: None,
            }),
            Some(ValueShape::Store { .. }) => reads.push(ContractReactiveRead {
                kind: "store-path".into(),
                label: "normalized store read".into(),
                parameter: None,
                path: None,
                composed_owner: None,
                composed_from: None,
            }),
            _ => {
                open.insert(ClaimDomain::Reads);
            }
        }
    }
    match knowledge {
        KnowledgeSet::Unknown if reads.is_empty() => ContractClaim::Open,
        _ => ContractClaim::Known(reads),
    }
}

fn project_return(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Option<ContractReturn>> {
    let knowledge = export
        .operation_claim(ClaimDomain::Returns)
        .expect("returns is an operation domain");
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Returns);
    }
    let mut returns = knowledge
        .items()
        .iter()
        .filter_map(|id| export.operation(&id.0))
        .filter_map(|operation| operation.output.as_ref())
        .filter_map(project_returned_output)
        .collect::<Vec<_>>();
    returns.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
    returns.dedup();
    // ADR 0113: a `plain` output carries no reactive capability, which is
    // exactly what `Known(None)` says of a function whose reactive analysis
    // described no return. A closed claim whose every return is plain is that
    // answer, not a shape this projection cannot represent -- reading it as the
    // latter reopened the domain the contract had closed.
    let plain_only = !knowledge.items().is_empty()
        && knowledge.items().iter().all(|id| {
            export
                .operation(&id.0)
                .is_some_and(|operation| matches!(operation.output, Some(ValueShape::Plain)))
        });
    // ADR 0115: a closed claim whose every return hands back something exact
    // -- a plain value, the caller's own argument, or a fresh array of the
    // caller's arguments -- with no one reactive leaf they all share. The
    // consumer's return is a single leaf, so it cannot say "the argument, or an
    // array holding it", and it reads the union as describing no reactive
    // return, exactly as it reads a local conditional whose branches disagree.
    // Contract returns are only ever read to *find* a reactive leaf, so this
    // can hide a finding and never invent one. A return whose output the
    // projection drops (`[]`) is part of the union too: reading the claim as
    // its one surviving leaf would say the export always returns that.
    // ADR 0116: what an invocation of the caller's argument returned is exact
    // and names no leaf this projection can reach -- the local summary of
    // `return f()` names none either -- so it is one more dropped return, alone
    // or in a union.
    // An array whose every element is plain (`Object.keys`' fresh array of
    // strings, the second 2026-09-24 amendment to ADR 0103) is exact the same
    // way: it holds nothing reactive, which is what describing no reactive
    // return says.
    // Item B round 2: a member of the caller's argument, and `undefined`, are
    // exact too, and both are dropped returns (`project_returned_output`), so
    // `callHandler`'s `event?.defaultPrevented` -- the member, or undefined --
    // reads as no reactive return, and so does a lone `return p.key`.
    let exact_only = !knowledge.items().is_empty()
        && knowledge.items().iter().all(|id| {
            export.operation(&id.0).is_some_and(|operation| {
                matches!(
                    operation.output,
                    Some(
                        ValueShape::Plain
                            | ValueShape::Parameter { .. }
                            | ValueShape::ArgumentArray { .. }
                            | ValueShape::InvocationResult { .. }
                            | ValueShape::Undefined
                            | ValueShape::DescribedCallable(_)
                    )
                ) || matches!(
                    &operation.output,
                    Some(ValueShape::Array { element, .. }) if **element == ValueShape::Plain
                )
            })
        });
    let dropped = knowledge
        .items()
        .iter()
        .filter_map(|id| export.operation(&id.0))
        .filter(|operation| {
            operation
                .output
                .as_ref()
                .and_then(project_returned_output)
                .is_none()
        })
        .count();
    match (knowledge, returns.as_slice()) {
        (KnowledgeSet::Complete(items), []) if items.is_empty() || plain_only => {
            ContractClaim::Known(None)
        }
        (KnowledgeSet::Complete(_), _) if exact_only && (returns.len() != 1 || dropped > 0) => {
            ContractClaim::Known(None)
        }
        (KnowledgeSet::Unknown, []) => ContractClaim::Open,
        (_, [returned]) => ContractClaim::Known(Some(returned.clone())),
        _ => {
            open.insert(ClaimDomain::Returns);
            ContractClaim::Open
        }
    }
}

/// One `return` operation's output as the consumer's return leaf: exactly
/// [`project_return_shape`], except that a member of the caller's argument
/// (`parameter i` at a non-empty path, item B round 2 of ways-to-improve
/// § 3.3) and `undefined` name no leaf.
///
/// The member is the value the argument holds at that key **when the return
/// reads it**, and code the call runs before then may have replaced it: its
/// own body, or a callback it invokes -- `callHandler`'s handler calls
/// `preventDefault()` on the very event whose `defaultPrevented` it returns.
/// The caller's literal at the call site therefore does not determine it, and
/// nothing the consumer can see does, so it is resolved nowhere. Reading it
/// as the whole argument -- what [`project_return_shape`] answers for any
/// `parameter`, path or not -- would invent a return. Contract returns are
/// only ever read to *find* a reactive leaf, so this can hide one (a store's
/// member, say) and never invents one.
fn project_returned_output(shape: &ValueShape) -> Option<ContractReturn> {
    match shape {
        ValueShape::Parameter { path, .. } if !path.is_empty() => None,
        ValueShape::Undefined => None,
        shape => project_return_shape(shape),
    }
}

fn project_return_shape(shape: &ValueShape) -> Option<ContractReturn> {
    match shape {
        ValueShape::Reactive { .. } => Some(ContractReturn {
            kind: "accessor".into(),
            label: "normalized reactive result".into(),
            ..ContractReturn::default()
        }),
        ValueShape::Store { .. } => Some(ContractReturn {
            kind: "store-path".into(),
            label: "normalized store result".into(),
            ..ContractReturn::default()
        }),
        // ADR 0109. Carries the caller's argument index and *no* label: this is
        // not a reactive leaf, it is a conditional one, and reading it as a
        // `store-path` would assert reactivity of a merge of plain objects.
        ValueShape::MergedProps { from } => Some(ContractReturn {
            kind: "merged-props".into(),
            parameter: Some(usize::from(*from)),
            ..ContractReturn::default()
        }),
        ValueShape::Parameter { index, .. } => Some(ContractReturn {
            kind: "argument".into(),
            parameter: Some(usize::from(*index)),
            ..ContractReturn::default()
        }),
        // ADR 0115: a fresh array of the caller's arguments is a tuple of
        // argument leaves; `[]` holds nothing to name.
        ValueShape::ArgumentArray { items } if !items.is_empty() => Some(ContractReturn {
            kind: "tuple".into(),
            elements: items
                .iter()
                .map(|index| {
                    Some(ContractReturn {
                        kind: "argument".into(),
                        parameter: Some(usize::from(*index)),
                        ..ContractReturn::default()
                    })
                })
                .collect(),
            ..ContractReturn::default()
        }),
        ValueShape::Tuple(KnowledgeSet::Complete(items)) => Some(ContractReturn {
            kind: "tuple".into(),
            elements: items.iter().map(project_return_shape).collect(),
            ..ContractReturn::default()
        }),
        ValueShape::Object(KnowledgeSet::Complete(properties)) => Some(ContractReturn {
            kind: "object".into(),
            properties: properties
                .iter()
                .filter_map(|property| {
                    project_return_shape(&property.value)
                        .map(|value| (property.name.clone(), value))
                })
                .collect(),
            ..ContractReturn::default()
        }),
        ValueShape::Promise(value) | ValueShape::AsyncIterable(value) => {
            project_return_shape(value)
        }
        // ADR 0145/0146: a callable whose every call claim is stated. One that
        // reads a signal when invoked is what the consumer calls an accessor
        // -- calling it is a reactive read, in whatever scope calls it -- so it
        // stays one. One that reads nothing names no leaf: calling it
        // observes nothing reactive, invokes nothing the caller handed over and
        // creates nothing, which is what describing no reactive return says.
        ValueShape::DescribedCallable(call) if !call.reads.is_empty() => Some(ContractReturn {
            kind: "accessor".into(),
            label: "described callable read".into(),
            ..ContractReturn::default()
        }),
        ValueShape::DescribedCallable(_) => None,
        // ADR 0146: only ever an item of a described callable's returns.
        ValueShape::ReadValue => None,
        ValueShape::Unknown
        | ValueShape::Plain
        | ValueShape::ArgumentArray { .. }
        | ValueShape::InvocationResult { .. }
        | ValueShape::Undefined
        | ValueShape::Tuple(_)
        | ValueShape::Array { .. }
        | ValueShape::Object(_)
        | ValueShape::Choice(_)
        | ValueShape::Callable
        | ValueShape::Action { .. }
        | ValueShape::Component
        | ValueShape::Cleanup { .. }
        | ValueShape::RefApplication
        | ValueShape::ServerFunctionReference { .. } => None,
    }
}

fn project_owner_requirements(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<Vec<ContractOwnerRequirement>> {
    let knowledge = export
        .operation_claim(ClaimDomain::Creates)
        .expect("creates is an operation domain");
    if !knowledge.is_closed() {
        open.insert(ClaimDomain::Creates);
    }
    // `cleanups` is read for its *items* only, deliberately: a cleanup owner
    // requirement is published as a `kind: cleanup` operation in that domain
    // (`inferred_contract.rs`'s `owner_requirement_operation`), so the
    // projection has to look there. That shape has **no audited precedent** --
    // every `kind: cleanup` operation in the bundled corpus is
    // `requires: forbidden`, `source: none`, because each describes a cleanup
    // the runtime runs rather than one the export installs on its caller's
    // owner -- so the filter below decides membership from the operation's own
    // `Requirement` triple and never from its kind.
    //
    // Inserting `ClaimDomain::Cleanups` into `open` would open the domain for
    // every Solid 1.x contract -- all of them omit `cleanups` entirely -- and
    // `contract_document`'s proven-non-callable assertion expects no call-path
    // domain left open. See
    // `docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`
    // § 2.2 item 5, which forbids taking the other option incidentally.
    let cleanups = export
        .operation_claim(ClaimDomain::Cleanups)
        .expect("cleanups is an operation domain");
    // `computations` (ADR 0114) is read the same way and for the same reason:
    // for its items only. Version 1 never closes it, so it could only ever
    // open; the completeness of the list is `creates`' closure above.
    let computations = export
        .operation_claim(ClaimDomain::Computations)
        .expect("computations is an operation domain");
    let mut requirements = Vec::new();
    for operation in knowledge
        .items()
        .iter()
        .chain(cleanups.items())
        .chain(computations.items())
        .filter_map(|id| export.operation(&id.0))
    {
        // A requirement projects when the operation requires an owner it does
        // not itself supply (`Operation::imposes_owner_requirement`, which the
        // withdrawal of an operation asks too).
        if operation.imposes_owner_requirement() {
            // ADR 0161: the lower bound decides the finding kind. Only a count
            // whose minimum is at least one says every call registers.
            let guaranteed = operation.cardinality.min.is_some_and(|min| min >= 1);
            let operation = match operation.kind {
                OperationKind::Cleanup | OperationKind::Dispose => {
                    OwnerRequirementOperation::Cleanup
                }
                // A `compute`, and the frozen 1.x authority documents'
                // `ambient-at-call` `create`.
                _ => OwnerRequirementOperation::Effect,
            };
            match requirements
                .iter_mut()
                .find(|existing: &&mut ContractOwnerRequirement| existing.operation == operation)
            {
                Some(existing) => existing.guaranteed |= guaranteed,
                None => requirements.push(ContractOwnerRequirement {
                    operation,
                    guaranteed,
                }),
            }
        }
    }
    requirements.sort_by_key(|requirement| format!("{:?}", requirement.operation));
    match knowledge {
        KnowledgeSet::Unknown if requirements.is_empty() => ContractClaim::Open,
        _ => ContractClaim::Known(requirements),
    }
}

/// Which published operation imposes an owner obligation on the *caller*.
///
/// The four shapes here are the ones a consumer can actually meet today: the
/// `ambient-at-call` `create` the two frozen Solid 1.x authority documents
/// still carry (`debounce-root-default.json` and `rootless-root-default.json`,
/// each one `owner-requirement-0`), audited `render`'s `source: created`
/// `create`, and the `kind: cleanup` and `kind: compute` (ADR 0114)
/// requirements the generator publishes. The distinction between the first
/// two is the whole content of the filter: both say `requires: required`, and
/// only one of them is the caller's problem. The last two prove the filter
/// reads the `Requirement` triple rather than the operation's kind.
///
/// A findings fixture can pin this since a fixture can hold an accepted
/// contract (`fixture_authorization.rs`):
/// `fixtures/reactive-ir/package-computation-consumer` reports `SC4001` for an
/// unowned call to an export stating a `compute`, and nothing for the same call
/// inside a component or for the same export stating none. `@solidjs/web`'s
/// audited `render` still reaches no analyzer — `EMBEDDED_SOLID1_BUNDLES` is
/// `&[]` and both first-party bundle producers validate their inputs and return
/// an empty vector.
#[cfg(test)]
mod owner_requirement_projection_tests {
    use std::collections::BTreeSet;

    use super::{project_export_semantics, project_owner_requirements, project_return};
    use crate::contract_semantics::{
        ArtifactIdentity, CallClaims, CallSemantics, Cardinality, CardinalityScope, ClaimDomain,
        Digest, Event, ExportIdentity, ExportSemantics, ExportTargetIdentity, GuardPartition,
        KnowledgeSet, Lifetime, Operation, OperationId, OperationKind, OwnerCapabilities,
        OwnerProduction, OwnerRelation, OwnerRequirements, OwnerSource, Requirement, Resource,
        ResourceId, ResourceKind, ResourceState, Schedule, StabilityKnowledge, Tracking, Trigger,
        UpperBound, ValueShape,
    };
    use crate::{
        ContractClaim, ContractOwnerRequirement, ContractReturn, OwnerRequirementOperation,
    };

    fn digest() -> Digest {
        Digest::parse(format!("sha256:{}", "a".repeat(64))).unwrap()
    }

    fn owner_resource(id: &str) -> Resource {
        Resource {
            id: ResourceId(id.into()),
            kind: ResourceKind::Owner,
            states: KnowledgeSet::Complete(vec![
                ResourceState::OwnerActive,
                ResourceState::OwnerDisposed,
            ]),
            capabilities: KnowledgeSet::Complete(Vec::new()),
            lifetime: Some(Lifetime::Owner(ResourceId(id.into()))),
        }
    }

    fn operation(id: &str, kind: OperationKind, resources: &[&str]) -> Operation {
        Operation {
            id: OperationId(id.into()),
            kind,
            guard: None,
            trigger: Some(Trigger::Event(Event::Call)),
            at: Some(Event::Call),
            schedule: Some(Schedule::SameStack),
            tracking: Tracking::Untracked,
            owner: OwnerRelation::default(),
            cardinality: Cardinality {
                scope: Some(CardinalityScope::Call),
                min: Some(0),
                max: Some(UpperBound::Many),
            },
            inputs: Vec::new(),
            output: None,
            resources: resources
                .iter()
                .map(|resource| ResourceId((*resource).into()))
                .collect(),
            composed_from: None,
            protocol: None,
        }
    }

    fn export(
        claims: CallClaims,
        operations: Vec<Operation>,
        resources: Vec<Resource>,
    ) -> ExportSemantics {
        let module = ArtifactIdentity {
            path: "./index.js".into(),
            digest: digest(),
        };
        let target = ExportTargetIdentity {
            module,
            export_name: "subject".into(),
        };
        ExportSemantics {
            identity: ExportIdentity {
                entrypoint: ".".into(),
                public_name: "subject".into(),
                runtime: target.clone(),
                declarations: target,
            },
            shape: ValueShape::Callable,
            stability: StabilityKnowledge::Unknown,
            call: CallSemantics::new(
                claims,
                operations,
                Vec::new(),
                resources,
                GuardPartition::default(),
            ),
        }
    }

    fn claims() -> CallClaims {
        CallClaims {
            callbacks: KnowledgeSet::Complete(Vec::new()),
            reads: KnowledgeSet::Complete(Vec::new()),
            writes: KnowledgeSet::Unknown,
            creates: KnowledgeSet::Complete(Vec::new()),
            invalidates: KnowledgeSet::Unknown,
            throws: KnowledgeSet::Unknown,
            returns: KnowledgeSet::Complete(Vec::new()),
            cleanups: KnowledgeSet::Unknown,
            disposals: KnowledgeSet::Unknown,
            computations: KnowledgeSet::Unknown,
        }
    }

    /// ADR 0145/0146: a closed claim over one described callable projects to
    /// no reactive return when invoking it reads nothing, and to an accessor
    /// when it reads a signal -- a returned accessor stays one. Neither is a
    /// closed-empty claim, and an open one opens the domain.
    #[test]
    fn a_described_callable_projects_as_no_return_or_as_an_accessor() {
        use crate::contract_semantics::{DescribedCall, DescribedRead};
        let project = |reads: Vec<DescribedRead>, returns: KnowledgeSet<OperationId>| {
            let mut returned = operation("return", OperationKind::Return, &[]);
            returned.output = Some(ValueShape::DescribedCallable(Box::new(DescribedCall {
                reads,
                returns: vec![ValueShape::Plain],
                callbacks: Vec::new(),
            })));
            project_export_semantics(&export(
                CallClaims {
                    returns,
                    ..claims()
                },
                vec![returned],
                Vec::new(),
            ))
        };
        let closed = || KnowledgeSet::Complete(vec![OperationId("return".into())]);
        let inert = project(Vec::new(), closed());
        assert_eq!(inert.returns, ContractClaim::Known(None));
        assert!(!inert.open_claims.contains(&ClaimDomain::Returns));
        assert!(!inert.returns_closed_empty);

        let reading = project(vec![DescribedRead::OwnedSignal], closed());
        let ContractClaim::Known(Some(returned)) = &reading.returns else {
            panic!("a described read projects to a leaf: {:?}", reading.returns);
        };
        assert_eq!(returned.kind, "accessor");
        assert!(!reading.open_claims.contains(&ClaimDomain::Returns));

        let open = project(
            Vec::new(),
            KnowledgeSet::Partial(vec![OperationId("return".into())]),
        );
        assert!(open.open_claims.contains(&ClaimDomain::Returns));
    }

    /// ADR 0143: `returns: []` and a closed claim over one `plain` return both
    /// project to `Known(None)` -- the consumer's single leaf names neither --
    /// and only the empty one sets `returns_closed_empty`, which is what the
    /// inherited premise reads. Before it, a re-exporting package restated a
    /// dependency's plain return as `returns: []`.
    #[test]
    fn only_the_empty_returns_closure_projects_as_closed_empty() {
        let empty = project_export_semantics(&export(claims(), Vec::new(), Vec::new()));
        assert_eq!(empty.returns, ContractClaim::Known(None));
        assert!(empty.returns_closed_empty);

        let mut plain = operation("return", OperationKind::Return, &[]);
        plain.output = Some(ValueShape::Plain);
        let projected = project_export_semantics(&export(
            CallClaims {
                returns: KnowledgeSet::Complete(vec![OperationId("return".into())]),
                ..claims()
            },
            vec![plain],
            Vec::new(),
        ));
        assert_eq!(projected.returns, ContractClaim::Known(None));
        assert!(
            !projected.returns_closed_empty,
            "a plain return is a value; restating it as `returns: []` is false"
        );

        let open = project_export_semantics(&export(
            CallClaims {
                returns: KnowledgeSet::Unknown,
                ..claims()
            },
            Vec::new(),
            Vec::new(),
        ));
        assert!(!open.returns_closed_empty);
    }

    /// Item A of ways-to-improve § 3.3: a closed `callbacks` whose items include
    /// non-call uses of an argument stays closed, and each row carries its
    /// protocol so re-emission republishes it; only the call row is an
    /// invocation any consumer pass models.
    #[test]
    fn a_closed_callbacks_with_protocol_items_projects_closed_with_each_protocol() {
        use crate::contract_semantics::{CallbackInvocation, InvokeProtocol, ValueSource};
        let invoke = |id: &str, protocol: Option<InvokeProtocol>| {
            let mut invoke = operation(id, OperationKind::Invoke, &[]);
            invoke.tracking = Tracking::AmbientAtExecution;
            invoke.protocol = protocol;
            invoke
        };
        let item = |index: u16, id: &str| CallbackInvocation {
            from: ValueSource::Parameter {
                index,
                path: Vec::new(),
            },
            operation: OperationId(id.into()),
        };
        let projected = project_export_semantics(&export(
            CallClaims {
                callbacks: KnowledgeSet::Complete(vec![
                    item(0, "callback-0"),
                    item(0, "callback-1"),
                    item(1, "callback-2"),
                ]),
                ..claims()
            },
            vec![
                invoke("callback-0", None),
                invoke("callback-1", Some(InvokeProtocol::Get)),
                invoke("callback-2", Some(InvokeProtocol::Coerce)),
            ],
            Vec::new(),
        ));
        assert!(
            !projected.open_claims.contains(&ClaimDomain::Callbacks),
            "{:?}",
            projected.open_claims
        );
        let rows = projected.callbacks.known().expect("the domain stays known");
        let protocols = rows
            .iter()
            .map(|row| (row.parameter, row.protocol, row.is_invocation()))
            .collect::<Vec<_>>();
        assert_eq!(protocols.len(), 3);
        for expected in [
            (0, InvokeProtocol::Call, true),
            (0, InvokeProtocol::Get, false),
            (1, InvokeProtocol::Coerce, false),
        ] {
            assert!(protocols.contains(&expected), "{protocols:?}");
        }
        assert!(
            rows.iter().all(|row| !row.clears_tracking),
            "an ambient row never claims to clear the caller's listener"
        );
    }

    /// The read-back is the generator's per-word mapping inverted: a document's
    /// `untracked` is a clearing on an `inline` or `deferred` row and nothing on
    /// a `tracked` one, and `ambient-at-execution` is never a clearing. This is
    /// the bit the wrapper fold reads to call a package row `Detaching`.
    #[test]
    fn a_projected_row_clears_tracking_exactly_where_its_word_says_so() {
        use crate::contract_semantics::{CallbackInvocation, ValueSource};
        let invoke = |id: &str, schedule: Schedule, tracking: Tracking| {
            let mut invoke = operation(id, OperationKind::Invoke, &[]);
            invoke.schedule = Some(schedule);
            invoke.tracking = tracking;
            invoke
        };
        let item = |index: u16, id: &str| CallbackInvocation {
            from: ValueSource::Parameter {
                index,
                path: Vec::new(),
            },
            operation: OperationId(id.into()),
        };
        let cases = [
            (Schedule::SameStack, Tracking::Untracked, "inline", true),
            (
                Schedule::SameStack,
                Tracking::AmbientAtExecution,
                "inline",
                false,
            ),
            (Schedule::Queued, Tracking::Untracked, "deferred", true),
            (
                Schedule::Queued,
                Tracking::AmbientAtExecution,
                "deferred",
                false,
            ),
            (Schedule::SameStack, Tracking::Tracked, "tracked", false),
        ];
        let ids = (0..cases.len())
            .map(|index| format!("callback-{index}"))
            .collect::<Vec<_>>();
        let projected = project_export_semantics(&export(
            CallClaims {
                callbacks: KnowledgeSet::Complete(
                    (0..cases.len())
                        .map(|index| item(u16::try_from(index).unwrap(), &ids[index]))
                        .collect(),
                ),
                ..claims()
            },
            cases
                .iter()
                .zip(&ids)
                .map(|((schedule, tracking, _, _), id)| invoke(id, *schedule, *tracking))
                .collect(),
            Vec::new(),
        ));
        let rows = projected.callbacks.known().expect("the domain stays known");
        for (index, (_, _, execution, clears)) in cases.iter().enumerate() {
            let row = rows
                .iter()
                .find(|row| row.parameter == index)
                .expect("one row per parameter");
            assert_eq!(
                (row.execution.as_str(), row.clears_tracking),
                (*execution, *clears),
                "{index}"
            );
        }
    }

    /// ADR 0139: a `result-access` item projects as the deferred row it is --
    /// an ambient deferral under an inherited owner, never a clearing -- that
    /// carries the event, and the domain stays closed.
    #[test]
    fn a_result_access_item_projects_as_a_deferred_row_that_keeps_its_event() {
        use crate::CallbackSchedule;
        use crate::contract_semantics::{
            CallbackInvocation, OwnerRelation, OwnerSource, ValueSource,
        };
        let mut kept = operation("callback-1", OperationKind::Invoke, &[]);
        kept.trigger = Some(Trigger::Event(Event::ResultAccess));
        kept.at = Some(Event::ResultAccess);
        kept.schedule = Some(Schedule::External);
        kept.tracking = Tracking::AmbientAtExecution;
        kept.owner = OwnerRelation {
            source: OwnerSource::AmbientAtExecution,
            ..OwnerRelation::default()
        };
        kept.cardinality.scope = Some(CardinalityScope::Trigger);
        let projected = project_export_semantics(&export(
            CallClaims {
                callbacks: KnowledgeSet::Complete(vec![CallbackInvocation {
                    from: ValueSource::Parameter {
                        index: 1,
                        path: Vec::new(),
                    },
                    operation: OperationId("callback-1".into()),
                }]),
                ..claims()
            },
            vec![kept],
            Vec::new(),
        ));
        assert!(
            !projected.open_claims.contains(&ClaimDomain::Callbacks),
            "{:?}",
            projected.open_claims
        );
        let rows = projected.callbacks.known().expect("the domain stays known");
        let [row] = rows.as_slice() else {
            panic!("one row: {rows:?}");
        };
        assert_eq!(row.parameter, 1);
        assert_eq!(row.execution, "deferred");
        assert_eq!(row.schedule, Some(CallbackSchedule::ResultAccess));
        assert!(row.is_result_access());
        assert!(row.invokes_argument());
        assert!(!row.clears_tracking);
        assert_eq!(row.owner.as_deref(), Some("inherited"));
    }

    /// ADR 0152: a slot is a returned invocation exactly when the closed
    /// `returns` is described callables every one of which invokes it and the
    /// closed `callbacks` keeps it at `result-access` and nowhere else.
    #[test]
    fn a_returned_invocation_needs_both_closures_and_every_alternative() {
        use crate::contract_semantics::{
            CallbackInvocation, DescribedCall, DescribedCallback, OwnerRelation, OwnerSource,
            ValueSource,
        };
        let kept = |slot: u16| {
            let mut kept = operation(&format!("callback-{slot}"), OperationKind::Invoke, &[]);
            kept.trigger = Some(Trigger::Event(Event::ResultAccess));
            kept.at = Some(Event::ResultAccess);
            kept.schedule = Some(Schedule::External);
            kept.tracking = Tracking::AmbientAtExecution;
            kept.owner = OwnerRelation {
                source: OwnerSource::AmbientAtExecution,
                ..OwnerRelation::default()
            };
            kept.cardinality.scope = Some(CardinalityScope::Trigger);
            kept
        };
        let returned = |id: &str, slots: &[u16]| {
            let mut returned = operation(id, OperationKind::Return, &[]);
            returned.output = Some(ValueShape::DescribedCallable(Box::new(DescribedCall {
                reads: Vec::new(),
                returns: Vec::new(),
                callbacks: slots
                    .iter()
                    .copied()
                    .map(DescribedCallback::same_stack_once)
                    .collect(),
            })));
            returned
        };
        let project = |callbacks: KnowledgeSet<CallbackInvocation>,
                       returns: Vec<Operation>,
                       mut operations: Vec<Operation>| {
            let ids = returns
                .iter()
                .map(|operation| operation.id.clone())
                .collect();
            operations.extend(returns);
            project_export_semantics(&export(
                CallClaims {
                    callbacks,
                    returns: KnowledgeSet::Complete(ids),
                    ..claims()
                },
                operations,
                Vec::new(),
            ))
            .returned_invocations
        };
        let item = |slot: u16| CallbackInvocation {
            from: ValueSource::Parameter {
                index: slot,
                path: Vec::new(),
            },
            operation: OperationId(format!("callback-{slot}")),
        };
        let both = || KnowledgeSet::Complete(vec![item(0), item(1)]);
        assert_eq!(
            project(
                both(),
                vec![returned("return", &[0, 1])],
                vec![kept(0), kept(1)]
            ),
            BTreeSet::from([0, 1])
        );
        // One alternative that does not invoke slot 1 leaves it "may run".
        assert_eq!(
            project(
                both(),
                vec![returned("return-0", &[0, 1]), returned("return-1", &[0])],
                vec![kept(0), kept(1)]
            ),
            BTreeSet::from([0])
        );
        // An open `callbacks`, or a slot the export also invokes inline.
        assert!(
            project(
                KnowledgeSet::Partial(vec![item(0), item(1)]),
                vec![returned("return", &[0, 1])],
                vec![kept(0), kept(1)]
            )
            .is_empty()
        );
        let mut inline = operation("callback-1", OperationKind::Invoke, &[]);
        inline.at = Some(Event::Call);
        inline.schedule = Some(Schedule::SameStack);
        assert_eq!(
            project(
                both(),
                vec![returned("return", &[0, 1])],
                vec![kept(0), inline]
            ),
            BTreeSet::from([0])
        );
    }

    /// ADR 0113: a closed `returns` whose every operation hands back a `plain`
    /// value is the consumer's own "no reactive return described", and leaves
    /// nothing open. The claim has to be closed, and the output plain: an open
    /// claim, or a closed one over an output this projection cannot represent,
    /// still opens the domain.
    #[test]
    fn a_closed_plain_return_projects_as_no_reactive_return() {
        let mut returned = operation("return", OperationKind::Return, &[]);
        returned.output = Some(ValueShape::Plain);
        let with_returns = |returns: KnowledgeSet<OperationId>, operation: &Operation| {
            let mut claims = claims();
            claims.returns = returns;
            export(claims, vec![operation.clone()], Vec::new())
        };

        let mut open = BTreeSet::new();
        let closed = with_returns(KnowledgeSet::Complete(vec![returned.id.clone()]), &returned);
        assert_eq!(
            project_return(&closed, &mut open),
            ContractClaim::Known(None)
        );
        assert!(open.is_empty(), "{open:?}");

        let mut open = BTreeSet::new();
        let partial = with_returns(KnowledgeSet::Partial(vec![returned.id.clone()]), &returned);
        assert_eq!(project_return(&partial, &mut open), ContractClaim::Open);
        assert!(open.contains(&ClaimDomain::Returns));

        let mut unknown = returned.clone();
        unknown.output = Some(ValueShape::Unknown);
        let mut open = BTreeSet::new();
        let unrepresented =
            with_returns(KnowledgeSet::Complete(vec![unknown.id.clone()]), &unknown);
        assert_eq!(
            project_return(&unrepresented, &mut open),
            ContractClaim::Open
        );
        assert!(open.contains(&ClaimDomain::Returns));
    }

    /// ADR 0115: returns of argument containers. A union with no one reactive
    /// leaf reads as no reactive return, including one whose `[]` the
    /// projection drops, which must not read as its one surviving leaf; a lone
    /// fresh array is a tuple of its argument leaves; and an open claim over
    /// the same returns stays open.
    #[test]
    fn argument_containers_project_as_their_one_leaf_or_as_no_reactive_return() {
        let returned = |id: &str, output: ValueShape| {
            let mut operation = operation(id, OperationKind::Return, &[]);
            operation.output = Some(output);
            operation
        };
        let parameter = |index| ValueShape::Parameter {
            index,
            path: Vec::new(),
        };
        let array = |items: &[u16]| ValueShape::ArgumentArray {
            items: items.to_vec(),
        };
        let with_returns = |closed: bool, operations: Vec<Operation>| {
            let ids = operations
                .iter()
                .map(|operation| operation.id.clone())
                .collect::<Vec<_>>();
            let mut claims = claims();
            claims.returns = if closed {
                KnowledgeSet::Complete(ids)
            } else {
                KnowledgeSet::Partial(ids)
            };
            export(claims, operations, Vec::new())
        };
        let project = |closed, operations| {
            let mut open = BTreeSet::new();
            let projected = project_return(&with_returns(closed, operations), &mut open);
            (projected, open.contains(&ClaimDomain::Returns))
        };

        let as_array = || {
            vec![
                returned("return-0", parameter(0)),
                returned("return-1", array(&[])),
                returned("return-2", array(&[0])),
            ]
        };
        assert_eq!(
            project(true, as_array()),
            (ContractClaim::Known(None), false)
        );
        assert_eq!(
            project(
                true,
                vec![
                    returned("return-0", parameter(0)),
                    returned("return-1", array(&[]))
                ]
            ),
            (ContractClaim::Known(None), false),
            "the argument, or an empty array, is not the argument"
        );
        assert_eq!(
            project(true, vec![returned("return-0", array(&[1]))]),
            (
                ContractClaim::Known(Some(ContractReturn {
                    kind: "tuple".into(),
                    elements: vec![Some(ContractReturn {
                        kind: "argument".into(),
                        parameter: Some(1),
                        ..ContractReturn::default()
                    })],
                    ..ContractReturn::default()
                })),
                false
            )
        );
        assert_eq!(project(false, as_array()), (ContractClaim::Open, true));

        // ADR 0116: `accessWith`'s two returns, and a lone invocation result.
        let invoked = |parameter| ValueShape::InvocationResult { parameter };
        assert_eq!(
            project(
                true,
                vec![
                    returned("return-0", invoked(0)),
                    returned("return-1", parameter(0))
                ]
            ),
            (ContractClaim::Known(None), false),
            "the argument, or what calling it returned, names no one leaf"
        );
        assert_eq!(
            project(true, vec![returned("return-0", invoked(1))]),
            (ContractClaim::Known(None), false)
        );
        assert_eq!(
            project(false, vec![returned("return-0", invoked(0))]),
            (ContractClaim::Open, true)
        );

        // `Object.keys`: a fresh array of primitives holds no reactive leaf.
        let strings = || ValueShape::Array {
            element: Box::new(ValueShape::Plain),
            length: crate::contract_semantics::ArrayLength::default(),
        };
        assert_eq!(
            project(true, vec![returned("return", strings())]),
            (ContractClaim::Known(None), false)
        );

        // Item B round 2: `callHandler`'s `event?.defaultPrevented` -- the
        // member of the caller's argument, or undefined -- and a lone
        // `return p.key`. The member is what the argument holds when the
        // return reads it, which nothing at the call site determines, so it
        // names no leaf: never the whole argument, which is what reading its
        // `parameter` alone would say. This is the hiding direction, and it is
        // deliberate: a store passed there, whose member is itself a store
        // path, is read as no reactive return.
        let member = |index, key: &str| ValueShape::Parameter {
            index,
            path: vec![key.into()],
        };
        assert_eq!(
            project(
                true,
                vec![
                    returned("return-0", member(0, "defaultPrevented")),
                    returned("return-1", ValueShape::Undefined)
                ]
            ),
            (ContractClaim::Known(None), false),
            "a member of the argument, or undefined, names no one leaf"
        );
        assert_eq!(
            project(true, vec![returned("return-0", member(0, "key"))]),
            (ContractClaim::Known(None), false),
            "a member of the argument is not the argument"
        );
        assert_eq!(
            project(true, vec![returned("return-0", ValueShape::Undefined)]),
            (ContractClaim::Known(None), false)
        );
        assert_eq!(
            project(false, vec![returned("return-0", member(0, "key"))]),
            (ContractClaim::Open, true)
        );
    }

    /// The shape the two frozen Solid 1.x authority documents still carry, and
    /// the only `ambient-at-call` `create` a consumer can meet: it needs an
    /// ambient owner it did not make. This is the obligation `SC4001` reports
    /// at an unowned call.
    ///
    /// The generator no longer produces it. A `create` naming a child owner
    /// resource would contradict `semantic-model.md` § creates -- a `create`
    /// registers a resource into a runtime *outside* the invocation -- so an
    /// `Effect` owner requirement was withheld by name instead, and since
    /// ADR 0114 is a `compute` in `computations` (the test after the cleanup
    /// one). This test pins how a consumer reads the frozen documents until
    /// their re-capture lands.
    #[test]
    fn an_ambient_at_call_requirement_projects_as_a_consumer_obligation() {
        let mut created = operation("register-effect", OperationKind::Create, &["child-owner"]);
        created.owner = OwnerRelation {
            source: OwnerSource::AmbientAtCall,
            requirements: OwnerRequirements {
                owner: Requirement::Required,
                child_owners: Requirement::Required,
                cleanup: Requirement::Unconstrained,
            },
            capabilities: OwnerCapabilities::default(),
            lifetime: None,
            productions: KnowledgeSet::Complete(vec![OwnerProduction {
                resource: ResourceId("child-owner".into()),
                capabilities: OwnerCapabilities::default(),
                lifetime: Some(Lifetime::Owner(ResourceId("child-owner".into()))),
            }]),
        };
        let mut claims = claims();
        claims.creates = KnowledgeSet::Complete(vec![created.id.clone()]);
        let export = export(claims, vec![created], vec![owner_resource("child-owner")]);

        let mut open = BTreeSet::new();
        assert_eq!(
            project_owner_requirements(&export, &mut open),
            ContractClaim::Known(vec![ContractOwnerRequirement {
                operation: OwnerRequirementOperation::Effect,
                guaranteed: false,
            }])
        );
        assert!(open.is_empty());
    }

    /// Audited `@solidjs/web` `render`'s `register-delegation`: `requires:
    /// required` *and* `source: created`. It runs under the root it made, so a
    /// top-level `render(() => <App/>, el)` owes its caller nothing.
    #[test]
    fn an_operation_that_created_its_own_owner_imposes_nothing_on_the_caller() {
        let mut created = operation(
            "register-delegation",
            OperationKind::Create,
            &["browser-root"],
        );
        created.owner = OwnerRelation {
            source: OwnerSource::Created(ResourceId("browser-root".into())),
            requirements: OwnerRequirements {
                owner: Requirement::Required,
                child_owners: Requirement::Unconstrained,
                cleanup: Requirement::Unconstrained,
            },
            capabilities: OwnerCapabilities::default(),
            lifetime: Some(Lifetime::Owner(ResourceId("browser-root".into()))),
            productions: KnowledgeSet::Complete(vec![OwnerProduction {
                resource: ResourceId("browser-root".into()),
                capabilities: OwnerCapabilities::default(),
                lifetime: Some(Lifetime::Owner(ResourceId("browser-root".into()))),
            }]),
        };
        let mut claims = claims();
        claims.creates = KnowledgeSet::Complete(vec![created.id.clone()]);
        let export = export(claims, vec![created], vec![owner_resource("browser-root")]);

        let mut open = BTreeSet::new();
        assert_eq!(
            project_owner_requirements(&export, &mut open),
            ContractClaim::Known(Vec::new())
        );
        assert!(open.is_empty());
    }

    /// The generated cleanup shape: `kind: cleanup` in the `cleanups` domain,
    /// `source: ambient-at-call`, `requires: required`,
    /// `requiresCleanup: required`, and **no resource**. No audited document
    /// carries it -- every bundled `kind: cleanup` operation is
    /// `requires: forbidden`, `source: none` -- so the projection has to read
    /// the domain for its items while leaving it out of `open`, because every
    /// 1.x contract omits `cleanups` entirely.
    ///
    /// The role in the result is the user-visible half: this requirement now
    /// round-trips as `OwnerRequirementOperation::Cleanup`, so `SC4001`'s
    /// remedy names `onCleanup` rather than an owner for an effect.
    #[test]
    fn a_cleanup_requirement_projects_from_the_cleanups_domain_without_opening_it() {
        let mut cleanup = operation("replace-cleanup", OperationKind::Cleanup, &[]);
        cleanup.owner = OwnerRelation {
            source: OwnerSource::AmbientAtCall,
            requirements: OwnerRequirements {
                owner: Requirement::Required,
                child_owners: Requirement::Unconstrained,
                cleanup: Requirement::Required,
            },
            capabilities: OwnerCapabilities::default(),
            lifetime: None,
            productions: KnowledgeSet::Complete(Vec::new()),
        };
        let mut claims = claims();
        claims.cleanups = KnowledgeSet::Partial(vec![cleanup.id.clone()]);
        let export = export(claims, vec![cleanup], Vec::new());

        let mut open = BTreeSet::new();
        assert_eq!(
            project_owner_requirements(&export, &mut open),
            ContractClaim::Known(vec![ContractOwnerRequirement {
                operation: OwnerRequirementOperation::Cleanup,
                guaranteed: false,
            }])
        );
        // `creates` is closed and empty here, and the *cleanups* read must not
        // add a domain of its own.
        assert!(open.is_empty());
    }

    /// ADR 0114's shape for an `Effect` requirement: `kind: compute` in the
    /// `computations` domain, requiring the ambient owner and child owners of
    /// it. It projects as the obligation the 1.x `ambient-at-call` `create`
    /// did, read for its items only, and with `creates` open instead the list
    /// it states is partial, so the domain that says "complete" is the one
    /// that opens.
    #[test]
    fn a_compute_requirement_projects_as_an_effect_from_the_computations_domain() {
        let mut compute = operation("owner-requirement-0", OperationKind::Compute, &[]);
        compute.owner = OwnerRelation {
            source: OwnerSource::AmbientAtCall,
            requirements: OwnerRequirements {
                owner: Requirement::Required,
                child_owners: Requirement::Required,
                cleanup: Requirement::Unconstrained,
            },
            capabilities: OwnerCapabilities::default(),
            lifetime: None,
            productions: KnowledgeSet::Unknown,
        };
        let mut claims = claims();
        claims.computations = KnowledgeSet::Partial(vec![compute.id.clone()]);
        let effect = ContractClaim::Known(vec![ContractOwnerRequirement {
            operation: OwnerRequirementOperation::Effect,
            guaranteed: false,
        }]);

        let closed = export(claims.clone(), vec![compute.clone()], Vec::new());
        let mut open = BTreeSet::new();
        assert_eq!(project_owner_requirements(&closed, &mut open), effect);

        // ADR 0161: the count decides whether the requirement is guaranteed.
        // `min: 0` may register; `min: 1` registers on every call, and one
        // such operation of the kind is enough.
        let mut every_call = compute.clone();
        every_call.id = OperationId("owner-requirement-1".into());
        every_call.cardinality.min = Some(1);
        let mut both = claims.clone();
        both.computations = KnowledgeSet::Partial(vec![compute.id.clone(), every_call.id.clone()]);
        let guaranteed = export(both, vec![compute.clone(), every_call], Vec::new());
        assert_eq!(
            project_owner_requirements(&guaranteed, &mut BTreeSet::new()),
            ContractClaim::Known(vec![ContractOwnerRequirement {
                operation: OwnerRequirementOperation::Effect,
                guaranteed: true,
            }])
        );
        assert!(open.is_empty(), "computations adds no domain of its own");

        claims.creates = KnowledgeSet::Unknown;
        let partial = export(claims, vec![compute], Vec::new());
        let mut open = BTreeSet::new();
        assert_eq!(project_owner_requirements(&partial, &mut open), effect);
        assert_eq!(open, BTreeSet::from([ClaimDomain::Creates]));
    }
}

fn project_async_behavior(
    export: &crate::contract_semantics::ExportSemantics,
    open: &mut BTreeSet<ClaimDomain>,
) -> ContractClaim<String> {
    let mut protocol = None;
    let returns = export
        .operation_claim(ClaimDomain::Returns)
        .expect("returns is an operation domain");
    if !returns.is_closed() {
        open.insert(ClaimDomain::Returns);
    }
    for operation in returns.items() {
        match export
            .operation(&operation.0)
            .and_then(|operation| operation.output.as_ref())
        {
            Some(ValueShape::Promise(_)) => protocol = Some("promise"),
            Some(ValueShape::AsyncIterable(_)) => protocol = Some("async-iterable"),
            _ => {}
        }
    }
    ContractClaim::Known(protocol.unwrap_or_default().into())
}
#[derive(Clone)]
pub(super) struct ResolvedContractBinding {
    pub(super) local_name: String,
    pub(super) imported_name: String,
    pub(super) package_name: String,
    pub(super) symbol: SymbolId,
    pub(super) runtime_identity: String,
    pub(super) contract_location: Location,
    pub(super) summary: ContractExport,
}

pub(super) struct ResolvedContracts {
    pub(super) bindings: Vec<ResolvedContractBinding>,
    pub(super) by_symbol: HashMap<SymbolId, ResolvedContractBinding>,
    pub(super) missing_exports: Vec<StaticDefect>,
    /// How binding answered per declaration, so a refusal is countable rather
    /// than merely silent. See [`crate::ContractBindingCounts`].
    pub(super) counts: crate::ContractBindingCounts,
}

fn runtime_identity_at(facts: &ProjectFacts, location: &Location) -> String {
    facts
        .typescript
        .entities()
        .find(|entity| entity.location == *location)
        .map_or_else(String::new, |entity| entity.runtime_identity.to_string())
}

fn source_name_at(facts: &ProjectFacts, location: &Location) -> String {
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == location.path.as_ref())
        .and_then(|file| {
            file.source_text(solid_facts::core::Span::new(
                u32::try_from(location.start_byte).ok()?,
                u32::try_from(location.end_byte).ok()?,
            ))
        })
        .unwrap_or_default()
        .to_owned()
}

fn push_runtime_identity_conflict(
    missing_exports: &mut Vec<StaticDefect>,
    location: &Location,
    seen_locations: &mut HashSet<(String, u64, u64)>,
) {
    let key = (
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
    );
    if !seen_locations.insert(key) {
        return;
    }
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: "<runtime-identity-conflict>".into(),
            export: "<conflicting-contract-summaries>".into(),
            reexported: true,
            site: crate::ContractDefectSite::Argument,
            admission_refusal: None,
        },
        location: location.clone(),
        analysis_context:
            "multiple exact package contracts describe the same runtime export differently".into(),
        fixes: vec![],
        uncertain: false,
    });
}

/// Joins package summaries through exact runtime identity after direct import
/// and explicit re-export discovery.
///
/// This is deliberately an O(entities + contracted-bindings) pass. It does
/// not resolve by spelling, scan every entity for every shorthand, or turn an
/// empty identity into project ownership. Export-star chains can participate
/// when TypeFacts exposes the same identity at a concrete binding; a missing
/// identity remains fail-closed.
fn join_runtime_identity_aliases(
    facts: &ProjectFacts,
    entities: &EntitySymbols,
    bindings: &mut Vec<ResolvedContractBinding>,
    by_symbol: &mut HashMap<SymbolId, ResolvedContractBinding>,
    missing_exports: &mut Vec<StaticDefect>,
) {
    let mut candidates = HashMap::<String, Vec<ResolvedContractBinding>>::new();
    for binding in bindings
        .iter()
        .filter(|binding| !binding.runtime_identity.is_empty())
    {
        let entries = candidates
            .entry(binding.runtime_identity.clone())
            .or_default();
        if !entries.iter().any(|existing| {
            existing.package_name == binding.package_name
                && existing.contract_location == binding.contract_location
                && existing.summary == binding.summary
        }) {
            entries.push(binding.clone());
        }
    }

    let mut index = HashMap::new();
    let mut conflicts = HashSet::new();
    for (identity, entries) in candidates {
        let Some(first) = entries.first().cloned() else {
            continue;
        };
        if entries
            .iter()
            .skip(1)
            .any(|entry| entry.package_name != first.package_name || entry.summary != first.summary)
        {
            conflicts.insert(identity);
            continue;
        }
        index.insert(identity, first);
    }

    let mut bound_symbols = by_symbol.keys().cloned().collect::<HashSet<_>>();
    let mut seen_locations = HashSet::new();
    for entity in facts
        .typescript
        .entities()
        .filter(|entity| !entity.runtime_identity.is_empty())
    {
        let Some(symbol) = entities.get(&entity.location).cloned() else {
            continue;
        };
        if conflicts.contains(entity.runtime_identity.as_ref()) {
            push_runtime_identity_conflict(missing_exports, &entity.location, &mut seen_locations);
            continue;
        }
        let Some(template) = index.get(entity.runtime_identity.as_ref()).cloned() else {
            continue;
        };
        if let Some(existing) = by_symbol.get(&symbol)
            && (existing.package_name != template.package_name
                || existing.summary != template.summary)
        {
            push_runtime_identity_conflict(missing_exports, &entity.location, &mut seen_locations);
            continue;
        }
        if !bound_symbols.insert(symbol.clone()) {
            continue;
        }
        let mut binding = template;
        binding.local_name = source_name_at(facts, &entity.location);
        binding.symbol = symbol.clone();
        binding.runtime_identity = entity.runtime_identity.to_string();
        by_symbol.insert(symbol, binding.clone());
        bindings.push(binding);
    }
}

/// A missing external-contract obligation must not invent a receipt
/// requirement for a primitive already modeled by the selected dialect.
/// Core contracts themselves are excluded at the analysis boundary.
fn native_vocabulary_outranks_contract(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    dialect.owns_module(module) && dialect.declares_primitive(imported)
}

fn missing_accepted_export_needs_obligation(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    !native_vocabulary_outranks_contract(dialect, module, imported)
}

/// Keep the known parts of a partial export usable while opening the existing
/// per-export contract obligation for every non-callback claim that cannot yet
/// be consumed demand-sensitively. Unknown callbacks are handled separately:
/// omitting their symbol from the callback map preserves the existing
/// callable-argument obligation and stays quiet for calls with no callable
/// argument.
/// The bound symbols whose `returns` **no consumer in this project can
/// reach**, so its openness discharges no proof obligation.
///
/// Demand read off every consumer in
/// `docs/package-contract-v2/phase21/2026-09-10-sc9005-demand-scoping-design.md`
/// § 8: `returns` is consulted where a call's result goes somewhere
/// (`source_discovery` 225/621/918/1011, `static_rules` 248) and where the
/// binding is a computation's argument (`owners` 1886, through
/// `asyncBehavior`). Both reduce to the same question about a *reference*: is
/// it the callee of a call that throws its result away, or is it anything
/// else?
///
/// Sound in one direction only, and that is the direction that matters. A
/// symbol sheds only when it has references and **every** one of them is a
/// discarded call's callee; a reference this cannot classify — an argument, a
/// member base, a re-export, one that resolves to no symbol — keeps the
/// obligation. Shedding wrongly drops a fail-closed answer silently, so the
/// predicate is written to fail toward reporting.
///
/// References are counted per file because an import's binding symbol is
/// file-local: every reference to it is in the file that imported it, which
/// is what makes "every one of them" decidable here at all.
fn returns_shed_symbols(facts: &ProjectFacts, entities: &EntitySymbols) -> HashSet<SymbolId> {
    let mut references = HashMap::<SymbolId, (usize, usize)>::new();
    for file in &facts.files {
        let discarded: HashSet<(u32, u32)> = file
            .ast
            .calls
            .iter()
            .filter(|call| call.result_discarded)
            .map(|call| (call.callee.start, call.callee.end))
            .collect();
        for identifier in &file.ast.identifiers {
            if identifier.role != solid_facts::ast::IdentifierRole::Reference {
                continue;
            }
            let Some(symbol) = entities.get(&location(file.path.shared(), identifier.span)) else {
                continue;
            };
            let counts = references.entry(symbol.clone()).or_default();
            counts.0 += 1;
            if discarded.contains(&(identifier.span.start, identifier.span.end)) {
                counts.1 += 1;
            }
        }
    }
    references
        .into_iter()
        .filter(|(_, (total, discarded))| *total > 0 && total == discarded)
        .map(|(symbol, _)| symbol)
        .collect()
}

/// Whether this binding's `reads` **completeness** is demanded here.
///
/// Always true, and now believed to be the right answer rather than a
/// placeholder for one.
///
/// The seam was cut expecting a narrowing. `reads` completeness proves an
/// export reads nothing beyond what it enumerates; rules consume `reads`
/// *items*, which arrive whether or not the domain is closed, and **only
/// SC9005 consumes the completeness**
/// (`docs/package-contract-v2/phase21/2026-09-10-reads-demand-population.md`
/// § 6). The narrowing that suggested itself was "demand it only where the
/// call site is tracked".
///
/// That predicate is wrong, and not merely unavailable here. A contract read
/// is consumed in six of the ten [`crate::ExecutionRole`]s — every stale-read
/// role in `reports_untracked_read`, `TrackedJsx` through the async boundary
/// rules, and `DeferredCallback` through the leaf-owner clause. Of the three
/// left, two are consumed nowhere only because no rule reports a pending read
/// in an event handler *yet*, so shedding them would freeze a rules gap into
/// the trust boundary. What remains is `DiscardedRendering`: a call site the
/// compiler deleted, which performs no reads at all and produces no finding
/// to shed. See `2026-09-10-sc9005-demand-scoping-design.md` § 13.
///
/// Kept as a named function rather than folded back into the conjunction: it
/// is where a future narrowing goes, and where the reason it has not happened
/// is written down.
const fn reads_completeness_demanded() -> bool {
    true
}

fn push_unknown_contract_claims(
    missing_exports: &mut Vec<StaticDefect>,
    summary: &ContractExport,
    module: &str,
    export: &str,
    reexported: bool,
    location: Location,
    returns_demanded: bool,
) {
    let mut claims = Vec::new();
    if reads_completeness_demanded()
        && (summary.reactive_reads.is_open()
            || summary
                .open_claims
                .contains(&crate::contract_semantics::ClaimDomain::Reads))
    {
        claims.push("reactiveReads");
    }
    // Scoped by demand (`returns_shed_symbols`): an open domain no consumer
    // can reach discharges no obligation, so reporting it is noise rather than
    // a fail-closed answer.
    //
    // `creates` is still unconditional because its demand really is "the
    // binding is called". `reads` is unconditional for a different reason —
    // see `reads_completeness_demanded` — and design § 9's claim that the two
    // are alike is corrected in § 12.
    if returns_demanded
        && (summary.returns.is_open()
            || summary
                .open_claims
                .contains(&crate::contract_semantics::ClaimDomain::Returns))
    {
        claims.push("returns");
    }
    if summary.owner_requirements.is_open()
        || summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Creates)
    {
        claims.push("ownerRequirements");
    }
    // No `open_claims` disjunct here, deliberately. `project_async_behavior`
    // derives this field from the **returns** domain and inserts
    // `ClaimDomain::Returns`; it never inserts `Throws`, and no other consumer
    // path does either, so the `Throws` disjunct this check used to carry was
    // both unreachable and a claim about the wrong domain. Returns is already
    // the conjunct above. The `is_open()` guard stays: it is the fail-closed
    // answer for any summary that arrives with the field genuinely open, which
    // `ContractExport::unknown_runtime_kind` still constructs.
    if summary.async_behavior.is_open() {
        claims.push("asyncBehavior");
    }
    if claims.is_empty() {
        return;
    }
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: module.to_owned(),
            export: export.to_owned(),
            reexported,
            site: crate::ContractDefectSite::Import,
            admission_refusal: None,
        },
        location,
        analysis_context: format!("unknown-contract-claims:{}", claims.join(",")),
        fixes: vec![],
        uncertain: false,
    });
}

/// ADR 0153 part 3: the `(package, export)` contexts this project provides,
/// or may provide, itself.
///
/// An accepted summary may state its claims under a context premise: they hold
/// only where the context the package exports under that name receives no
/// value from outside the package. This is the consumer's half of that
/// premise, and it fails toward "provided". A premise export is provided when
/// any reference to a binding of it that this project imports from the package
/// is anything but the one argument of the dialect's `useContext`:
///
/// - a JSX element `<RouterContext value={…}>` or `createComponent(RouterContext, …)`,
///   which is how a value is provided;
/// - a member access (`RouterContext.Provider`), an alias, an argument of any
///   other call, a re-export, a return: anything a value could be provided
///   through, from here or from code this analysis does not follow.
///
/// A namespace import of the package counts every premise export of it as
/// provided unless each reference is a member access naming another export or
/// a `useContext(ns.Context)` argument. A re-export of a premise export, an
/// `export * from` the package, `import … = require`, a dynamic `import()` and
/// a `require` of it all count as provided. Absence of a reference is the only
/// way a premise holds; nothing here is read as proof that one does not.
///
/// Packages other than the one certified may provide the context too, and the
/// analysis does not see their code. That half is the backend's: it names every
/// package whose installed tree holds another package depending on it
/// ([`AcceptedContractIndex::context_provided_packages`]), and every premise
/// of such a package is provided.
fn provided_context_premises(
    facts: &ProjectFacts,
    exact: &HashMap<(String, String), PackageContract>,
    accepted: &AcceptedContractIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> BTreeSet<(String, String)> {
    // Every premise any bound summary states, by package.
    let mut premises = HashMap::<String, BTreeSet<String>>::new();
    for contract in exact.values() {
        for entrypoint in contract.entrypoints.values() {
            for summary in entrypoint.exports.values() {
                if !summary.context_premises.is_empty() {
                    premises
                        .entry(contract.package.name.clone())
                        .or_default()
                        .extend(summary.context_premises.iter().cloned());
                }
            }
        }
    }
    let mut provided = BTreeSet::new();
    if premises.is_empty() {
        return provided;
    }
    for (package, names) in &premises {
        if accepted.context_provided_package(package) {
            provided.extend(names.iter().map(|name| (package.clone(), name.clone())));
        }
    }
    let package_of = |file: &solid_facts::FileFacts, module: &str| -> Option<String> {
        exact
            .get(&(file.path.to_string(), module.to_owned()))
            .map(|contract| contract.package.name.clone())
            .or_else(|| {
                // Fails toward "provided": a specifier naming the package by
                // its own name, or one of its subpaths, is the package here.
                premises
                    .keys()
                    .find(|package| {
                        module == package.as_str()
                            || module
                                .strip_prefix(package.as_str())
                                .is_some_and(|rest| rest.starts_with('/'))
                    })
                    .cloned()
            })
    };
    for file in &facts.files {
        // The allowed reference: the one argument of a dialect `useContext`.
        let read_arguments: HashSet<(u32, u32)> = file
            .ast
            .calls
            .iter()
            .filter(|call| {
                call.arguments.len() == 1
                    && !call.arguments[0].spread
                    && crate::known_primitive(&crate::call_primitive_name(
                        file,
                        call,
                        entities,
                        symbol_names,
                        dialect,
                    )) == Some(solid_dialect::Primitive::UseContext)
            })
            .map(|call| {
                let span = file.ast.peel_ts_sugar_span(call.arguments[0].span);
                (span.start, span.end)
            })
            .collect();
        let references_of = |symbol: &SymbolId| {
            file.ast
                .identifiers
                .iter()
                .filter(|identifier| identifier.role == solid_facts::ast::IdentifierRole::Reference)
                .filter(|identifier| {
                    entities.get(&location(file.path.shared(), identifier.span)) == Some(symbol)
                })
                .map(|identifier| identifier.span)
                .collect::<Vec<_>>()
        };
        // A tag naming the binding, or a dotted tag whose object is it
        // (`<Ctx.Provider>`, a type error in Solid 2 but still a use).
        let jsx_names_of = |symbol: &SymbolId| {
            file.ast.jsx_elements.iter().any(|element| {
                entities.get(&location(file.path.shared(), element.name.span)) == Some(symbol)
                    || element.member_object.is_some_and(|object| {
                        entities.get(&location(file.path.shared(), object)) == Some(symbol)
                    })
            })
        };
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(package) = package_of(file, &import.module) else {
                continue;
            };
            let Some(names) = premises.get(&package) else {
                continue;
            };
            for binding in &import.bindings {
                if binding.type_only {
                    continue;
                }
                let binding_location = location(file.path.shared(), binding.local.span);
                let Some(symbol) = entities.get(&binding_location) else {
                    // A binding this analysis cannot name has references it
                    // cannot classify.
                    if binding.kind == solid_facts::ast::ImportKind::Namespace
                        || binding
                            .imported
                            .as_deref()
                            .is_some_and(|name| names.contains(name))
                    {
                        provided.extend(names.iter().map(|name| (package.clone(), name.clone())));
                    }
                    continue;
                };
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    let members: Vec<&solid_facts::ast::MemberFact> = file
                        .ast
                        .members
                        .iter()
                        .filter(|member| {
                            entities.get(&location(file.path.shared(), member.object))
                                == Some(symbol)
                        })
                        .collect();
                    // A dotted tag through the namespace. Its object is matched
                    // by symbol when the tag is `<ns.Name>`; any deeper or
                    // unresolved dotted tag spelled from the namespace's local
                    // name provides every premise, since the name alone cannot
                    // say which export it reaches.
                    let local = file.source_text(binding.local.span).unwrap_or_default();
                    for element in &file.ast.jsx_elements {
                        let direct = element.member_object.is_some_and(|object| {
                            entities.get(&location(file.path.shared(), object)) == Some(symbol)
                        });
                        let spelled = !local.is_empty()
                            && file.source_text(element.name.span).is_some_and(|name| {
                                name.strip_prefix(local)
                                    .is_some_and(|rest| rest.starts_with('.'))
                            });
                        if direct
                            && let Some(property) = element
                                .member_property
                                .and_then(|property| file.source_text(property))
                        {
                            if names.contains(property) {
                                provided.insert((package.clone(), property.to_owned()));
                            }
                        } else if direct || spelled {
                            provided
                                .extend(names.iter().map(|name| (package.clone(), name.clone())));
                        }
                    }
                    for reference in references_of(symbol) {
                        let Some(member) = members.iter().find(|member| member.object == reference)
                        else {
                            // The namespace object itself escapes.
                            provided
                                .extend(names.iter().map(|name| (package.clone(), name.clone())));
                            continue;
                        };
                        let computed = file
                            .ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_ok();
                        let property = file.source_text(member.property).unwrap_or_default();
                        if computed {
                            provided
                                .extend(names.iter().map(|name| (package.clone(), name.clone())));
                        } else if names.contains(property)
                            && !read_arguments.contains(&(member.span.start, member.span.end))
                        {
                            provided.insert((package.clone(), property.to_owned()));
                        }
                    }
                    continue;
                }
                let imported = binding.imported.as_deref().or_else(|| {
                    (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
                });
                let Some(imported) = imported.filter(|name| names.contains(*name)) else {
                    continue;
                };
                let escapes = jsx_names_of(symbol)
                    || references_of(symbol)
                        .into_iter()
                        .any(|span| !read_arguments.contains(&(span.start, span.end)));
                if escapes {
                    provided.insert((package.clone(), imported.to_owned()));
                }
            }
        }
        // Re-exports, CommonJS and dynamic loads of the package.
        for export in &file.ast.exports {
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            let Some(package) = package_of(file, module) else {
                continue;
            };
            let Some(names) = premises.get(&package) else {
                continue;
            };
            let star = export.specifiers.is_empty() || export.namespace.is_some();
            for name in names {
                if star
                    || export.specifiers.iter().any(|specifier| {
                        file.source_text(specifier.local.span) == Some(name.as_str())
                    })
                {
                    provided.insert((package.clone(), name.clone()));
                }
            }
        }
        let loaded = file
            .ast
            .import_equals
            .iter()
            .map(|fact| fact.module.as_str())
            .chain(
                file.ast
                    .module_loads
                    .iter()
                    .filter_map(|fact| fact.specifier.as_deref()),
            )
            .collect::<Vec<_>>();
        for module in loaded {
            if let Some(package) = package_of(file, module)
                && let Some(names) = premises.get(&package)
            {
                provided.extend(names.iter().map(|name| (package.clone(), name.clone())));
            }
        }
    }
    provided
}

/// The premises of `summary` this project provides, in order, or none.
fn unmet_context_premises(
    summary: &ContractExport,
    package: &str,
    provided: &BTreeSet<(String, String)>,
) -> Vec<String> {
    summary
        .context_premises
        .iter()
        .filter(|name| provided.contains(&(package.to_owned(), (*name).clone())))
        .cloned()
        .collect()
}

/// `summary` with every claim it states withdrawn: the reading a consumer
/// gives an export whose context premise this project does not meet.
fn premise_unmet_summary(summary: &ContractExport) -> ContractExport {
    ContractExport {
        reactive_reads: ContractClaim::Open,
        returns: ContractClaim::Open,
        callbacks: ContractClaim::Open,
        owner_requirements: ContractClaim::Open,
        async_behavior: ContractClaim::Open,
        open_claims: [
            ClaimDomain::Callbacks,
            ClaimDomain::Reads,
            ClaimDomain::Returns,
            ClaimDomain::Creates,
        ]
        .into_iter()
        .collect(),
        creates_closed_empty: false,
        returns_closed_empty: false,
        ..summary.clone()
    }
}

/// The finding for an import whose premise is unmet: uncertifiable, and an
/// error rather than ADR 0119's open-claims warning, because the contract's
/// claims are not partial here, they are unusable.
fn push_unmet_context_premise(
    missing_exports: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    reexported: bool,
    location: Location,
    unmet: &[String],
) {
    missing_exports.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: module.to_owned(),
            export: export.to_owned(),
            reexported,
            site: crate::ContractDefectSite::Import,
            admission_refusal: None,
        },
        location,
        analysis_context: format!("{CONTEXT_PREMISE_UNMET_CONTEXT}{}", unmet.join(",")),
        fixes: vec![],
        uncertain: false,
    });
}

/// The analysis context of [`push_unmet_context_premise`], followed by the
/// premise exports the project provides.
pub(crate) const CONTEXT_PREMISE_UNMET_CONTEXT: &str = "context-premise-unmet:";

pub(super) fn resolve_accepted_contract_imports(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> ResolvedContracts {
    let projected = project_accepted_contracts(facts, contracts);
    resolve_contract_imports_inner(
        facts,
        &projected,
        contracts,
        entities,
        symbol_names,
        dialect,
    )
}

fn project_accepted_contracts(
    facts: &ProjectFacts,
    contracts: &AcceptedContractIndex,
) -> HashMap<(String, String), PackageContract> {
    let mut projected = HashMap::new();
    for file in &facts.files {
        let modules = file
            .ast
            .imports
            .iter()
            .filter(|import| !import.type_only)
            .map(|import| import.module.as_str())
            .chain(
                file.ast
                    .exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .filter_map(|export| export.module.as_deref()),
            )
            .collect::<BTreeSet<_>>();
        for module in modules {
            let Ok(contract) = contracts.contract(file.path.as_str(), module) else {
                continue;
            };
            let artifact_case = contract.artifact_case();
            let exports = artifact_case
                .exports
                .keys()
                .filter_map(|name| {
                    contracts
                        .resolve_name(file.path.as_str(), module, name)
                        .ok()
                        .map(|accepted| (name.clone(), project_accepted_export(&accepted)))
                })
                .collect();
            projected.insert(
                (file.path.to_string(), module.to_owned()),
                PackageContract {
                    package: crate::ContractPackage {
                        name: contract.package().name.clone(),
                        version: contract.package().version.clone(),
                        integrity: contract.package().integrity.clone(),
                    },
                    entrypoints: BTreeMap::from([(
                        artifact_case.entrypoint.clone(),
                        crate::ContractEntrypoint { exports },
                    )]),
                    source_path: format!(
                        "accepted:{}",
                        contract.receipt().semantic_digest.as_str()
                    ),
                },
            );
        }
    }
    projected
}

fn resolve_contract_imports_inner(
    facts: &ProjectFacts,
    exact: &HashMap<(String, String), PackageContract>,
    accepted: &AcceptedContractIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> ResolvedContracts {
    let mut bindings = Vec::new();
    let mut by_symbol = HashMap::new();
    let mut missing_exports = Vec::new();
    let mut counts = crate::ContractBindingCounts::default();
    let returns_shed = returns_shed_symbols(facts, entities);
    let provided_contexts =
        provided_context_premises(facts, exact, accepted, entities, symbol_names, dialect);
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.type_only {
                continue;
            }
            let Some(contract) = exact.get(&(file.path.to_string(), import.module.to_string()))
            else {
                if let Some(reason) =
                    accepted.uncertifiable_reason(file.path.as_str(), &import.module)
                {
                    // Only the acceptance gate carries the note: an obsolete
                    // policy-1 receipt is its own, already specific answer.
                    let refusal = (reason == UncertifiableImportReason::Unspecified)
                        .then(|| accepted.admission_refusal_at(file.path.as_str(), &import.module))
                        .flatten();
                    push_missing_accepted_import(
                        &mut missing_exports,
                        file,
                        import,
                        entities,
                        dialect,
                        reason,
                        refusal,
                    );
                }
                continue;
            };
            counts.bound += 1;
            for binding in &import.bindings {
                if binding.type_only {
                    continue;
                }
                if binding.kind == solid_facts::ast::ImportKind::Namespace {
                    let namespace_location = location(file.path.shared(), binding.local.span);
                    let Some(namespace_symbol) = entities.get(&namespace_location) else {
                        continue;
                    };
                    for member in file.ast.members.iter().filter(|member| {
                        file.ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_err()
                            && entities.get(&location(file.path.shared(), member.object))
                                == Some(namespace_symbol)
                    }) {
                        let imported = file
                            .source_text(member.property)
                            .unwrap_or_default()
                            .to_owned();
                        let Some(summary) = contract
                            .exports_for_module(&import.module)
                            .and_then(|exports| exports.get(imported.as_str()))
                            .cloned()
                        else {
                            if missing_accepted_export_needs_obligation(
                                dialect,
                                &import.module,
                                &imported,
                            ) {
                                missing_exports.push(StaticDefect {
                                    kind: StaticDefectKind::PackageContractExportMissing {
                                        module: import.module.to_string(),
                                        export: imported,
                                        reexported: false,
                                        site: crate::ContractDefectSite::Import,
                                        admission_refusal: None,
                                    },
                                    location: location(file.path.shared(), member.property),
                                    analysis_context: String::new(),
                                    fixes: vec![],
                                    uncertain: false,
                                });
                            }
                            continue;
                        };
                        let member_location = location(file.path.shared(), member.property);
                        let Some(symbol) = entities.get(&member_location).cloned() else {
                            continue;
                        };
                        let unmet = unmet_context_premises(
                            &summary,
                            &contract.package.name,
                            &provided_contexts,
                        );
                        let summary = if unmet.is_empty() {
                            summary
                        } else {
                            push_unmet_context_premise(
                                &mut missing_exports,
                                &import.module,
                                &imported,
                                false,
                                member_location.clone(),
                                &unmet,
                            );
                            premise_unmet_summary(&summary)
                        };
                        if unmet.is_empty() && !summary.open_claims.is_empty() {
                            push_unknown_contract_claims(
                                &mut missing_exports,
                                &summary,
                                &import.module,
                                &imported,
                                false,
                                member_location.clone(),
                                !returns_shed.contains(&symbol),
                            );
                        }
                        let resolved = ResolvedContractBinding {
                            local_name: imported.clone(),
                            imported_name: imported.clone(),
                            package_name: contract.package.name.clone(),
                            symbol: symbol.clone(),
                            runtime_identity: runtime_identity_at(facts, &member_location),
                            contract_location: Location {
                                path: format!("{}#{imported}", contract.source_path).into(),
                                start_byte: 0,
                                end_byte: 0,
                            },
                            summary,
                        };
                        // External namespace bindings use the same exact
                        // accepted semantics as named imports.
                        bindings.push(resolved.clone());
                        by_symbol.insert(symbol, resolved);
                    }
                    continue;
                }
                let Some(imported) = binding.imported.as_deref().or_else(|| {
                    (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
                }) else {
                    continue;
                };
                let binding_location = location(file.path.shared(), binding.local.span);
                let Some(symbol) = entities.get(&binding_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract
                    .exports_for_module(&import.module)
                    .and_then(|exports| exports.get(imported))
                    .cloned()
                else {
                    let import_entity = facts.typescript.entities().find(|entity| {
                        entity.location.path == binding_location.path
                            && entity.location.start_byte == binding_location.start_byte
                            && entity.location.end_byte == binding_location.end_byte
                    });
                    let runtime_referenced = import_entity
                        .and_then(|entity| entity.reference_space)
                        .is_none_or(|space| {
                            matches!(space, ReferenceSpace::Value | ReferenceSpace::Both)
                        });
                    if !runtime_referenced {
                        // TypeScript reports no value-space reference for
                        // mixed imports such as `import { JSX, Portal }`.
                        // A type-only binding cannot consume runtime
                        // reactivity and therefore needs no export summary.
                        continue;
                    }
                    if missing_accepted_export_needs_obligation(dialect, &import.module, imported) {
                        missing_exports.push(StaticDefect {
                            kind: StaticDefectKind::PackageContractExportMissing {
                                module: import.module.to_string(),
                                export: imported.to_owned(),
                                reexported: false,
                                site: crate::ContractDefectSite::Import,
                                admission_refusal: None,
                            },
                            location: binding_location,
                            analysis_context: String::new(),
                            fixes: vec![],
                            uncertain: false,
                        });
                    }
                    continue;
                };
                let unmet =
                    unmet_context_premises(&summary, &contract.package.name, &provided_contexts);
                let summary = if unmet.is_empty() {
                    summary
                } else {
                    push_unmet_context_premise(
                        &mut missing_exports,
                        &import.module,
                        imported,
                        false,
                        binding_location.clone(),
                        &unmet,
                    );
                    premise_unmet_summary(&summary)
                };
                if unmet.is_empty() && !summary.open_claims.is_empty() {
                    push_unknown_contract_claims(
                        &mut missing_exports,
                        &summary,
                        &import.module,
                        imported,
                        false,
                        binding_location.clone(),
                        !returns_shed.contains(&symbol),
                    );
                }
                let resolved = ResolvedContractBinding {
                    local_name: file
                        .source_text(binding.local.span)
                        .unwrap_or_default()
                        .to_owned(),
                    imported_name: imported.into(),
                    package_name: contract.package.name.clone(),
                    symbol: symbol.clone(),
                    runtime_identity: runtime_identity_at(facts, &binding_location),
                    contract_location: Location {
                        path: format!("{}#{imported}", contract.source_path).into(),
                        start_byte: 0,
                        end_byte: 0,
                    },
                    summary,
                };
                // Only external package bindings enter this projection.
                bindings.push(resolved.clone());
                by_symbol.insert(symbol, resolved);
            }
        }
        for export in &file.ast.exports {
            if export.type_only {
                continue;
            }
            let Some(module) = export.module.as_deref() else {
                continue;
            };
            let Some(contract) = exact.get(&(file.path.to_string(), module.to_owned())) else {
                continue;
            };
            counts.bound += 1;
            for specifier in &export.specifiers {
                if specifier.type_only {
                    continue;
                }
                let imported = file.source_text(specifier.local.span).unwrap_or_default();
                let specifier_location = location(file.path.shared(), specifier.local.span);
                let Some(symbol) = entities.get(&specifier_location).cloned() else {
                    continue;
                };
                let Some(summary) = contract
                    .exports_for_module(module)
                    .and_then(|exports| exports.get(imported))
                    .cloned()
                else {
                    if missing_accepted_export_needs_obligation(dialect, module, imported) {
                        missing_exports.push(StaticDefect {
                            kind: StaticDefectKind::PackageContractExportMissing {
                                module: module.to_owned(),
                                export: imported.to_owned(),
                                reexported: true,
                                site: crate::ContractDefectSite::Import,
                                admission_refusal: None,
                            },
                            location: specifier_location,
                            analysis_context: String::new(),
                            fixes: vec![],
                            uncertain: false,
                        });
                    }
                    continue;
                };
                let unmet =
                    unmet_context_premises(&summary, &contract.package.name, &provided_contexts);
                let summary = if unmet.is_empty() {
                    summary
                } else {
                    push_unmet_context_premise(
                        &mut missing_exports,
                        module,
                        imported,
                        true,
                        specifier_location.clone(),
                        &unmet,
                    );
                    premise_unmet_summary(&summary)
                };
                if unmet.is_empty() && !summary.open_claims.is_empty() {
                    push_unknown_contract_claims(
                        &mut missing_exports,
                        &summary,
                        module,
                        imported,
                        true,
                        specifier_location.clone(),
                        !returns_shed.contains(&symbol),
                    );
                }
                let resolved = ResolvedContractBinding {
                    local_name: specifier.exported.to_string(),
                    imported_name: imported.to_owned(),
                    package_name: contract.package.name.clone(),
                    symbol: symbol.clone(),
                    runtime_identity: runtime_identity_at(facts, &specifier_location),
                    contract_location: Location {
                        path: format!("{}#{imported}", contract.source_path).into(),
                        start_byte: 0,
                        end_byte: 0,
                    },
                    summary,
                };
                bindings.push(resolved.clone());
                by_symbol.insert(symbol, resolved);
            }
        }
    }
    join_runtime_identity_aliases(
        facts,
        entities,
        &mut bindings,
        &mut by_symbol,
        &mut missing_exports,
    );
    ResolvedContracts {
        bindings,
        by_symbol,
        missing_exports,
        counts,
    }
}

fn push_missing_accepted_import(
    missing: &mut Vec<StaticDefect>,
    file: &solid_facts::FileFacts,
    import: &solid_facts::ast::ImportFact,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
    reason: UncertifiableImportReason,
    refusal: Option<&str>,
) {
    for binding in &import.bindings {
        if binding.type_only || !binding.runtime_referenced {
            continue;
        }
        if binding.kind == solid_facts::ast::ImportKind::Namespace {
            let namespace_location = location(file.path.shared(), binding.local.span);
            let Some(namespace_symbol) = entities.get(&namespace_location) else {
                continue;
            };
            for member in file.ast.members.iter().filter(|member| {
                file.ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                    && entities.get(&location(file.path.shared(), member.object))
                        == Some(namespace_symbol)
            }) {
                let export = file.source_text(member.property).unwrap_or_default();
                if !native_vocabulary_outranks_contract(dialect, &import.module, export) {
                    push_missing_accepted_export(
                        missing,
                        &import.module,
                        export,
                        location(file.path.shared(), member.property),
                        reason,
                        refusal,
                    );
                }
            }
            continue;
        }
        let Some(export) = binding.imported.as_deref().or_else(|| {
            (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
        }) else {
            continue;
        };
        if !native_vocabulary_outranks_contract(dialect, &import.module, export) {
            push_missing_accepted_export(
                missing,
                &import.module,
                export,
                location(file.path.shared(), import.span),
                reason,
                refusal,
            );
        }
    }
}

/// The analysis context of the **acceptance** gate: this project accepted no
/// contract for the import at all, so nothing export-specific has been read
/// yet.
///
/// Named rather than spelled twice because three places have to agree on it —
/// the producer below, the evidence wording, and the per-package collapse in
/// `projection` — and two of them are deciding whether a finding is about the
/// package or about one of its exports. A drift between them would silently
/// re-scatter the collapse or mislabel the evidence.
pub(crate) const UNACCEPTED_IMPORT_CONTEXT: &str =
    "no receipt-accepted contract matches this exact import";

fn push_missing_accepted_export(
    missing: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    location: Location,
    reason: UncertifiableImportReason,
    refusal: Option<&str>,
) {
    missing.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: module.into(),
            export: export.into(),
            reexported: false,
            site: crate::ContractDefectSite::Import,
            admission_refusal: refusal.map(str::to_owned),
        },
        location,
        analysis_context: match reason {
            UncertifiableImportReason::Unspecified => UNACCEPTED_IMPORT_CONTEXT,
            UncertifiableImportReason::ObsoletePolicy1 => {
                "obsolete-policy1-receipt: policy 1 cannot authorize analyzer semantics"
            }
        }
        .into(),
        fixes: vec![],
        uncertain: false,
    });
}

pub(super) struct ContractSemantics<'a> {
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
}

pub(super) struct ContractGraph<'a> {
    pub(super) nodes: &'a [SummaryNode],
    pub(super) nodes_by_path: &'a HashMap<String, Vec<usize>>,
    pub(super) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(super) entities: &'a EntitySymbols,
}

pub(super) struct ContractAnalysis<'a> {
    pub(super) summaries: &'a [SummaryReads],
    pub(super) returned: &'a [SummaryReads],
    pub(super) structured_returns: &'a [Option<ContractReturn>],
    pub(super) callbacks: &'a [Vec<ContractCallback>],
    /// Per node, the parameters the node calls itself, directly, in its own
    /// body (ADR 0100) -- see `InterproceduralGraphContribution`.
    pub(super) direct_callback_parameters: &'a [Vec<usize>],
    /// Per node, the parameters it reads a property of or coerces directly in
    /// its own body (`interproc::direct_protocol_parameters`).
    pub(super) direct_protocol_parameters:
        &'a [Vec<(crate::contract_semantics::InvokeProtocol, usize)>],
    /// Per node, the literal-keyed members of its own parameters it calls
    /// directly in its own body (item B of ways-to-improve § 3.3).
    pub(super) direct_member_callback_parameters: &'a [Vec<(usize, Vec<String>)>],
    /// Per node, the parameters whose caller-supplied value the analysis never
    /// accounted for. Any one of them makes this export's `callbacks` domain
    /// its callback domain open — see
    /// `interproc::push_unaccounted_parameter_escapes`.
    pub(super) escaped_parameters: &'a [Vec<usize>],
    pub(super) invoked_parameter_members: &'a [Vec<ParameterMemberInvocation>],
    pub(super) semantics: ContractSemantics<'a>,
}

/// One node's inputs to [`contract_export_function`], indexed out of a
/// [`ContractAnalysis`].
struct ContractExportNode<'a> {
    node: &'a SummaryNode,
    summary: &'a SummaryReads,
    returned_summary: &'a SummaryReads,
    structured_return: Option<&'a ContractReturn>,
    callbacks: &'a [ContractCallback],
    direct_callback_parameters: &'a [usize],
    direct_protocol_parameters: &'a [(crate::contract_semantics::InvokeProtocol, usize)],
    direct_member_callback_parameters: &'a [(usize, Vec<String>)],
    escaped_parameters: &'a [usize],
    invoked_parameter_members: &'a [ParameterMemberInvocation],
}

impl<'a> ContractExportNode<'a> {
    fn at(analysis: &ContractAnalysis<'a>, node: &'a SummaryNode, index: usize) -> Self {
        Self {
            node,
            summary: &analysis.summaries[index],
            returned_summary: &analysis.returned[index],
            structured_return: analysis.structured_returns[index].as_ref(),
            callbacks: &analysis.callbacks[index],
            direct_callback_parameters: &analysis.direct_callback_parameters[index],
            direct_protocol_parameters: &analysis.direct_protocol_parameters[index],
            direct_member_callback_parameters: &analysis.direct_member_callback_parameters[index],
            escaped_parameters: &analysis.escaped_parameters[index],
            invoked_parameter_members: &analysis.invoked_parameter_members[index],
        }
    }
}

fn contract_export_function(
    inputs: ContractExportNode<'_>,
    semantics: &ContractSemantics<'_>,
) -> ContractExport {
    let ContractExportNode {
        node,
        summary,
        returned_summary,
        structured_return,
        callbacks,
        direct_callback_parameters,
        direct_protocol_parameters,
        direct_member_callback_parameters,
        escaped_parameters,
        invoked_parameter_members,
    } = inputs;
    let mut seen_reactive_reads = HashSet::new();
    let mut reactive_reads = summary
        .iter()
        .filter_map(|read| {
            let reactive_read = ContractReactiveRead {
                kind: read.kind.clone().unwrap_or_else(|| "accessor".into()),
                label: read.display.to_string(),
                parameter: None,
                path: None,
                // Provenance is stated exactly when the read was discovered in
                // a *different* node and travelled here across a call edge.
                // A read the export performs itself carries none, and neither
                // does a row whose discovering node the summary could not
                // identify — `None` is the fail-closed value at both ends.
                composed_owner: read
                    .owner
                    .as_ref()
                    .filter(|owner| node.symbol.as_ref() != Some(*owner))
                    .map(|owner| owner.as_str().to_owned()),
                composed_from: None,
            };
            // The dedup key carries the provenance, so a read the export
            // performs itself and a read of the same `(kind, label)` it
            // performs through a call stay two rows. Collapsing them onto one
            // would publish a single claim that the export's own census has to
            // witness *and* a composed claim it cannot, and the stronger of
            // the two demands would silently disappear.
            seen_reactive_reads
                .insert((
                    reactive_read.kind.clone(),
                    reactive_read.label.clone(),
                    reactive_read.composed_owner.clone(),
                ))
                .then_some(reactive_read)
        })
        .collect::<Vec<_>>();
    // One row per parameter, carrying the access path only when every
    // contributing access agrees on it exactly. Paths are compared whole:
    // `props.of.values()` and `props.other.values()` are two different
    // accesses that a last-segment comparison would have collapsed into one
    // claim about `values`.
    let mut paths_by_parameter = BTreeMap::<usize, HashSet<&[String]>>::new();
    for ParameterMemberInvocation {
        parameter, path, ..
    } in invoked_parameter_members
    {
        paths_by_parameter
            .entry(*parameter)
            .or_default()
            .insert(path.as_slice());
    }
    for (parameter, paths) in paths_by_parameter {
        if seen_reactive_reads.insert(("parameter-member".into(), parameter.to_string(), None)) {
            reactive_reads.push(ContractReactiveRead {
                kind: "parameter-member".into(),
                label: String::new(),
                parameter: Some(parameter),
                path: (paths.len() == 1).then(|| paths.into_iter().next().unwrap().to_vec()),
                // A parameter-member read is rooted at *this* export's own
                // parameter, so it is never composed from another export's
                // row.
                composed_owner: None,
                composed_from: None,
            });
        }
    }
    let first_returned =
        returned_summary
            .iter()
            .fold(None::<&SummaryRead>, |current, candidate| match current {
                None => Some(candidate),
                Some(best) if location_order(&candidate.declaration, &best.declaration).is_lt() => {
                    Some(candidate)
                }
                Some(best) => Some(best),
            });
    let returns = structured_return.cloned().or_else(|| {
        first_returned.map(|read| ContractReturn {
            kind: if semantics.source_kinds.get(&read.symbol) == Some(&ReactiveSourceKind::Store) {
                "store-path".into()
            } else {
                "accessor".into()
            },
            label: read.display.to_string(),
            parameter: None,
            elements: Vec::new(),
            properties: BTreeMap::new(),
        })
    });
    let mut callback_summary = callbacks.to_vec();
    callback_summary.sort_by_key(|callback| callback.parameter);
    // An omitted `callbacks` list is a negative claim. Where a caller-supplied
    // parameter escaped without being accounted for, the honest list is not the
    // rows that were proven -- it is "unknown".
    let callbacks = if escaped_parameters.is_empty()
        && !callbacks_contradict_on_a_parameter(&callback_summary)
    {
        callback_summary.into()
    } else {
        ContractClaim::Open
    };
    ContractExport {
        kind: "function".into(),
        // ADR 0013: an access path alone does not establish the execution of
        // a nested callable. The compact model cannot express this uncertainty
        // beside known reads, so keep the whole domain open, never empty.
        reactive_reads: if invoked_parameter_members
            .iter()
            .all(|read| read.in_owner_body)
        {
            reactive_reads.into()
        } else {
            ContractClaim::Open
        },
        callbacks,
        owner_requirements: Vec::new().into(),
        returns: returns.into(),
        async_behavior: if node.r#async {
            String::from("promise").into()
        } else {
            String::new().into()
        },
        open_claims: BTreeSet::new(),
        // Neither field is decided here. `creates_closed_empty` describes an
        // *accepted dependency's* domain and only `project_accepted_export`
        // sets it; `creates_walk_clean` is attached at the emit boundary from
        // `Program::creates_proposal_walk`. Both defaults refuse.
        creates_closed_empty: false,
        returns_closed_empty: false,
        creates_walk_clean: false,
        // This summary *is* the local inference, so it is never inherited.
        inherited_from: None,
        // A locally inferred summary is stated unconditionally.
        context_premises: Vec::new(),
        // Attached at the emit boundary from `Program::merged_props_returns`,
        // beside the other two walk verdicts.
        merged_props_return: None,
        creates_walk_declines: Vec::new(),
        returns_walk_clean: false,
        returns_value_completion: false,
        returns_described_callables: Vec::new(),
        returns_reading_callables: Vec::new(),
        member_alias_initializer: false,
        member_alias_spelling: None,
        returns_argument_containers: Vec::new(),
        // ADR 0100: a proposal input read beside the rows. Kept whether or not
        // the callbacks domain above stayed known -- the generator's filter
        // reads both, and an open domain proposes nothing either way.
        direct_callback_parameters: direct_callback_parameters.iter().copied().collect(),
        direct_member_callback_parameters: direct_member_callback_parameters
            .iter()
            .cloned()
            .collect(),
        direct_accessor_parameters: direct_protocol_parameters
            .iter()
            .filter(|(protocol, _)| *protocol == crate::contract_semantics::InvokeProtocol::Get)
            .map(|(_, parameter)| *parameter)
            .collect(),
        direct_coerced_parameters: direct_protocol_parameters
            .iter()
            .filter(|(protocol, _)| *protocol == crate::contract_semantics::InvokeProtocol::Coerce)
            .map(|(_, parameter)| *parameter)
            .collect(),
        iterated_parameters: direct_protocol_parameters
            .iter()
            .filter(|(protocol, _)| *protocol == crate::contract_semantics::InvokeProtocol::Iterate)
            .map(|(_, parameter)| *parameter)
            .collect(),
        // A function node is not a construction; ADR 0139's walk is attached
        // to a class export at the emit boundary.
        result_access_parameters: BTreeSet::new(),
        // ADR 0152: a projection of an accepted contract only.
        returned_invocations: BTreeSet::new(),
    }
}

/// Whether two rows claim different executions for the same parameter.
///
/// One row is pushed per *invocation site*, and `push_contract_callback` dedups
/// only exactly equal rows, so a parameter invoked twice with two schedules
/// publishes both -- `@solid-primitives/range`'s `mapRange` carried
/// `callbacks[2]` as `deferred` and as `tracked` in the same summary. Schema v1
/// has one execution axis per parameter, and the runtime has one behavior, so
/// at least one of the two rows is false and a consumer choosing either is
/// guessing. The generator-local knowledge state stays open for that domain.
///
/// Rows that agree on `execution` and differ elsewhere (argument descriptors,
/// owner) are *not* contradictory: those are additional facts about the same
/// schedule, and collapsing them would discard proven claims.
///
/// **The sentinel is per export, and that is wider than the contradiction.** One
/// contradicted parameter discards the *other* parameters' undisputed rows too
/// (`fixtures/package-contracts/multi-role-callback-parameter`'s
/// `contradictOnZeroOnly` pins it). Schema v1 offers no narrower spelling: the
/// only granularity below `{"status": "unknown"}` is whether a row is present,
/// and an absent row is a certified *negative* — "never invokes a
/// caller-supplied callback there" (docs/package-contracts.md, the
/// "no callback execution row" review section). Dropping only the contradicted
/// parameter's rows would therefore replace one contradiction with one
/// affirmative false negative, and there is no encoding for "unknown at this
/// parameter, proven at that one". The pre-existing `escaped_parameters`
/// sentinel two lines up has exactly the same width for the same reason.
///
/// `callbacks` must already be sorted by parameter.
fn callbacks_contradict_on_a_parameter(callbacks: &[ContractCallback]) -> bool {
    // Per invoked value: the argument itself, or one member path of it (item
    // B of ways-to-improve § 3.3). A row for `handler` and a row for
    // `handler[0]` describe two different invocations and cannot contradict
    // each other. With no member row this is exactly the adjacent-pair check
    // over rows sorted by parameter it replaces.
    let mut executions = HashMap::<(usize, &[String]), &str>::new();
    callbacks.iter().any(|callback| {
        let execution = executions
            .entry((callback.parameter, callback.path.as_slice()))
            .or_insert(callback.execution.as_str());
        *execution != callback.execution.as_str()
    })
}

fn resolve_local_reexport(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    module: &str,
    name: &str,
) -> Option<(ContractExport, usize)> {
    let mut visiting = HashSet::new();
    resolve_local_reexport_with_visiting(
        facts,
        graph,
        by_symbol,
        source_file,
        module,
        name,
        &mut visiting,
    )
}

fn resolve_local_import(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    local_name: &str,
) -> Option<(ContractExport, usize)> {
    let mut visiting = HashSet::new();
    resolve_local_import_with_visiting(
        facts,
        graph,
        by_symbol,
        source_file,
        local_name,
        &mut visiting,
    )
}

fn resolve_local_binding_initializer(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    local: solid_facts::core::Span,
) -> Option<(ContractExport, usize)> {
    let local_symbol = graph.entities.get(&location(file.path.shared(), local));
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.names.iter().any(|name| {
            name.span == local
                || local_symbol.is_some_and(|symbol| {
                    graph.entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
                })
        })
    })?;
    let initializer = binding.initializer?;
    graph
        .entities
        .get(&location(file.path.shared(), initializer))
        .and_then(|symbol| {
            graph.by_symbol.get(symbol).and_then(|index| {
                by_symbol
                    .get(symbol)
                    .cloned()
                    .map(|summary| (summary, *index))
            })
        })
        .or_else(|| {
            file.source_text(initializer)
                .and_then(|name| resolve_local_import(facts, graph, by_symbol, file, name))
        })
}

fn resolve_local_import_with_visiting(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    local_name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    for import in source_file
        .ast
        .imports
        .iter()
        .filter(|import| !import.type_only)
    {
        for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
            if source_file.source_text(binding.local.span) != Some(local_name) {
                continue;
            }
            let imported_name = binding.imported.as_deref().or_else(|| {
                (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
            })?;
            if import.module.starts_with('.') {
                return resolve_local_reexport_with_visiting(
                    facts,
                    graph,
                    by_symbol,
                    source_file,
                    import.module.as_str(),
                    imported_name,
                    visiting,
                );
            }
        }
    }
    None
}

fn resolve_local_reexport_with_visiting(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    source_file: &solid_facts::FileFacts,
    module: &str,
    name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    let source_path = Path::new(source_file.path.as_str());
    let target_path = source_path.parent()?.join(module).canonicalize().ok()?;
    let target = facts.files.iter().find(|file| {
        Path::new(file.path.as_str()).canonicalize().ok().as_ref() == Some(&target_path)
    })?;
    resolve_named_export(facts, graph, by_symbol, target, name, visiting)
}

fn resolve_named_export(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
    file: &solid_facts::FileFacts,
    name: &str,
    visiting: &mut HashSet<(String, String)>,
) -> Option<(ContractExport, usize)> {
    if !visiting.insert((file.path.to_string(), name.to_owned())) {
        return None;
    }
    for index in graph
        .nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
    {
        let node = &graph.nodes[index];
        if node.exported
            && node.name.as_deref() == Some(name)
            && let Some(symbol) = &node.symbol
            && let Some(summary) = by_symbol.get(symbol)
        {
            return Some((summary.clone(), index));
        }
    }
    // `module_level_exports`, not `exports`: an `export` nested in a
    // `namespace` body publishes a member of the namespace object, which no
    // importer of this module can name. See `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only && specifier.exported.as_str() == name)
        {
            let local_name = file.source_text(specifier.local.span).unwrap_or(name);
            if let Some(module) = export.module.as_deref()
                && let Some(summary) = resolve_local_reexport_with_visiting(
                    facts, graph, by_symbol, file, module, local_name, visiting,
                )
            {
                return Some(summary);
            }
            if let Some(summary) = resolve_local_import_with_visiting(
                facts, graph, by_symbol, file, local_name, visiting,
            ) {
                return Some(summary);
            }
            if let Some(summary) = resolve_local_binding_initializer(
                facts,
                file,
                graph,
                by_symbol,
                specifier.local.span,
            ) {
                return Some(summary);
            }
            if let Some(resolved) = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .and_then(|symbol| {
                    graph.by_symbol.get(symbol).and_then(|index| {
                        by_symbol
                            .get(symbol)
                            .cloned()
                            .map(|summary| (summary, *index))
                    })
                })
            {
                return Some(resolved);
            }
        }
        if export.kind == solid_facts::ast::ExportKind::All
            && let Some(module) = export.module.as_deref()
            && let Some(summary) = resolve_local_reexport_with_visiting(
                facts, graph, by_symbol, file, module, name, visiting,
            )
        {
            return Some(summary);
        }
    }
    None
}

fn contract_export_fragment(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    project_directory: Option<&Path>,
    graph: &ContractGraph<'_>,
    node_keys: &[ContractNodeKey],
    node_contracts: &HashMap<ContractNodeKey, ContractExport>,
    by_symbol: &HashMap<SymbolId, ContractExport>,
) -> ContractExportFragment {
    let mut fragment = ContractExportFragment::default();
    if project_directory
        .is_some_and(|directory| !path_within_project(Path::new(file.path.as_str()), directory))
    {
        return fragment;
    }
    for index in graph
        .nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
    {
        let node = &graph.nodes[index];
        if node.exported
            && let (Some(name), Some(symbol)) = (&node.name, &node.symbol)
            && let Some(target) = graph.by_symbol.get(symbol).copied()
            && let Some(summary) = node_contracts.get(&node_keys[target])
        {
            fragment.dependencies.insert(node_keys[target].clone());
            fragment.direct.push((name.clone(), summary.clone()));
            fragment.owners.push((name.clone(), symbol.clone()));
        }
    }
    // `module_level_exports`, not `exports`: an `export` inside a `namespace`
    // body publishes a member of the namespace object, which no importer of
    // this module can name. See `AstFacts::module_level_exports`.
    for export in file
        .ast
        .module_level_exports()
        .filter(|export| !export.type_only)
    {
        for specifier in export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .filter(|specifier| !specifier.type_only)
        {
            let target = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .and_then(|symbol| graph.by_symbol.get(symbol))
                .copied();
            let summary = export
                .module
                .as_deref()
                .and_then(|module| {
                    let local_name = file
                        .source_text(specifier.local.span)
                        .unwrap_or(specifier.exported.as_str());
                    resolve_local_reexport(facts, graph, by_symbol, file, module, local_name)
                })
                .or_else(|| {
                    let local_name = file
                        .source_text(specifier.local.span)
                        .unwrap_or(specifier.exported.as_str());
                    resolve_local_import(facts, graph, by_symbol, file, local_name)
                })
                .or_else(|| {
                    resolve_local_binding_initializer(
                        facts,
                        file,
                        graph,
                        by_symbol,
                        specifier.local.span,
                    )
                })
                .map(|(summary, index)| {
                    fragment.dependencies.insert(node_keys[index].clone());
                    summary
                })
                .or_else(|| {
                    target.and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                })
                .unwrap_or_else(|| fallback_value_export(file, graph, specifier.local.span));
            let summary = promote_callable_export(facts, file, specifier.local.span, summary);
            if let Some(symbol) = graph
                .entities
                .get(&location(file.path.shared(), specifier.local.span))
                .filter(|symbol| graph.by_symbol.contains_key(*symbol))
            {
                fragment
                    .owners
                    .push((specifier.exported.to_string(), symbol.clone()));
            }
            fragment
                .syntax
                .push((specifier.exported.to_string(), summary, true));
        }
        for binding in file.ast.exported_bindings(export) {
            for name in &binding.names {
                let target = graph
                    .entities
                    .get(&location(file.path.shared(), name.span))
                    .and_then(|symbol| graph.by_symbol.get(symbol))
                    .copied();
                let summary = target
                    .and_then(|index| {
                        fragment.dependencies.insert(node_keys[index].clone());
                        node_contracts.get(&node_keys[index]).cloned()
                    })
                    .or_else(|| {
                        resolve_local_binding_initializer(facts, file, graph, by_symbol, name.span)
                            .map(|(summary, index)| {
                                fragment.dependencies.insert(node_keys[index].clone());
                                summary
                            })
                    })
                    .unwrap_or_else(|| fallback_value_export(file, graph, name.span));
                let summary = promote_callable_export(facts, file, name.span, summary);
                let exported = file.source_text(name.span).unwrap_or_default().to_owned();
                if let Some(symbol) = graph
                    .entities
                    .get(&location(file.path.shared(), name.span))
                    .filter(|symbol| graph.by_symbol.contains_key(*symbol))
                {
                    fragment.owners.push((exported.clone(), symbol.clone()));
                }
                fragment.syntax.push((exported, summary, false));
            }
        }
    }
    fragment
}

pub(super) fn contract_export_summaries_incremental(
    cache: &mut CachedContractExports,
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    reverse_edges: &[Vec<usize>],
    graph_node_reused_paths: &HashSet<&str>,
    changed_semantic_symbols: Option<&HashSet<SymbolId>>,
    analysis: &ContractAnalysis<'_>,
) -> Arc<BTreeMap<String, ContractExport>> {
    let mut ordinals = HashMap::<&str, usize>::new();
    let node_keys = graph
        .nodes
        .iter()
        .map(|node| {
            let ordinal = ordinals.entry(node.path.as_str()).or_default();
            let key = ContractNodeKey {
                path: node.path.clone(),
                ordinal: *ordinal,
            };
            *ordinal += 1;
            key
        })
        .collect::<Vec<_>>();
    let current_keys = node_keys.iter().cloned().collect::<HashSet<_>>();
    let mut dirty = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            (!graph_node_reused_paths.contains(node.path.as_str())
                || !cache.nodes.contains_key(&node_keys[index])
                || changed_semantic_symbols.is_some_and(|changed| {
                    analysis.summaries[index]
                        .iter()
                        .chain(analysis.returned[index].iter())
                        .any(|read| changed.contains(&read.symbol))
                }))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let mut dirty_set = dirty.iter().copied().collect::<HashSet<_>>();
    while let Some(target) = dirty.pop() {
        for owner in reverse_edges.get(target).into_iter().flatten().copied() {
            if dirty_set.insert(owner) {
                dirty.push(owner);
            }
        }
    }
    let mut dirty_indices = dirty_set.into_iter().collect::<Vec<_>>();
    dirty_indices.sort_unstable();
    let rebuilt_nodes = parallel_slice_results(&dirty_indices, |index| {
        contract_export_function(
            ContractExportNode::at(analysis, &graph.nodes[*index], *index),
            &analysis.semantics,
        )
    });
    let mut changed_nodes = HashSet::<ContractNodeKey>::new();
    for (index, contract) in dirty_indices.into_iter().zip(rebuilt_nodes) {
        let key = node_keys[index].clone();
        if cache.nodes.get(&key) != Some(&contract) {
            changed_nodes.insert(key.clone());
            cache.nodes.insert(key, contract);
        }
    }
    let removed_nodes = cache
        .nodes
        .keys()
        .filter(|key| !current_keys.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    for key in removed_nodes {
        cache.nodes.remove(&key);
        changed_nodes.insert(key);
    }
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    let removed_files = cache
        .files
        .keys()
        .filter(|path| !current_paths.contains(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut fragments_changed = !removed_files.is_empty();
    for path in removed_files {
        cache.files.remove(&path);
    }
    let project_directory = Path::new(&facts.project_id).parent();
    let by_symbol = graph
        .by_symbol
        .iter()
        .filter_map(|(symbol, index)| {
            cache
                .nodes
                .get(&node_keys[*index])
                .cloned()
                .map(|summary| (symbol.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let rebuild_files = facts
        .files
        .iter()
        .filter(|file| {
            !graph_node_reused_paths.contains(file.path.as_str())
                || cache
                    .files
                    .get(file.path.as_str())
                    .is_none_or(|fragment| !fragment.dependencies.is_disjoint(&changed_nodes))
        })
        .collect::<Vec<_>>();
    let rebuilt_fragments = parallel_slice_results(&rebuild_files, |file| {
        contract_export_fragment(
            facts,
            file,
            project_directory,
            graph,
            &node_keys,
            &cache.nodes,
            &by_symbol,
        )
    });
    for (file, fragment) in rebuild_files.into_iter().zip(rebuilt_fragments) {
        fragments_changed |= cache.files.get(file.path.as_str()) != Some(&fragment);
        cache.files.insert(file.path.to_string(), fragment);
    }
    if !fragments_changed && let Some(aggregate) = &cache.aggregate {
        return Arc::clone(aggregate);
    }
    let aggregate = aggregate_contract_fragments(facts, &cache.files);
    let aggregate = Arc::new(aggregate);
    cache.aggregate = Some(Arc::clone(&aggregate));
    aggregate
}

pub(super) fn contract_export_summaries(
    facts: &ProjectFacts,
    graph: &ContractGraph<'_>,
    analysis: &ContractAnalysis<'_>,
) -> BTreeMap<String, ContractExport> {
    let mut ordinals = HashMap::<&str, usize>::new();
    let node_keys = graph
        .nodes
        .iter()
        .map(|node| {
            let ordinal = ordinals.entry(node.path.as_str()).or_default();
            let key = ContractNodeKey {
                path: node.path.clone(),
                ordinal: *ordinal,
            };
            *ordinal += 1;
            key
        })
        .collect::<Vec<_>>();
    let node_contracts = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            (
                node_keys[index].clone(),
                contract_export_function(
                    ContractExportNode::at(analysis, node, index),
                    &analysis.semantics,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let by_symbol = graph
        .by_symbol
        .iter()
        .filter_map(|(symbol, index)| {
            node_contracts
                .get(&node_keys[*index])
                .cloned()
                .map(|summary| (symbol.clone(), summary))
        })
        .collect::<HashMap<_, _>>();
    let project_directory = Path::new(&facts.project_id).parent();
    let fragments = facts
        .files
        .iter()
        .map(|file| {
            (
                file.path.to_string(),
                contract_export_fragment(
                    facts,
                    file,
                    project_directory,
                    graph,
                    &node_keys,
                    &node_contracts,
                    &by_symbol,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    aggregate_contract_fragments(facts, &fragments)
}

fn aggregate_contract_fragments(
    facts: &ProjectFacts,
    fragments: &HashMap<String, ContractExportFragment>,
) -> BTreeMap<String, ContractExport> {
    let mut aggregate = BTreeMap::new();
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, summary) in &fragment.direct {
                aggregate.insert(name.clone(), summary.clone());
            }
        }
    }
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, summary, replace) in &fragment.syntax {
                if *replace {
                    aggregate.insert(name.clone(), summary.clone());
                } else {
                    aggregate
                        .entry(name.clone())
                        .or_insert_with(|| summary.clone());
                }
            }
        }
    }
    resolve_composed_reactive_reads(facts, fragments, &mut aggregate);
    aggregate
}

/// Resolve every row's unpublished `composed_owner` into a published
/// `composed_from`, and clear the unpublished half.
///
/// This is the only place in the pipeline that knows both halves of the
/// question: the per-node projection knows *which node* discovered a read but
/// not the name it is exported under, and each file's fragment knows its own
/// export bindings but not another file's. Aggregation has all of them.
///
/// Every step publishes nothing rather than approximating:
///
/// * an owner symbol no export of this project names — a private helper, a
///   node reached only through an unnameable re-export — stays unresolved,
///   because "some node in this package performs the read" is exactly the
///   claim the scoping study forbids;
/// * an owner exported under **more than one** name stays unresolved: the two
///   names are two claims, and picking one would name a target a call site may
///   not resolve to;
/// * an owner whose own read list carries no row with this row's identity
///   stays unresolved. The identity is `(kind, label)`, the same key the
///   projection deduplicates by, so at most one row can match — the ordinal it
///   is found at is what `normalize_export` names `read-<ordinal>`;
/// * a row whose owner is the export publishing it is not a composition at
///   all.
///
/// The resolution is a *nomination*, never authority. The certifier proves the
/// composing call's callee resolves to the named export through the compiler's
/// own authenticated export table, and proves the named export's own demand
/// for the named operation separately, so a provenance this pass got wrong
/// refuses rather than discharges.
fn resolve_composed_reactive_reads(
    facts: &ProjectFacts,
    fragments: &HashMap<String, ContractExportFragment>,
    aggregate: &mut BTreeMap<String, ContractExport>,
) {
    let mut names_by_owner = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file in &facts.files {
        if let Some(fragment) = fragments.get(file.path.as_str()) {
            for (name, symbol) in &fragment.owners {
                names_by_owner
                    .entry(symbol.as_str())
                    .or_default()
                    .insert(name.as_str());
            }
        }
    }
    // The read identities of every export, snapshotted before anything is
    // rewritten: an ordinal has to be read off the list as published, and the
    // rewrite below never reorders one.
    let identities = aggregate
        .iter()
        .filter_map(|(name, summary)| {
            summary.reactive_reads.known().map(|reads| {
                (
                    name.clone(),
                    reads
                        .iter()
                        .map(|read| (read.kind.clone(), read.label.clone()))
                        .collect::<Vec<_>>(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for (name, summary) in aggregate.iter_mut() {
        let ContractClaim::Known(reads) = &mut summary.reactive_reads else {
            continue;
        };
        for read in reads.iter_mut() {
            let Some(owner) = read.composed_owner.take() else {
                continue;
            };
            let Some(exports) = names_by_owner.get(owner.as_str()) else {
                continue;
            };
            let [export] = exports.iter().copied().collect::<Vec<_>>()[..] else {
                continue;
            };
            if export == name.as_str() {
                continue;
            }
            let Some(rows) = identities.get(export) else {
                continue;
            };
            let matched = rows
                .iter()
                .enumerate()
                .filter(|(_, (kind, label))| *kind == read.kind && *label == read.label)
                .map(|(ordinal, _)| ordinal)
                .collect::<Vec<_>>();
            let [ordinal] = matched[..] else {
                continue;
            };
            read.composed_from = Some(crate::ComposedReactiveRead {
                export: export.to_owned(),
                read: ordinal,
            });
        }
    }
}

fn path_within_project(path: &Path, directory: &Path) -> bool {
    path.starts_with(directory)
        || path.canonicalize().is_ok_and(|path| {
            directory
                .canonicalize()
                .is_ok_and(|directory| path.starts_with(directory))
        })
}

fn value_contract_export() -> ContractExport {
    ContractExport {
        kind: "value".into(),
        ..ContractExport::default()
    }
}

/// [`value_contract_export`] for the export at `local`, which no body and no
/// resolved binding summarized, marking it a member alias when its binding is
/// one (see [`ContractExport::member_alias_initializer`]): a `const` of one
/// identifier -- matched by the same symbol identity
/// `resolve_local_binding_initializer` uses, so `export { entries }` finds the
/// declaration it names -- initialized by exactly a non-computed member access.
/// A destructuring pattern names a property *of* the member's value, not the
/// value, and is never one.
fn fallback_value_export(
    file: &solid_facts::FileFacts,
    graph: &ContractGraph<'_>,
    local: solid_facts::core::Span,
) -> ContractExport {
    let mut summary = value_contract_export();
    let local_symbol = graph.entities.get(&location(file.path.shared(), local));
    let alias = file.ast.bindings.iter().find_map(|binding| {
        let initializer = binding.initializer?;
        (binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.iter().any(|name| {
                name.span == local
                    || local_symbol.is_some_and(|symbol| {
                        graph.entities.get(&location(file.path.shared(), name.span)) == Some(symbol)
                    })
            })
            && file
                .ast
                .members
                .iter()
                .any(|member| member.span == initializer)
            && file
                .ast
                .computed_members
                .binary_search(&initializer)
                .is_err())
        .then_some(initializer)
    });
    summary.member_alias_initializer = alias.is_some();
    summary.member_alias_spelling =
        alias.and_then(|initializer| file.source_text(initializer).map(str::to_owned));
    summary
}

fn entity_at<'a>(facts: &'a ProjectFacts, target: &Location) -> Option<&'a typefacts::EntityFact> {
    facts.typescript.entities().find(|entity| {
        entity.location.path == target.path
            && entity.location.start_byte == target.start_byte
            && entity.location.end_byte == target.end_byte
    })
}

/// The honest summary for an export *raised* to `kind: "function"`:
/// callbacks fail closed.
///
/// An omitted `callbacks` list is a *negative* claim — "invokes no
/// caller-supplied function" — and a consumer reads `new Store(onChange)`
/// through exactly the same contract path as `store(onChange)`, so publishing
/// that silence certifies an inertness the export can contradict. Every raise
/// reaches here with a summary whose `kind` was still `value`, which is
/// precisely the state in which no function body was summarized for it:
///
/// - a **class** never has one, because the generator summarizes function
///   declarations, not construct signatures, so nothing carries what a
///   constructor — the class's own, or the one it inherits through `extends` —
///   does with its arguments;
/// - a **callable** binding reaching a raise had no summary node either. Had
///   its body been analyzed, the summary would already say `kind: "function"`
///   and carry that analysis's claims, and no raise would happen. Leaving the
///   domains absent here certified "invokes no caller-supplied callback" for a
///   body this run never read.
///
/// The open callback domain is demand-sensitive at the consumer: constructing or calling
/// with no callable argument stays clean.
pub fn raised_function_export(mut summary: ContractExport) -> ContractExport {
    summary.kind = "function".into();
    summary.callbacks = ContractClaim::Open;
    summary
}

/// What this analysis can prove about an exported binding's runtime `kind`.
///
/// `kind` is a structural inference premise, and `validate_export` bars a
/// `kind: "value"` summary from carrying any function claim. A value summary
/// therefore demands a proof that the export is not a function, not merely
/// the absence of a proof that it is.
///
/// Two facts decide it, and only together. Neither
/// [`typefacts::Callability`] nor [`typefacts::Constructability`] answers "is
/// this a runtime function" alone: the type system reads a construct signature
/// as *not* a call signature, so every class answers `NonCallable`, while
/// `Constructable` says nothing about a plain function. See the producer's ADR
/// 0020.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportKindProof {
    /// The type has a call signature, belongs to the signature-less
    /// `Function` family, has a construct signature, or combines them.
    /// `typeof === "function"` at runtime either way, and a raise leaves
    /// `callbacks` unknown because no body was summarized — see
    /// [`raised_function_export`].
    ///
    /// A class lands here through `Constructable`, which is what retired the
    /// syntactic class search this decision used to run first. That search was
    /// defeated by exactly the shapes a published package contains:
    /// `@solidjs/web@2.0.0-rc.1`'s `ResponseEnvelope`
    /// (`const C = (() => { class C {…}; …; return C; })()`, whose initializer
    /// is a *call*) and `@tanstack/*-devtools`' `*DevtoolsCore`
    /// (`const C = pair[0]`, whose element type is a class declared in another
    /// package) have no class expression in the analyzed artifact to find. The
    /// type has the answer in both.
    Callable,
    /// The type has neither a call nor a construct signature: `NonCallable`
    /// **and** `NonConstructable`.
    ///
    /// This is the full negative. Signature-less `Function` supertypes no
    /// longer reach it: producer ADR 0021 gives them the distinct positive
    /// `UntypedCallable` answer. Broad non-callable supertypes such as
    /// `object`, `{}`, and `Record<string, unknown>` remain here because those
    /// declared types genuinely admit non-function values.
    NonCallable,
    /// One or both facts are present and closed nothing. `Unknown` is `any`,
    /// `unknown`, `never` or an error type; `Mixed` is a union holding both a
    /// signature-carrying and a signature-less constituent. The two aggregate
    /// *independently*, so `Mixed` on both is not a per-constituent proof
    /// either: `(() => void) | number | (new () => X)` answers `Mixed` twice
    /// and still holds a constituent that is neither. `typeof` is therefore
    /// not statically determined and neither `kind` is a claim this analysis
    /// can make.
    Unresolvable(Callability, Constructability),
    /// One or both facts are absent at this location.
    ///
    /// Not "undemanded": `demand_plan` requests both facts at *every* export
    /// specifier and every exported declaration name — the only spans this
    /// decision is ever asked about, pinned by an assertion in
    /// `solid_facts_backend::semantic_demands`' tests — so absence here is the
    /// producer finding no node to classify at a span this analysis did ask
    /// about. That is missing evidence, and missing evidence may not publish
    /// the maximal certified negative a `value` summary is.
    Unanswered,
}

/// The `kind` proof for the binding at `target`.
///
/// The whole rule, in order:
///
/// 1. `(Callable ∨ UntypedCallable) ∨ Constructable ⇒`
///    [`ExportKindProof::Callable`]. Either callability positive or a construct
///    signature is a function at runtime, and neither fact's own absence or
///    uncertainty can subtract from the other's positive. `UntypedCallable`
///    deliberately proves no readable signature or parameter facts.
/// 2. `NonCallable ∧ NonConstructable ⇒` [`ExportKindProof::NonCallable`].
///    Both closed negatives, and only both.
/// 3. Either fact absent `⇒` [`ExportKindProof::Unanswered`].
/// 4. Anything else — `Mixed` or `Unknown` on either side `⇒`
///    [`ExportKindProof::Unresolvable`].
///
/// The multi-hop class search that used to run before any type answer — an
/// alias-and-initializer symbol walk, a class-expression initializer fact, and
/// an assignment scan — is gone: `Constructable` subsumes it, and reaches the
/// IIFE-wrapped and cross-package shapes it could not. What remains of syntax
/// is [`class_declaration_name`], and it is not a proof about a value. It
/// picks which question the facts are answering, because for the class
/// declaration shapes — a declaration name, and an anonymous
/// `export default class {}`, whose recorded span is the class node — the
/// demanded span is not the export's *value*.
pub fn export_kind_proof(facts: &ProjectFacts, target: &Location) -> ExportKindProof {
    export_kind_proof_from_entity(facts, target, entity_at(facts, target))
}

/// Decides an export's runtime kind using an already indexed exact entity.
///
/// Contract generation asks this question for every public name in wide
/// barrels. Accepting the entity separately lets that caller retain one exact
/// location index without changing the proof rule or bypassing the class-name
/// addressing correction.
pub fn export_kind_proof_from_entity(
    facts: &ProjectFacts,
    target: &Location,
    entity: Option<&typefacts::EntityFact>,
) -> ExportKindProof {
    if class_declaration_name(facts, target) {
        return ExportKindProof::Callable;
    }
    let callability = entity.and_then(|entity| entity.callability);
    let constructability = entity.and_then(|entity| entity.constructability);
    let signature_proof = match (callability, constructability) {
        (Some(Callability::Callable | Callability::UntypedCallable), _)
        | (_, Some(Constructability::Constructable)) => ExportKindProof::Callable,
        (Some(Callability::NonCallable), Some(Constructability::NonConstructable)) => {
            ExportKindProof::NonCallable
        }
        (Some(callability), Some(constructability)) => {
            ExportKindProof::Unresolvable(callability, constructability)
        }
        _ => ExportKindProof::Unanswered,
    };
    // A closed census of the exact runtime binding corrects a declaration-
    // surface negative. Published packages commonly ship `index.js` beside
    // `index.d.ts`; the configured compiler may attach the declaration
    // module's non-callable answer to the runtime declarator even though the
    // exact runtime symbol is initialized and remains bound to a function.
    // Only the closed callable census corrects that contradiction. Open/mixed
    // censuses and a non-callable census beside a positive signature retain
    // the established fail-closed signature rule.
    if entity.and_then(|entity| entity.runtime_binding_kind) == Some(RuntimeBindingKind::Callable) {
        return ExportKindProof::Callable;
    }
    match signature_proof {
        ExportKindProof::Unresolvable(_, _) | ExportKindProof::Unanswered => {
            match entity.and_then(|entity| entity.runtime_binding_kind) {
                Some(RuntimeBindingKind::Callable) => ExportKindProof::Callable,
                Some(RuntimeBindingKind::NonCallable) => ExportKindProof::NonCallable,
                // A mixed or open write census is explicit evidence that no
                // closed runtime kind exists. Preserve the signature failure
                // so the caller refuses with its established diagnostic.
                Some(RuntimeBindingKind::Mixed | RuntimeBindingKind::Open) | None => {
                    signature_proof
                }
            }
        }
        _ => signature_proof,
    }
}

/// Whether `target` is exactly a class's binding name, or exactly an anonymous
/// class node, in the file that declares it.
///
/// **A span-addressing fact, not a class-ness proof.** Most export shapes carry
/// a span whose type *is* the exported value: an export specifier, a variable
/// declarator's name, a function declaration's name. A class declaration is the
/// exception, in two spellings, and neither has a specifier span to demand at
/// instead:
///
/// - `export class C {}` — the demanded span is the declaration's *name*, where
///   the compiler's type is the class's **instance** type. It honestly answers
///   `NonCallable` and `NonConstructable`, because an instance is neither.
/// - `export default class {}` — anonymous, so there is no name to record and
///   the export carries the `class …` node's own span. The facts there describe
///   the instance type for the same reason.
///
/// The producer pins that by test and its ADR 0020 says so outright: demand at
/// the export-specifier span, never at a declaration name. These are the shapes
/// where the two facts answer about a different value than the export's and must
/// not be read at all.
///
/// What is left over after that is not a heuristic. `class C {}` binds the
/// constructor by language definition — named or anonymous, `typeof` of a class
/// declaration is `"function"` and needs no type answer. Nor can a bundler
/// defeat it the way it defeated the retired search: a lowered class has no
/// class *declaration* left, so nothing reaches here and the facts — asked at a
/// declarator name, which types as the constructor — decide it correctly.
fn class_declaration_name(facts: &ProjectFacts, target: &Location) -> bool {
    let span = solid_facts::core::Span::new(
        u32::try_from(target.start_byte).unwrap_or(u32::MAX),
        u32::try_from(target.end_byte).unwrap_or(u32::MAX),
    );
    facts
        .files
        .iter()
        .any(|file| file.path.as_str() == target.path.as_ref() && file.ast.declares_class_at(span))
}

/// Raises a `value` summary to what the binding at `span` is proven to be.
///
/// This is the project-wide analysis map (`Program::contract_exports`), not a
/// published entrypoint: it cannot refuse, so an unprovable kind stays the
/// `value` default here and the emission path
/// (`promote_entry_callable` in the backend) is what refuses to publish it.
fn promote_callable_export(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    summary: ContractExport,
) -> ContractExport {
    if summary.kind != "value" {
        return summary;
    }
    let target = location(file.path.shared(), span);
    match export_kind_proof(facts, &target) {
        // Both raises carry `callbacks` unknown, and for the same reason: a
        // summary still saying `value` here is one no function body was
        // analyzed for. See `raised_function_export`.
        ExportKindProof::Callable => raised_function_export(summary),
        ExportKindProof::NonCallable
        | ExportKindProof::Unresolvable(_, _)
        | ExportKindProof::Unanswered => summary,
    }
}

#[cfg(test)]
mod native_obligation_tests {
    use super::missing_accepted_export_needs_obligation;
    use solid_dialect::Solid2;

    #[test]
    fn partial_accepted_contract_does_not_reopen_dialect_owned_primitives() {
        assert!(!missing_accepted_export_needs_obligation(
            &Solid2,
            "solid-js",
            "createSignal"
        ));
        assert!(missing_accepted_export_needs_obligation(
            &Solid2,
            "@solidjs/signals",
            "isEqual"
        ));
        assert!(missing_accepted_export_needs_obligation(
            &Solid2,
            "solid-js",
            "notAPrimitive"
        ));
        assert!(missing_accepted_export_needs_obligation(
            &Solid2,
            "some-other-package",
            "createSignal"
        ));
    }
}

#[cfg(test)]
mod callback_contradiction_tests {
    use super::{ContractCallback, callbacks_contradict_on_a_parameter};

    fn row(parameter: usize, execution: &str) -> ContractCallback {
        ContractCallback {
            parameter,
            execution: execution.into(),
            schedule: None,
            clears_tracking: false,
            arguments: Vec::new(),
            owner: None,
            protocol: crate::contract_semantics::InvokeProtocol::Call,
            path: Vec::new(),
        }
    }

    #[test]
    fn two_executions_for_one_parameter_are_contradictory() {
        // `@solid-primitives/range`'s `mapRange`: parameter 2 invoked in the
        // export body and again inside the accessor it returns.
        let rows = [row(2, "deferred"), row(2, "tracked")];
        assert!(callbacks_contradict_on_a_parameter(&rows));
        // `createDerivedSpring`: an inline site and a tracked site.
        let rows = [row(0, "inline"), row(0, "tracked")];
        assert!(callbacks_contradict_on_a_parameter(&rows));
        // Three rows, with the disagreeing pair not adjacent by execution: all
        // rows for one parameter are contiguous after the sort, so any pair of
        // distinct executions produces at least one differing neighbour.
        let rows = [row(0, "inline"), row(0, "inline"), row(0, "tracked")];
        assert!(callbacks_contradict_on_a_parameter(&rows));
    }

    #[test]
    fn agreeing_and_distinct_parameters_stay_known() {
        // Different parameters may of course differ.
        let rows = [row(0, "inline"), row(1, "tracked")];
        assert!(!callbacks_contradict_on_a_parameter(&rows));
        // Two invocation sites with the same schedule: `push_contract_callback`
        // dedups identical rows, and rows that agree on `execution` while
        // differing elsewhere are additional facts about one schedule, not a
        // contradiction.
        let mut accessor = row(0, "tracked");
        accessor.arguments = vec![None];
        let rows = [row(0, "tracked"), accessor];
        assert!(!callbacks_contradict_on_a_parameter(&rows));
        assert!(!callbacks_contradict_on_a_parameter(&[]));
        assert!(!callbacks_contradict_on_a_parameter(&[row(0, "inline")]));
    }
}

/// The `kind` decision table, every combination, against synthetic facts.
///
/// The two process tests in
/// rust/crates/solid-facts-backend/tests/contracts_process.rs pin what the
/// *generator* publishes end to end; these pin the decision itself over the
/// complete 6x5 product of what the two facts can say — each callability
/// answer, each constructability answer, and absence on either side —
/// including the combinations no fixture reaches.
#[cfg(test)]
mod export_kind_proof_tests {
    use super::{ExportKindProof, ProjectFacts, export_kind_proof, raised_function_export};
    use crate::{ContractClaim, ContractExport};
    use solid_facts::FileFacts;
    use solid_facts::TypeScriptTable;
    use solid_facts::ast;
    use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
    use solid_facts::core::{Generation, Span};
    use typefacts::{
        Callability, Constructability, EntityFact, Location, PrimitiveValueDomain,
        RuntimeBindingKind,
    };

    const PATH: &str = "artifact.ts";

    fn entity(
        span: Span,
        callability: Option<Callability>,
        constructability: Option<Constructability>,
    ) -> EntityFact {
        EntityFact {
            location: Location {
                path: PATH.into(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            },
            symbol: "".into(),
            symbol_unresolved: false,
            type_descriptor: None,
            resolved_call: None,
            callability,
            constructability,
            runtime_binding_kind: None,
            runtime_value_domain: None,
            primitive_value_domain: PrimitiveValueDomain::default(),
            primitive_literal_candidates: None,
            call_result_domain: None,
            constant_value: None,
            array_shape: None,
            tuple_shape: None,
            library_types: None,
            reference_space: None,
            runtime_identity: "".into(),
        }
    }

    /// The proof for the binding named `name` in `source`, with the two facts
    /// standing in for what Type Facts answered at that exact span.
    ///
    /// The span is the *declarator's* name, which is what an export
    /// declaration's specifier carries. No symbol is recorded and no syntax is
    /// consulted any more: the answer is the fact pair and nothing else, which
    /// is the property these tests exist to hold.
    fn proof(
        source: &str,
        name: &str,
        callability: Option<Callability>,
        constructability: Option<Constructability>,
    ) -> ExportKindProof {
        proof_with_binding(source, name, callability, constructability, None)
    }

    fn proof_with_binding(
        source: &str,
        name: &str,
        callability: Option<Callability>,
        constructability: Option<Constructability>,
        runtime_binding_kind: Option<RuntimeBindingKind>,
    ) -> ExportKindProof {
        let ast = ast::extract(PATH, source).unwrap();
        let start = u32::try_from(source.find(name).expect("name occurs in source")).unwrap();
        let span = Span::new(start, start + u32::try_from(name.len()).unwrap());
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: ast.source.hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: Vec::new(),
            untracked_regions: Vec::new(),
            discarded_regions: Vec::new(),
            ownership_regions: Vec::new(),
            callback_roles: Vec::new(),
            jsx_operations: Vec::new(),
        };
        let generation = Generation::new(1).unwrap();
        let file = FileFacts::new(generation, source, ast, compiler).unwrap();
        let location = Location {
            path: file.path.as_str().into(),
            start_byte: u64::from(span.start),
            end_byte: u64::from(span.end),
        };
        let facts = ProjectFacts {
            generation,
            project_id: "fixture".into(),
            files: vec![file],
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                "fixture",
                Vec::new(),
                vec![EntityFact {
                    runtime_binding_kind,
                    ..entity(span, callability, constructability)
                }],
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_symbol_redirects: Default::default(),
        };
        export_kind_proof(&facts, &location)
    }

    #[test]
    fn exact_runtime_binding_census_closes_only_signature_failures() {
        let source = "export const value = opaque();";
        assert_eq!(
            proof_with_binding(
                source,
                "value",
                Some(Callability::Unknown),
                Some(Constructability::Unknown),
                Some(RuntimeBindingKind::Callable),
            ),
            ExportKindProof::Callable
        );
        assert_eq!(
            proof_with_binding(
                source,
                "value",
                Some(Callability::Unknown),
                Some(Constructability::Unknown),
                Some(RuntimeBindingKind::NonCallable),
            ),
            ExportKindProof::NonCallable
        );
        for open in [RuntimeBindingKind::Mixed, RuntimeBindingKind::Open] {
            assert_eq!(
                proof_with_binding(
                    source,
                    "value",
                    Some(Callability::Unknown),
                    Some(Constructability::Unknown),
                    Some(open),
                ),
                ExportKindProof::Unresolvable(Callability::Unknown, Constructability::Unknown)
            );
        }
    }

    #[test]
    fn exact_runtime_binding_census_corrects_sibling_declaration_signature_conflicts() {
        let source = "export const published = () => {};";
        assert_eq!(
            proof_with_binding(
                source,
                "published",
                Some(Callability::NonCallable),
                Some(Constructability::NonConstructable),
                Some(RuntimeBindingKind::Callable),
            ),
            ExportKindProof::Callable
        );
    }

    const CALLABILITIES: [Option<Callability>; 6] = [
        Some(Callability::Callable),
        Some(Callability::UntypedCallable),
        Some(Callability::NonCallable),
        Some(Callability::Mixed),
        Some(Callability::Unknown),
        None,
    ];

    const CONSTRUCTABILITIES: [Option<Constructability>; 5] = [
        Some(Constructability::Constructable),
        Some(Constructability::NonConstructable),
        Some(Constructability::Mixed),
        Some(Constructability::Unknown),
        None,
    ];

    /// The whole rule, restated independently of the implementation: either
    /// positive wins, both closed negatives prove a value, any absence is
    /// missing evidence, and everything left is unresolvable.
    fn expected(
        callability: Option<Callability>,
        constructability: Option<Constructability>,
    ) -> ExportKindProof {
        if matches!(
            callability,
            Some(Callability::Callable | Callability::UntypedCallable)
        ) || constructability == Some(Constructability::Constructable)
        {
            return ExportKindProof::Callable;
        }
        match (callability, constructability) {
            (Some(Callability::NonCallable), Some(Constructability::NonConstructable)) => {
                ExportKindProof::NonCallable
            }
            (Some(callability), Some(constructability)) => {
                ExportKindProof::Unresolvable(callability, constructability)
            }
            _ => ExportKindProof::Unanswered,
        }
    }

    /// A class declaration's span is not the export's *value* — the compiler
    /// answers with the instance type there, which is honestly neither callable
    /// nor constructable. `export class C {}` has no specifier span to ask at
    /// instead, so this shape is decided by the declaration and the facts are
    /// not read. Two spans carry it: a declaration's name, and — for an
    /// anonymous `export default class {}`, which has no name — the class node
    /// itself.
    #[test]
    fn a_class_declaration_is_decided_before_the_facts_are_read() {
        for (source, name) in [
            ("export class Widget {}", "Widget"),
            ("export default class Widget {}", "Widget"),
            ("class Widget {} export { Widget as Widget };", "Widget"),
            // Anonymous: the export records the class node's span, so that is
            // the span the decision is asked about.
            ("export default class {}", "class {}"),
            (
                "export default class extends Base {}",
                "class extends Base {}",
            ),
        ] {
            for callability in CALLABILITIES {
                for constructability in CONSTRUCTABILITIES {
                    assert_eq!(
                        proof(source, name, callability, constructability),
                        ExportKindProof::Callable,
                        "{source}: {callability:?}, {constructability:?}"
                    );
                }
            }
        }
        // And nothing else reaches the gate: a declarator initialized with a
        // class expression, an IIFE, or a tuple element is decided by the
        // facts, because the span there types as the constructor.
        for source in [
            "const Widget = class {}; export { Widget };",
            "const Widget = (() => { class Inner {} return Inner; })(); export { Widget };",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::NonConstructable),
                ),
                ExportKindProof::NonCallable,
                "{source}"
            );
        }
    }

    #[test]
    fn every_fact_combination_decides_the_documented_way() {
        // A shape whose own syntax says nothing: the decision must come from
        // the fact pair alone.
        let source = "export const value = host.create();";
        for callability in CALLABILITIES {
            for constructability in CONSTRUCTABILITIES {
                assert_eq!(
                    proof(source, "value", callability, constructability),
                    expected(callability, constructability),
                    "callability {callability:?}, constructability {constructability:?}"
                );
            }
        }
        // Fifteen of the thirty decide anything, and the shape of that split
        // is the claim: fourteen functions, one value, eight unresolvable, seven
        // unanswered — counted so a rule change cannot widen the certified
        // side unnoticed.
        let mut function = 0;
        let mut value = 0;
        let mut unresolvable = 0;
        let mut unanswered = 0;
        for callability in CALLABILITIES {
            for constructability in CONSTRUCTABILITIES {
                match expected(callability, constructability) {
                    ExportKindProof::Callable => function += 1,
                    ExportKindProof::NonCallable => value += 1,
                    ExportKindProof::Unresolvable(_, _) => unresolvable += 1,
                    ExportKindProof::Unanswered => unanswered += 1,
                }
            }
        }
        assert_eq!((function, value, unresolvable, unanswered), (14, 1, 8, 7));
    }

    #[test]
    fn an_untyped_callable_proves_kind_without_a_signature_claim() {
        let source = "export const value = host.create();";
        assert_eq!(
            proof(
                source,
                "value",
                Some(Callability::UntypedCallable),
                Some(Constructability::NonConstructable),
            ),
            ExportKindProof::Callable
        );
    }

    #[test]
    fn a_class_is_proven_by_constructability_alone() {
        // Every class type truthfully answers `nonCallable`; the construct
        // signature is the whole proof. These are the shapes the retired
        // syntactic search used to have to recognize — and the last two are
        // the shapes it could not: an IIFE-wrapped class and a class reached
        // as a tuple element type have no class expression to find. None of
        // them is a class *declaration name*, so none reaches the one
        // remaining syntactic gate.
        for source in [
            "const Widget = class Named {};",
            "var Widget = class {};",
            "const Widget = (() => { class Inner {} return Inner; })(); export { Widget };",
            "const pair = build(); const Widget = pair[0]; export { Widget };",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::Constructable),
                ),
                ExportKindProof::Callable,
                "{source}"
            );
        }
    }

    #[test]
    fn a_class_expression_declarator_is_decided_by_the_facts_alone() {
        // The retired search turned on a class-expression initializer fact
        // (`BindingFact::initializer_class`, deleted with it) and an assignment
        // scan, so a reassigned class-expression binding used to answer
        // differently from a `const` one. The fact pair is the only input now:
        // whatever the type says at that span is the answer, for either
        // syntax.
        for source in [
            "var Widget = class {}; Widget = { notAFunction: true };",
            "var Widget = class {}; Widget.marker = true;",
            "const Widget = class {};",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::NonConstructable),
                ),
                ExportKindProof::NonCallable,
                "{source}"
            );
            assert_eq!(
                proof(
                    source,
                    "Widget",
                    Some(Callability::NonCallable),
                    Some(Constructability::Constructable),
                ),
                ExportKindProof::Callable,
                "{source}"
            );
        }
        // `const Widget = class {} as unknown` used to be pinned as `function`
        // by the retired `initializer_class` fact. It is a refusal now, and
        // that is the honest answer: an `unknown` assertion erases the class,
        // both facts report `Unknown` at the declarator, and nothing left in
        // the artifact proves the binding holds a constructor at runtime.
        assert_eq!(
            proof(
                "const Widget = class {} as unknown; export { Widget };",
                "Widget",
                Some(Callability::Unknown),
                Some(Constructability::Unknown),
            ),
            ExportKindProof::Unresolvable(Callability::Unknown, Constructability::Unknown),
        );
    }

    #[test]
    fn a_destructured_binding_is_decided_like_any_other() {
        // This is what the constructability fact discharged. An object pattern
        // binds a *member* and an array pattern an *element*, which the
        // retired syntactic search could not reason about at all, so
        // `nonCallable` was refused there rather than believed. The type
        // answers the pattern directly: `(class Named {}).name` is a string
        // and provably not a function; a static class member and a tuple
        // element whose type is a class are `Constructable`.
        for source in [
            "const { Inner } = Container;",
            "const [Inner] = pair;",
            "export const { Inner } = Container;",
        ] {
            assert_eq!(
                proof(
                    source,
                    "Inner",
                    Some(Callability::NonCallable),
                    Some(Constructability::NonConstructable),
                ),
                ExportKindProof::NonCallable,
                "{source}"
            );
            assert_eq!(
                proof(
                    source,
                    "Inner",
                    Some(Callability::NonCallable),
                    Some(Constructability::Constructable),
                ),
                ExportKindProof::Callable,
                "{source}"
            );
        }
    }

    #[test]
    fn absence_on_either_fact_is_unanswered_not_a_negative() {
        // `demand_plan` asks for both facts at every export specifier and
        // every exported declaration name, so absence at a span this decision
        // is asked about is the producer finding no node to classify. Half an
        // answer is not an answer: a present `nonCallable` beside an absent
        // constructability must not publish `value`.
        let source = "export const value = host.create();";
        assert_eq!(
            proof(source, "value", Some(Callability::NonCallable), None),
            ExportKindProof::Unanswered
        );
        assert_eq!(
            proof(
                source,
                "value",
                None,
                Some(Constructability::NonConstructable)
            ),
            ExportKindProof::Unanswered
        );
        assert_eq!(
            proof(source, "value", None, None),
            ExportKindProof::Unanswered
        );
        // A positive still decides across an absence: a call signature is a
        // call signature whether or not the other walk ran.
        assert_eq!(
            proof(source, "value", Some(Callability::Callable), None),
            ExportKindProof::Callable
        );
        assert_eq!(
            proof(source, "value", None, Some(Constructability::Constructable)),
            ExportKindProof::Callable
        );
    }

    #[test]
    fn mixed_does_not_compose_across_the_two_facts() {
        // The producer aggregates the two independently, so `Mixed` twice is
        // not a per-constituent proof:
        // `(() => void) | number | (new () => X)` answers exactly this and
        // still holds a constituent that is neither callable nor
        // constructable.
        let source = "export const value = host.create();";
        assert_eq!(
            proof(
                source,
                "value",
                Some(Callability::Mixed),
                Some(Constructability::Mixed)
            ),
            ExportKindProof::Unresolvable(Callability::Mixed, Constructability::Mixed)
        );
        // `any`, `unknown`, `never` or an error type on both sides. No
        // `typeof` follows.
        assert_eq!(
            proof(
                source,
                "value",
                Some(Callability::Unknown),
                Some(Constructability::Unknown)
            ),
            ExportKindProof::Unresolvable(Callability::Unknown, Constructability::Unknown)
        );
    }

    #[test]
    fn every_raise_fails_closed_on_callbacks() {
        // The asymmetry this replaced: the class raise marked callbacks
        // unknown and the callable raise published silence, which is the
        // negative claim "invokes no caller-supplied callback" about a body
        // that was never analyzed.
        let raised = raised_function_export(ContractExport {
            kind: "value".into(),
            ..ContractExport::default()
        });
        assert_eq!(raised.kind, "function");
        assert!(matches!(raised.callbacks, ContractClaim::Open));
    }

    #[test]
    fn shape_may_be_callable_keeps_unproven_callability_open() {
        // Guards the value-export call-path closing in `project_accepted_export`:
        // only a *proven* non-callable shape may have its open call-path domains
        // closed to empty. A shape whose callability is not proven must return
        // true here so its domains stay open and it fails closed -- closing them
        // would manufacture "invokes no callback" from missing knowledge.
        use super::shape_may_be_callable;
        use crate::contract_semantics::{KnowledgeSet, ValueShape};

        // Callable / possibly-callable shapes: keep open.
        assert!(shape_may_be_callable(&ValueShape::Callable));
        assert!(shape_may_be_callable(&ValueShape::Component));
        assert!(shape_may_be_callable(&ValueShape::Unknown));
        // A union with a callable member is possibly callable.
        assert!(shape_may_be_callable(&ValueShape::Choice(
            KnowledgeSet::Complete(vec![ValueShape::Plain, ValueShape::Callable,])
        )));
        // A union whose membership is not exhaustively known could hide a
        // callable member.
        assert!(shape_may_be_callable(&ValueShape::Choice(
            KnowledgeSet::Partial(vec![ValueShape::Plain,])
        )));
        assert!(shape_may_be_callable(&ValueShape::Choice(
            KnowledgeSet::Unknown
        )));

        // Proven non-callable value shapes: closable.
        assert!(!shape_may_be_callable(&ValueShape::Plain));
        assert!(!shape_may_be_callable(&ValueShape::Object(
            KnowledgeSet::Unknown
        )));
        assert!(!shape_may_be_callable(&ValueShape::Choice(
            KnowledgeSet::Complete(vec![
                ValueShape::Plain,
                ValueShape::Object(KnowledgeSet::Unknown),
            ])
        )));
    }
}
