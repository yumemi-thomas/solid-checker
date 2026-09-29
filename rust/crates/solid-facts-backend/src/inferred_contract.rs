//! Projection of the analyzer's existing inference result into the normalized
//! proposal model.
//!
//! `PackageContract` remains an internal inference accumulator while the rule
//! engine is migrated. No legacy document is decoded or emitted here: exact
//! artifact identity arrives independently and the only output is normalized
//! semantics suitable for the stable package-contract encoder.

use std::collections::BTreeSet;

use crate::ClosureHazardKind;

use solid_reactive_ir::{
    CallbackSchedule, ContractCallback, ContractClaim, ContractExport, ContractOwnerRequirement,
    ContractReturn, OwnerRequirementOperation, PackageContract,
    contract_semantics::{
        ArrayLength, ArtifactCase, CallClaims, CallSemantics, CallbackInvocation,
        CapabilityKnowledge, Cardinality, CardinalityScope, ClaimDomain, ClaimPath, ComposedFrom,
        ContractProposal, Event, ExportIdentity, ExportSemantics, ExportTargetIdentity,
        GuardPartition, KnowledgeSet, Lifetime, NormalizedContract, ObjectProperty, Operation,
        OperationId, OperationKind, OwnerCapabilities, OwnerProduction, OwnerRelation,
        OwnerRequirements, OwnerSource, ReactiveRole, Requirement, Resource, ResourceId,
        ResourceKind, ResourceState, Schedule, SemanticClaimPath, SemanticClaimSubject,
        StabilityKnowledge, Tracking, Trigger, UpperBound, ValueShape, ValueSource,
    },
};

use crate::{
    artifact_resolution::{
        ResolvedImport, proposal_identity, select_and_bind, select_and_bind_with_external_targets,
    },
    contract_interface::ContractFailure,
};

/// One normalization's outputs: the proposal, the closure candidates it may
/// propose but not finalize, and the claims it refused to publish by name.
pub(crate) struct NormalizedInference {
    pub(crate) contract: NormalizedContract,
    pub(crate) closure_candidates: Vec<SemanticClaimSubject>,
    pub(crate) withheld: Vec<WithheldOwnerRequirementRecord>,
    /// Why no `creates` closure was proposed, per export. Measurement only.
    pub(crate) declined: Vec<DeclinedClosureRecord>,
    /// Which proposed closures rest on a dependency's contract rather than on
    /// a walk of this archive. Measurement only.
    pub(crate) inherited: Vec<InheritedClosureRecord>,
}

pub(crate) fn normalize_inferred_contract(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
) -> Result<NormalizedContract, ContractFailure> {
    normalize_inferred_contract_with_candidates(inferred, resolved)
        .map(|normalized| normalized.contract)
}

pub(crate) fn normalize_inferred_contract_with_candidates(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
) -> Result<NormalizedInference, ContractFailure> {
    normalize_inferred_contract_with_candidates_and_external_targets(
        inferred,
        resolved,
        &BTreeSet::new(),
    )
}

pub(crate) fn normalize_inferred_contract_with_candidates_and_external_targets(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<NormalizedInference, ContractFailure> {
    let (selected, withheld, declined) =
        normalize_inferred_contract_identity(inferred, resolved, external_targets)?;
    let package = selected.package().clone();
    let mut cases = selected.artifact_cases().to_vec();
    let mut candidates = Vec::new();
    let mut inherited_records = Vec::new();
    for artifact_case in &mut cases {
        for (name, export) in &mut artifact_case.exports {
            // The summary this export was normalized from, when the entrypoint
            // under generation declares it. Two questions are asked of it and
            // they are not the same question: `direct_callback_parameters` is a
            // *local* walk's output, and `inherited_from` says the summary is
            // not a local walk's output at all.
            let summary = inferred
                .entrypoints
                .get(&artifact_case.entrypoint)
                .and_then(|entrypoint| entrypoint.exports.get(name));
            // A projected re-export. Its closure is the dependency's, proved
            // by the dependency's own certification, and none of the local
            // confirmability filters below can say anything about it: they all
            // read walks over an implementation this archive does not contain,
            // and all of them therefore answer "do not propose". Running them
            // is what dropped `@solid-primitives/utils`'s certified `access`
            // and `mergeRefs` closures on the floor
            // (`phase21/2026-09-15-closure-gap-plan.md` § 1).
            //
            // The *hazard* filters below are a different kind and still apply:
            // a hazard is a fact about this package's own module closure, which
            // an inherited claim does not answer.
            let inherited = summary.is_some_and(|summary| summary.inherited_from.is_some());
            let paths = export.open_proposed_closure();
            // The weakening alone *loses* the candidate. The certifier rebuilds
            // its candidate universe by weakening the emitted document's own
            // closed claims, and the canonical main its receipt binds is that
            // same document, so a closure withdrawn here is a closure nothing
            // downstream can plan, prove, or bind — which is why
            // `census_creates_domain` had never run on generated input.
            //
            // So every candidate the certifier has a census for is republished
            // as a *proposed* closure: stated in the document, and labelled as
            // this generator's inference rather than a reviewed claim. The rest
            // stay withdrawn and travel to the plan sidecar below as
            // measurement only — publishing a closure no census can decide
            // would refuse the row instead of proving anything.
            //
            // The returns census decides empty completion (ADR 0035), a single
            // whole-parameter identity, one merged props root (ADR 0109), one
            // plain return over a primitive completion (ADR 0113) and returns
            // of argument containers (ADR 0115) and invocation results
            // (ADR 0116). Other described return shapes remain partial; their
            // enumeration has no complete census.
            let proposable = paths
                .iter()
                .filter_map(|path| match path {
                    ClaimPath::Call(domain) if domain.is_proposable() => Some(*domain),
                    _ => None,
                })
                // A `reads` closure over a module that installs a property
                // accessor at run time is a claim the census structurally
                // cannot refuse: TypeScript types a `Proxy` as its target and
                // a run-time descriptor changes no declared type, so the read
                // records no form. The hazard is the only place the premise is
                // still visible, and it is a fact about the *closure*, so it
                // withdraws the proposal — see
                // `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`
                // § 6-§ 9 — unless the export bounds every such site (ADR 0153
                // item C), which the certifier's census then has to confirm
                // site by site.
                .filter(|domain| {
                    *domain != ClaimDomain::Reads
                        || accessor_hazards_bounded(export.call.accessor_bounds(), resolved)
                })
                // The callbacks census decides the empty enumeration by the
                // call walk dispositioning no caller-supplied invocation, and
                // (ADR 0100) a described one whose every item the same walk
                // confirms: `from` a bare parameter `at` the call event on the
                // same stack — the `inline` row the interprocedural pass
                // writes for a call written directly in the export's body. A
                // `deferred` or `tracked` item, or one rooted at a member,
                // carries a timing the walk does not derive, so proposing it
                // would publish a closure the census must refuse — such an
                // enumeration stays partial.
                .filter(|domain| {
                    *domain != ClaimDomain::Callbacks
                        || inherited
                        || callbacks_enumeration_is_confirmable(export, summary)
                })
                // ADR 0101: a described `reads` enumeration is proposable when
                // every item is the generator's own `parameter-member` row -- a
                // member invocation on a caller parameter at the call event, in
                // this export's own body -- which the census confirms site for
                // site against the transcript's member invocations. A read of a
                // source the export owns, a composed row, or a deferred, tracked
                // or guarded one is a claim the census cannot confirm, so
                // proposing it would publish a closure the census must refuse;
                // such an enumeration stays partial.
                .filter(|domain| {
                    *domain != ClaimDomain::Reads
                        || inherited
                        || reads_enumeration_is_confirmable(export)
                })
                // ADR 0109: a single merged-props return is proposable — the
                // census decides it against the producer's control-flow and
                // call censuses, exactly as it decides the whole-parameter
                // identity beside it. ADR 0113 adds a single `plain` return,
                // which the census decides from the producer's types for the
                // completion and for every return site.
                .filter(|domain| {
                    *domain != ClaimDomain::Returns
                        || inherited
                        || export.operation_claim(ClaimDomain::Returns).is_some_and(|claim| {
                            matches!(claim.items(), [id] if export.operation(&id.0).is_some_and(|operation| {
                                operation.kind == OperationKind::Return
                                    && matches!(
                                        &operation.output,
                                        Some(ValueShape::MergedProps { .. } | ValueShape::Plain)
                                    )
                            }))
                        })
                        || export
                            .operation_claim(
                                ClaimDomain::Returns,
                            )
                            .is_some_and(|claim| {
                                claim.items().is_empty()
                                    || matches!(claim.items(), [id] if export.operation(&id.0).is_some_and(|operation| {
                                        operation.kind == OperationKind::Return
                                            && matches!(&operation.output, Some(ValueShape::Parameter { path, .. }) if path.is_empty())
                                    }))
                            })
                        // The second 2026-09-24 amendment to ADR 0103: the
                        // one array of primitives `Object.keys` hands back,
                        // which the alias census decides from the member's
                        // reviewed row.
                        || export
                            .operation_claim(ClaimDomain::Returns)
                            .is_some_and(|claim| {
                                matches!(claim.items(), [id] if export.operation(&id.0).is_some_and(|operation| {
                                    operation.kind == OperationKind::Return
                                        && matches!(&operation.output, Some(ValueShape::Array { element, .. }) if **element == ValueShape::Plain)
                                }))
                            })
                        // ADR 0145: returns that each hand back a described
                        // callable, which the census decides from the
                        // producer's `callable` fact and the literal's own
                        // transcript.
                        || export
                            .operation_claim(ClaimDomain::Returns)
                            .is_some_and(|claim| {
                                !claim.items().is_empty()
                                    && claim.items().iter().all(|id| {
                                        export.operation(&id.0).is_some_and(|operation| {
                                            operation.kind == OperationKind::Return
                                                && matches!(
                                                    operation.output,
                                                    Some(ValueShape::DescribedCallable(_))
                                                )
                                        })
                                    })
                            })
                        // ADR 0115: returns that each hand back an argument
                        // container, which the census decides from the
                        // producer's arms of every return -- since ADR 0116
                        // also one array or one invocation result alone.
                        || export
                            .operation_claim(ClaimDomain::Returns)
                            .is_some_and(|claim| {
                                !claim.items().is_empty()
                                    && claim.items().iter().all(|id| {
                                        export.operation(&id.0).is_some_and(|operation| {
                                            operation.kind == OperationKind::Return
                                                && operation
                                                    .output
                                                    .as_ref()
                                                    .and_then(solid_reactive_ir::ArgumentContainer::of)
                                                    .is_some()
                                        })
                                    })
                            })
                })
                .collect::<Vec<_>>();
            if let Some(origin) = summary.and_then(|summary| summary.inherited_from.as_ref()) {
                inherited_records.extend(proposable.iter().map(|domain| InheritedClosureRecord {
                    export: name.clone(),
                    domain: domain.wire_name(),
                    origin: origin.clone(),
                }));
            }
            // A bound conditions a closed `reads`; one the proposal does not
            // close leaves it nothing to bound.
            if !proposable.contains(&ClaimDomain::Reads) {
                export.open_call_domains([ClaimDomain::Reads]);
            }
            export.propose_closures(proposable);
            candidates.extend(paths.into_iter().map(|path| SemanticClaimSubject {
                artifact_case: artifact_case.id.clone(),
                export: name.clone(),
                path: SemanticClaimPath::Domain(path),
            }));
        }
    }
    let contract = ContractProposal::new(package, cases)
        .normalize()
        .map_err(model_failure)?;
    Ok(NormalizedInference {
        contract,
        closure_candidates: candidates,
        withheld,
        declined,
        inherited: inherited_records,
    })
}

fn normalize_inferred_contract_identity(
    inferred: &PackageContract,
    resolved: &ResolvedImport,
    external_targets: &BTreeSet<(String, String)>,
) -> Result<
    (
        NormalizedContract,
        Vec<WithheldOwnerRequirementRecord>,
        Vec<DeclinedClosureRecord>,
    ),
    ContractFailure,
> {
    let entrypoint = inferred
        .entrypoints
        .get(&resolved.requested_entrypoint)
        .ok_or_else(|| ContractFailure::InvalidSemanticModel {
            reason: format!(
                "inference has no entrypoint {:?}",
                resolved.requested_entrypoint
            ),
        })?;
    let (package, mut artifact_case) = proposal_identity(resolved)?;
    // See [`GenerationScope`]: the archive under generation being one of the
    // dialects' own primitive-defining packages is decided once, from the
    // resolved package name, and applies to every export of the case.
    let scope = GenerationScope::for_package(&resolved.package_name);
    let mut withheld = Vec::new();
    let mut declined = Vec::new();
    // ADR 0153 item C: the closure's accessor-installation hazard sites, which
    // an export whose `reads` the inference closes proposes to be bounded
    // against. Only the certifier's census can confirm a bound, site by site;
    // one it refuses withholds the export's `reads` like any other census
    // refusal.
    let accessor_sources = resolved
        .closure
        .hazards
        .iter()
        .filter(|hazard| hazard.kind == ClosureHazardKind::RuntimeAccessorInstallation)
        .map(|hazard| hazard.source.clone())
        .collect::<BTreeSet<_>>();
    for (name, summary) in &entrypoint.exports {
        let mut export = normalize_export(
            &artifact_case,
            name,
            summary,
            scope,
            &mut withheld,
            &mut declined,
        )?;
        if !accessor_sources.is_empty() && !export.call.claim_state(ClaimDomain::Reads).is_open() {
            export.add_accessor_bounds(accessor_sources.iter().cloned());
        }
        artifact_case.exports.insert(name.clone(), export);
    }
    let normalized = ContractProposal::new(package, vec![artifact_case])
        .normalize()
        .map_err(model_failure)?;
    let selected = if external_targets.is_empty() {
        select_and_bind(&normalized, resolved)?
    } else {
        select_and_bind_with_external_targets(&normalized, resolved, external_targets)?
    };
    declined.extend(hazard_declines(&selected, resolved));
    Ok((selected, withheld, declined))
}

/// Whether an export bounds every accessor-installation hazard of the closure
/// (ADR 0153 item C), so that binding left its `reads` closed.
fn accessor_hazards_bounded(bounds: &BTreeSet<String>, resolved: &ResolvedImport) -> bool {
    resolved
        .closure
        .hazards
        .iter()
        .filter(|hazard| hazard.kind == ClosureHazardKind::RuntimeAccessorInstallation)
        .all(|hazard| bounds.contains(&hazard.source))
}

/// Why a closure hazard left a proposable domain open, per export.
///
/// `bind_exports` opens every domain the closure's hazards name
/// (`artifact_resolution`'s `open_domains`), and until this existed that was
/// the end of it: the export carried an open domain and no record anywhere
/// said which hazard opened it. A domain lost this way is indistinguishable
/// in the emitted material from one the walk declined, from one no census can
/// decide, and from one there was simply nothing to say about — and the three
/// need completely different work.
///
/// **Proposable domains only.** A hazard opening `writes` explains nothing:
/// `writes` has no census, so it would be open whatever the closure looked
/// like, and recording the hazard as its blocker would name a cause that is
/// not one. `creates`, `returns` and `reads` are the domains where a closure
/// *could* have been proposed, so they are the only ones where "why was it
/// not" has an answer.
///
/// One record per (export, domain, hazard): a hazard whose `affected_exports`
/// is empty is a fact about every export of the case, and the record is
/// per-export because that is the question a reader asks. Three unaccepted
/// dependencies across seven exports is therefore sixty-three rows for three
/// facts — the cost of each row standing alone.
fn hazard_declines(
    selected: &NormalizedContract,
    resolved: &ResolvedImport,
) -> Vec<DeclinedClosureRecord> {
    let mut records = Vec::new();
    for artifact_case in selected.artifact_cases() {
        for (name, export) in &artifact_case.exports {
            for (domain, hazard) in resolved.closure.domain_openings(name) {
                if !domain.is_proposable() {
                    continue;
                }
                // A bounded site opened nothing (ADR 0153 item C); whether the
                // bound holds is the certifier's census to say.
                if hazard.kind == ClosureHazardKind::RuntimeAccessorInstallation
                    && export.call.accessor_bounds().contains(&hazard.source)
                {
                    continue;
                }
                records.push(DeclinedClosureRecord {
                    export: name.clone(),
                    domain: domain.wire_name(),
                    decline: ClosureDecline::Hazard {
                        kind: hazard.kind,
                        source: hazard.source.clone(),
                    },
                });
            }
        }
    }
    records
}

/// What this generation may claim about the archive it is describing.
///
/// The one distinction it carries is whether the archive is a dialect's own
/// primitive-defining package. Inside such an archive the analyzer's primitive
/// recognition is granted by declaration *path*
/// (`solid-reactive-ir`'s `declaration_path_is_solid_package`), so the
/// package's own local `createSignal`, `createTrackedEffect`, `onCleanup`, …
/// are read as dialect primitive calls, and the owner census and the reactive
/// read census then attribute *the runtime's own internals* to the export as
/// consumer-visible operations. The audited bundled contracts for those exact
/// bytes close both domains as absent
/// (`pkg/contracts/bundled/solid-v1/solid-root-browser-production.json`,
/// `pkg/contracts/bundled/solid-v2/solidjs-signals.json`), so publishing those
/// operations asserts what the audit denies — see ADR 0005 and
/// `docs/package-contract-v2/phase21/2026-09-03-solid-js-self-certification-diagnosis.md`.
///
/// This is a **generation-scope decision, not a proof**: the predicate is an
/// exact package-name match with no version and no integrity behind it, which
/// is not identity. That is admissible only because the decision can act in
/// exactly one direction — it *withholds* claims, turning the affected domains
/// open, and can never establish one. The generator cannot answer the other
/// way either: emitting `reads: []`/`creates: []` *closed*, as the audits do,
/// would manufacture a negative claim from a derivation that produced nothing.
/// ADR 0017 also withholds callbacks: compact summaries do not retain whether
/// callback timing came from this bootstrap, including through local helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenerationScope {
    /// An ordinary consuming package: every derived domain is published.
    ConsumingPackage,
    /// A dialect's own primitive-defining package.
    DialectDefiningPackage,
}

impl GenerationScope {
    pub(crate) fn for_package(package_name: &str) -> Self {
        if solid_dialect::primitive_defining_package(package_name) {
            Self::DialectDefiningPackage
        } else {
            Self::ConsumingPackage
        }
    }

    /// Whether path-bootstrapped primitive recognition inside this archive may
    /// reach the published document as callbacks, owner-requirement cleanups,
    /// reactive reads or creates proposals.
    fn publishes_bootstrapped_reactive_domains(self) -> bool {
        matches!(self, Self::ConsumingPackage)
    }
}

/// ADR 0100: whether the implementation census can confirm every item of this
/// export's `callbacks` enumeration — an unguarded, untracked `invoke` `from` a
/// bare parameter `at` the call event on the same stack, **and** written by
/// the interprocedural pass for a call of that parameter itself in the
/// export's own body (`direct_callback_parameters`). The second condition is
/// what the wire cannot say: `untrack(cb)` publishes the same `inline` row as
/// `cb()`, and only the latter has a site the census walks to. The empty
/// enumeration is confirmable vacuously (the walk finds no caller-supplied
/// invocation). The certifier's `described_callbacks` applies the semantic
/// half of the same test to the proposal it receives, so a document that
/// passed here and one it plans agree; a summary with no direct set (`None`,
/// a summary the loop cannot find) proposes nothing described.
/// Whether every item of the export's `reads` enumeration is one the
/// implementation census confirms (ADR 0101): a `read` whose input is a caller
/// parameter, unguarded, untracked, `at` the call event on the same stack, and
/// performed in this export's own frame. The empty enumeration is trivially
/// so.
/// Whether the export leaves the caller's listener alone for this operation.
///
/// Both confirmability predicates below used to spell this
/// `tracking == Tracking::Untracked`, which was exact while *every* non-tracked
/// operation serialized as `untracked`. Now that a transparent wrapper says
/// `ambient-at-execution` and only a proven clearing says `untracked`, the
/// question they were asking is the disjunction: the export does not subscribe
/// this operation. Writing it as inequality against `Tracked` would also admit
/// `Unknown`, which is an open claim rather than an answer.
pub(crate) const fn export_does_not_subscribe(tracking: Tracking) -> bool {
    matches!(tracking, Tracking::Untracked | Tracking::AmbientAtExecution)
}

fn reads_enumeration_is_confirmable(export: &ExportSemantics) -> bool {
    export
        .operation_claim(ClaimDomain::Reads)
        .is_some_and(|claim| {
            claim.items().iter().all(|id| {
                export.operation(&id.0).is_some_and(|operation| {
                    operation.kind == OperationKind::Read
                        && matches!(operation.inputs.first(), Some(ValueShape::Parameter { .. }))
                        && operation.at == Some(Event::Call)
                        && operation.schedule == Some(Schedule::SameStack)
                        && export_does_not_subscribe(operation.tracking)
                        && operation.guard.is_none()
                        && operation.composed_from.is_none()
                })
            })
        })
}

///
/// A non-call item (item A of ways-to-improve § 3.3) is admissible on the same
/// footing: a `get` whose parameter this export's own body reads a property of
/// (`direct_accessor_parameters`), or a `coerce` whose parameter it coerces
/// (`direct_coerced_parameters`), each an `ambient-at-execution` invoke at the
/// call event on the same stack. `iterate` and `has-instance` are vocabulary
/// the generator does not derive, so an item naming one proposes nothing.
///
/// Item B of the same section: a call item whose `from` names a member of the
/// parameter -- `{arg: 1, path: ["0"]}` for `handler[0](…)` -- is admissible
/// when the export's own body calls exactly that literal-keyed member of that
/// parameter's unwritten binding (`direct_member_callback_parameters`). Every
/// other member path keeps the enumeration partial, as ADR 0100 decided.
fn callbacks_enumeration_is_confirmable(
    export: &ExportSemantics,
    summary: Option<&ContractExport>,
) -> bool {
    use solid_reactive_ir::contract_semantics::InvokeProtocol;
    // A non-call enumeration beside an iteration of a caller's value is one
    // the generator could not describe whole -- it derives no `iterate` item --
    // so it is not proposed. The census would refuse it, and under the
    // declared-signature premise may not even see the iteration: the veto's
    // Proxy does (`@kobalte/utils`' `isPointInPolygon`).
    let describes_protocols = export.callbacks().items().iter().any(|item| {
        export
            .operation(&item.operation.0)
            .is_some_and(solid_reactive_ir::contract_semantics::Operation::is_protocol_invocation)
    });
    if describes_protocols && summary.is_some_and(|summary| !summary.iterated_parameters.is_empty())
    {
        return false;
    }
    export.callbacks().items().iter().all(|item| {
        let Some(operation) = export.operation(&item.operation.0) else {
            return false;
        };
        // ADR 0139: a `result-access` item is admissible when the generator's
        // byte walk found the argument kept on the instance for members only
        // (`result_access_parameters`); its one shape is the model's.
        if operation.is_result_access() {
            return matches!(
                &item.from,
                ValueSource::Parameter { index, path }
                    if path.is_empty()
                        && summary.is_some_and(|summary| {
                            summary
                                .result_access_parameters
                                .contains(&usize::from(*index))
                        })
            );
        }
        let direct = |index: u16, path: &[String]| {
            summary.is_some_and(|summary| {
                let index = usize::from(index);
                match (operation.invoke_protocol(), path) {
                    (InvokeProtocol::Call, []) => {
                        summary.direct_callback_parameters.contains(&index)
                    }
                    (InvokeProtocol::Call, path) => summary
                        .direct_member_callback_parameters
                        .contains(&(index, path.to_vec())),
                    (InvokeProtocol::Get, []) => {
                        summary.direct_accessor_parameters.contains(&index)
                    }
                    (InvokeProtocol::Coerce, []) => {
                        summary.direct_coerced_parameters.contains(&index)
                    }
                    _ => false,
                }
            })
        };
        matches!(&item.from, ValueSource::Parameter { index, path } if direct(*index, path))
            && operation.kind == OperationKind::Invoke
            && operation.at == Some(Event::Call)
            && operation.schedule == Some(Schedule::SameStack)
            && export_does_not_subscribe(operation.tracking)
            && operation.guard.is_none()
    })
}

/// Re-derives what generation would have produced for one **inherited**
/// export: the projection of a dependency's certified export
/// ([`solid_reactive_ir::project_export_semantics`]), normalized under *this*
/// package's artifact case and public name.
///
/// The certifier's half of the inherited-closure premise. A parent's closure on
/// a re-exported name is admissible exactly when its items *are* the projection
/// of the dependency's, and the only way to answer that without inventing a
/// second notion of "the projection" is to run the generator's own derivation
/// again and compare. A comparison written independently would be a second
/// answer, and the certifier's is the one that silently admits a claim the
/// generator never made.
///
/// The two sinks are discarded on purpose: a withheld owner requirement and a
/// declined `creates` walk are *generation* records, and this is not a
/// generation. Nothing here reaches a document.
pub(crate) fn inherited_export_projection(
    artifact_case: &ArtifactCase,
    name: &str,
    dependency_export: &ExportSemantics,
    origin: solid_reactive_ir::InheritedExportOrigin,
    scope: GenerationScope,
) -> Result<ExportSemantics, ContractFailure> {
    let summary = ContractExport {
        inherited_from: Some(origin),
        ..solid_reactive_ir::project_export_semantics(dependency_export)
    };
    normalize_export(
        artifact_case,
        name,
        &summary,
        scope,
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

fn normalize_export(
    artifact_case: &ArtifactCase,
    name: &str,
    summary: &ContractExport,
    scope: GenerationScope,
    withheld: &mut Vec<WithheldOwnerRequirementRecord>,
    declined: &mut Vec<DeclinedClosureRecord>,
) -> Result<ExportSemantics, ContractFailure> {
    let prefix = format!("{}:{name}:operation:", artifact_case.id);
    let mut operations = Vec::new();
    let mut resources = Vec::new();

    let callbacks = match &summary.callbacks {
        // The compact summary loses bootstrap provenance, including through
        // local helper composition. ADR 0017 withholds this domain wholesale
        // inside primitive-defining archives rather than granting self-trust.
        _ if !scope.publishes_bootstrapped_reactive_domains() => KnowledgeSet::Unknown,
        // ADR 0103, amended 2026-09-23: a member alias has no body to summarize,
        // so its raised summary leaves `callbacks` open -- a claim of this
        // summary that stays as it is. What changes is the proposal: an empty
        // enumeration the certifier's default-library alias census decides from
        // the producer's identity fact and its reviewed member table, and refuses
        // for every member the table does not name.
        ContractClaim::Open if summary.kind == "function" && summary.member_alias_initializer => {
            KnowledgeSet::Complete(Vec::new())
        }
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(callbacks) => KnowledgeSet::Complete(
            with_derived_protocol_items(callbacks, summary)
                .iter()
                .enumerate()
                .map(|(index, callback)| {
                    let id = OperationId(format!("{prefix}callback-{index}"));
                    operations.push(callback_operation(id.clone(), callback, &mut resources)?);
                    Ok(CallbackInvocation {
                        from: ValueSource::Parameter {
                            index: u16::try_from(callback.parameter).map_err(|_| {
                                ContractFailure::InvalidSemanticModel {
                                    reason: format!(
                                        "callback parameter {} exceeds the normalized model limit",
                                        callback.parameter
                                    ),
                                }
                            })?,
                            // The member of the argument the row calls, whole
                            // (item B of ways-to-improve § 3.3): empty for the
                            // argument itself.
                            path: callback.path.clone(),
                        },
                        operation: id,
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        ),
    };

    let reads = match &summary.reactive_reads {
        // Withheld, not emptied: the domain is *open*, which says only that
        // this generation does not describe it.
        _ if !scope.publishes_bootstrapped_reactive_domains() => KnowledgeSet::Unknown,
        ContractClaim::Open => KnowledgeSet::Unknown,
        ContractClaim::Known(reads) => KnowledgeSet::Complete(
            reads
                .iter()
                .enumerate()
                .map(|(index, read)| {
                    let id = OperationId(format!("{prefix}read-{index}"));
                    let input = match (read.parameter, read.path.as_ref()) {
                        (Some(parameter), path) => ValueShape::Parameter {
                            index: u16::try_from(parameter).map_err(|_| {
                                ContractFailure::InvalidSemanticModel {
                                    reason: format!(
                                        "reactive-read parameter {parameter} exceeds the normalized model limit"
                                    ),
                                }
                            })?,
                            // The whole access path from the parameter. An
                            // absent path named no segment exactly and stays
                            // empty, which claims only "read through this
                            // parameter" -- the weakest claim the model has.
                            path: path.cloned().unwrap_or_default(),
                        },
                        (None, _) => ValueShape::Reactive {
                            role: ReactiveRole::Accessor,
                            resource: None,
                            capabilities: KnowledgeSet::Unknown,
                        },
                    };
                    let mut read_operation =
                        operation(id.clone(), OperationKind::Read, vec![input], None);
                    // The provenance the IR could name exactly. `read-<n>` is
                    // this same naming rule applied to the *other* export's
                    // own list, which is why the IR carries an ordinal rather
                    // than a label: the ordinal is the operation's name.
                    read_operation.composed_from =
                        read.composed_from
                            .as_ref()
                            .map(|composed| ComposedFrom {
                                export: composed.export.clone(),
                                operation: OperationId(format!(
                                    "{}:{}:operation:read-{}",
                                    artifact_case.id, composed.export, composed.read
                                )),
                            });
                    operations.push(read_operation);
                    Ok(id)
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        ),
    };

    // ADR 0035. `Known(None)` is the reactive analysis having *described* no
    // return — which is true of every export returning a plain value — and is
    // not a derived "yields nothing" claim, so it proposes nothing by itself.
    // The empty closure is proposed exactly when the generator's own
    // valueless-completion walk cleared this function export
    // (`solid_reactive_ir::valueless_completion`); the census then proves it.
    // A described return keeps publishing its positive operation, which the
    // weakening below turns into a partial claim rather than a closure.
    // ADR 0115 and ADR 0116: one `return` per value the argument-container walk
    // saw, each with its exact output. Read wherever the reactive analysis left
    // the return undescribed -- `Open` for a lone `[value]`, `Known(None)` for a
    // union whose branches disagree -- and never over a described return. Item
    // B round 2 of ways-to-improve § 3.3 adds a literal member of an argument
    // and the `undefined` of an optional chain (`callHandler`'s
    // `event?.defaultPrevented`).
    let container_returns = |operations: &mut Vec<Operation>| {
        (scope.publishes_bootstrapped_reactive_domains()
            && summary.kind == "function"
            && !summary.returns_argument_containers.is_empty()
            && summary.inherited_from.is_none()
            && matches!(&summary.async_behavior, ContractClaim::Known(protocol) if protocol.is_empty()))
        .then(|| {
            let mut ids = Vec::new();
            for (index, container) in summary.returns_argument_containers.iter().enumerate() {
                let id = OperationId(format!("{prefix}return-{index}"));
                operations.push(operation(
                    id.clone(),
                    OperationKind::Return,
                    Vec::new(),
                    Some(container.value_shape()),
                ));
                ids.push(id);
            }
            KnowledgeSet::Complete(ids)
        })
    };
    // The second 2026-09-24 amendment to ADR 0103: a member alias whose
    // spelling names a reviewed row that states the member's return proposes
    // that one `return`. The spelling decides nothing; the certifier's census
    // reads the member from the producer's identity fact.
    let alias_return = |operations: &mut Vec<Operation>| {
        (scope.publishes_bootstrapped_reactive_domains()
            && summary.kind == "function"
            && summary.member_alias_initializer)
            .then_some(())?;
        let output = crate::contract_certification::reviewed_default_library_alias_return(
            summary.member_alias_spelling.as_deref()?,
        )?;
        let id = OperationId(format!("{prefix}return"));
        operations.push(operation(
            id.clone(),
            OperationKind::Return,
            Vec::new(),
            Some(output),
        ));
        Some(KnowledgeSet::Complete(vec![id]))
    };
    // ADR 0145: one `return` per distinct described callable the walk saw --
    // every value-carrying completion a function literal, each with the call
    // claims its own syntax does not rule out. Read wherever the reactive
    // analysis left the return undescribed, never over a described one, and
    // after the argument-container and alias proposals, which never overlap
    // it (a literal is neither a parameter nor a member alias).
    let described_returns = |operations: &mut Vec<Operation>| {
        (scope.publishes_bootstrapped_reactive_domains()
            && summary.kind == "function"
            && !summary.returns_described_callables.is_empty()
            && summary.inherited_from.is_none()
            && matches!(&summary.async_behavior, ContractClaim::Known(protocol) if protocol.is_empty()))
        .then(|| {
            let single = summary.returns_described_callables.len() == 1;
            let mut ids = Vec::new();
            for (index, call) in summary.returns_described_callables.iter().enumerate() {
                let id = if single {
                    OperationId(format!("{prefix}return"))
                } else {
                    OperationId(format!("{prefix}return-{index}"))
                };
                operations.push(operation(
                    id.clone(),
                    OperationKind::Return,
                    Vec::new(),
                    Some(ValueShape::DescribedCallable(Box::new(call.clone()))),
                ));
                ids.push(id);
            }
            KnowledgeSet::Complete(ids)
        })
    };
    // ADR 0164: the valueless-completion walk is evidence about the export's
    // own completions, and nothing a reactive description of the return could
    // miss changes it: an unresolved call can hand a reactive member to a value
    // the export returns, and a body that returns no value hands nothing. So
    // the empty closure is proposed over an `Open` return exactly as over an
    // undescribed one, and ADR 0035's census proves it from the producer's
    // control-flow census either way.
    let valueless_returns = || {
        (scope.publishes_bootstrapped_reactive_domains()
            && summary.kind == "function"
            && summary.returns_walk_clean
            && summary.inherited_from.is_none())
        .then(|| KnowledgeSet::Complete(Vec::new()))
    };
    let returns = match &summary.returns {
        ContractClaim::Open => valueless_returns()
            .or_else(|| container_returns(&mut operations))
            .or_else(|| alias_return(&mut operations))
            .or_else(|| described_returns(&mut operations))
            .unwrap_or(KnowledgeSet::Unknown),
        // ADR 0109, before the empty closure and deliberately: a body that
        // returns a props merge *does* yield a value, so the two are mutually
        // exclusive by construction — the valueless-completion walk declines on
        // the very return this one reads. Ordering them makes that explicit
        // rather than relying on it.
        ContractClaim::Known(None)
            if scope.publishes_bootstrapped_reactive_domains()
                && summary.kind == "function"
                && summary.merged_props_return.is_some() =>
        {
            let from = summary
                .merged_props_return
                .expect("checked in the guard above");
            let id = OperationId(format!("{prefix}return"));
            operations.push(operation(
                id.clone(),
                OperationKind::Return,
                Vec::new(),
                Some(ValueShape::MergedProps {
                    from: u16::try_from(from).map_err(|_| {
                        ContractFailure::InvalidSemanticModel {
                            reason: format!(
                                "merged-props parameter {from} exceeds the normalized model limit"
                            ),
                        }
                    })?,
                }),
            ));
            KnowledgeSet::Complete(vec![id])
        }
        ContractClaim::Known(None) => {
            // A projected summary has no local implementation, so
            // `returns_walk_clean` is `false` for every re-export and this arm
            // used to discard the dependency's certified `returns: []`. The
            // inherited premise replaces the walk rather than joining it: the
            // dependency closed the domain, and the certifier discharges that
            // by composition from the dependency's receipt.
            if scope.publishes_bootstrapped_reactive_domains()
                && (summary.inherited_closure(ClaimDomain::Returns)
                    || (summary.kind == "function" && summary.returns_walk_clean))
            {
                KnowledgeSet::Complete(Vec::new())
            } else if let Some(containers) = container_returns(&mut operations) {
                // ADR 0115, before ADR 0113's plain return because it is the
                // narrower claim: the syntax walk saw nothing but the caller's
                // own arguments, fresh arrays of them, and (ADR 0116) calls of
                // them, so this proposes one `return` per container, each with
                // its exact output, and the census decides the enumeration from
                // the producer's arms of every return.
                containers
            } else if let Some(reviewed) = alias_return(&mut operations) {
                // An alias's summary describes no return because it has no
                // body, which is where this arm reads it.
                reviewed
            } else if let Some(described) = described_returns(&mut operations) {
                // ADR 0145, before ADR 0113's plain return: a function literal
                // is never a primitive, so the two walks never both answer.
                described
            } else if scope.publishes_bootstrapped_reactive_domains()
                && summary.kind == "function"
                && summary.returns_value_completion
                && summary.inherited_from.is_none()
                && matches!(&summary.async_behavior, ContractClaim::Known(protocol) if protocol.is_empty())
            {
                // ADR 0113. The body hands its caller a value and the reactive
                // analysis described none of it as reactive, which is what a
                // primitive completion looks like from here -- and also what a
                // plain object does, which syntax cannot always tell apart.
                // So this proposes the one shape the census can decide from
                // the producer's types, a single `return` whose output is
                // `plain`, and the certifier either proves it or withdraws the
                // operation, and the closure with it, by name.
                let id = OperationId(format!("{prefix}return"));
                operations.push(operation(
                    id.clone(),
                    OperationKind::Return,
                    Vec::new(),
                    Some(ValueShape::Plain),
                ));
                KnowledgeSet::Complete(vec![id])
            } else {
                KnowledgeSet::Unknown
            }
        }
        // ADR 0146: the reactive analysis described the return as an accessor,
        // and every value-carrying completion is a literal or an identifier:
        // propose what invoking it does -- an owned-signal read -- and let the
        // census decide whether the signal is one this export created, inert,
        // and exactly what is read.
        ContractClaim::Known(Some(returned))
            if returned.kind == "accessor"
                && scope.publishes_bootstrapped_reactive_domains()
                && summary.kind == "function"
                && !summary.returns_reading_callables.is_empty()
                && summary.inherited_from.is_none()
                && matches!(&summary.async_behavior, ContractClaim::Known(protocol) if protocol.is_empty()) =>
        {
            let single = summary.returns_reading_callables.len() == 1;
            let mut ids = Vec::new();
            for (index, call) in summary.returns_reading_callables.iter().enumerate() {
                let id = if single {
                    OperationId(format!("{prefix}return"))
                } else {
                    OperationId(format!("{prefix}return-{index}"))
                };
                operations.push(operation(
                    id.clone(),
                    OperationKind::Return,
                    Vec::new(),
                    Some(ValueShape::DescribedCallable(Box::new(call.clone()))),
                ));
                ids.push(id);
            }
            KnowledgeSet::Complete(ids)
        }
        ContractClaim::Known(Some(returned)) => {
            let id = OperationId(format!("{prefix}return"));
            let mut output = return_shape(returned)?;
            if let ContractClaim::Known(protocol) = &summary.async_behavior {
                output = match protocol.as_str() {
                    "promise" => ValueShape::Promise(Box::new(output)),
                    "async-iterable" => ValueShape::AsyncIterable(Box::new(output)),
                    _ => output,
                };
            }
            operations.push(operation(
                id.clone(),
                OperationKind::Return,
                Vec::new(),
                Some(output),
            ));
            KnowledgeSet::Complete(vec![id])
        }
    };

    let owner_requirements = match &summary.owner_requirements {
        // Same withholding as `reads`, for the same reason: the requirement's
        // only evidence inside this archive is the path heuristic naming the
        // primitive's own guarded implementation as a consumer obligation.
        // This one is not a *named* withholding: nothing was derived to
        // withhold. The scope decision refuses the whole derivation, and the
        // audits for these exact bytes state the domains themselves.
        _ if !scope.publishes_bootstrapped_reactive_domains() => None,
        ContractClaim::Open => None,
        ContractClaim::Known(requirements) => Some(requirements.as_slice()),
    };
    let mut requirement_cleanups = Vec::new();
    let mut requirement_computations = Vec::new();
    // Whether the document states every owner requirement this export has: the
    // owner census decided, and each requirement it found was published. A
    // consumer reads a closed `creates` as "no owner requirement beyond the
    // published items" (`project_owner_requirements`), so `creates` may close
    // only where that reading is true; see the `creates` gate below.
    let mut requirements_published = owner_requirements.is_some();
    for (index, requirement) in owner_requirements.unwrap_or_default().iter().enumerate() {
        let id = OperationId(format!("{prefix}owner-requirement-{index}"));
        match owner_requirement_operation(&id, requirement) {
            Ok(operation) => {
                if operation.kind == OperationKind::Compute {
                    requirement_computations.push(id);
                } else {
                    requirement_cleanups.push(id);
                }
                operations.push(operation);
            }
            Err(role) => {
                requirements_published = false;
                withheld.push(WithheldOwnerRequirementRecord {
                    export: name.to_owned(),
                    role,
                });
            }
        }
    }
    // `creates` carries no owner requirement any more: registering a
    // computation on the caller's owner is not a registration into a runtime
    // outside the invocation (`semantic-model.md` § creates). What the domain
    // may carry is a **proposal of absence**, and only that.
    //
    // `Complete(vec![])` here is *not* the generator asserting the negative
    // claim the hand audits assert. `normalize_knowledge` weakens an empty
    // `Complete` into a closure *candidate*: the proposal states the claim, the
    // verifier schedules a `DomainExhaustiveness` demand for it, and
    // `require_census_decides_closure`'s implementation census either proves it
    // against the authenticated archive's own transcripts or refuses the row by
    // name (`docs/adr/0008-implementation-census-for-creates.md`). Proposing is
    // therefore admissible where deriving is not — a proposal cannot certify
    // anything, and this one is refused unless a census decides it.
    //
    // Three gates, each fail-closed:
    //
    // * `GenerationScope::ConsumingPackage` only. Inside a dialect's own
    //   primitive-defining archive the primitive recognition this walk reads is
    //   granted by declaration *path*, which is the circularity ADR 0005
    //   objection 5 names; the existing withholding stays.
    // * A function export only. A `value` export's `creates` is decided by
    //   `validate_export`'s function-effect rule, not by an implementation the
    //   census could walk.
    // * `creates_walk_clean`, which is `false` unless
    //   [`solid_reactive_ir::CreatesProposalWalk`] actually walked this export's
    //   implementation and found no call that a `creates: []` claim would
    //   contradict. Silence is "do not propose". A member alias
    //   (`member_alias_initializer`, ADR 0103 amended 2026-09-23) stands in for
    //   the walk: there is no body to walk, and the certifier's default-library
    //   alias census, not an implementation census, is what decides it.
    //
    // * an **inherited** closure, which is none of the three. A cross-package
    //   re-export has no local symbol, so `creates_walk_clean` is `false` for
    //   it whatever the dependency certified, and the `function` gate answers
    //   about a body this archive does not contain. What closes the domain is
    //   the accepted dependency contract this summary was projected from, and
    //   the certifier discharges it against that dependency's receipt instead
    //   of a census of bytes that are not here.
    //
    // And under all of them, `requirements_published`. The documented rule
    // for a withheld requirement is that `creates` stays open, and before
    // 2026-09-23 only the walk kept it: the walk declines a call it has no
    // row for, and `createEffect` has none. But `createTrackedEffect` does --
    // its `creates` is audited closed -- and its call carries an `Effect`
    // requirement, which was withheld then, so an export calling it walked
    // clean and proposed `creates: []`. A consumer accepting that contract
    // read "no owner requirement" and stayed silent on an unowned call. Since
    // ADR 0114 an `Effect` requirement is published and only `Boundary` is
    // withheld; an owner census that did not decide (`Open`, from an
    // unresolved defect) is the same hole.
    let creates = if scope.publishes_bootstrapped_reactive_domains()
        && requirements_published
        && (summary.inherited_closure(ClaimDomain::Creates)
            || (summary.kind == "function"
                && (summary.creates_walk_clean || summary.member_alias_initializer)))
    {
        KnowledgeSet::Complete(Vec::new())
    } else {
        // Record *why* nothing was proposed, but only where a proposal was
        // actually on the table: a `ConsumingPackage` function export. The
        // other two gates are structural — a primitive-defining archive and a
        // `value` export have no implementation walk to blame — and reporting
        // their silence as a blocker would put rows in the ranking that no
        // dialect audit could ever clear.
        if scope.publishes_bootstrapped_reactive_domains() && summary.kind == "function" {
            declined.extend(summary.creates_walk_declines.iter().map(|decline| {
                DeclinedClosureRecord {
                    export: name.to_owned(),
                    domain: ClaimDomain::Creates.wire_name(),
                    decline: ClosureDecline::Call(decline.clone()),
                }
            }));
        }
        KnowledgeSet::Unknown
    };
    // `cleanups` is never *closed* here either, for the same reason: the owner
    // census establishes the cleanup obligations it walked, not that no other
    // cleanup exists. `partial` collapses an empty list back to `Unknown`.
    //
    // It is published `Partial` rather than left `Unknown` because the items
    // are real positive facts a consumer reads: `project_owner_requirements`
    // (`solid-reactive-ir`'s `contracts.rs`) reads this domain for its items
    // *only* and deliberately does not insert `ClaimDomain::Cleanups` into the
    // export's open claims -- which is why publishing items here opens nothing
    // for a consumer. See the scoping note in
    // `phase21/2026-09-03-implementation-census-plan.md` § 2.2 item 5.
    let cleanups = KnowledgeSet::partial(requirement_cleanups).unwrap_or(KnowledgeSet::Unknown);
    // ADR 0114: `computations` is stated by item only, and its items are what
    // a consumer reads, exactly as for the cleanup requirements above.
    let computations =
        KnowledgeSet::partial(requirement_computations).unwrap_or(KnowledgeSet::Unknown);

    let claims = CallClaims {
        callbacks,
        reads,
        writes: KnowledgeSet::Unknown,
        creates,
        invalidates: KnowledgeSet::Unknown,
        throws: KnowledgeSet::Unknown,
        returns,
        cleanups,
        disposals: KnowledgeSet::Unknown,
        computations,
    };
    let root = ExportTargetIdentity {
        module: artifact_case.runtime.clone(),
        export_name: name.into(),
    };
    Ok(ExportSemantics {
        identity: ExportIdentity {
            entrypoint: artifact_case.entrypoint.clone(),
            public_name: name.into(),
            runtime: root,
            declarations: ExportTargetIdentity {
                module: artifact_case.declarations.clone(),
                export_name: name.into(),
            },
        },
        shape: match summary.kind.as_str() {
            "unknown" => ValueShape::Unknown,
            "function" => ValueShape::Callable,
            "component" => ValueShape::Component,
            _ => ValueShape::Plain,
        },
        stability: StabilityKnowledge::Unknown,
        call: CallSemantics::new(
            claims,
            operations,
            Vec::new(),
            resources,
            GuardPartition::default(),
        ),
    })
}

/// A known enumeration's rows, followed by one non-call item per property read
/// (`get`) and per coercion (`coerce`) of a parameter the export's own body
/// performs (item A of ways-to-improve § 3.3). The generator derives these
/// itself, from its own syntax walk (ADR 0006); the implementation census
/// confirms them site for site. An inherited summary carries no such sets, and
/// its projected non-call rows are republished as they were rather than
/// derived again, so a row is never stated twice.
fn with_derived_protocol_items(
    callbacks: &[ContractCallback],
    summary: &ContractExport,
) -> Vec<ContractCallback> {
    use solid_reactive_ir::contract_semantics::InvokeProtocol;
    let mut rows = callbacks.to_vec();
    let derived = summary
        .direct_accessor_parameters
        .iter()
        .map(|parameter| (InvokeProtocol::Get, *parameter))
        .chain(
            summary
                .direct_coerced_parameters
                .iter()
                .map(|parameter| (InvokeProtocol::Coerce, *parameter)),
        );
    for (protocol, parameter) in derived {
        let row = ContractCallback {
            parameter,
            execution: "inline".into(),
            schedule: None,
            clears_tracking: false,
            arguments: Vec::new(),
            owner: None,
            protocol,
            path: Vec::new(),
        };
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    rows
}

fn callback_operation(
    id: OperationId,
    callback: &ContractCallback,
    resources: &mut Vec<Resource>,
) -> Result<Operation, ContractFailure> {
    // A non-call row is one shape and only one (the model's
    // `validate_protocol_operation`): at the call event on the same stack, in
    // the caller's tracking context, counted per call from zero to many,
    // unguarded, with no inputs and no owner claim. Built directly, so no
    // attribute a call row's execution word carries can leak into it, and a
    // projected row is republished as that shape whatever it was read from.
    if !callback.is_invocation() {
        let mut operation = operation(id, OperationKind::Invoke, Vec::new(), None);
        operation.tracking = Tracking::AmbientAtExecution;
        operation.protocol = Some(callback.protocol);
        return Ok(operation);
    }
    // ADR 0139's retention item is one shape too (the model's
    // `validate_result_access_operation`): triggered by and at the
    // `result-access` event, on an external schedule, in the tracking context
    // and under the owner of whoever invokes it through the returned value,
    // counted per trigger from zero to many. Built directly for the same
    // reason as a non-call row: nothing the row's other fields say can leak
    // into it.
    if callback.is_result_access() {
        if !callback.path.is_empty() {
            return invalid("a result-access row names the argument itself, never a member");
        }
        let mut operation = operation(id, OperationKind::Invoke, Vec::new(), None);
        operation.trigger = Some(Trigger::Event(Event::ResultAccess));
        operation.at = Some(Event::ResultAccess);
        operation.schedule = Some(Schedule::External);
        operation.tracking = Tracking::AmbientAtExecution;
        operation.owner = owner_ambient();
        operation.cardinality = Cardinality {
            scope: Some(CardinalityScope::Trigger),
            min: Some(0),
            max: Some(UpperBound::Many),
        };
        return Ok(operation);
    }
    // `inline` and `deferred` carry their schedule in the word. `tracked` does
    // not: it is an attribution word, and 1.x `createMemo`/`mergeProps` have
    // already run the callback when the export returns while 1.x `createEffect`
    // has not. Reading `queued` out of `tracked` published that falsehood for
    // every retained row; the schedule now travels with the row, from
    // [`solid_dialect::Dialect::tracked_callback_timing`] by way of
    // `composed_tracked_schedule`. `Unestablished` is the dialect refusing to
    // say, and emits no execution point rather than a guessed one -- the
    // attribution claim survives on its own.
    let (schedule, tracking) = match callback.execution.as_str() {
        // `inline` is the schedule axis alone: it promises the callback runs
        // before the export returns and says nothing about the listener.
        // `untrack(fn)` and a bare `fn()` are both inline, and publishing
        // `untracked` for both made the field unfalsifiable --
        // `@solid-primitives/utils`' `access` is `typeof v === "function" ? v() : v`
        // and its row claimed the same clearing `untrack`'s did. Only a chain
        // with a proven `Detaching` wrapper says `untracked` now; the rest say
        // what a transparent wrapper actually does.
        "inline" => (
            Some(Schedule::SameStack),
            if callback.clears_tracking {
                Tracking::Untracked
            } else {
                Tracking::AmbientAtExecution
            },
        ),
        // `deferred` is the schedule axis alone too. "Runs after the export
        // returns" does not mean no tracking scope is open when it runs: a
        // callback reached through a returned closure (`safe(transform)` is
        // `(raw) => transform(raw)`, `pipe`), a bound function or an event
        // listener runs on whoever calls it, inside whatever computation that
        // caller is in. Only a deferral proven to clear -- a reviewed
        // fresh-stack host queue, a dialect slot stated untracked, a clearing
        // wrapper inside the deferral -- says `untracked`
        // (`ContractCallback::clears_tracking`); every other deferral says
        // `ambient-at-execution`.
        "deferred" => (
            Some(Schedule::Queued),
            if callback.clears_tracking {
                Tracking::Untracked
            } else {
                Tracking::AmbientAtExecution
            },
        ),
        "tracked" => (
            match callback.schedule {
                Some(CallbackSchedule::SameStack) => Some(Schedule::SameStack),
                Some(CallbackSchedule::External) => Some(Schedule::External),
                Some(CallbackSchedule::Unestablished) => None,
                // A producer that stated no schedule for the row keeps the
                // consumer's historical default. Recorded in
                // docs/precision-backlog.md: the compiler-lowering callback
                // roles are the remaining path that reaches here.
                Some(CallbackSchedule::Queued) | None => Some(Schedule::Queued),
                Some(CallbackSchedule::ResultAccess) => {
                    return invalid("only a deferred row carries the result-access event");
                }
            },
            Tracking::Tracked,
        ),
        other => return invalid(format!("unknown callback execution {other:?}")),
    };
    let mut operation = operation(
        id.clone(),
        OperationKind::Invoke,
        callback
            .arguments
            .iter()
            .map(|argument| {
                argument
                    .as_ref()
                    .map_or(Ok(ValueShape::Unknown), return_shape)
            })
            .collect::<Result<Vec<_>, ContractFailure>>()?,
        None,
    );
    operation.schedule = schedule;
    // The execution point and the schedule are one fact in the document format
    // and must be known together, so an unestablished schedule drops both.
    if schedule.is_none() {
        operation.at = None;
    }
    operation.tracking = tracking;
    operation.owner = match callback.owner.as_deref() {
        Some("none" | "unowned") => owner_none(),
        Some("inherited") => owner_ambient(),
        Some("created" | "leaf") => {
            let resource = ResourceId(format!("{}owner", id.0));
            resources.push(Resource {
                id: resource.clone(),
                kind: ResourceKind::Owner,
                states: KnowledgeSet::Complete(vec![
                    ResourceState::OwnerActive,
                    ResourceState::OwnerDisposed,
                ]),
                capabilities: KnowledgeSet::Complete(Vec::new()),
                lifetime: Some(Lifetime::Owner(resource.clone())),
            });
            owner_created(resource, callback.owner.as_deref() == Some("leaf"))
        }
        Some("conditional") | None => OwnerRelation::default(),
        Some(other) => return invalid(format!("unknown callback owner {other:?}")),
    };
    Ok(operation)
}

fn operation(
    id: OperationId,
    kind: OperationKind,
    inputs: Vec<ValueShape>,
    output: Option<ValueShape>,
) -> Operation {
    Operation {
        id,
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
        inputs,
        output,
        resources: BTreeSet::new(),
        composed_from: None,
        protocol: None,
    }
}

fn owner_none() -> OwnerRelation {
    OwnerRelation {
        source: OwnerSource::None,
        requirements: OwnerRequirements {
            owner: Requirement::Forbidden,
            child_owners: Requirement::Unconstrained,
            cleanup: Requirement::Unconstrained,
        },
        capabilities: OwnerCapabilities {
            child_owners: CapabilityKnowledge::Forbidden,
            cleanup: CapabilityKnowledge::Forbidden,
        },
        lifetime: Some(Lifetime::Call),
        productions: KnowledgeSet::Complete(Vec::new()),
    }
}

fn owner_ambient() -> OwnerRelation {
    OwnerRelation {
        source: OwnerSource::AmbientAtExecution,
        productions: KnowledgeSet::Complete(Vec::new()),
        ..OwnerRelation::default()
    }
}

fn owner_created(resource: ResourceId, leaf: bool) -> OwnerRelation {
    let capabilities = OwnerCapabilities {
        child_owners: if leaf {
            CapabilityKnowledge::Forbidden
        } else {
            CapabilityKnowledge::Allowed
        },
        cleanup: if leaf {
            CapabilityKnowledge::Forbidden
        } else {
            CapabilityKnowledge::Allowed
        },
    };
    OwnerRelation {
        source: OwnerSource::Created(resource.clone()),
        requirements: OwnerRequirements {
            owner: Requirement::Required,
            child_owners: if leaf {
                Requirement::Forbidden
            } else {
                Requirement::Unconstrained
            },
            cleanup: if leaf {
                Requirement::Forbidden
            } else {
                Requirement::Unconstrained
            },
        },
        capabilities: capabilities.clone(),
        lifetime: Some(Lifetime::Owner(resource.clone())),
        productions: KnowledgeSet::Complete(vec![OwnerProduction {
            resource: resource.clone(),
            capabilities,
            lifetime: Some(Lifetime::Owner(resource)),
        }]),
    }
}

/// One withheld owner requirement, named by the export it belongs to.
///
/// A withholding that leaves only an absence behind is indistinguishable from
/// "the census found nothing", which is a different claim. This record is what
/// makes the generator's refusal a *named* one: it travels out of
/// normalization, through [`crate::ProposalArtifacts`], and onto the emit
/// boundary's machine-readable record so the generator's refusal audit can
/// carry the export, the role, and the reason.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WithheldOwnerRequirementRecord {
    pub export: String,
    pub role: WithheldOwnerRequirement,
}

/// One reason the generator declined to *propose* a closed `creates` for one
/// export, named by the blocker's own resolved identity.
///
/// The measurement channel of [`solid_reactive_ir::CreatesProposalWalk`], and
/// only that. It travels the same road as
/// [`WithheldOwnerRequirementRecord`] — out of normalization, through
/// [`crate::ProposalArtifacts`], onto the emit boundary's machine-readable
/// record, into the generator's proposal refusal audit — and for the same
/// reason: an unproposed candidate leaves only an open domain behind, which is
/// indistinguishable from "there was nothing to propose". It is not a refusal
/// (no artifact case is refused) and not a withheld claim (no claim was
/// derivable): it is why the earlier, weaker question was answered no.
///
/// **Nothing is certified from it.** A `dialect-silent` record is the audits'
/// silence about a spelling, and a `unresolved-callee` record is this build's
/// own ignorance; neither says the callee performs a `create`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DeclinedClosureRecord {
    /// The export whose domain stayed open.
    pub export: String,
    /// The domain the decline is about, by its
    /// [`solid_reactive_ir::contract_semantics::ClaimDomain::wire_name`].
    ///
    /// It was `creates` for as long as the walk was the only source. It is now
    /// any *proposable* domain, because the second source — a closure hazard —
    /// is domain-generic: the hazard names the domains it affects, and `reads`
    /// is one of them.
    pub domain: &'static str,
    /// Why the domain stayed open.
    pub decline: ClosureDecline,
}

/// One closure this generation proposed on an **inherited** premise: the name
/// is a cross-package re-export, and the domain is closed because the accepted
/// dependency contract it was projected from closes it.
///
/// Measurement and attribution, like [`DeclinedClosureRecord`] beside it. It
/// travels to the emit boundary's machine-readable record rather than into the
/// plan sidecar, and deliberately: the certifier does **not** read this
/// provenance. It rebinds the re-export from the parent's own
/// snapshot-verified runtime binding and the dependency node's plan, because a
/// provenance string a document carries about itself is the kind of
/// self-report the precision contract refuses to read as proof. What this
/// record answers is the auditor's question — which of a package's proposed
/// closures rest on a dependency's receipt rather than on a census of its own
/// bytes — which nothing else in either artifact can distinguish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritedClosureRecord {
    /// The public name this package re-exports.
    pub export: String,
    /// The domain proposed closed, by its
    /// [`solid_reactive_ir::contract_semantics::ClaimDomain::wire_name`].
    pub domain: &'static str,
    /// The accepted dependency export the closure came from.
    pub origin: solid_reactive_ir::InheritedExportOrigin,
}

/// The two reasons a proposable domain does not reach a closure candidate.
///
/// They are genuinely different facts and a record that flattened them would
/// lie about one of them. A [`Self::Call`] is a *call site inside the export*
/// the walk would not propose across — this build's own ignorance of one
/// callee. A [`Self::Hazard`] is a fact about the whole artifact closure that
/// opens the domain for every export it names, before any walk is consulted:
/// nothing about the export is unknown, the closure is.
///
/// Both answer the same question, so both reach the same channel with the same
/// columns; the `kind` column is what tells them apart.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClosureDecline {
    /// A call site the `creates` walk refused inside
    /// ([`solid_reactive_ir::CreatesProposalWalk`]).
    Call(solid_reactive_ir::CreatesDecline),
    /// A closure hazard that opened the domain at binding
    /// ([`crate::artifact_resolution::ClosureManifest::domain_openings`]).
    Hazard {
        kind: crate::artifact_resolution::ClosureHazardKind,
        /// The hazard's own `source`: a `path:start-end` for a syntactic
        /// hazard, a `module:specifier` for an unaccepted dependency.
        source: String,
    },
}

impl ClosureDecline {
    /// The stable wire name of the reason.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Call(decline) => decline.kind.name(),
            Self::Hazard { kind, .. } => kind.name(),
        }
    }

    /// Where the decline is anchored: the refusing call for a walk decline,
    /// the hazard's source for a hazard.
    #[must_use]
    pub fn location(&self) -> String {
        match self {
            Self::Call(decline) => decline.location(),
            Self::Hazard { source, .. } => source.clone(),
        }
    }

    /// The callee's resolved package, or `""`. A hazard names no callee.
    #[must_use]
    pub fn package(&self) -> &str {
        match self {
            Self::Call(decline) => decline.kind.package(),
            Self::Hazard { .. } => "",
        }
    }

    /// The callee's resolved export name, or `""`.
    #[must_use]
    pub fn callee_export(&self) -> &str {
        match self {
            Self::Call(decline) => decline.kind.callee_export(),
            Self::Hazard { .. } => "",
        }
    }

    /// The refusing callee's declaration site, or `""`.
    #[must_use]
    pub fn declaration(&self) -> &str {
        match self {
            Self::Call(decline) => decline.kind.declaration(),
            Self::Hazard { .. } => "",
        }
    }

    /// The unresolved callee's observed shape name, or `""`.
    #[must_use]
    pub const fn shape(&self) -> &'static str {
        match self {
            Self::Call(decline) => decline.kind.shape(),
            Self::Hazard { .. } => "",
        }
    }

    /// The one concrete string that shape observed, or `""`.
    #[must_use]
    pub fn shape_spelling(&self) -> &str {
        match self {
            Self::Call(decline) => decline.kind.shape_spelling(),
            Self::Hazard { .. } => "",
        }
    }
}

/// The owner-requirement role this generation withheld, named so the
/// withholding can be recorded rather than inferred from an absence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WithheldOwnerRequirement {
    /// An async-boundary JSX element the archive ships, which needs an owner
    /// at the consumer's call site. The `Effect` role was withheld too until
    /// ADR 0114 gave it the `computations` domain.
    Boundary,
}

impl WithheldOwnerRequirement {
    /// The stable role name a withholding record carries.
    pub const fn role(self) -> &'static str {
        match self {
            Self::Boundary => "boundary",
        }
    }

    /// Why this generation cannot publish the role, in one line.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Boundary => {
                "an async-boundary JSX element's owner requirement is a compiler-lowering fact the implementation census does not record"
            }
        }
    }
}

/// The operation one owner requirement publishes, in the shape the audited
/// documents use for that act -- `Err(role)` when the requirement is one this
/// generation refuses to publish at all.
///
/// An owner requirement is *not* a resourceless `create`: `creates` is the
/// domain of published `create` operations, and a `create` registers a
/// version-1 resource into a runtime outside the invocation, naming what it
/// registered (`docs/package-contract-v2/semantic-model.md` § creates, decision
/// 2026-09-03). Two of the three roles have a home in the model:
///
/// - A `Cleanup`/`SettledCleanup` requirement registers a cleanup on the
///   caller's owner: `kind: cleanup` in the `cleanups` domain, carrying
///   `source: ambient-at-call`, `requires: required`, and
///   `requiresCleanup: required`. **There is no audited precedent for that
///   shape** -- every `kind: cleanup` operation in the bundled corpus
///   (`solid-js`'s `replace-cleanup`, `@solidjs/signals`'s `returned-cleanup`,
///   `@solidjs/web`'s `ref-cleanup`, `--web-node-server`'s
///   `retract-declaration`) is `requires: forbidden`, `source: none`, because
///   each describes a cleanup the *runtime* runs rather than one the export
///   installs on its caller's owner. The shape is chosen for the fact rather
///   than copied: the requirement is a `Requirement` triple on the operation
///   that needs the owner (§ creates' third "not a `creates` item"), the
///   installing act is a cleanup, and `require_owner_operation_call` witnesses
///   it from the archive's own `onCleanup` call. It names **no resource**, as
///   `returned-cleanup` and `ref-cleanup` also do: a resource declaration is a
///   positive fact of its own -- `PositiveFactSubject::Resource`, demanded as
///   `ProofFamily::RecursiveValueShape` -- and no witness exists for a
///   resource axis today, so declaring one would assert what this generation
///   cannot prove.
/// - An `Effect` requirement registers a computation on the caller's owner:
///   `kind: compute` in the `computations` domain (ADR 0114), carrying
///   `source: ambient-at-call`, `requires: required` and
///   `requiresChildren: required`. It is not a `create` -- registering on the
///   caller's owner is what the audits publish beside `creates: []` *closed*
///   -- and until ADR 0114 version 1 had no domain for it, so it was withheld
///   by name. `require_owner_operation_call` witnesses it from the archive's
///   own call of an `Effect`-role primitive, keyed on `requiresChildren`, as it
///   witnessed the resourceless `create` this shape replaces. What it produces
///   stays **unknown**, not empty: the registered computation is itself an
///   owner, and no witness exists for a resource axis.
/// - A `Boundary` requirement is withheld for a second, independent reason: it
///   is a *compiler lowering* fact (the JSX loop at
///   `rust/crates/solid-reactive-ir/src/owners.rs:1213-1231`), and the
///   producer's implementation census records neither JSX elements nor their
///   lowering, so nothing could discharge it.
fn owner_requirement_operation(
    id: &OperationId,
    requirement: &ContractOwnerRequirement,
) -> Result<Operation, WithheldOwnerRequirement> {
    match requirement.operation {
        OwnerRequirementOperation::Cleanup | OwnerRequirementOperation::SettledCleanup => {
            let mut cleanup = operation(id.clone(), OperationKind::Cleanup, Vec::new(), None);
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
            cleanup.cardinality.min = Some(u32::from(requirement.guaranteed));
            Ok(cleanup)
        }
        OwnerRequirementOperation::Effect => {
            let mut compute = operation(id.clone(), OperationKind::Compute, Vec::new(), None);
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
            // ADR 0161: every call registers exactly when the requirement is
            // guaranteed; the census proves that lower bound or withdraws it.
            compute.cardinality.min = Some(u32::from(requirement.guaranteed));
            Ok(compute)
        }
        OwnerRequirementOperation::Boundary => Err(WithheldOwnerRequirement::Boundary),
    }
}

fn return_shape(returned: &ContractReturn) -> Result<ValueShape, ContractFailure> {
    Ok(match returned.kind.as_str() {
        "accessor" => ValueShape::Reactive {
            role: ReactiveRole::Accessor,
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        },
        "store-path" => ValueShape::Store {
            resource: None,
            capabilities: KnowledgeSet::Unknown,
        },
        // ADR 0109: the conditional props root. The parameter is the claim, so
        // an absent one refuses rather than defaulting to argument 0.
        "merged-props" => ValueShape::MergedProps {
            from: u16::try_from(returned.parameter.ok_or_else(|| {
                ContractFailure::InvalidSemanticModel {
                    reason: "merged-props return shape requires a parameter index".into(),
                }
            })?)
            .map_err(|_| ContractFailure::InvalidSemanticModel {
                reason: format!(
                    "return parameter {} exceeds the normalized model limit",
                    returned.parameter.expect("checked above")
                ),
            })?,
        },
        "argument" => ValueShape::Parameter {
            index: u16::try_from(returned.parameter.ok_or_else(|| {
                ContractFailure::InvalidSemanticModel {
                    reason: "argument return shape requires a parameter index".into(),
                }
            })?)
            .map_err(|_| ContractFailure::InvalidSemanticModel {
                reason: format!(
                    "return parameter {} exceeds the normalized model limit",
                    returned.parameter.expect("checked above")
                ),
            })?,
            path: Vec::new(),
        },
        "tuple" => ValueShape::Tuple(KnowledgeSet::Complete(
            returned
                .elements
                .iter()
                .map(|item| item.as_ref().map_or(Ok(ValueShape::Unknown), return_shape))
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        )),
        "object" => ValueShape::Object(KnowledgeSet::Complete(
            returned
                .properties
                .iter()
                .map(|(name, value)| {
                    Ok(ObjectProperty {
                        name: name.clone(),
                        value: return_shape(value)?,
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        )),
        "array" => ValueShape::Array {
            element: Box::new(ValueShape::Unknown),
            length: ArrayLength::default(),
        },
        _ => ValueShape::Unknown,
    })
}

fn invalid<T>(reason: impl Into<String>) -> Result<T, ContractFailure> {
    Err(ContractFailure::InvalidSemanticModel {
        reason: reason.into(),
    })
}

fn model_failure(error: solid_reactive_ir::contract_semantics::ModelError) -> ContractFailure {
    ContractFailure::InvalidSemanticModel {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests;
