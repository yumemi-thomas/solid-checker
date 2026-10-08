//! Decoder and normalization boundary for the stable package-contract wire
//! format.
//!
//! Everything in this module is crate-private on purpose. Summary IDs,
//! closure lists, omission rules, and schema spellings terminate here; the
//! rest of the analyzer can only observe the wire-independent semantic model.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};

use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use sha2::{Digest as _, Sha256};
use solid_reactive_ir::contract_semantics::{
    ArrayLength, ArtifactCase, ArtifactIdentity, CallbackInvocation, CapabilityClaim,
    CapabilityKnowledge, Cardinality, CardinalityScope, ClaimDomain, ContractProposal, Digest,
    EdgeKind, Event, ExportIdentity, ExportSemantics, ExportTargetIdentity, Guard, GuardAtom,
    GuardPartition, GuardedCase, InvokeProtocol, KnowledgeSet, Lifetime, Literal, MemberClass,
    ModuleInitializationClaim, NormalizedContract, ObjectProperty, ObservableCapability, Operation,
    OperationEdge, OperationId, OperationKind, OwnerCapabilities, OwnerProduction, OwnerRelation,
    OwnerRequirements, OwnerSource, PackageIdentity, ReactiveRole, Requirement, ResolutionStep,
    Resource, ResourceCapability, ResourceId, ResourceKind, ResourceState, SEMANTIC_MODEL_VERSION,
    Schedule, StabilityKnowledge, StrictRead, Tracking, Trigger, UpperBound, ValueKind, ValueShape,
    ValueSource,
};

use crate::contract_interface::ContractFailure;

const FORMAT: &str = "solid-reactivity-contract";
const SCHEMA_VERSION: u16 = 1;
const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_RECURSIVE_DEPTH: usize = 32;
const MAX_JSON_DEPTH: usize = 128;
const MAX_JSON_NODES: usize = 250_000;
const MAX_STRING_BYTES: usize = 16 * 1024;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_ENTRYPOINTS: usize = 1_024;
const MAX_ARTIFACT_CASES: usize = 1_024;
const MAX_SUMMARIES: usize = 16_384;
const MAX_EXPORTS: usize = 65_536;
const MAX_OPERATIONS: usize = 4_096;
const MAX_RESOURCES: usize = 4_096;
const MAX_EDGES: usize = 8_192;
const MAX_GUARD_CASES: usize = 256;
const MAX_GUARD_ATOMS: usize = 256;
// The product of summary size and reference count is also bounded so a small
// document cannot expand into an unbounded in-memory semantic graph.
const MAX_EXPANDED_NODES: usize = 1_000_000;

pub(crate) struct DecodedContractDocument {
    document: WireDocument,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SidecarDigests {
    pub proof: Option<Digest>,
    pub probes: Option<Digest>,
}

impl DecodedContractDocument {
    pub(crate) fn normalize(self) -> Result<NormalizedContract, ContractFailure> {
        expand(self.document)
    }

    pub(crate) fn sidecar_digests(&self) -> Result<SidecarDigests, ContractFailure> {
        Ok(SidecarDigests {
            proof: self
                .document
                .sidecars
                .proof
                .as_ref()
                .map(|reference| parse_wire_digest(&reference.sha256))
                .transpose()?,
            probes: self
                .document
                .sidecars
                .probes
                .as_ref()
                .map(|reference| parse_wire_digest(&reference.sha256))
                .transpose()?,
        })
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<DecodedContractDocument, ContractFailure> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(ContractFailure::DocumentTooLarge {
            limit: MAX_DOCUMENT_BYTES,
        });
    }
    let value = crate::bounded_json::value(
        bytes,
        crate::bounded_json::Limits {
            bytes: MAX_DOCUMENT_BYTES,
            depth: MAX_JSON_DEPTH,
            nodes: MAX_JSON_NODES,
            string_bytes: MAX_STRING_BYTES,
        },
    )
    .map_err(|message| ContractFailure::DocumentDecode { message })?;
    let document: WireDocument = serde_json::from_value(value).map_err(document_decode)?;
    if document.format != FORMAT {
        return invalid_document(format!(
            "format must be {FORMAT:?}, got {:?}",
            document.format
        ));
    }
    if document.schema_version != SCHEMA_VERSION {
        return Err(ContractFailure::UnsupportedSchemaVersion {
            expected: SCHEMA_VERSION,
            actual: document.schema_version,
        });
    }
    if document.semantic_model_version != SEMANTIC_MODEL_VERSION {
        return invalid_model(format!(
            "semantic model version {} is unsupported; expected {SEMANTIC_MODEL_VERSION}",
            document.semantic_model_version
        ));
    }
    Ok(DecodedContractDocument { document })
}

/// Canonical stable-v1 emission. This is the inverse of [`decode`] for a
/// normalized contract that originated at this boundary. Compact summary IDs
/// and local operation/resource spellings are created here and nowhere else.
pub(crate) fn encode(
    contract: &NormalizedContract,
    sidecars: &SidecarDigests,
    pretty: bool,
) -> Result<Vec<u8>, ContractFailure> {
    let document = compact(contract, sidecars)?;
    let mut bytes = if pretty {
        serde_json::to_vec_pretty(&document)
    } else {
        serde_json::to_vec(&document)
    }
    .map_err(document_decode)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn compact(
    contract: &NormalizedContract,
    sidecars: &SidecarDigests,
) -> Result<JsonValue, ContractFailure> {
    let package = contract.package();
    let mut summaries = BTreeMap::<String, JsonValue>::new();
    let mut summary_ids = BTreeMap::<Vec<u8>, String>::new();
    let mut entrypoint_cases = BTreeMap::<String, Vec<JsonValue>>::new();

    for artifact_case in contract.artifact_cases() {
        let mut exports = JsonMap::new();
        for (public_name, export) in &artifact_case.exports {
            if export.identity.entrypoint != artifact_case.entrypoint
                || export.identity.public_name != *public_name
            {
                return invalid_model(format!(
                    "export {public_name:?} identity does not match its artifact case"
                ));
            }
            let wire_case_id = compact_artifact_case_id(artifact_case)?;
            let ids = CompactIds::new(&artifact_case.id, &wire_case_id, public_name);
            let summary = compact_summary(export, &ids)?;
            let key = serde_json::to_vec(&summary).map_err(document_decode)?;
            let summary_id = if let Some(existing) = summary_ids.get(&key) {
                existing.clone()
            } else {
                let digest = Sha256::digest(&key);
                let id = format!("summary-{:x}", digest);
                summary_ids.insert(key, id.clone());
                summaries.insert(id.clone(), summary);
                id
            };
            let reference = if export.stability == StabilityKnowledge::Experimental {
                json!({"summary": summary_id, "stability": "experimental"})
            } else {
                JsonValue::String(summary_id)
            };
            exports.insert(public_name.clone(), reference);
        }

        let mut case = JsonMap::new();
        if !artifact_case.resolution_trace.is_empty() {
            let runtime = trace_target(artifact_case, "runtime")?;
            let declarations = trace_target(artifact_case, "types")?;
            if artifact_case.resolution_trace.len() != 2 {
                return invalid_model(format!(
                    "artifact case {:?} has a non-wire resolution trace",
                    artifact_case.id
                ));
            }
            case.insert(
                "resolution".into(),
                json!({"runtimeBranch": runtime, "typesBranch": declarations}),
            );
        }
        case.insert(
            "artifact".into(),
            json!({
                "path": artifact_case.runtime.path,
                "sha256": wire_digest(&artifact_case.runtime.digest)?,
                "closureSha256": wire_digest(&artifact_case.dependency_closure)?,
            }),
        );
        case.insert(
            "declarations".into(),
            compact_artifact(&artifact_case.declarations)?,
        );
        if let Some(transform) = &artifact_case.transform {
            case.insert("transform".into(), compact_artifact(transform)?);
        }
        if artifact_case.stability == StabilityKnowledge::Experimental {
            case.insert("stability".into(), json!("experimental"));
        }
        if let Some(initialization) = artifact_case.initialization {
            case.insert(
                "initialization".into(),
                match initialization {
                    ModuleInitializationClaim::Inert => json!("inert"),
                },
            );
        }
        case.insert("exports".into(), JsonValue::Object(exports));
        entrypoint_cases
            .entry(artifact_case.entrypoint.clone())
            .or_default()
            .push(JsonValue::Object(case));
    }

    let mut entrypoints = JsonMap::new();
    for (entrypoint, mut cases) in entrypoint_cases {
        cases.sort_by_key(|case| serde_json::to_vec(case).unwrap_or_default());
        if cases.len() == 1 && cases[0].get("resolution").is_none() {
            entrypoints.insert(entrypoint, cases.pop().expect("one case"));
        } else {
            if cases.iter().any(|case| case.get("resolution").is_none()) {
                return invalid_model(format!(
                    "conditional entrypoint {entrypoint:?} contains an untraced artifact case"
                ));
            }
            entrypoints.insert(entrypoint, json!({"cases": cases}));
        }
    }

    let mut sidecar_document = JsonMap::new();
    if let Some(proof) = &sidecars.proof {
        sidecar_document.insert("proof".into(), json!({"sha256": wire_digest(proof)?}));
    }
    if let Some(probes) = &sidecars.probes {
        sidecar_document.insert("probes".into(), json!({"sha256": wire_digest(probes)?}));
    }

    Ok(json!({
        "format": FORMAT,
        "schemaVersion": SCHEMA_VERSION,
        "semanticModelVersion": contract.semantic_model_version(),
        "package": {
            "name": package.name,
            "version": package.version,
            "integrity": package.integrity,
            "manifest": compact_artifact(&package.manifest)?,
        },
        "summaries": summaries,
        "entrypoints": entrypoints,
        "sidecars": sidecar_document,
    }))
}

fn trace_target<'a>(
    artifact_case: &'a ArtifactCase,
    axis: &str,
) -> Result<&'a str, ContractFailure> {
    artifact_case
        .resolution_trace
        .iter()
        .find(|step| step.condition == axis)
        .map(|step| step.target.as_str())
        .ok_or_else(|| ContractFailure::InvalidSemanticModel {
            reason: format!(
                "artifact case {:?} is missing its {axis} resolution branch",
                artifact_case.id
            ),
        })
}

fn compact_artifact(artifact: &ArtifactIdentity) -> Result<JsonValue, ContractFailure> {
    Ok(json!({
        "path": artifact.path,
        "sha256": wire_digest(&artifact.digest)?,
    }))
}

fn wire_digest(digest: &Digest) -> Result<&str, ContractFailure> {
    digest
        .as_str()
        .strip_prefix("sha256:")
        .ok_or_else(|| ContractFailure::InvalidSemanticModel {
            reason: "semantic digest is not a sha256 digest".into(),
        })
}

struct CompactIds {
    source_case: String,
    wire_case: String,
    operation_prefix: String,
    resource_prefix: String,
}

impl CompactIds {
    fn new(source_case: &str, wire_case: &str, export: &str) -> Self {
        Self {
            source_case: source_case.into(),
            wire_case: wire_case.into(),
            operation_prefix: format!("{source_case}:{export}:operation:"),
            resource_prefix: format!("{source_case}:{export}:resource:"),
        }
    }

    fn operation<'a>(&self, id: &'a OperationId) -> Result<&'a str, ContractFailure> {
        Ok(id.0.strip_prefix(&self.operation_prefix).unwrap_or(&id.0))
    }

    /// The local spelling of an operation of *another* export of the same
    /// artifact case.
    ///
    /// The prefix has to match exactly: an id that does not carry this
    /// artifact case and that export is a provenance the document cannot
    /// state, and emitting the qualified id verbatim would put another
    /// artifact case's identity into a field this side re-qualifies with its
    /// own on the way back in.
    fn operation_in<'a>(
        &self,
        export: &str,
        id: &'a OperationId,
    ) -> Result<&'a str, ContractFailure> {
        id.0.strip_prefix(&format!("{}:{export}:operation:", self.source_case))
            .ok_or_else(|| ContractFailure::InvalidSemanticModel {
                reason: format!(
                    "composed provenance {:?} does not name an operation of {export:?} in this artifact case",
                    id.0
                ),
            })
    }

    fn resource<'a>(&self, id: &'a ResourceId) -> Result<&'a str, ContractFailure> {
        Ok(id.0.strip_prefix(&self.resource_prefix).unwrap_or(&id.0))
    }
}

/// The content address this encoder would give one export's claims.
///
/// Exactly the `summary-<sha256>` the document writes: `compact_summary`
/// renders the export in the document's *local* operation spelling
/// (`callback-0`), so the address depends on what the export claims and on
/// nothing about where it was found. Two certifications of one published
/// artifact therefore share it whenever they agree, which is the question
/// artifact admission has to answer when a host declares no export conditions
/// and two cases reach the file it resolved.
///
/// Reusing the encoder rather than restating it is the point. The in-memory
/// form cannot be compared with `==`: decoding qualifies every operation id
/// with the artifact-case id (`IdScope::operation_in`), so two contracts that
/// claim the same thing differ in every `OperationId` they carry. Measured on
/// `@kobalte/utils@0.9.2`, whose two `.` cases differ in exactly that and in
/// nothing else, for 13 of 59 exports.
pub(crate) fn export_claims_address(
    artifact_case: &ArtifactCase,
    public_name: &str,
    export: &ExportSemantics,
) -> Result<String, ContractFailure> {
    let wire_case_id = compact_artifact_case_id(artifact_case)?;
    let ids = CompactIds::new(&artifact_case.id, &wire_case_id, public_name);
    let summary = compact_summary(export, &ids)?;
    let key = serde_json::to_vec(&summary).map_err(document_decode)?;
    Ok(format!("summary-{:x}", Sha256::digest(&key)))
}

fn compact_artifact_case_id(artifact_case: &ArtifactCase) -> Result<String, ContractFailure> {
    let trace = if artifact_case.resolution_trace.is_empty() {
        Vec::new()
    } else {
        vec![
            ResolutionStep {
                condition: "runtime".into(),
                target: trace_target(artifact_case, "runtime")?.into(),
            },
            ResolutionStep {
                condition: "types".into(),
                target: trace_target(artifact_case, "types")?.into(),
            },
        ]
    };
    Ok(artifact_case_id(
        &artifact_case.entrypoint,
        &trace,
        &artifact_case.runtime,
        &artifact_case.declarations,
        &artifact_case.dependency_closure,
        artifact_case.transform.as_ref(),
    ))
}

fn compact_summary(
    export: &ExportSemantics,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    Ok(json!({
        "shape": compact_value(&export.shape, ids)?,
        "call": compact_call(&export.call, ids)?,
    }))
}

fn compact_call(
    call: &solid_reactive_ir::contract_semantics::CallSemantics,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let claims = call.claims();
    let mut object = JsonMap::new();
    if !call.captures().is_empty() {
        object.insert("captures".into(), JsonValue::Array(call.captures().iter().map(|capture| {
            Ok(json!({ "id": capture.id, "from": compact_value_source(&capture.from, ids)? }))
        }).collect::<Result<Vec<_>, ContractFailure>>()?));
    }
    if !call.callback_results().is_empty() {
        let results = call
            .callback_results()
            .iter()
            .map(|result| {
                let mut item = JsonMap::new();
                item.insert("producer".into(), json!(ids.operation(&result.producer)?));
                item.insert("shape".into(), compact_value(&result.shape, ids)?);
                if !result.callable_only.is_empty() {
                    item.insert(
                        "callableOnly".into(),
                        JsonValue::Array(
                            result
                                .callable_only
                                .iter()
                                .map(|id| Ok(json!(ids.operation(id)?)))
                                .collect::<Result<Vec<_>, ContractFailure>>()?,
                        ),
                    );
                }
                let mut closed = Vec::new();
                compact_knowledge(&mut item, &mut closed, "uses", &result.uses, |id| {
                    Ok(json!(ids.operation(id)?))
                })?;
                if !closed.is_empty() {
                    item.insert("closed".into(), json!(closed));
                }
                Ok(JsonValue::Object(item))
            })
            .collect::<Result<Vec<_>, ContractFailure>>()?;
        object.insert("callbackResults".into(), JsonValue::Array(results));
    }
    let mut closed = Vec::new();
    compact_knowledge(
        &mut object,
        &mut closed,
        "callbacks",
        &claims.callbacks,
        |callback| compact_callback(callback, ids),
    )?;
    for (name, knowledge) in [
        ("reads", &claims.reads),
        ("writes", &claims.writes),
        ("creates", &claims.creates),
        ("invalidates", &claims.invalidates),
        ("throws", &claims.throws),
        ("returns", &claims.returns),
        ("cleanups", &claims.cleanups),
        ("disposals", &claims.disposals),
        // ADR 0114: items only, so it never reaches `closed`; validation
        // refuses a closed one before the encoder sees it.
        ("computations", &claims.computations),
    ] {
        compact_knowledge(&mut object, &mut closed, name, knowledge, |id| {
            Ok(json!(ids.operation(id)?))
        })?;
    }
    if !closed.is_empty() {
        object.insert("closed".into(), json!(closed));
    }
    // A proposed closure is a sibling of `closed`, never a member of it: the
    // domain it names stays open in this document, and a consumer that does
    // not understand the key reads exactly the open domain it read before the
    // key existed.
    if !call.proposed_closures().is_empty() {
        object.insert(
            "proposedClosures".into(),
            json!(
                call.proposed_closures()
                    .iter()
                    .map(|domain| call_domain_name(*domain))
                    .collect::<Vec<_>>()
            ),
        );
    }
    // ADR 0153 part 3: the conditions every claim of this export holds
    // under, by the package export each context escapes as. Additive: a
    // document that omits the key states its claims unconditionally, which is
    // what every document said before the key existed.
    if !call.context_premises().is_empty() {
        object.insert(
            "contextPremises".into(),
            json!(
                call.context_premises()
                    .iter()
                    .map(|premise| premise.export.as_str())
                    .collect::<Vec<_>>()
            ),
        );
    }
    // ADR 0153 item C: the accessor-installation hazard sites this export's
    // closed `reads` holds against. Additive: a document that omits the key
    // bounds none, which is what every document said before the key existed.
    if !call.accessor_bounds().is_empty() {
        object.insert(
            "accessorBounds".into(),
            json!(call.accessor_bounds().iter().collect::<Vec<_>>()),
        );
    }
    if !call.operations.is_empty() {
        object.insert(
            "operations".into(),
            JsonValue::Array(
                call.operations
                    .iter()
                    .map(|operation| compact_operation(operation, ids))
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if !call.edges.is_empty() {
        object.insert(
            "edges".into(),
            JsonValue::Array(
                call.edges
                    .iter()
                    .map(|edge| {
                        Ok(json!({
                            "kind": edge_kind(edge.kind),
                            "from": ids.operation(&edge.from)?,
                            "to": ids.operation(&edge.to)?,
                        }))
                    })
                    .collect::<Result<_, ContractFailure>>()?,
            ),
        );
    }
    if !call.resources.is_empty() {
        object.insert(
            "resources".into(),
            JsonValue::Array(
                call.resources
                    .iter()
                    .map(|resource| compact_resource(resource, ids))
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    compact_guard_partition(&mut object, &call.guards, ids)?;
    Ok(JsonValue::Object(object))
}

fn compact_knowledge<T>(
    object: &mut JsonMap<String, JsonValue>,
    closed: &mut Vec<&'static str>,
    name: &'static str,
    knowledge: &KnowledgeSet<T>,
    mut item: impl FnMut(&T) -> Result<JsonValue, ContractFailure>,
) -> Result<(), ContractFailure> {
    match knowledge {
        KnowledgeSet::Unknown => {}
        KnowledgeSet::Partial(items) | KnowledgeSet::Complete(items) => {
            object.insert(
                name.into(),
                JsonValue::Array(items.iter().map(&mut item).collect::<Result<_, _>>()?),
            );
            if knowledge.is_closed() {
                closed.push(name);
            }
        }
    }
    Ok(())
}

fn compact_callback(
    callback: &CallbackInvocation,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    Ok(json!({
        "from": compact_value_source(&callback.from, ids)?,
        "operation": ids.operation(&callback.operation)?,
    }))
}

/// ADR 0152: a nested item in the top-level item's shape, `{from,
/// operation}`, with the operation written inline -- it is referenced by
/// nothing, so it carries no id -- in the spellings an `invoke` operation's
/// own fields use.
fn compact_described_callback(
    callback: &solid_reactive_ir::contract_semantics::DescribedCallback,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let mut operation = JsonMap::new();
    operation.insert("kind".into(), json!(operation_kind(OperationKind::Invoke)));
    if let Some(trigger) = &callback.trigger {
        operation.insert("trigger".into(), compact_trigger(trigger, ids)?);
    }
    match (callback.at, callback.schedule) {
        (Some(event), Some(schedule)) => {
            operation.insert(
                "at".into(),
                json!({"event": event_name(event), "schedule": schedule_name(schedule)}),
            );
        }
        (None, None) => {}
        _ => {
            return invalid_model(
                "a described callback's execution point and schedule must be known together",
            );
        }
    }
    if callback.tracking != Tracking::Unknown {
        operation.insert("tracking".into(), json!(tracking_name(callback.tracking)));
    }
    if callback.owner != OwnerRelation::default() {
        operation.insert("owner".into(), compact_owner(&callback.owner, ids)?);
    }
    if callback.cardinality != Cardinality::default() {
        operation.insert(
            "count".into(),
            compact_cardinality(&callback.cardinality, ids)?,
        );
    }
    Ok(json!({
        "from": compact_value_source(&callback.from, ids)?,
        "operation": JsonValue::Object(operation),
    }))
}

fn compact_value_source(
    source: &ValueSource,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    Ok(match source {
        ValueSource::Capture { capture, path } => json!({"capture": capture, "path": path}),
        ValueSource::Parameter { index, path } => json!({"arg": index, "path": path}),
        ValueSource::ParameterMembers { index, path, class } => {
            json!({"arg": index, "path": path, "members": class.wire()})
        }
        ValueSource::OperationOutput { operation, path } => {
            json!({"operation": ids.operation(operation)?, "path": path})
        }
        ValueSource::Resource { resource, path } => {
            json!({"resource": ids.resource(resource)?, "path": path})
        }
    })
}

fn compact_operation(
    operation: &Operation,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let mut object = JsonMap::new();
    object.insert("id".into(), json!(ids.operation(&operation.id)?));
    object.insert("kind".into(), json!(operation_kind(operation.kind)));
    if let Some(guard) = &operation.guard {
        object.insert("guard".into(), compact_guard(guard, ids)?);
    }
    if let Some(trigger) = &operation.trigger {
        object.insert("trigger".into(), compact_trigger(trigger, ids)?);
    }
    match (operation.at, operation.schedule) {
        (Some(event), Some(schedule)) => {
            object.insert(
                "at".into(),
                json!({"event": event_name(event), "schedule": schedule_name(schedule)}),
            );
        }
        (None, None) => {}
        _ => return invalid_model("operation execution point and schedule must be known together"),
    }
    if operation.tracking != Tracking::Unknown {
        object.insert("tracking".into(), json!(tracking_name(operation.tracking)));
    }
    if let Some(strict_read) = operation.strict_read {
        object.insert(
            "strictRead".into(),
            json!(match strict_read {
                StrictRead::Cleared => "cleared",
            }),
        );
    }
    if operation.owner != OwnerRelation::default() {
        object.insert("owner".into(), compact_owner(&operation.owner, ids)?);
    }
    if operation.cardinality != Cardinality::default() {
        object.insert(
            "count".into(),
            compact_cardinality(&operation.cardinality, ids)?,
        );
    }
    if !operation.inputs.is_empty() {
        object.insert(
            "inputs".into(),
            JsonValue::Array(
                operation
                    .inputs
                    .iter()
                    .map(|value| compact_value(value, ids))
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(output) = &operation.output {
        object.insert("output".into(), compact_value(output, ids)?);
    }
    if !operation.resources.is_empty() {
        object.insert(
            "resources".into(),
            JsonValue::Array(
                operation
                    .resources
                    .iter()
                    .map(|resource| Ok(json!(ids.resource(resource)?)))
                    .collect::<Result<_, ContractFailure>>()?,
            ),
        );
    }
    if let Some(protocol) = operation
        .protocol
        .filter(|protocol| *protocol != InvokeProtocol::Call)
    {
        object.insert("protocol".into(), json!(protocol.wire_name()));
    }
    if let Some(composed) = &operation.composed_from {
        object.insert(
            "composedFrom".into(),
            json!({
                "export": composed.export,
                "operation": ids.operation_in(&composed.export, &composed.operation)?,
            }),
        );
    }
    Ok(JsonValue::Object(object))
}

fn compact_trigger(trigger: &Trigger, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    Ok(match trigger {
        Trigger::Event(event) => json!({"event": event_name(*event)}),
        Trigger::Operation(operation) => json!({"operation": ids.operation(operation)?}),
        Trigger::Resource { resource, event } => {
            json!({"resource": ids.resource(resource)?, "event": event_name(*event)})
        }
    })
}

fn compact_owner(owner: &OwnerRelation, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    let mut object = JsonMap::new();
    match &owner.source {
        OwnerSource::Unknown => {}
        OwnerSource::None => {
            object.insert("source".into(), json!("none"));
        }
        OwnerSource::AmbientAtCall => {
            object.insert("source".into(), json!("ambient-at-call"));
        }
        OwnerSource::AmbientAtExecution => {
            object.insert("source".into(), json!("ambient-at-execution"));
        }
        OwnerSource::Captured(resource) => {
            object.insert("source".into(), json!("captured"));
            object.insert("resource".into(), json!(ids.resource(resource)?));
        }
        OwnerSource::Created(resource) => {
            object.insert("source".into(), json!("created"));
            object.insert("resource".into(), json!(ids.resource(resource)?));
        }
    }
    compact_requirement(&mut object, "requires", owner.requirements.owner);
    compact_requirement(
        &mut object,
        "requiresChildren",
        owner.requirements.child_owners,
    );
    compact_requirement(&mut object, "requiresCleanup", owner.requirements.cleanup);
    compact_capability(
        &mut object,
        "children",
        owner.capabilities.child_owners,
        "allowed",
    );
    compact_capability(
        &mut object,
        "cleanup",
        owner.capabilities.cleanup,
        "supported",
    );
    if let Some(lifetime) = &owner.lifetime {
        object.insert("lifetime".into(), compact_lifetime(lifetime, ids)?);
    }
    match &owner.productions {
        KnowledgeSet::Unknown => {}
        KnowledgeSet::Partial(productions) | KnowledgeSet::Complete(productions) => {
            object.insert(
                "productions".into(),
                JsonValue::Array(
                    productions
                        .iter()
                        .map(|production| compact_owner_production(production, ids))
                        .collect::<Result<_, _>>()?,
                ),
            );
            if owner.productions.is_closed() {
                object.insert("closed".into(), json!(["productions"]));
            }
        }
    }
    Ok(JsonValue::Object(object))
}

fn compact_owner_production(
    production: &OwnerProduction,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let mut object = JsonMap::new();
    object.insert(
        "resource".into(),
        json!(ids.resource(&production.resource)?),
    );
    compact_capability(
        &mut object,
        "children",
        production.capabilities.child_owners,
        "allowed",
    );
    compact_capability(
        &mut object,
        "cleanup",
        production.capabilities.cleanup,
        "supported",
    );
    if let Some(lifetime) = &production.lifetime {
        object.insert("lifetime".into(), compact_lifetime(lifetime, ids)?);
    }
    Ok(JsonValue::Object(object))
}

fn compact_requirement(object: &mut JsonMap<String, JsonValue>, name: &str, value: Requirement) {
    if value != Requirement::Unconstrained {
        object.insert(
            name.into(),
            json!(match value {
                Requirement::Required => "required",
                Requirement::Forbidden => "forbidden",
                Requirement::Unconstrained => "unconstrained",
            }),
        );
    }
}

fn compact_capability(
    object: &mut JsonMap<String, JsonValue>,
    name: &str,
    value: CapabilityKnowledge,
    positive: &str,
) {
    if value != CapabilityKnowledge::Unknown {
        object.insert(
            name.into(),
            json!(if value == CapabilityKnowledge::Allowed {
                positive
            } else {
                "forbidden"
            }),
        );
    }
}

fn compact_lifetime(lifetime: &Lifetime, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    Ok(match lifetime {
        Lifetime::Call => json!("call"),
        Lifetime::Resource(resource) => {
            json!({"kind": "resource", "resource": ids.resource(resource)?})
        }
        Lifetime::Owner(resource) => {
            json!({"kind": "owner", "resource": ids.resource(resource)?})
        }
        Lifetime::Request(resource) => {
            json!({"kind": "request", "resource": ids.resource(resource)?})
        }
        Lifetime::Transition(resource) => {
            json!({"kind": "transition", "resource": ids.resource(resource)?})
        }
        Lifetime::AsyncSource(resource) => {
            json!({"kind": "async-source", "resource": ids.resource(resource)?})
        }
    })
}

fn compact_cardinality(
    cardinality: &Cardinality,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let mut object = JsonMap::new();
    if let Some(scope) = &cardinality.scope {
        match scope {
            CardinalityScope::Trigger => {
                object.insert("scope".into(), json!("trigger"));
            }
            CardinalityScope::Call => {
                object.insert("scope".into(), json!("call"));
            }
            CardinalityScope::Resource(resource) => {
                object.insert("scope".into(), json!("resource"));
                object.insert("resource".into(), json!(ids.resource(resource)?));
            }
        }
    }
    if let Some(min) = cardinality.min {
        object.insert("min".into(), json!(min));
    }
    if let Some(max) = cardinality.max {
        object.insert(
            "max".into(),
            match max {
                UpperBound::Finite(value) => json!(value),
                UpperBound::Many => json!("many"),
            },
        );
    }
    Ok(JsonValue::Object(object))
}

fn compact_resource(resource: &Resource, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    let mut object = JsonMap::new();
    object.insert("id".into(), json!(ids.resource(&resource.id)?));
    object.insert("kind".into(), json!(resource_kind(resource.kind)));
    let mut closed = Vec::new();
    compact_knowledge(
        &mut object,
        &mut closed,
        "states",
        &resource.states,
        |state| Ok(json!(resource_state(*state))),
    )?;
    compact_knowledge(
        &mut object,
        &mut closed,
        "capabilities",
        &resource.capabilities,
        |capability| Ok(json!(resource_capability(*capability))),
    )?;
    if !closed.is_empty() {
        object.insert("closed".into(), json!(closed));
    }
    if let Some(lifetime) = &resource.lifetime {
        object.insert("lifetime".into(), compact_lifetime(lifetime, ids)?);
    }
    Ok(JsonValue::Object(object))
}

fn compact_guard_partition(
    object: &mut JsonMap<String, JsonValue>,
    partition: &GuardPartition,
    ids: &CompactIds,
) -> Result<(), ContractFailure> {
    let cases = match &partition.cases {
        KnowledgeSet::Unknown => return Ok(()),
        KnowledgeSet::Partial(cases) | KnowledgeSet::Complete(cases) => cases,
    };
    let values = cases
        .iter()
        .map(|case| match case {
            GuardedCase::When { guard, operations } => {
                let mut value = JsonMap::new();
                value.insert("when".into(), compact_guard(guard, ids)?);
                compact_guard_operations(&mut value, operations, ids)?;
                Ok(JsonValue::Object(value))
            }
            GuardedCase::Otherwise { operations } => {
                let mut value = JsonMap::new();
                value.insert("otherwise".into(), JsonValue::Bool(true));
                compact_guard_operations(&mut value, operations, ids)?;
                Ok(JsonValue::Object(value))
            }
        })
        .collect::<Result<Vec<_>, ContractFailure>>()?;
    object.insert("cases".into(), JsonValue::Array(values));
    Ok(())
}

fn compact_guard_operations(
    object: &mut JsonMap<String, JsonValue>,
    operations: &KnowledgeSet<OperationId>,
    ids: &CompactIds,
) -> Result<(), ContractFailure> {
    match operations {
        KnowledgeSet::Unknown => Ok(()),
        KnowledgeSet::Partial(operations) => {
            let encoded = operations
                .iter()
                .map(|operation| Ok(ids.operation(operation)?.to_owned()))
                .collect::<Result<Vec<_>, ContractFailure>>()?;
            object.insert("operations".into(), json!(encoded));
            object.insert("operationsOpen".into(), JsonValue::Bool(true));
            Ok(())
        }
        KnowledgeSet::Complete(operations) => {
            let encoded = operations
                .iter()
                .map(|operation| Ok(ids.operation(operation)?.to_owned()))
                .collect::<Result<Vec<_>, ContractFailure>>()?;
            object.insert("operations".into(), json!(encoded));
            Ok(())
        }
    }
}

fn compact_guard(guard: &Guard, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    Ok(json!({
        "all": guard
            .0
            .iter()
            .map(|atom| compact_guard_atom(atom, ids))
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

fn compact_guard_atom(atom: &GuardAtom, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    Ok(match atom {
        GuardAtom::Signature(signature) => json!({"signature": signature}),
        GuardAtom::ArgumentCount { min, max } => {
            let mut count = JsonMap::new();
            count.insert("min".into(), json!(min));
            if let Some(max) = max {
                count.insert("max".into(), json!(max));
            }
            json!({"argumentCount": count})
        }
        GuardAtom::Literal {
            argument,
            path,
            value,
        } => json!({"arg": argument, "path": path, "literal": compact_literal(value)?}),
        GuardAtom::ValueKind {
            argument,
            path,
            kind,
        } => json!({"arg": argument, "path": path, "kind": value_kind(*kind)}),
        GuardAtom::Property {
            argument,
            path,
            name,
            callable,
        } => {
            let mut value = json!({"arg": argument, "path": path, "property": name});
            if let Some(callable) = callable {
                value["callable"] = json!(callable);
            }
            value
        }
        GuardAtom::TupleAlternative {
            argument,
            alternative,
        } => json!({"arg": argument, "tupleAlternative": alternative}),
        GuardAtom::ResultProtocol(kind) => json!({"resultProtocol": value_kind(*kind)}),
        GuardAtom::ArtifactCase(case) if case == &ids.source_case => {
            json!({"artifactCase": ids.wire_case})
        }
        GuardAtom::ArtifactCase(case) => {
            return invalid_model(format!(
                "artifact-case guard {case:?} crosses its selected case"
            ));
        }
    })
}

fn compact_literal(literal: &Literal) -> Result<JsonValue, ContractFailure> {
    match literal {
        Literal::Null => Ok(JsonValue::Null),
        Literal::Bool(value) => Ok(json!(value)),
        Literal::String(value) => Ok(json!(value)),
        Literal::Number(value) => value
            .parse::<serde_json::Number>()
            .map(JsonValue::Number)
            .map_err(|_| ContractFailure::InvalidSemanticModel {
                reason: format!("guard number {value:?} is not valid JSON"),
            }),
    }
}

fn compact_value(value: &ValueShape, ids: &CompactIds) -> Result<JsonValue, ContractFailure> {
    Ok(match value {
        ValueShape::Unknown => json!("unknown"),
        ValueShape::Plain => json!("plain"),
        ValueShape::Parameter { index, path } => {
            json!({"kind": "parameter", "index": index, "path": path})
        }
        ValueShape::Tuple(items) => compact_value_collection("tuple", "items", items, ids)?,
        ValueShape::Array { element, length } => {
            let mut node = JsonMap::new();
            node.insert("kind".into(), json!("array"));
            node.insert("element".into(), compact_value(element, ids)?);
            if length != &ArrayLength::default() {
                let mut wire_length = JsonMap::new();
                if let Some(min) = length.min {
                    wire_length.insert("min".into(), json!(min));
                }
                if let Some(max) = length.max {
                    wire_length.insert(
                        "max".into(),
                        match max {
                            UpperBound::Finite(max) => json!(max),
                            UpperBound::Many => json!("many"),
                        },
                    );
                }
                node.insert("length".into(), JsonValue::Object(wire_length));
            }
            JsonValue::Object(node)
        }
        ValueShape::Object(properties) => {
            let mut node = JsonMap::new();
            node.insert("kind".into(), json!("object"));
            compact_value_knowledge(&mut node, "properties", properties, |property| {
                Ok(json!({
                    "name": property.name,
                    "value": compact_value(&property.value, ids)?,
                }))
            })?;
            JsonValue::Object(node)
        }
        ValueShape::Choice(alternatives) => {
            compact_value_collection("choice", "alternatives", alternatives, ids)?
        }
        ValueShape::Callable => json!("callable"),
        ValueShape::Promise(value) => {
            json!({"kind": "promise", "value": compact_value(value, ids)?})
        }
        ValueShape::AsyncIterable(element) => {
            json!({"kind": "async-iterable", "element": compact_value(element, ids)?})
        }
        ValueShape::Reactive {
            role,
            resource,
            capabilities,
        } => compact_capability_value("reactive", Some(*role), resource, capabilities, ids)?,
        ValueShape::Store {
            resource,
            capabilities,
        } => compact_capability_value("store", None, resource, capabilities, ids)?,
        // ADR 0109. One field, and it is the whole claim: which of the caller's
        // arguments this object's property reads reach through to.
        ValueShape::MergedProps { from } => json!({"kind": "merged-props", "from": from}),
        ValueShape::ArgumentArray { items } => json!({"kind": "argument-array", "items": items}),
        ValueShape::InvocationResult { parameter } => {
            json!({"kind": "invocation-result", "parameter": parameter})
        }
        // Item B round 2 of ways-to-improve § 3.3: exactly `undefined`, a
        // shorthand like `plain`, with a detailed spelling beside it.
        ValueShape::Undefined => json!("undefined"),
        // ADR 0145: both lists always written, so an absent one is never read
        // as a closed empty enumeration.
        // ADR 0152: `callbacks` is written only when it names an item, so every
        // document stating a described callable that invokes nothing keeps its
        // bytes; absent, it reads as ADR 0145's `callbacks: []`.
        // ADR 0235.
        ValueShape::EffectfulCallable(call) => json!({
            "kind": "effectful-callable",
            "call": compact_call(call, ids)?,
        }),
        ValueShape::ReturnedCallable { call, members } => {
            let mut node = json!({ "kind": "returned-callable" });
            if let Some(call) = call {
                node["call"] = compact_call(call, ids)?;
            }
            if !members.is_empty() {
                node["members"] = json!(
                    members
                        .iter()
                        .map(|member| {
                            Ok(json!({
                                "name": member.name,
                                "value": compact_value(&member.value, ids)?,
                            }))
                        })
                        .collect::<Result<Vec<_>, ContractFailure>>()?
                );
            }
            node
        }
        ValueShape::DescribedCallable(call) => {
            let mut node = json!({
                "kind": "described-callable",
                "reads": call.reads.iter().map(|read| read.wire_name()).collect::<Vec<_>>(),
                "returns": call
                    .returns
                    .iter()
                    .map(|returned| compact_value(returned, ids))
                    .collect::<Result<Vec<_>, _>>()?,
            });
            if !call.callbacks.is_empty() {
                node["callbacks"] = JsonValue::Array(
                    call.callbacks
                        .iter()
                        .map(|callback| compact_described_callback(callback, ids))
                        .collect::<Result<_, _>>()?,
                );
            }
            node
        }
        // ADR 0146: a shorthand, like `undefined`, with a detailed spelling.
        ValueShape::ReadValue => json!("read-value"),
        ValueShape::PrototypeInstance {
            members,
            population,
        } => json!({
            "kind": "prototype-instance",
            "population": match population {
                solid_reactive_ir::contract_semantics::PrototypePopulation::Opaque => "opaque",
                solid_reactive_ir::contract_semantics::PrototypePopulation::Values => "values",
                solid_reactive_ir::contract_semantics::PrototypePopulation::Entries => "entries",
            },
            "members": members.iter().map(|member| {
                let kind = match member.kind {
                    solid_reactive_ir::contract_semantics::PrototypeMemberKind::Method => "method",
                    solid_reactive_ir::contract_semantics::PrototypeMemberKind::Getter => "getter",
                    solid_reactive_ir::contract_semantics::PrototypeMemberKind::Iterator => "iterator",
                };
                let tracks = member.tracks.iter().map(|track| {
                    let mut value = json!({"cache": track.cache});
                    if let Some(argument) = track.argument { value["argument"] = json!(argument); }
                    if let Some(shared) = &track.shared { value["shared"] = json!(shared); }
                    value
                }).collect::<Vec<_>>();
                json!({"name": member.name, "kind": kind, "tracks": tracks})
            }).collect::<Vec<_>>()
        }),
        ValueShape::LazyGetterObject { keys, from } => {
            let mut value = json!({"kind": "lazy-getter-object", "keys": keys});
            if let Some(from) = from {
                value["from"] = json!(from);
            }
            value
        }
        ValueShape::Action { transition } => {
            let mut node = json!({"kind": "action"});
            if let Some(transition) = transition {
                node["transition"] = json!(ids.resource(transition)?);
            }
            node
        }
        ValueShape::Component => json!("component"),
        ValueShape::Cleanup { resource, lifetime } => {
            let mut node = json!({"kind": "cleanup"});
            if let Some(resource) = resource {
                node["resource"] = json!(ids.resource(resource)?);
            }
            if let Some(lifetime) = lifetime {
                node["lifetime"] = compact_lifetime(lifetime, ids)?;
            }
            node
        }
        ValueShape::RefApplication => json!("ref-application"),
        ValueShape::ServerFunctionReference { resource } => {
            let mut node = json!({"kind": "server-function-reference"});
            if let Some(resource) = resource {
                node["resource"] = json!(ids.resource(resource)?);
            }
            node
        }
    })
}

fn compact_value_collection(
    kind: &str,
    field: &'static str,
    values: &KnowledgeSet<ValueShape>,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let mut node = JsonMap::new();
    node.insert("kind".into(), json!(kind));
    compact_value_knowledge(&mut node, field, values, |value| compact_value(value, ids))?;
    Ok(JsonValue::Object(node))
}

fn compact_value_knowledge<T>(
    node: &mut JsonMap<String, JsonValue>,
    field: &'static str,
    knowledge: &KnowledgeSet<T>,
    mut item: impl FnMut(&T) -> Result<JsonValue, ContractFailure>,
) -> Result<(), ContractFailure> {
    match knowledge {
        KnowledgeSet::Unknown => {}
        KnowledgeSet::Partial(items) | KnowledgeSet::Complete(items) => {
            node.insert(
                field.into(),
                JsonValue::Array(items.iter().map(&mut item).collect::<Result<_, _>>()?),
            );
            if knowledge.is_closed() {
                node.insert("closed".into(), json!([field]));
            }
        }
    }
    Ok(())
}

fn compact_capability_value(
    kind: &str,
    role: Option<ReactiveRole>,
    resource: &Option<ResourceId>,
    capabilities: &KnowledgeSet<CapabilityClaim>,
    ids: &CompactIds,
) -> Result<JsonValue, ContractFailure> {
    let mut node = JsonMap::new();
    node.insert("kind".into(), json!(kind));
    if let Some(role) = role {
        node.insert(
            "role".into(),
            json!(match role {
                ReactiveRole::Accessor => "accessor",
                ReactiveRole::Setter => "setter",
            }),
        );
    }
    if let Some(resource) = resource {
        node.insert("resource".into(), json!(ids.resource(resource)?));
    }
    compact_value_knowledge(&mut node, "capabilities", capabilities, |claim| {
        if let Some(resource) = &claim.resource {
            Ok(json!({
                "capability": observable_capability(claim.capability),
                "resource": ids.resource(resource)?,
            }))
        } else {
            Ok(json!(observable_capability(claim.capability)))
        }
    })?;
    Ok(JsonValue::Object(node))
}

const fn operation_kind(value: OperationKind) -> &'static str {
    match value {
        OperationKind::Invoke => "invoke",
        OperationKind::Return => "return",
        OperationKind::Read => "read",
        OperationKind::Write => "write",
        OperationKind::Invalidate => "invalidate",
        OperationKind::Create => "create",
        OperationKind::Cleanup => "cleanup",
        OperationKind::Dispose => "dispose",
        OperationKind::Compute => "compute",
    }
}

const fn event_name(value: Event) -> &'static str {
    match value {
        Event::Call => "call",
        Event::Render => "render",
        Event::Flush => "flush",
        Event::Settle => "settle",
        Event::Transition => "transition",
        Event::AsyncEmission => "async-emission",
        Event::Cleanup => "cleanup",
        Event::External => "external-event",
        Event::Request => "request",
        Event::ResponseCommitment => "response-commitment",
        Event::ResultAccess => "result-access",
    }
}

const fn schedule_name(value: Schedule) -> &'static str {
    match value {
        Schedule::SameStack => "same-stack",
        Schedule::Queued => "queued",
        Schedule::External => "external",
    }
}

const fn tracking_name(value: Tracking) -> &'static str {
    match value {
        Tracking::Tracked => "tracked",
        Tracking::Untracked => "untracked",
        Tracking::AmbientAtExecution => "ambient-at-execution",
        Tracking::Unknown => "unknown",
    }
}

const fn call_domain_name(value: ClaimDomain) -> &'static str {
    match value {
        ClaimDomain::Callbacks => "callbacks",
        ClaimDomain::Reads => "reads",
        ClaimDomain::Writes => "writes",
        ClaimDomain::Creates => "creates",
        ClaimDomain::Invalidates => "invalidates",
        ClaimDomain::Throws => "throws",
        ClaimDomain::Returns => "returns",
        ClaimDomain::Cleanups => "cleanups",
        ClaimDomain::Disposals => "disposals",
        ClaimDomain::Computations => "computations",
    }
}

const fn edge_kind(value: EdgeKind) -> &'static str {
    match value {
        EdgeKind::Orders => "orders",
        EdgeKind::Data => "data",
        EdgeKind::Invalidates => "invalidates",
        EdgeKind::Error => "error",
        EdgeKind::Cleanup => "cleanup",
        EdgeKind::Lifetime => "lifetime",
    }
}

const fn resource_kind(value: ResourceKind) -> &'static str {
    match value {
        ResourceKind::Owner => "owner",
        ResourceKind::ReactiveSource => "reactive-source",
        ResourceKind::AsyncComputation => "async-computation",
        ResourceKind::Transition => "transition",
        ResourceKind::Cleanup => "cleanup",
        ResourceKind::Request => "request",
        ResourceKind::Response => "response",
        ResourceKind::Stream => "stream",
        ResourceKind::ServerFunctionReference => "server-function-reference",
    }
}

const fn resource_state(value: ResourceState) -> &'static str {
    match value {
        ResourceState::OwnerActive | ResourceState::TransitionActive => "active",
        ResourceState::OwnerDisposed | ResourceState::CleanupDisposed => "disposed",
        ResourceState::CleanupInstalled => "installed",
        ResourceState::AsyncPending => "pending",
        ResourceState::AsyncSettled | ResourceState::TransitionSettled => "settled",
        ResourceState::AsyncErrored => "errored",
        ResourceState::AsyncCancelled => "cancelled",
        ResourceState::TransitionReverted => "reverted",
        ResourceState::ResponseUncommitted => "uncommitted",
        ResourceState::ResponseCommitted => "committed",
        ResourceState::StreamUnclaimed => "unclaimed",
        ResourceState::StreamClaimed => "claimed",
    }
}

const fn resource_capability(value: ResourceCapability) -> &'static str {
    match value {
        ResourceCapability::Refreshable => "refreshable",
        ResourceCapability::Writable => "writable",
    }
}

const fn value_kind(value: ValueKind) -> &'static str {
    match value {
        ValueKind::Plain => "plain",
        ValueKind::Callable => "callable",
        ValueKind::Promise => "promise",
        ValueKind::AsyncIterable => "async-iterable",
    }
}

const fn observable_capability(value: ObservableCapability) -> &'static str {
    match value {
        ObservableCapability::Readable => "readable",
        ObservableCapability::Writable => "writable",
        ObservableCapability::Refreshable => "refreshable",
        ObservableCapability::PendingAware => "pending-aware",
        ObservableCapability::Optimistic => "optimistic",
    }
}

fn document_decode(error: serde_json::Error) -> ContractFailure {
    ContractFailure::DocumentDecode {
        message: error.to_string(),
    }
}

fn invalid_document<T>(message: impl Into<String>) -> Result<T, ContractFailure> {
    Err(ContractFailure::DocumentDecode {
        message: message.into(),
    })
}

fn invalid_model<T>(reason: impl Into<String>) -> Result<T, ContractFailure> {
    Err(ContractFailure::InvalidSemanticModel {
        reason: reason.into(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDocument {
    format: String,
    schema_version: u16,
    semantic_model_version: u16,
    package: WirePackage,
    summaries: BTreeMap<String, WireSummary>,
    entrypoints: BTreeMap<String, WireEntrypoint>,
    sidecars: WireSidecars,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WirePackage {
    name: String,
    version: String,
    integrity: String,
    manifest: WireFile,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireFile {
    path: String,
    sha256: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRuntimeArtifact {
    path: String,
    sha256: String,
    closure_sha256: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSidecars {
    proof: Option<WireHashReference>,
    probes: Option<WireHashReference>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireHashReference {
    sha256: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireEntrypoint {
    Unconditional(WireUnconditionalEntrypoint),
    Conditional(WireConditionalEntrypoint),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireUnconditionalEntrypoint {
    #[serde(default, deserialize_with = "deserialize_initialization")]
    initialization: Option<WireModuleInitialization>,
    artifact: WireRuntimeArtifact,
    declarations: WireFile,
    #[serde(default)]
    transform: Option<WireFile>,
    #[serde(default)]
    stability: Option<WireStability>,
    exports: BTreeMap<String, WireExportReference>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConditionalEntrypoint {
    cases: Vec<WireArtifactCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireArtifactCase {
    #[serde(default, deserialize_with = "deserialize_initialization")]
    initialization: Option<WireModuleInitialization>,
    resolution: WireResolution,
    artifact: WireRuntimeArtifact,
    declarations: WireFile,
    #[serde(default)]
    transform: Option<WireFile>,
    #[serde(default)]
    stability: Option<WireStability>,
    exports: BTreeMap<String, WireExportReference>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireModuleInitialization {
    Inert,
}

fn deserialize_initialization<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<WireModuleInitialization>, D::Error> {
    WireModuleInitialization::deserialize(deserializer).map(Some)
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireResolution {
    runtime_branch: String,
    types_branch: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WireExportReference {
    Summary(String),
    Detailed(WireDetailedExportReference),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDetailedExportReference {
    summary: String,
    #[serde(default)]
    stability: Option<WireStability>,
}

impl WireExportReference {
    fn parts(&self) -> (&str, StabilityKnowledge) {
        match self {
            Self::Summary(summary) => (summary, StabilityKnowledge::Unknown),
            Self::Detailed(reference) => (
                &reference.summary,
                reference
                    .stability
                    .map_or(StabilityKnowledge::Unknown, Into::into),
            ),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireStability {
    Experimental,
}

impl From<WireStability> for StabilityKnowledge {
    fn from(_: WireStability) -> Self {
        Self::Experimental
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSummary {
    shape: WireValue,
    #[serde(default)]
    call: Option<WireCall>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCall {
    #[serde(default)]
    captures: Vec<WireCapturedValue>,
    #[serde(default)]
    closed: Vec<WireCallDomain>,
    /// The domains this document proposes closed without claiming closure.
    /// Additive to `schemaVersion: 1`: a document that omits it proposes
    /// nothing, which is what every document said before the key existed.
    #[serde(default, rename = "proposedClosures")]
    proposed_closures: Vec<WireCallDomain>,
    /// ADR 0153 part 3: the package exports whose contexts the claims assume
    /// no value from outside the package for. Additive to `schemaVersion: 1`.
    #[serde(default, rename = "contextPremises")]
    context_premises: Vec<String>,
    /// ADR 0153 item C: the closure hazard sources the closed `reads` holds
    /// against. Additive to `schemaVersion: 1`.
    #[serde(default, rename = "accessorBounds")]
    accessor_bounds: Vec<String>,
    #[serde(default, rename = "callbackResults")]
    callback_results: Vec<WireCallbackResult>,
    #[serde(default)]
    callbacks: Option<Vec<WireCallback>>,
    #[serde(default)]
    reads: Option<Vec<String>>,
    #[serde(default)]
    writes: Option<Vec<String>>,
    #[serde(default)]
    creates: Option<Vec<String>>,
    #[serde(default)]
    invalidates: Option<Vec<String>>,
    #[serde(default)]
    throws: Option<Vec<String>>,
    #[serde(default)]
    returns: Option<Vec<String>>,
    #[serde(default)]
    cleanups: Option<Vec<String>>,
    #[serde(default)]
    disposals: Option<Vec<String>>,
    /// ADR 0114. Never named in `closed`: `WireCallDomain` has no variant for
    /// it, so a document that tries is refused as it decodes.
    #[serde(default)]
    computations: Option<Vec<String>>,
    #[serde(default)]
    operations: Vec<WireOperation>,
    #[serde(default)]
    edges: Vec<WireEdge>,
    #[serde(default)]
    resources: Vec<WireResource>,
    #[serde(default)]
    cases: Option<Vec<WireGuardedCase>>,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum WireCallDomain {
    Callbacks,
    Reads,
    Writes,
    Creates,
    Invalidates,
    Throws,
    Returns,
    Cleanups,
    Disposals,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCallbackResult {
    producer: String,
    shape: WireValue,
    #[serde(default)]
    uses: Option<Vec<String>>,
    #[serde(default)]
    closed: Vec<String>,
    #[serde(default, rename = "callableOnly")]
    callable_only: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCapturedValue {
    id: String,
    from: WireValueSource,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCallback {
    from: WireValueSource,
    operation: String,
}

/// ADR 0152: one nested item of a described callable, `{from, operation}` as a
/// top-level item is, with its operation inline.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDescribedCallback {
    from: WireValueSource,
    operation: WireDescribedInvocation,
}

/// The inline operation of a nested item: an `invoke` operation's own fields,
/// less an id (nothing references it) and less everything validation refuses
/// for a nested item anyway, which `deny_unknown_fields` then refuses here.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDescribedInvocation {
    kind: WireOperationKind,
    #[serde(default)]
    trigger: Option<WireTrigger>,
    #[serde(default)]
    at: Option<WireExecutionPoint>,
    #[serde(default)]
    tracking: Option<WireTracking>,
    #[serde(default)]
    owner: Option<WireOwner>,
    #[serde(default)]
    count: Option<WireCardinality>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireValueSource {
    #[serde(default)]
    capture: Option<String>,
    #[serde(default)]
    arg: Option<u16>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    path: Vec<String>,
    /// ADR 0207: a class of the value's own members, beside `arg` only.
    #[serde(default)]
    members: Option<WireMemberClass>,
}

#[derive(Clone, Copy, Deserialize)]
enum WireMemberClass {
    #[serde(rename = "event-handler-props")]
    EventHandlerProps,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireOperation {
    #[serde(default, deserialize_with = "deserialize_strict_read")]
    strict_read: Option<WireStrictRead>,
    id: String,
    kind: WireOperationKind,
    #[serde(default)]
    guard: Option<WireGuard>,
    #[serde(default)]
    trigger: Option<WireTrigger>,
    #[serde(default)]
    at: Option<WireExecutionPoint>,
    #[serde(default)]
    tracking: Option<WireTracking>,
    #[serde(default)]
    owner: Option<WireOwner>,
    #[serde(default)]
    count: Option<WireCardinality>,
    #[serde(default)]
    inputs: Vec<WireValue>,
    #[serde(default)]
    output: Option<WireValue>,
    #[serde(default)]
    resources: Vec<String>,
    #[serde(default, rename = "composedFrom")]
    composed_from: Option<WireComposedFrom>,
    /// Which protocol of the caller's value an `invoke` runs. Absent is a
    /// call, and a call is never encoded; a decoder that predates the field
    /// refuses the document through `deny_unknown_fields`, which is intended.
    #[serde(default)]
    protocol: Option<WireInvokeProtocol>,
}

#[derive(Clone, Copy, Deserialize)]
enum WireStrictRead {
    #[serde(rename = "cleared")]
    Cleared,
}

// Omission means no assertion; a stated null is not an omitted assertion.
fn deserialize_strict_read<'de, D>(deserializer: D) -> Result<Option<WireStrictRead>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    WireStrictRead::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireInvokeProtocol {
    Call,
    Get,
    Iterate,
    Coerce,
    HasInstance,
    GetEnumerableStringValues,
    GetOwnEnumerableValues,
}

impl WireInvokeProtocol {
    /// The model's one meaning for a call is the absent protocol.
    const fn normalized(self) -> Option<InvokeProtocol> {
        match self {
            Self::Call => None,
            Self::Get => Some(InvokeProtocol::Get),
            Self::Iterate => Some(InvokeProtocol::Iterate),
            Self::Coerce => Some(InvokeProtocol::Coerce),
            Self::HasInstance => Some(InvokeProtocol::HasInstance),
            Self::GetEnumerableStringValues => Some(InvokeProtocol::GetEnumerableStringValues),
            Self::GetOwnEnumerableValues => Some(InvokeProtocol::GetOwnEnumerableValues),
        }
    }
}

/// The `(export, operation)` a composed operation was composed from, inside
/// the same artifact case.
///
/// The operation is named in the document's *local* spelling — `read-0`, the
/// same spelling this export's own operation ids use — and is qualified with
/// the named export by [`IdScope::operation_in`]. A document therefore cannot
/// state a provenance outside its own artifact case at all: the qualification
/// is this side's, not the document's.
#[derive(Clone, Deserialize)]
struct WireComposedFrom {
    export: String,
    operation: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireOperationKind {
    Invoke,
    Return,
    Read,
    Write,
    Invalidate,
    Create,
    Cleanup,
    Dispose,
    Compute,
}

impl From<WireOperationKind> for OperationKind {
    fn from(value: WireOperationKind) -> Self {
        match value {
            WireOperationKind::Invoke => Self::Invoke,
            WireOperationKind::Return => Self::Return,
            WireOperationKind::Read => Self::Read,
            WireOperationKind::Write => Self::Write,
            WireOperationKind::Invalidate => Self::Invalidate,
            WireOperationKind::Create => Self::Create,
            WireOperationKind::Cleanup => Self::Cleanup,
            WireOperationKind::Dispose => Self::Dispose,
            WireOperationKind::Compute => Self::Compute,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTrigger {
    #[serde(default)]
    event: Option<WireEvent>,
    #[serde(default)]
    operation: Option<String>,
    #[serde(default)]
    resource: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireExecutionPoint {
    event: WireEvent,
    schedule: WireSchedule,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireEvent {
    Call,
    Render,
    Flush,
    Settle,
    Transition,
    AsyncEmission,
    Cleanup,
    #[serde(rename = "external-event")]
    External,
    Request,
    ResponseCommitment,
    ResultAccess,
}

impl From<WireEvent> for Event {
    fn from(value: WireEvent) -> Self {
        match value {
            WireEvent::Call => Self::Call,
            WireEvent::Render => Self::Render,
            WireEvent::Flush => Self::Flush,
            WireEvent::Settle => Self::Settle,
            WireEvent::Transition => Self::Transition,
            WireEvent::AsyncEmission => Self::AsyncEmission,
            WireEvent::Cleanup => Self::Cleanup,
            WireEvent::External => Self::External,
            WireEvent::Request => Self::Request,
            WireEvent::ResponseCommitment => Self::ResponseCommitment,
            WireEvent::ResultAccess => Self::ResultAccess,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireSchedule {
    SameStack,
    Queued,
    External,
}

impl From<WireSchedule> for Schedule {
    fn from(value: WireSchedule) -> Self {
        match value {
            WireSchedule::SameStack => Self::SameStack,
            WireSchedule::Queued => Self::Queued,
            WireSchedule::External => Self::External,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireTracking {
    Tracked,
    Untracked,
    AmbientAtExecution,
}

impl From<WireTracking> for Tracking {
    fn from(value: WireTracking) -> Self {
        match value {
            WireTracking::Tracked => Self::Tracked,
            WireTracking::Untracked => Self::Untracked,
            WireTracking::AmbientAtExecution => Self::AmbientAtExecution,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireOwner {
    #[serde(default)]
    source: Option<WireOwnerSource>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    requires: Option<WireRequirement>,
    #[serde(default)]
    requires_children: Option<WireRequirement>,
    #[serde(default)]
    requires_cleanup: Option<WireRequirement>,
    #[serde(default)]
    children: Option<WireChildCapability>,
    #[serde(default)]
    cleanup: Option<WireCleanupCapability>,
    #[serde(default)]
    lifetime: Option<WireLifetime>,
    #[serde(default)]
    closed: Vec<WireOwnerDomain>,
    #[serde(default)]
    productions: Option<Vec<WireOwnerProduction>>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireOwnerSource {
    None,
    AmbientAtCall,
    AmbientAtExecution,
    Captured,
    Created,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireRequirement {
    Required,
    Forbidden,
    Unconstrained,
}

impl From<WireRequirement> for Requirement {
    fn from(value: WireRequirement) -> Self {
        match value {
            WireRequirement::Required => Self::Required,
            WireRequirement::Forbidden => Self::Forbidden,
            WireRequirement::Unconstrained => Self::Unconstrained,
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireChildCapability {
    Allowed,
    Forbidden,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireCleanupCapability {
    Supported,
    Forbidden,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum WireOwnerDomain {
    Productions,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOwnerProduction {
    resource: String,
    #[serde(default)]
    children: Option<WireChildCapability>,
    #[serde(default)]
    cleanup: Option<WireCleanupCapability>,
    #[serde(default)]
    lifetime: Option<WireLifetime>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum WireLifetime {
    Named(WireLifetimeKind),
    Bound(WireBoundLifetime),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBoundLifetime {
    kind: WireLifetimeKind,
    resource: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireLifetimeKind {
    Call,
    Resource,
    Owner,
    Request,
    Transition,
    AsyncSource,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCardinality {
    #[serde(default)]
    scope: Option<WireCardinalityScope>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    min: Option<u32>,
    #[serde(default)]
    max: Option<WireUpperBound>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireCardinalityScope {
    Trigger,
    Call,
    Resource,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(untagged)]
enum WireUpperBound {
    Finite(u32),
    Named(WireMany),
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireMany {
    Many,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEdge {
    kind: WireEdgeKind,
    from: String,
    to: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireEdgeKind {
    Orders,
    Data,
    Invalidates,
    Error,
    Cleanup,
    Lifetime,
}

impl From<WireEdgeKind> for EdgeKind {
    fn from(value: WireEdgeKind) -> Self {
        match value {
            WireEdgeKind::Orders => Self::Orders,
            WireEdgeKind::Data => Self::Data,
            WireEdgeKind::Invalidates => Self::Invalidates,
            WireEdgeKind::Error => Self::Error,
            WireEdgeKind::Cleanup => Self::Cleanup,
            WireEdgeKind::Lifetime => Self::Lifetime,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResource {
    id: String,
    kind: WireResourceKind,
    #[serde(default)]
    closed: Vec<WireResourceDomain>,
    #[serde(default)]
    states: Option<Vec<WireResourceState>>,
    #[serde(default)]
    capabilities: Option<Vec<WireResourceCapability>>,
    #[serde(default)]
    lifetime: Option<WireLifetime>,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum WireResourceKind {
    Owner,
    ReactiveSource,
    AsyncComputation,
    Transition,
    Cleanup,
    Request,
    Response,
    Stream,
    ServerFunctionReference,
}

impl From<WireResourceKind> for ResourceKind {
    fn from(value: WireResourceKind) -> Self {
        match value {
            WireResourceKind::Owner => Self::Owner,
            WireResourceKind::ReactiveSource => Self::ReactiveSource,
            WireResourceKind::AsyncComputation => Self::AsyncComputation,
            WireResourceKind::Transition => Self::Transition,
            WireResourceKind::Cleanup => Self::Cleanup,
            WireResourceKind::Request => Self::Request,
            WireResourceKind::Response => Self::Response,
            WireResourceKind::Stream => Self::Stream,
            WireResourceKind::ServerFunctionReference => Self::ServerFunctionReference,
        }
    }
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum WireResourceDomain {
    States,
    Capabilities,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireResourceState {
    Active,
    Disposed,
    Installed,
    Pending,
    Settled,
    Errored,
    Cancelled,
    Reverted,
    Uncommitted,
    Committed,
    Unclaimed,
    Claimed,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireResourceCapability {
    Refreshable,
    Writable,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGuard {
    all: Vec<WireGuardAtom>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum WireGuardAtom {
    Signature(WireSignatureAtom),
    ArgumentCount(WireArgumentCountAtom),
    Literal(WireLiteralAtom),
    ValueKind(WireValueKindAtom),
    Property(WirePropertyAtom),
    TupleAlternative(WireTupleAlternativeAtom),
    ResultProtocol(WireResultProtocolAtom),
    ArtifactCase(WireArtifactCaseAtom),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSignatureAtom {
    signature: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireArgumentCountAtom {
    argument_count: WireArgumentCount,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireArgumentCount {
    min: u16,
    #[serde(default)]
    max: Option<u16>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLiteralAtom {
    arg: u16,
    #[serde(default)]
    path: Vec<String>,
    literal: serde_json::Value,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireValueKindAtom {
    arg: u16,
    #[serde(default)]
    path: Vec<String>,
    kind: WireGuardValueKind,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePropertyAtom {
    arg: u16,
    #[serde(default)]
    path: Vec<String>,
    property: String,
    #[serde(default)]
    callable: Option<bool>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireTupleAlternativeAtom {
    arg: u16,
    tuple_alternative: u16,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireResultProtocolAtom {
    result_protocol: WireGuardValueKind,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireArtifactCaseAtom {
    artifact_case: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireGuardValueKind {
    Plain,
    Callable,
    Promise,
    AsyncIterable,
}

impl From<WireGuardValueKind> for ValueKind {
    fn from(value: WireGuardValueKind) -> Self {
        match value {
            WireGuardValueKind::Plain => Self::Plain,
            WireGuardValueKind::Callable => Self::Callable,
            WireGuardValueKind::Promise => Self::Promise,
            WireGuardValueKind::AsyncIterable => Self::AsyncIterable,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireGuardedCase {
    #[serde(default)]
    when: Option<WireGuard>,
    #[serde(default)]
    otherwise: Option<bool>,
    #[serde(default)]
    operations: Option<Vec<String>>,
    #[serde(default, rename = "operationsOpen")]
    operations_open: bool,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum WireValue {
    Shorthand(WireValueKind),
    Detailed(WireValueNode),
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireValueKind {
    Unknown,
    Plain,
    Undefined,
    ReadValue,
    Callable,
    Component,
    RefApplication,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WirePrototypePopulation {
    Opaque,
    Values,
    Entries,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePrototypeMember {
    name: String,
    kind: WirePrototypeMemberKind,
    tracks: Vec<WirePrototypeTrack>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WirePrototypeMemberKind {
    Method,
    Getter,
    Iterator,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePrototypeTrack {
    cache: String,
    #[serde(default)]
    argument: Option<u16>,
    #[serde(default)]
    shared: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum WireValueNode {
    Unknown,
    Plain,
    Parameter {
        index: u16,
        #[serde(default)]
        path: Vec<String>,
    },
    Tuple {
        #[serde(default)]
        closed: Vec<WireValueDomain>,
        #[serde(default)]
        items: Option<Vec<WireValue>>,
    },
    Array {
        #[serde(default)]
        element: Option<Box<WireValue>>,
        #[serde(default)]
        length: Option<WireArrayLength>,
    },
    Object {
        #[serde(default)]
        closed: Vec<WireValueDomain>,
        #[serde(default)]
        properties: Option<Vec<WireObjectProperty>>,
    },
    Choice {
        #[serde(default)]
        closed: Vec<WireValueDomain>,
        #[serde(default)]
        alternatives: Option<Vec<WireValue>>,
    },
    Callable,
    Promise {
        #[serde(default)]
        value: Option<Box<WireValue>>,
    },
    AsyncIterable {
        #[serde(default)]
        element: Option<Box<WireValue>>,
    },
    Reactive {
        role: WireReactiveRole,
        #[serde(default)]
        resource: Option<String>,
        #[serde(default)]
        closed: Vec<WireValueDomain>,
        #[serde(default)]
        capabilities: Option<Vec<WireCapabilityClaim>>,
    },
    Store {
        #[serde(default)]
        resource: Option<String>,
        #[serde(default)]
        closed: Vec<WireValueDomain>,
        #[serde(default)]
        capabilities: Option<Vec<WireCapabilityClaim>>,
    },
    /// ADR 0109. `from` is required: a merged-props shape whose argument index
    /// is absent states nothing at all, and defaulting it to zero would name a
    /// parameter the producer never claimed.
    MergedProps {
        from: u16,
    },
    /// ADR 0115. `items` is required for the same reason `from` is: an absent
    /// list would read as `[]`, a claim the producer never made.
    ArgumentArray {
        items: Vec<u16>,
    },
    /// ADR 0116. `parameter` is required: an invocation result that names no
    /// argument states nothing, and defaulting it would name one.
    InvocationResult {
        parameter: u16,
    },
    /// Item B round 2. Exactly `undefined`; it carries no field, and a struct
    /// variant so that `deny_unknown_fields` refuses one it was given (a unit
    /// variant of an internally tagged enum ignores them).
    Undefined {},
    /// ADR 0145. Both lists are required: an absent one would read as `[]`,
    /// the strongest claim each can make. ADR 0152's `callbacks` is the one
    /// exception, and deliberately: every document before it states a
    /// described callable without the field, and meant `callbacks: []` by it,
    /// so absence keeps exactly that meaning.
    DescribedCallable {
        reads: Vec<String>,
        returns: Vec<WireValue>,
        #[serde(default)]
        callbacks: Vec<WireDescribedCallback>,
    },
    /// ADR 0146. Carries no field, as `undefined` does not.
    ReadValue {},
    PrototypeInstance {
        members: Vec<WirePrototypeMember>,
        population: WirePrototypePopulation,
    },
    LazyGetterObject {
        keys: Vec<String>,
        #[serde(default)]
        from: Option<u16>,
    },
    /// ADR 0235: a returned member's own call graph, in the export's id
    /// namespace. Additive to `schemaVersion: 1`.
    EffectfulCallable {
        call: Box<WireCall>,
    },
    ReturnedCallable {
        #[serde(default)]
        call: Option<Box<WireCall>>,
        #[serde(default)]
        members: Vec<WireObjectProperty>,
    },
    Action {
        #[serde(default)]
        transition: Option<String>,
    },
    Component,
    Cleanup {
        #[serde(default)]
        resource: Option<String>,
        #[serde(default)]
        lifetime: Option<WireLifetime>,
    },
    RefApplication,
    ServerFunctionReference {
        #[serde(default)]
        resource: Option<String>,
    },
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum WireValueDomain {
    Items,
    Properties,
    Alternatives,
    Capabilities,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireArrayLength {
    #[serde(default)]
    min: Option<u32>,
    #[serde(default)]
    max: Option<WireUpperBound>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireObjectProperty {
    name: String,
    value: WireValue,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireReactiveRole {
    Accessor,
    Setter,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum WireCapabilityClaim {
    Named(WireObservableCapability),
    Bound(WireBoundCapability),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBoundCapability {
    capability: WireObservableCapability,
    resource: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WireObservableCapability {
    Readable,
    Writable,
    Refreshable,
    PendingAware,
    Optimistic,
}

fn expand(document: WireDocument) -> Result<NormalizedContract, ContractFailure> {
    validate_count("entrypoints", document.entrypoints.len(), MAX_ENTRYPOINTS)?;
    validate_count("summaries", document.summaries.len(), MAX_SUMMARIES)?;
    validate_nonempty(&document.package.name, "package name")?;
    validate_nonempty(&document.package.version, "package version")?;
    validate_nonempty(&document.package.integrity, "package integrity")?;
    validate_path(&document.package.manifest.path)?;

    if let Some(proof) = &document.sidecars.proof {
        parse_wire_digest(&proof.sha256)?;
    }
    if let Some(probes) = &document.sidecars.probes {
        parse_wire_digest(&probes.sha256)?;
    }

    let package = PackageIdentity {
        name: document.package.name,
        version: document.package.version,
        integrity: document.package.integrity,
        manifest: expand_file(&document.package.manifest)?,
    };

    let mut artifact_cases = Vec::new();
    let mut used_summaries = BTreeSet::new();
    let mut effective_exports = 0usize;
    let mut expansion_nodes = 0usize;
    for (entrypoint, definition) in document.entrypoints {
        validate_entrypoint(&entrypoint)?;
        match definition {
            WireEntrypoint::Unconditional(case) => {
                effective_exports = checked_add_exports(effective_exports, case.exports.len())?;
                expansion_nodes =
                    checked_expansion(expansion_nodes, &case.exports, &document.summaries)?;
                artifact_cases.push(expand_artifact_case(
                    &entrypoint,
                    None,
                    case.artifact,
                    case.declarations,
                    case.transform,
                    case.stability,
                    case.initialization,
                    case.exports,
                    &document.summaries,
                    &mut used_summaries,
                )?);
            }
            WireEntrypoint::Conditional(conditional) => {
                if conditional.cases.is_empty() {
                    return invalid_document(format!(
                        "entrypoint {entrypoint:?} has an empty conditional case list"
                    ));
                }
                for case in conditional.cases {
                    effective_exports = checked_add_exports(effective_exports, case.exports.len())?;
                    expansion_nodes =
                        checked_expansion(expansion_nodes, &case.exports, &document.summaries)?;
                    artifact_cases.push(expand_artifact_case(
                        &entrypoint,
                        Some(case.resolution),
                        case.artifact,
                        case.declarations,
                        case.transform,
                        case.stability,
                        case.initialization,
                        case.exports,
                        &document.summaries,
                        &mut used_summaries,
                    )?);
                }
            }
        }
        validate_count("artifact cases", artifact_cases.len(), MAX_ARTIFACT_CASES)?;
    }
    validate_count("effective exports", effective_exports, MAX_EXPORTS)?;
    validate_count(
        "expanded semantic nodes",
        expansion_nodes,
        MAX_EXPANDED_NODES,
    )?;

    if let Some(unused) = document
        .summaries
        .keys()
        .find(|summary| !used_summaries.contains(*summary))
    {
        return invalid_document(format!("summary {unused:?} is never referenced"));
    }

    ContractProposal::new(package, artifact_cases)
        .normalize()
        .map_err(|error| ContractFailure::InvalidSemanticModel {
            reason: error.to_string(),
        })
}

fn checked_add_exports(current: usize, additional: usize) -> Result<usize, ContractFailure> {
    let total =
        current
            .checked_add(additional)
            .ok_or_else(|| ContractFailure::InvalidSemanticModel {
                reason: "effective export count overflowed".into(),
            })?;
    validate_count("effective exports", total, MAX_EXPORTS)?;
    Ok(total)
}

fn checked_expansion(
    current: usize,
    exports: &BTreeMap<String, WireExportReference>,
    summaries: &BTreeMap<String, WireSummary>,
) -> Result<usize, ContractFailure> {
    let mut total = current;
    for reference in exports.values() {
        let (summary, _) = reference.parts();
        let summary =
            summaries
                .get(summary)
                .ok_or_else(|| ContractFailure::InvalidSemanticModel {
                    reason: format!("export references missing summary {summary:?}"),
                })?;
        total = total.checked_add(summary.expansion_cost()).ok_or_else(|| {
            ContractFailure::InvalidSemanticModel {
                reason: "expanded semantic node count overflowed".into(),
            }
        })?;
        validate_count("expanded semantic nodes", total, MAX_EXPANDED_NODES)?;
    }
    Ok(total)
}

impl WireSummary {
    fn expansion_cost(&self) -> usize {
        self.call.as_ref().map_or(1, |call| {
            1usize
                .saturating_add(call.operations.len())
                .saturating_add(call.resources.len())
                .saturating_add(call.edges.len())
                .saturating_add(call.cases.as_ref().map_or(0, Vec::len))
                .saturating_add(call.callback_results.len())
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn expand_artifact_case(
    entrypoint: &str,
    resolution: Option<WireResolution>,
    runtime: WireRuntimeArtifact,
    declarations: WireFile,
    transform: Option<WireFile>,
    stability: Option<WireStability>,
    initialization: Option<WireModuleInitialization>,
    exports: BTreeMap<String, WireExportReference>,
    summaries: &BTreeMap<String, WireSummary>,
    used_summaries: &mut BTreeSet<String>,
) -> Result<ArtifactCase, ContractFailure> {
    validate_path(&runtime.path)?;
    validate_path(&declarations.path)?;
    if let Some(transform) = &transform {
        validate_path(&transform.path)?;
    }
    let runtime_identity = ArtifactIdentity {
        path: runtime.path,
        digest: parse_wire_digest(&runtime.sha256)?,
    };
    let declaration_identity = expand_file(&declarations)?;
    let transform_identity = transform.as_ref().map(expand_file).transpose()?;
    let closure = parse_wire_digest(&runtime.closure_sha256)?;
    let resolution_trace = resolution.as_ref().map_or_else(Vec::new, |resolution| {
        vec![
            ResolutionStep {
                condition: "runtime".into(),
                target: resolution.runtime_branch.clone(),
            },
            ResolutionStep {
                condition: "types".into(),
                target: resolution.types_branch.clone(),
            },
        ]
    });
    let case_id = artifact_case_id(
        entrypoint,
        &resolution_trace,
        &runtime_identity,
        &declaration_identity,
        &closure,
        transform_identity.as_ref(),
    );

    let mut expanded_exports = BTreeMap::new();
    for (public_name, reference) in exports {
        validate_nonempty(&public_name, "public export name")?;
        let (summary_id, export_stability) = reference.parts();
        validate_nonempty(summary_id, "summary reference")?;
        let summary =
            summaries
                .get(summary_id)
                .ok_or_else(|| ContractFailure::InvalidSemanticModel {
                    reason: format!(
                        "export {public_name:?} references missing summary {summary_id:?}"
                    ),
                })?;
        used_summaries.insert(summary_id.to_owned());
        let ids = IdScope::new(&case_id, &public_name);
        expanded_exports.insert(
            public_name.clone(),
            ExportSemantics {
                identity: ExportIdentity {
                    entrypoint: entrypoint.into(),
                    public_name: public_name.clone(),
                    runtime: ExportTargetIdentity {
                        module: runtime_identity.clone(),
                        export_name: public_name.clone(),
                    },
                    declarations: ExportTargetIdentity {
                        module: declaration_identity.clone(),
                        export_name: public_name,
                    },
                },
                shape: expand_value(&summary.shape, &ids)?,
                stability: export_stability,
                call: expand_call(summary.call.as_ref(), &ids)?,
            },
        );
    }

    Ok(ArtifactCase {
        initialization: initialization.map(|value| match value {
            WireModuleInitialization::Inert => ModuleInitializationClaim::Inert,
        }),
        id: case_id,
        entrypoint: entrypoint.into(),
        resolution_trace,
        runtime: runtime_identity,
        declarations: declaration_identity,
        dependency_closure: closure,
        transform: transform_identity,
        stability: stability.map_or(StabilityKnowledge::Unknown, Into::into),
        exports: expanded_exports,
    })
}

fn artifact_case_id(
    entrypoint: &str,
    trace: &[ResolutionStep],
    runtime: &ArtifactIdentity,
    declarations: &ArtifactIdentity,
    closure: &Digest,
    transform: Option<&ArtifactIdentity>,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"solid-checker:artifact-case:v1");
    hash_text(&mut hash, entrypoint);
    for step in trace {
        hash_text(&mut hash, &step.condition);
        hash_text(&mut hash, &step.target);
    }
    hash_artifact(&mut hash, runtime);
    hash_artifact(&mut hash, declarations);
    hash_text(&mut hash, closure.as_str());
    if let Some(transform) = transform {
        hash.update([1]);
        hash_artifact(&mut hash, transform);
    } else {
        hash.update([0]);
    }
    format!("artifact-case:{:x}", hash.finalize())
}

fn hash_artifact(hash: &mut Sha256, artifact: &ArtifactIdentity) {
    hash_text(hash, &artifact.path);
    hash_text(hash, artifact.digest.as_str());
}

fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hash.update(value.as_bytes());
}

struct IdScope {
    case: String,
    prefix: String,
}

impl IdScope {
    fn new(case: &str, export: &str) -> Self {
        Self {
            case: case.into(),
            prefix: format!("{case}:{export}"),
        }
    }

    fn operation(&self, id: &str) -> OperationId {
        OperationId(format!("{}:operation:{id}", self.prefix))
    }

    /// The qualified id of an operation of *another* export of the same
    /// artifact case.
    ///
    /// The case comes from this scope, never from the document, so a stated
    /// provenance is confined to the artifact case whose document stated it.
    fn operation_in(&self, export: &str, id: &str) -> OperationId {
        OperationId(format!("{}:{export}:operation:{id}", self.case))
    }

    fn resource(&self, id: &str) -> ResourceId {
        ResourceId(format!("{}:resource:{id}", self.prefix))
    }
}

fn expand_file(file: &WireFile) -> Result<ArtifactIdentity, ContractFailure> {
    validate_path(&file.path)?;
    Ok(ArtifactIdentity {
        path: file.path.clone(),
        digest: parse_wire_digest(&file.sha256)?,
    })
}

fn parse_wire_digest(value: &str) -> Result<Digest, ContractFailure> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid_document("sha256 fields must contain exactly 64 hexadecimal digits");
    }
    Digest::parse(format!("sha256:{value}")).map_err(|error| {
        ContractFailure::InvalidSemanticModel {
            reason: error.to_string(),
        }
    })
}

fn validate_nonempty(value: &str, field: &str) -> Result<(), ContractFailure> {
    if value.is_empty() {
        invalid_document(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_path(value: &str) -> Result<(), ContractFailure> {
    validate_nonempty(value, "package-relative path")?;
    if value.len() > MAX_PATH_BYTES {
        return invalid_document(format!(
            "package-relative path exceeds {MAX_PATH_BYTES} bytes"
        ));
    }
    let windows_absolute = value.as_bytes().get(1) == Some(&b':')
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    let has_parent = value.split(['/', '\\']).any(|component| component == "..");
    if Path::new(value).is_absolute()
        || Path::new(value)
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
        || value.starts_with('\\')
        || windows_absolute
        || has_parent
    {
        return invalid_document(format!(
            "path {value:?} is not confined to the package root"
        ));
    }
    Ok(())
}

fn validate_entrypoint(value: &str) -> Result<(), ContractFailure> {
    if value == "." {
        return Ok(());
    }
    let Some(path) = value.strip_prefix("./") else {
        return invalid_document(format!(
            "entrypoint {value:?} must be the package root or a package subpath"
        ));
    };
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return invalid_document(format!(
            "entrypoint {value:?} contains a non-canonical or traversing segment"
        ));
    }
    Ok(())
}

fn validate_count(name: &str, actual: usize, limit: usize) -> Result<(), ContractFailure> {
    if actual > limit {
        invalid_document(format!("{name} count {actual} exceeds limit {limit}"))
    } else {
        Ok(())
    }
}

fn validate_closed<T: Ord + Copy>(
    closed: &[T],
    allowed: &[T],
    path: &str,
) -> Result<BTreeSet<T>, ContractFailure> {
    let mut domains = BTreeSet::new();
    for domain in closed {
        if !allowed.contains(domain) {
            return invalid_document(format!("{path} closes a non-local domain"));
        }
        if !domains.insert(*domain) {
            return invalid_document(format!("{path} contains a duplicate closed domain"));
        }
    }
    Ok(domains)
}

fn knowledge<T>(
    items: Option<Vec<T>>,
    closed: bool,
    path: &str,
) -> Result<KnowledgeSet<T>, ContractFailure> {
    match (items, closed) {
        (None, false) => Ok(KnowledgeSet::Unknown),
        (None, true) => invalid_document(format!("closed domain {path} omits its collection")),
        (Some(items), true) => Ok(KnowledgeSet::Complete(items)),
        (Some(items), false) if items.is_empty() => {
            invalid_document(format!("open domain {path} has an empty collection"))
        }
        (Some(items), false) => Ok(KnowledgeSet::Partial(items)),
    }
}

fn expand_call(
    call: Option<&WireCall>,
    ids: &IdScope,
) -> Result<solid_reactive_ir::contract_semantics::CallSemantics, ContractFailure> {
    let call = call.cloned().unwrap_or_default();
    validate_count("operations", call.operations.len(), MAX_OPERATIONS)?;
    validate_count("captures", call.captures.len(), MAX_OPERATIONS)?;
    validate_count(
        "callback result censuses",
        call.callback_results.len(),
        MAX_OPERATIONS,
    )?;
    validate_count("resources", call.resources.len(), MAX_RESOURCES)?;
    validate_count("edges", call.edges.len(), MAX_EDGES)?;
    validate_count(
        "guard cases",
        call.cases.as_ref().map_or(0, Vec::len),
        MAX_GUARD_CASES,
    )?;
    let closed = validate_closed(&call.closed, &WireCallDomain::ALL, "call.closed")?;
    let proposed = validate_closed(
        &call.proposed_closures,
        &WireCallDomain::ALL,
        "call.proposedClosures",
    )?;
    let mut premises = BTreeSet::new();
    for export in &call.context_premises {
        if export.is_empty() || !premises.insert(export.clone()) {
            return invalid_document("call.contextPremises must name distinct, non-empty exports");
        }
    }
    let mut bounds = BTreeSet::new();
    for source in &call.accessor_bounds {
        if source.is_empty() || !bounds.insert(source.clone()) {
            return invalid_document(
                "call.accessorBounds must name distinct, non-empty hazard sources",
            );
        }
    }

    let callbacks = call
        .callbacks
        .map(|callbacks| {
            callbacks
                .into_iter()
                .map(|callback| expand_callback(callback, ids))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let claims = solid_reactive_ir::contract_semantics::CallClaims {
        callbacks: knowledge(
            callbacks,
            closed.contains(&WireCallDomain::Callbacks),
            "call.callbacks",
        )?,
        reads: operation_knowledge(call.reads, WireCallDomain::Reads, &closed, ids)?,
        writes: operation_knowledge(call.writes, WireCallDomain::Writes, &closed, ids)?,
        creates: operation_knowledge(call.creates, WireCallDomain::Creates, &closed, ids)?,
        invalidates: operation_knowledge(
            call.invalidates,
            WireCallDomain::Invalidates,
            &closed,
            ids,
        )?,
        throws: operation_knowledge(call.throws, WireCallDomain::Throws, &closed, ids)?,
        returns: operation_knowledge(call.returns, WireCallDomain::Returns, &closed, ids)?,
        cleanups: operation_knowledge(call.cleanups, WireCallDomain::Cleanups, &closed, ids)?,
        disposals: operation_knowledge(call.disposals, WireCallDomain::Disposals, &closed, ids)?,
        computations: knowledge(
            call.computations
                .map(|items| items.into_iter().map(|id| ids.operation(&id)).collect()),
            false,
            "call operation claim",
        )?,
    };

    let operations = call
        .operations
        .into_iter()
        .map(|operation| expand_operation(operation, ids))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = call
        .edges
        .into_iter()
        .map(|edge| OperationEdge {
            kind: edge.kind.into(),
            from: ids.operation(&edge.from),
            to: ids.operation(&edge.to),
        })
        .collect();
    let resources = call
        .resources
        .into_iter()
        .map(|resource| expand_resource(resource, ids))
        .collect::<Result<Vec<_>, _>>()?;
    let guards = expand_guard_partition(call.cases, ids)?;
    Ok(solid_reactive_ir::contract_semantics::CallSemantics::new(
        claims, operations, edges, resources, guards,
    )
    // Whether the proposal is admissible at all — the domain open, and the
    // domain one the certifier has a proof mode for — is a normalization
    // invariant, refused by name in `validate_proposed_closures`.
    .with_proposed_closures(proposed.into_iter().map(ClaimDomain::from))
    .with_context_premises(
        premises
            .into_iter()
            .map(|export| solid_reactive_ir::contract_semantics::ContextPremise { export }),
    )
    // Whether a bound is admissible -- `reads` closed -- is a normalization
    // invariant, refused by name in `validate_accessor_bounds`.
    .with_accessor_bounds(bounds)
    .with_captures(
        call.captures
            .into_iter()
            .map(|capture| {
                Ok(solid_reactive_ir::contract_semantics::CapturedValue {
                    id: capture.id,
                    from: expand_value_source(capture.from, ids)?,
                })
            })
            .collect::<Result<Vec<_>, ContractFailure>>()?,
    )
    .with_callback_results(
        call.callback_results
            .into_iter()
            .map(|result| {
                if result.closed.iter().any(|domain| domain != "uses") || result.closed.len() > 1 {
                    return invalid_model("a callback result may close only uses, once");
                }
                if result
                    .callable_only
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != result.callable_only.len()
                {
                    return invalid_model("duplicate callableOnly result use");
                }
                Ok(solid_reactive_ir::contract_semantics::CallbackResult {
                    producer: ids.operation(&result.producer),
                    callable_only: result
                        .callable_only
                        .into_iter()
                        .map(|id| ids.operation(&id))
                        .collect(),
                    shape: expand_value(&result.shape, ids)?,
                    uses: knowledge(
                        result
                            .uses
                            .map(|items| items.into_iter().map(|id| ids.operation(&id)).collect()),
                        !result.closed.is_empty(),
                        "callback result uses",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, ContractFailure>>()?,
    ))
}

impl From<WireCallDomain> for ClaimDomain {
    fn from(value: WireCallDomain) -> Self {
        match value {
            WireCallDomain::Callbacks => Self::Callbacks,
            WireCallDomain::Reads => Self::Reads,
            WireCallDomain::Writes => Self::Writes,
            WireCallDomain::Creates => Self::Creates,
            WireCallDomain::Invalidates => Self::Invalidates,
            WireCallDomain::Throws => Self::Throws,
            WireCallDomain::Returns => Self::Returns,
            WireCallDomain::Cleanups => Self::Cleanups,
            WireCallDomain::Disposals => Self::Disposals,
        }
    }
}

impl WireCallDomain {
    const ALL: [Self; 9] = [
        Self::Callbacks,
        Self::Reads,
        Self::Writes,
        Self::Creates,
        Self::Invalidates,
        Self::Throws,
        Self::Returns,
        Self::Cleanups,
        Self::Disposals,
    ];
}

fn operation_knowledge(
    items: Option<Vec<String>>,
    domain: WireCallDomain,
    closed: &BTreeSet<WireCallDomain>,
    ids: &IdScope,
) -> Result<KnowledgeSet<OperationId>, ContractFailure> {
    knowledge(
        items.map(|items| items.into_iter().map(|id| ids.operation(&id)).collect()),
        closed.contains(&domain),
        "call operation claim",
    )
}

fn expand_callback(
    callback: WireCallback,
    ids: &IdScope,
) -> Result<CallbackInvocation, ContractFailure> {
    let from = expand_value_source(callback.from, ids)?;
    Ok(CallbackInvocation {
        from,
        operation: ids.operation(&callback.operation),
    })
}

/// ADR 0152: a nested item, through the same expansions a top-level `invoke`
/// operation's fields go through. Only an `invoke` is an item; validation
/// then admits the one invocation the census proves.
fn expand_described_callback(
    callback: &WireDescribedCallback,
    ids: &IdScope,
) -> Result<solid_reactive_ir::contract_semantics::DescribedCallback, ContractFailure> {
    let operation = callback.operation.clone();
    if !matches!(operation.kind, WireOperationKind::Invoke) {
        return invalid_document("a described callable's callback item is an invoke operation");
    }
    let (at, schedule) = operation.at.map_or((None, None), |at| {
        (Some(at.event.into()), Some(at.schedule.into()))
    });
    Ok(solid_reactive_ir::contract_semantics::DescribedCallback {
        from: expand_value_source(callback.from.clone(), ids)?,
        trigger: operation
            .trigger
            .map(|trigger| expand_trigger(trigger, ids))
            .transpose()?,
        at,
        schedule,
        tracking: operation.tracking.map_or(Tracking::Unknown, Into::into),
        owner: operation
            .owner
            .map(|owner| expand_owner(owner, ids))
            .transpose()?
            .unwrap_or_default(),
        cardinality: operation
            .count
            .map(|count| expand_cardinality(count, ids))
            .transpose()?
            .unwrap_or_default(),
    })
}

fn expand_value_source(
    source: WireValueSource,
    ids: &IdScope,
) -> Result<ValueSource, ContractFailure> {
    if let Some(capture) = source.capture {
        if source.arg.is_some()
            || source.operation.is_some()
            || source.resource.is_some()
            || source.members.is_some()
        {
            return invalid_document("a capture source cannot name another source");
        }
        validate_nonempty(&capture, "capture id")?;
        return Ok(ValueSource::Capture {
            capture,
            path: source.path,
        });
    }
    if let Some(class) = source.members {
        let (Some(index), None, None) = (source.arg, &source.operation, &source.resource) else {
            return invalid_document("a callback source's members class requires arg");
        };
        return Ok(ValueSource::ParameterMembers {
            index,
            path: source.path,
            class: match class {
                WireMemberClass::EventHandlerProps => MemberClass::EventHandlerProps,
            },
        });
    }
    match (source.arg, source.operation, source.resource) {
        (Some(index), None, None) => Ok(ValueSource::Parameter {
            index,
            path: source.path,
        }),
        (None, Some(operation), None) => Ok(ValueSource::OperationOutput {
            operation: ids.operation(&operation),
            path: source.path,
        }),
        (None, None, Some(resource)) => Ok(ValueSource::Resource {
            resource: ids.resource(&resource),
            path: source.path,
        }),
        _ => {
            invalid_document("callback source must name exactly one of arg, operation, or resource")
        }
    }
}

fn expand_operation(operation: WireOperation, ids: &IdScope) -> Result<Operation, ContractFailure> {
    validate_nonempty(&operation.id, "operation id")?;
    let mut resources = BTreeSet::new();
    for resource in operation.resources {
        if !resources.insert(ids.resource(&resource)) {
            return invalid_document(format!(
                "operation {:?} contains duplicate resource {resource:?}",
                operation.id
            ));
        }
    }
    let trigger = operation
        .trigger
        .map(|trigger| expand_trigger(trigger, ids))
        .transpose()?;
    let (at, schedule) = operation.at.map_or((None, None), |at| {
        (Some(at.event.into()), Some(at.schedule.into()))
    });
    let owner = operation
        .owner
        .map(|owner| expand_owner(owner, ids))
        .transpose()?
        .unwrap_or_default();
    let cardinality = operation
        .count
        .map(|count| expand_cardinality(count, ids))
        .transpose()?
        .unwrap_or_default();
    Ok(Operation {
        id: ids.operation(&operation.id),
        kind: operation.kind.into(),
        guard: operation
            .guard
            .map(|guard| expand_guard(guard, ids))
            .transpose()?,
        trigger,
        at,
        schedule,
        tracking: operation.tracking.map_or(Tracking::Unknown, Into::into),
        strict_read: operation.strict_read.map(|strict_read| match strict_read {
            WireStrictRead::Cleared => StrictRead::Cleared,
        }),
        owner,
        cardinality,
        inputs: operation
            .inputs
            .iter()
            .map(|value| expand_value(value, ids))
            .collect::<Result<Vec<_>, _>>()?,
        output: operation
            .output
            .as_ref()
            .map(|value| expand_value(value, ids))
            .transpose()?,
        resources,
        composed_from: operation.composed_from.as_ref().map(|composed| {
            solid_reactive_ir::contract_semantics::ComposedFrom {
                export: composed.export.clone(),
                operation: ids.operation_in(&composed.export, &composed.operation),
            }
        }),
        protocol: operation.protocol.and_then(WireInvokeProtocol::normalized),
    })
}

fn expand_trigger(trigger: WireTrigger, ids: &IdScope) -> Result<Trigger, ContractFailure> {
    match (trigger.event, trigger.operation, trigger.resource) {
        (Some(event), None, None) => Ok(Trigger::Event(event.into())),
        (None, Some(operation), None) => Ok(Trigger::Operation(ids.operation(&operation))),
        (Some(event), None, Some(resource)) => Ok(Trigger::Resource {
            resource: ids.resource(&resource),
            event: event.into(),
        }),
        _ => invalid_document(
            "trigger must name exactly an event, an operation, or a resource plus event",
        ),
    }
}

fn expand_owner(owner: WireOwner, ids: &IdScope) -> Result<OwnerRelation, ContractFailure> {
    let source = match (owner.source, owner.resource.as_deref()) {
        (None, None) => OwnerSource::Unknown,
        (None, Some(_)) => return invalid_document("owner resource requires an owner source"),
        (Some(WireOwnerSource::None), None) => OwnerSource::None,
        (Some(WireOwnerSource::AmbientAtCall), None) => OwnerSource::AmbientAtCall,
        (Some(WireOwnerSource::AmbientAtExecution), None) => OwnerSource::AmbientAtExecution,
        (Some(WireOwnerSource::Captured), Some(resource)) => {
            OwnerSource::Captured(ids.resource(resource))
        }
        (Some(WireOwnerSource::Created), Some(resource)) => {
            OwnerSource::Created(ids.resource(resource))
        }
        _ => {
            return invalid_document(
                "captured/created owner sources require resource; other sources forbid it",
            );
        }
    };
    let closed = validate_closed(
        &owner.closed,
        &[WireOwnerDomain::Productions],
        "owner.closed",
    )?;
    let implicit = owner.resource.as_deref();
    let productions = owner
        .productions
        .map(|productions| {
            productions
                .into_iter()
                .map(|production| {
                    let resource = ids.resource(&production.resource);
                    Ok(OwnerProduction {
                        capabilities: OwnerCapabilities {
                            child_owners: expand_child_capability(production.children),
                            cleanup: expand_cleanup_capability(production.cleanup),
                        },
                        lifetime: production
                            .lifetime
                            .as_ref()
                            .map(|lifetime| {
                                expand_lifetime(lifetime, Some(&production.resource), ids)
                            })
                            .transpose()?,
                        resource,
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()
        })
        .transpose()?;
    Ok(OwnerRelation {
        source,
        requirements: OwnerRequirements {
            owner: owner
                .requires
                .map_or(Requirement::Unconstrained, Into::into),
            child_owners: owner
                .requires_children
                .map_or(Requirement::Unconstrained, Into::into),
            cleanup: owner
                .requires_cleanup
                .map_or(Requirement::Unconstrained, Into::into),
        },
        capabilities: OwnerCapabilities {
            child_owners: expand_child_capability(owner.children),
            cleanup: expand_cleanup_capability(owner.cleanup),
        },
        lifetime: owner
            .lifetime
            .as_ref()
            .map(|lifetime| expand_lifetime(lifetime, implicit, ids))
            .transpose()?,
        productions: knowledge(
            productions,
            closed.contains(&WireOwnerDomain::Productions),
            "owner.productions",
        )?,
    })
}

fn expand_child_capability(value: Option<WireChildCapability>) -> CapabilityKnowledge {
    match value {
        None => CapabilityKnowledge::Unknown,
        Some(WireChildCapability::Allowed) => CapabilityKnowledge::Allowed,
        Some(WireChildCapability::Forbidden) => CapabilityKnowledge::Forbidden,
    }
}

fn expand_cleanup_capability(value: Option<WireCleanupCapability>) -> CapabilityKnowledge {
    match value {
        None => CapabilityKnowledge::Unknown,
        Some(WireCleanupCapability::Supported) => CapabilityKnowledge::Allowed,
        Some(WireCleanupCapability::Forbidden) => CapabilityKnowledge::Forbidden,
    }
}

fn expand_lifetime(
    lifetime: &WireLifetime,
    implicit_resource: Option<&str>,
    ids: &IdScope,
) -> Result<Lifetime, ContractFailure> {
    let (kind, resource) = match lifetime {
        WireLifetime::Named(WireLifetimeKind::Call) => return Ok(Lifetime::Call),
        WireLifetime::Named(kind) => (*kind, implicit_resource),
        WireLifetime::Bound(bound) if matches!(bound.kind, WireLifetimeKind::Call) => {
            return invalid_document("call lifetime must not name a resource");
        }
        WireLifetime::Bound(bound) => (bound.kind, Some(bound.resource.as_str())),
    };
    let resource = resource.ok_or_else(|| ContractFailure::DocumentDecode {
        message: "resource-bound lifetime requires an exact resource".into(),
    })?;
    let resource = ids.resource(resource);
    Ok(match kind {
        WireLifetimeKind::Call => Lifetime::Call,
        WireLifetimeKind::Resource => Lifetime::Resource(resource),
        WireLifetimeKind::Owner => Lifetime::Owner(resource),
        WireLifetimeKind::Request => Lifetime::Request(resource),
        WireLifetimeKind::Transition => Lifetime::Transition(resource),
        WireLifetimeKind::AsyncSource => Lifetime::AsyncSource(resource),
    })
}

fn expand_cardinality(
    count: WireCardinality,
    ids: &IdScope,
) -> Result<Cardinality, ContractFailure> {
    let scope = match (count.scope, count.resource) {
        (None, None) => None,
        (None, Some(_)) => return invalid_document("cardinality resource requires resource scope"),
        (Some(WireCardinalityScope::Trigger), None) => Some(CardinalityScope::Trigger),
        (Some(WireCardinalityScope::Call), None) => Some(CardinalityScope::Call),
        (Some(WireCardinalityScope::Resource), Some(resource)) => {
            Some(CardinalityScope::Resource(ids.resource(&resource)))
        }
        _ => return invalid_document("only resource cardinality scope accepts a resource"),
    };
    let max = count.max.map(|max| match max {
        WireUpperBound::Finite(max) => UpperBound::Finite(max),
        WireUpperBound::Named(WireMany::Many) => UpperBound::Many,
    });
    Ok(Cardinality {
        scope,
        min: count.min,
        max,
    })
}

fn expand_resource(resource: WireResource, ids: &IdScope) -> Result<Resource, ContractFailure> {
    validate_nonempty(&resource.id, "resource id")?;
    let closed = validate_closed(
        &resource.closed,
        &[WireResourceDomain::States, WireResourceDomain::Capabilities],
        "resource.closed",
    )?;
    let states = resource
        .states
        .map(|states| {
            states
                .into_iter()
                .map(|state| expand_resource_state(resource.kind, state))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let capabilities = resource.capabilities.map(|capabilities| {
        capabilities
            .into_iter()
            .map(|capability| match capability {
                WireResourceCapability::Refreshable => ResourceCapability::Refreshable,
                WireResourceCapability::Writable => ResourceCapability::Writable,
            })
            .collect()
    });
    Ok(Resource {
        id: ids.resource(&resource.id),
        kind: resource.kind.into(),
        states: knowledge(
            states,
            closed.contains(&WireResourceDomain::States),
            "resource.states",
        )?,
        capabilities: knowledge(
            capabilities,
            closed.contains(&WireResourceDomain::Capabilities),
            "resource.capabilities",
        )?,
        lifetime: resource
            .lifetime
            .as_ref()
            .map(|lifetime| expand_lifetime(lifetime, Some(&resource.id), ids))
            .transpose()?,
    })
}

fn expand_resource_state(
    kind: WireResourceKind,
    state: WireResourceState,
) -> Result<ResourceState, ContractFailure> {
    match (kind, state) {
        (WireResourceKind::Owner, WireResourceState::Active) => Ok(ResourceState::OwnerActive),
        (WireResourceKind::Owner, WireResourceState::Disposed) => Ok(ResourceState::OwnerDisposed),
        (WireResourceKind::Cleanup, WireResourceState::Installed) => {
            Ok(ResourceState::CleanupInstalled)
        }
        (WireResourceKind::Cleanup, WireResourceState::Disposed) => {
            Ok(ResourceState::CleanupDisposed)
        }
        (WireResourceKind::AsyncComputation, WireResourceState::Pending) => {
            Ok(ResourceState::AsyncPending)
        }
        (WireResourceKind::AsyncComputation, WireResourceState::Settled) => {
            Ok(ResourceState::AsyncSettled)
        }
        (WireResourceKind::AsyncComputation, WireResourceState::Errored) => {
            Ok(ResourceState::AsyncErrored)
        }
        (WireResourceKind::AsyncComputation, WireResourceState::Cancelled) => {
            Ok(ResourceState::AsyncCancelled)
        }
        (WireResourceKind::Transition, WireResourceState::Active) => {
            Ok(ResourceState::TransitionActive)
        }
        (WireResourceKind::Transition, WireResourceState::Settled) => {
            Ok(ResourceState::TransitionSettled)
        }
        (WireResourceKind::Transition, WireResourceState::Reverted) => {
            Ok(ResourceState::TransitionReverted)
        }
        (WireResourceKind::Response, WireResourceState::Uncommitted) => {
            Ok(ResourceState::ResponseUncommitted)
        }
        (WireResourceKind::Response, WireResourceState::Committed) => {
            Ok(ResourceState::ResponseCommitted)
        }
        (WireResourceKind::Stream, WireResourceState::Unclaimed) => {
            Ok(ResourceState::StreamUnclaimed)
        }
        (WireResourceKind::Stream, WireResourceState::Claimed) => Ok(ResourceState::StreamClaimed),
        _ => invalid_model("resource state is incompatible with its resource kind"),
    }
}

fn expand_guard_partition(
    cases: Option<Vec<WireGuardedCase>>,
    ids: &IdScope,
) -> Result<GuardPartition, ContractFailure> {
    let Some(cases) = cases else {
        return Ok(GuardPartition {
            cases: KnowledgeSet::Unknown,
        });
    };
    if cases.is_empty() {
        return Ok(GuardPartition {
            cases: KnowledgeSet::Complete(Vec::new()),
        });
    }
    validate_count("guard cases", cases.len(), MAX_GUARD_CASES)?;
    let complete = cases.iter().any(|case| case.otherwise == Some(true));
    let mut expanded = Vec::with_capacity(cases.len());
    for case in cases {
        if case.operations_open && case.operations.is_none() {
            return invalid_document("operationsOpen requires a non-empty operations array");
        }
        let operations = match case.operations {
            Some(operations) if case.operations_open => KnowledgeSet::partial(
                operations
                    .into_iter()
                    .map(|operation| ids.operation(&operation))
                    .collect(),
            )
            .ok_or_else(|| ContractFailure::DocumentDecode {
                message: "operationsOpen requires a non-empty operations array".into(),
            })?,
            Some(operations) => KnowledgeSet::Complete(
                operations
                    .into_iter()
                    .map(|operation| ids.operation(&operation))
                    .collect(),
            ),
            None => KnowledgeSet::Unknown,
        };
        match (case.when, case.otherwise) {
            (Some(guard), None) => expanded.push(GuardedCase::When {
                guard: expand_guard(guard, ids)?,
                operations,
            }),
            (None, Some(true)) => expanded.push(GuardedCase::Otherwise { operations }),
            _ => {
                return invalid_document("guard case must contain exactly when or otherwise: true");
            }
        }
    }
    Ok(GuardPartition {
        cases: if complete {
            KnowledgeSet::Complete(expanded)
        } else {
            KnowledgeSet::Partial(expanded)
        },
    })
}

fn expand_guard(guard: WireGuard, ids: &IdScope) -> Result<Guard, ContractFailure> {
    validate_count("guard atoms", guard.all.len(), MAX_GUARD_ATOMS)?;
    if guard.all.is_empty() {
        return invalid_document("guard must contain at least one atom");
    }
    Ok(Guard(
        guard
            .all
            .into_iter()
            .map(|atom| expand_guard_atom(atom, ids))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn expand_guard_atom(atom: WireGuardAtom, _ids: &IdScope) -> Result<GuardAtom, ContractFailure> {
    match atom {
        WireGuardAtom::Signature(atom) => Ok(GuardAtom::Signature(atom.signature)),
        WireGuardAtom::ArgumentCount(atom) => Ok(GuardAtom::ArgumentCount {
            min: atom.argument_count.min,
            max: atom.argument_count.max,
        }),
        WireGuardAtom::Literal(atom) => Ok(GuardAtom::Literal {
            argument: atom.arg,
            path: atom.path,
            value: expand_literal(atom.literal)?,
        }),
        WireGuardAtom::ValueKind(atom) => Ok(GuardAtom::ValueKind {
            argument: atom.arg,
            path: atom.path,
            kind: atom.kind.into(),
        }),
        WireGuardAtom::Property(atom) => Ok(GuardAtom::Property {
            argument: atom.arg,
            path: atom.path,
            name: atom.property,
            callable: atom.callable,
        }),
        WireGuardAtom::TupleAlternative(atom) => Ok(GuardAtom::TupleAlternative {
            argument: atom.arg,
            alternative: atom.tuple_alternative,
        }),
        WireGuardAtom::ResultProtocol(atom) => {
            Ok(GuardAtom::ResultProtocol(atom.result_protocol.into()))
        }
        WireGuardAtom::ArtifactCase(atom) => Ok(GuardAtom::ArtifactCase(atom.artifact_case)),
    }
}

fn expand_literal(value: serde_json::Value) -> Result<Literal, ContractFailure> {
    match value {
        serde_json::Value::Null => Ok(Literal::Null),
        serde_json::Value::Bool(value) => Ok(Literal::Bool(value)),
        serde_json::Value::Number(value) => Ok(Literal::Number(value.to_string())),
        serde_json::Value::String(value) => Ok(Literal::String(value)),
        _ => invalid_document("guard literals must be null, boolean, number, or string"),
    }
}

fn expand_value(value: &WireValue, ids: &IdScope) -> Result<ValueShape, ContractFailure> {
    expand_value_at(value, ids, 0)
}

fn expand_value_at(
    value: &WireValue,
    ids: &IdScope,
    depth: usize,
) -> Result<ValueShape, ContractFailure> {
    if depth > MAX_RECURSIVE_DEPTH {
        return invalid_document(format!(
            "recursive value depth exceeds {MAX_RECURSIVE_DEPTH}"
        ));
    }
    match value {
        WireValue::Shorthand(kind) => Ok(match kind {
            WireValueKind::Unknown => ValueShape::Unknown,
            WireValueKind::Plain => ValueShape::Plain,
            WireValueKind::Undefined => ValueShape::Undefined,
            WireValueKind::ReadValue => ValueShape::ReadValue,
            WireValueKind::Callable => ValueShape::Callable,
            WireValueKind::Component => ValueShape::Component,
            WireValueKind::RefApplication => ValueShape::RefApplication,
        }),
        WireValue::Detailed(node) => expand_value_node(node, ids, depth),
    }
}

fn expand_value_node(
    node: &WireValueNode,
    ids: &IdScope,
    depth: usize,
) -> Result<ValueShape, ContractFailure> {
    match node {
        WireValueNode::Unknown => Ok(ValueShape::Unknown),
        WireValueNode::Plain => Ok(ValueShape::Plain),
        WireValueNode::Parameter { index, path } => Ok(ValueShape::Parameter {
            index: *index,
            path: path.clone(),
        }),
        WireValueNode::Tuple { closed, items } => {
            let closed = validate_closed(closed, &[WireValueDomain::Items], "tuple.closed")?;
            Ok(ValueShape::Tuple(knowledge(
                items
                    .as_ref()
                    .map(|items| {
                        items
                            .iter()
                            .map(|item| expand_value_at(item, ids, depth + 1))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?,
                closed.contains(&WireValueDomain::Items),
                "tuple.items",
            )?))
        }
        WireValueNode::Array { element, length } => Ok(ValueShape::Array {
            element: Box::new(
                element
                    .as_ref()
                    .map(|element| expand_value_at(element, ids, depth + 1))
                    .transpose()?
                    .unwrap_or(ValueShape::Unknown),
            ),
            length: length
                .as_ref()
                .map_or_else(ArrayLength::default, |length| ArrayLength {
                    min: length.min,
                    max: length.max.map(|max| match max {
                        WireUpperBound::Finite(max) => UpperBound::Finite(max),
                        WireUpperBound::Named(WireMany::Many) => UpperBound::Many,
                    }),
                }),
        }),
        WireValueNode::Object { closed, properties } => {
            let closed = validate_closed(closed, &[WireValueDomain::Properties], "object.closed")?;
            let properties = properties
                .as_ref()
                .map(|properties| {
                    let mut names = BTreeSet::new();
                    properties
                        .iter()
                        .map(|property| {
                            validate_nonempty(&property.name, "object property name")?;
                            if !names.insert(&property.name) {
                                return invalid_document(format!(
                                    "object contains duplicate property {:?}",
                                    property.name
                                ));
                            }
                            Ok(ObjectProperty {
                                name: property.name.clone(),
                                value: expand_value_at(&property.value, ids, depth + 1)?,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?;
            Ok(ValueShape::Object(knowledge(
                properties,
                closed.contains(&WireValueDomain::Properties),
                "object.properties",
            )?))
        }
        WireValueNode::Choice {
            closed,
            alternatives,
        } => {
            let closed =
                validate_closed(closed, &[WireValueDomain::Alternatives], "choice.closed")?;
            Ok(ValueShape::Choice(knowledge(
                alternatives
                    .as_ref()
                    .map(|alternatives| {
                        alternatives
                            .iter()
                            .map(|alternative| expand_value_at(alternative, ids, depth + 1))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?,
                closed.contains(&WireValueDomain::Alternatives),
                "choice.alternatives",
            )?))
        }
        WireValueNode::Callable => Ok(ValueShape::Callable),
        WireValueNode::Promise { value } => Ok(ValueShape::Promise(Box::new(
            value
                .as_ref()
                .map(|value| expand_value_at(value, ids, depth + 1))
                .transpose()?
                .unwrap_or(ValueShape::Unknown),
        ))),
        WireValueNode::AsyncIterable { element } => Ok(ValueShape::AsyncIterable(Box::new(
            element
                .as_ref()
                .map(|element| expand_value_at(element, ids, depth + 1))
                .transpose()?
                .unwrap_or(ValueShape::Unknown),
        ))),
        WireValueNode::Reactive {
            role,
            resource,
            closed,
            capabilities,
        } => Ok(ValueShape::Reactive {
            role: match role {
                WireReactiveRole::Accessor => ReactiveRole::Accessor,
                WireReactiveRole::Setter => ReactiveRole::Setter,
            },
            resource: resource.as_ref().map(|resource| ids.resource(resource)),
            capabilities: expand_capabilities(
                closed,
                capabilities.as_ref(),
                resource.as_deref(),
                ids,
            )?,
        }),
        WireValueNode::Store {
            resource,
            closed,
            capabilities,
        } => Ok(ValueShape::Store {
            resource: resource.as_ref().map(|resource| ids.resource(resource)),
            capabilities: expand_capabilities(
                closed,
                capabilities.as_ref(),
                resource.as_deref(),
                ids,
            )?,
        }),
        WireValueNode::MergedProps { from } => Ok(ValueShape::MergedProps { from: *from }),
        WireValueNode::ArgumentArray { items } => Ok(ValueShape::ArgumentArray {
            items: items.clone(),
        }),
        WireValueNode::InvocationResult { parameter } => Ok(ValueShape::InvocationResult {
            parameter: *parameter,
        }),
        WireValueNode::Undefined {} => Ok(ValueShape::Undefined),
        WireValueNode::ReadValue {} => Ok(ValueShape::ReadValue),
        WireValueNode::PrototypeInstance {
            members,
            population,
        } => Ok(ValueShape::PrototypeInstance {
            population: match population {
                WirePrototypePopulation::Opaque => {
                    solid_reactive_ir::contract_semantics::PrototypePopulation::Opaque
                }
                WirePrototypePopulation::Values => {
                    solid_reactive_ir::contract_semantics::PrototypePopulation::Values
                }
                WirePrototypePopulation::Entries => {
                    solid_reactive_ir::contract_semantics::PrototypePopulation::Entries
                }
            },
            members: members
                .iter()
                .map(
                    |member| solid_reactive_ir::contract_semantics::PrototypeMember {
                        name: member.name.clone(),
                        kind: match member.kind {
                            WirePrototypeMemberKind::Method => {
                                solid_reactive_ir::contract_semantics::PrototypeMemberKind::Method
                            }
                            WirePrototypeMemberKind::Getter => {
                                solid_reactive_ir::contract_semantics::PrototypeMemberKind::Getter
                            }
                            WirePrototypeMemberKind::Iterator => {
                                solid_reactive_ir::contract_semantics::PrototypeMemberKind::Iterator
                            }
                        },
                        tracks: member
                            .tracks
                            .iter()
                            .map(
                                |track| solid_reactive_ir::contract_semantics::PrototypeTrack {
                                    cache: track.cache.clone(),
                                    argument: track.argument,
                                    shared: track.shared.clone(),
                                },
                            )
                            .collect(),
                    },
                )
                .collect(),
        }),
        WireValueNode::LazyGetterObject { keys, from } => Ok(ValueShape::LazyGetterObject {
            keys: keys.clone(),
            from: *from,
        }),
        WireValueNode::EffectfulCallable { call } => Ok(ValueShape::EffectfulCallable(Box::new(
            expand_call(Some(call), ids)?,
        ))),
        WireValueNode::ReturnedCallable { call, members } => Ok(ValueShape::ReturnedCallable {
            call: call
                .as_ref()
                .map(|call| expand_call(Some(call), ids).map(Box::new))
                .transpose()?,
            members: members
                .iter()
                .map(|member| {
                    Ok(ObjectProperty {
                        name: member.name.clone(),
                        value: expand_value_at(&member.value, ids, depth + 1)?,
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()?,
        }),
        WireValueNode::DescribedCallable {
            reads,
            returns,
            callbacks,
        } => Ok(ValueShape::DescribedCallable(Box::new(
            solid_reactive_ir::contract_semantics::DescribedCall {
                reads: reads
                    .iter()
                    .map(|read| {
                        solid_reactive_ir::contract_semantics::DescribedRead::from_wire(read)
                            .ok_or_else(|| ContractFailure::DocumentDecode {
                                message: format!("unknown described read {read:?}"),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                returns: returns
                    .iter()
                    .map(|returned| expand_value_at(returned, ids, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?,
                callbacks: callbacks
                    .iter()
                    .map(|callback| expand_described_callback(callback, ids))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        ))),
        WireValueNode::Action { transition } => Ok(ValueShape::Action {
            transition: transition.as_ref().map(|resource| ids.resource(resource)),
        }),
        WireValueNode::Component => Ok(ValueShape::Component),
        WireValueNode::Cleanup { resource, lifetime } => Ok(ValueShape::Cleanup {
            resource: resource.as_ref().map(|resource| ids.resource(resource)),
            lifetime: lifetime
                .as_ref()
                .map(|lifetime| expand_lifetime(lifetime, resource.as_deref(), ids))
                .transpose()?,
        }),
        WireValueNode::RefApplication => Ok(ValueShape::RefApplication),
        WireValueNode::ServerFunctionReference { resource } => {
            Ok(ValueShape::ServerFunctionReference {
                resource: resource.as_ref().map(|resource| ids.resource(resource)),
            })
        }
    }
}

fn expand_capabilities(
    closed: &[WireValueDomain],
    capabilities: Option<&Vec<WireCapabilityClaim>>,
    implicit_resource: Option<&str>,
    ids: &IdScope,
) -> Result<KnowledgeSet<CapabilityClaim>, ContractFailure> {
    let closed = validate_closed(closed, &[WireValueDomain::Capabilities], "value.closed")?;
    let capabilities = capabilities
        .map(|capabilities| {
            capabilities
                .iter()
                .map(|claim| {
                    let (capability, resource, explicitly_bound) = match claim {
                        WireCapabilityClaim::Named(capability) => {
                            (*capability, implicit_resource, false)
                        }
                        WireCapabilityClaim::Bound(claim) => {
                            (claim.capability, Some(claim.resource.as_str()), true)
                        }
                    };
                    let capability = match capability {
                        WireObservableCapability::Readable => ObservableCapability::Readable,
                        WireObservableCapability::Writable => ObservableCapability::Writable,
                        WireObservableCapability::Refreshable => ObservableCapability::Refreshable,
                        WireObservableCapability::PendingAware => {
                            ObservableCapability::PendingAware
                        }
                        WireObservableCapability::Optimistic => ObservableCapability::Optimistic,
                    };
                    let intrinsic = matches!(
                        capability,
                        ObservableCapability::Readable | ObservableCapability::Writable
                    );
                    if intrinsic && explicitly_bound {
                        return invalid_document(
                            "readable and writable capabilities must not name a resource",
                        );
                    }
                    Ok(CapabilityClaim {
                        capability,
                        resource: if intrinsic {
                            None
                        } else {
                            Some(ids.resource(resource.ok_or_else(|| {
                                ContractFailure::DocumentDecode {
                                    message: "resource-bound value capability requires a resource"
                                        .into(),
                                }
                            })?))
                        },
                    })
                })
                .collect::<Result<Vec<_>, ContractFailure>>()
        })
        .transpose()?;
    knowledge(
        capabilities,
        closed.contains(&WireValueDomain::Capabilities),
        "value.capabilities",
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn prototype_instance_round_trips_binds_digest_and_refuses_certification() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/reactive-ir/package-prototype-instance-consumer/node_modules/reactive-package/solid-reactivity.json"
        ));
        let kept = normalized(bytes);
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert_eq!(normalized(&encoded), kept);
        let Err(refusal) = solid_reactive_ir::contract_semantics::certification::proof_policy_2()
            .inspect_candidates(&kept)
        else {
            panic!("prototype recipe certified without a census")
        };
        assert!(refusal.to_string().contains("authored-only"));
        let mut document: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let output = &mut document["summaries"]["set-instance"]["call"]["operations"][0]["output"];
        output["members"][0]["tracks"][0]["cache"] = json!("other-cache");
        let changed = normalized(&serde_json::to_vec(&document).unwrap());
        assert_ne!(changed.semantic_digest(), kept.semantic_digest());
        document["summaries"]["set-instance"]["call"]["operations"][0]["output"]["members"][0]["extra"] =
            json!(true);
        assert!(decode(&serde_json::to_vec(&document).unwrap()).is_err());
        let mut nested: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let output = &mut nested["summaries"]["set-instance"]["call"]["operations"][0]["output"];
        *output = json!({"kind": "tuple", "closed": ["tuple-items"], "items": [output.clone()]});
        // A prototype-instance value nested in a tuple is refused, whether the
        // wire decoder or normalization is the stage that refuses it.
        assert!(
            decode(&serde_json::to_vec(&nested).unwrap())
                .map_or(true, |document| document.normalize().is_err())
        );
    }
    use super::*;

    const MINIMAL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../benchmarks/package-contract-v2/phase6/minimal-unknown.json"
    ));
    const SIGNAL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../benchmarks/package-contract-v2/phase6/signal-pair-complete.json"
    ));
    const CONDITIONAL: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../benchmarks/package-contract-v2/phase6/conditional-owned-effect.json"
    ));
    /// The generator fixture that publishes a composed `read` operation.
    ///
    /// Included as a golden so `composedFrom` is exercised in **both**
    /// directions by the round-trip below: `WireComposedFrom` and
    /// `IdScope::operation_in` on the way in, `CompactIds::operation_in` on
    /// the way out. A field that only ever encoded, or only ever decoded,
    /// would pass every other test in this module.
    const COMPOSED: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/package-contracts/composed-operation-provenance/expected.json"
    ));

    /// The property artifact admission leans on: the address this computes for
    /// an export is the very `summary-…` id the encoder writes for it.
    ///
    /// That is what makes it a valid comparator between two *different*
    /// certifications of one published artifact. The encoder hashes the compact
    /// form, whose operation spelling is local (`callback-0`) because
    /// `CompactIds` strips the artifact-case qualification back off — so the
    /// address depends on what the export claims and on nothing about where it
    /// was found. The decoded form cannot be compared directly at all:
    /// `IdScope::operation_in` qualifies every operation id with the
    /// artifact-case id, which is what `@kobalte/utils@0.9.2`'s two `.` cases
    /// differ in, for 13 of their 59 exports and in nothing else.
    ///
    /// Asserted against the *encoded* document rather than the fixture bytes:
    /// these fixtures name their summaries by hand (`"signal-pair"`), and it is
    /// the encoder's id this has to reproduce.
    #[test]
    fn an_export_claims_address_is_the_summary_id_the_encoder_writes() {
        for bytes in [SIGNAL, CONDITIONAL, COMPOSED] {
            let decoded = decode(bytes).unwrap();
            let sidecars = decoded.sidecar_digests().unwrap();
            let contract = decoded.normalize().unwrap();
            let encoded = encode(&contract, &sidecars, false).unwrap();
            let wire: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
            let case = &contract.artifact_cases()[0];
            let stated = wire["entrypoints"][&case.entrypoint].clone();
            let exports = stated
                .get("cases")
                .and_then(|cases| cases.get(0))
                .unwrap_or(&stated)["exports"]
                .clone();
            assert!(!case.exports.is_empty());
            for (name, export) in &case.exports {
                let reference = &exports[name];
                // An experimental export states `{summary, stability}`; every
                // other one states the id directly.
                let stated_id = reference
                    .get("summary")
                    .unwrap_or(reference)
                    .as_str()
                    .expect("the encoded document names this export's summary");
                assert_eq!(
                    export_claims_address(case, name, export).unwrap(),
                    stated_id,
                    "{name} must re-derive the address the encoder wrote"
                );
            }
        }
    }

    /// And it moves when the claim moves, or admission would apply one of two
    /// contracts that say different things about one artifact.
    #[test]
    fn an_export_claims_address_changes_with_the_claim() {
        let contract = normalized(SIGNAL);
        let case = &contract.artifact_cases()[0];
        let (name, export) = case.exports.iter().next().expect("an export");
        let before = export_claims_address(case, name, export).unwrap();
        let mut altered = export.clone();
        altered.shape = ValueShape::Unknown;
        assert_ne!(
            export_claims_address(case, name, &altered).unwrap(),
            before,
            "a different shape is a different claim"
        );
    }

    fn normalized(bytes: &[u8]) -> NormalizedContract {
        decode(bytes).unwrap().normalize().unwrap()
    }

    #[test]
    fn minimal_document_normalizes() {
        let bytes = br#"{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{"name":"solid-js","version":"2.0.0-rc.3","integrity":"sha512:test","manifest":{"path":"package.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"summaries":{"plain":{"shape":"plain"}},"entrypoints":{".":{"artifact":{"path":"dist/solid.js","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","closureSha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"},"declarations":{"path":"types/index.d.ts","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},"exports":{"version":"plain"}}},"sidecars":{}}"#;
        let contract = decode(bytes).unwrap().normalize().unwrap();
        assert_eq!(contract.semantic_model_version(), SEMANTIC_MODEL_VERSION);
        assert_eq!(contract.artifact_cases().len(), 1);
    }

    #[test]
    fn stable_decoder_rejects_temporary_v2_and_legacy_v1() {
        let temporary = br#"{"format":"solid-reactivity-contract","schemaVersion":2,"semanticModelVersion":1,"package":{"name":"solid-js","version":"2.0.0-rc.3","integrity":"sha512:test","manifest":{"path":"package.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},"summaries":{"plain":{"shape":"plain"}},"entrypoints":{".":{"artifact":{"path":"dist/solid.js","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","closureSha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"},"declarations":{"path":"types/index.d.ts","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},"exports":{"version":"plain"}}},"sidecars":{}}"#;
        assert!(matches!(
            decode(temporary),
            Err(ContractFailure::UnsupportedSchemaVersion {
                actual: 2,
                expected: 1
            })
        ));

        let legacy = br#"{"schemaVersion":1,"package":{"name":"example","version":"1.0.0"},"entrypoints":{}}"#;
        assert!(matches!(
            decode(legacy),
            Err(ContractFailure::DocumentDecode { .. })
        ));
    }

    #[test]
    fn all_goldens_round_trip_through_identical_normalized_semantics() {
        for bytes in [MINIMAL, SIGNAL, CONDITIONAL, COMPOSED] {
            let first = normalized(bytes);
            let encoded = encode(&first, &SidecarDigests::default(), true).unwrap();
            let second = normalized(&encoded);
            assert_eq!(first, second);
            assert_eq!(
                encoded,
                encode(&second, &SidecarDigests::default(), true).unwrap()
            );
        }
    }

    #[test]
    fn inert_initialization_is_explicit_round_tripped_and_identity_bound() {
        let mut document: JsonValue = serde_json::from_slice(MINIMAL).unwrap();
        document["entrypoints"]["."]["exports"] = json!({});
        document["summaries"] = json!({});
        let absent = normalized(&serde_json::to_vec(&document).unwrap());
        document["entrypoints"]["."]["initialization"] = json!("inert");
        let inert = normalized(&serde_json::to_vec(&document).unwrap());
        assert_eq!(
            inert.artifact_cases()[0].initialization,
            Some(ModuleInitializationClaim::Inert)
        );
        assert_ne!(absent.semantic_digest(), inert.semantic_digest());
        let encoded = encode(&inert, &SidecarDigests::default(), true).unwrap();
        assert_eq!(inert, normalized(&encoded));
        for unsupported in [JsonValue::Null, json!(false), json!("unknown"), json!({})] {
            document["entrypoints"]["."]["initialization"] = unsupported;
            assert!(decode(&serde_json::to_vec(&document).unwrap()).is_err());
        }
        document["entrypoints"]["."]
            .as_object_mut()
            .unwrap()
            .remove("initialization");
        assert_eq!(absent, normalized(&serde_json::to_vec(&document).unwrap()));
    }

    /// The composed golden really carries the field, and it survives the
    /// round trip as the same `(export, operation)` pair.
    ///
    /// Without this the round-trip above would still pass if both directions
    /// dropped `composedFrom` together.
    #[test]
    fn composed_provenance_survives_the_round_trip_in_both_directions() {
        let normalized_contract = normalized(COMPOSED);
        let composed = normalized_contract.artifact_cases()[0]
            .exports
            .values()
            .flat_map(|export| export.call.operations.iter())
            .filter_map(|operation| operation.composed_from.as_ref())
            .collect::<Vec<_>>();
        // Two rows in that fixture compose the same target: the export that
        // only calls it, and the export that also reads its own signal.
        assert_eq!(composed.len(), 2, "{composed:?}");
        for provenance in composed {
            assert_eq!(provenance.export, "readsItsOwnSignal");
            // The qualified id is this side's, built from the artifact case
            // and the named export — a document states only the local
            // `read-0`.
            assert!(
                provenance
                    .operation
                    .0
                    .ends_with(":readsItsOwnSignal:operation:read-0"),
                "{:?}",
                provenance.operation
            );
        }
        let encoded = encode(&normalized_contract, &SidecarDigests::default(), true).unwrap();
        assert!(
            String::from_utf8_lossy(&encoded).contains("\"composedFrom\""),
            "the encoder dropped the provenance"
        );
        assert_eq!(normalized(&encoded), normalized_contract);
    }

    /// The generator fixture that publishes non-call `callbacks` items.
    const PROTOCOLS: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../fixtures/package-contracts/implementation-census-described-accessor/expected.json"
    ));

    /// Item A of ways-to-improve § 3.3: `protocol` on an `invoke` survives the
    /// round trip in both directions; a stated `call` is the absent protocol
    /// and is never written back; an unknown spelling, and a protocol on any
    /// other kind of operation, refuse the document.
    #[test]
    fn an_invoke_protocol_survives_the_round_trip_and_call_is_never_written() {
        let normalized_contract = normalized(PROTOCOLS);
        let protocols = normalized_contract.artifact_cases()[0]
            .exports
            .iter()
            .flat_map(|(name, export)| {
                export
                    .call
                    .operations
                    .iter()
                    .filter_map(move |operation| Some((name.as_str(), operation.protocol?)))
            })
            .collect::<Vec<_>>();
        assert!(
            protocols.contains(&("access", InvokeProtocol::Get))
                && protocols.contains(&("compare", InvokeProtocol::Coerce)),
            "{protocols:?}"
        );
        assert!(
            protocols
                .iter()
                .all(|(_, protocol)| *protocol != InvokeProtocol::Call)
        );
        let encoded = encode(&normalized_contract, &SidecarDigests::default(), true).unwrap();
        let text = String::from_utf8_lossy(&encoded);
        assert!(
            text.contains("\"protocol\": \"get\""),
            "the encoder dropped the protocol"
        );
        assert!(!text.contains("\"protocol\": \"call\""));
        assert_eq!(normalized(&encoded), normalized_contract);

        // A stated `call` decodes to the absent protocol.
        let stated_call = serde_json::to_vec(&protocols_rewritten("get", &json!("call"))).unwrap();
        let decoded = decode(&stated_call).unwrap().normalize().unwrap();
        assert!(
            decoded.artifact_cases()[0].exports["access"]
                .call
                .operations
                .iter()
                .all(|operation| operation.protocol.is_none())
        );
        // An unknown spelling refuses.
        let unknown = serde_json::to_vec(&protocols_rewritten("get", &json!("read"))).unwrap();
        assert!(decode(&unknown).is_err());
        // A protocol on another kind of operation is refused by the model.
        let mut on_return = protocols_rewritten("get", &json!("get"));
        for summary in on_return["summaries"].as_object_mut().unwrap().values_mut() {
            for operation in summary
                .get_mut("call")
                .and_then(|call| call.get_mut("operations"))
                .and_then(JsonValue::as_array_mut)
                .into_iter()
                .flatten()
            {
                if operation["kind"] == json!("return") {
                    operation["protocol"] = json!("coerce");
                }
            }
        }
        let refused = decode(&serde_json::to_vec(&on_return).unwrap())
            .and_then(|decoded| decoded.normalize())
            .expect_err("only an invoke states a protocol");
        assert!(
            refused
                .to_string()
                .contains("only an invoke operation may state a protocol"),
            "{refused}"
        );
    }

    #[test]
    fn value_enumerations_round_trip_with_exact_counts_and_distinct_identity() {
        let broad = normalized(PROTOCOLS);
        for (protocol, min) in [
            ("get-enumerable-string-values", 1),
            ("get-own-enumerable-values", 0),
        ] {
            let mut document = protocols_rewritten("get", &json!(protocol));
            for summary in document["summaries"].as_object_mut().unwrap().values_mut() {
                for operation in summary["call"]["operations"]
                    .as_array_mut()
                    .into_iter()
                    .flatten()
                {
                    if operation["protocol"] == json!(protocol) {
                        operation["trigger"] = json!({"event": "call"});
                        operation["at"] = json!({"event": "call", "schedule": "same-stack"});
                        operation["owner"] = json!({"source": "ambient-at-execution"});
                        operation["count"] = json!({"scope": "call", "min": min, "max": 1});
                    }
                }
            }
            let bytes = serde_json::to_vec(&document).unwrap();
            let kept = normalized(&bytes);
            assert_ne!(kept.semantic_digest(), broad.semantic_digest());
            assert_eq!(
                normalized(&encode(&kept, &SidecarDigests::default(), true).unwrap()),
                kept
            );
            for (field, bad) in [
                ("count", json!({"scope": "call", "min": min, "max": "many"})),
                ("tracking", json!("untracked")),
                ("owner", json!({"source": "none"})),
                ("trigger", json!({"event": "result-access"})),
                ("guard", json!({"all": [{"argumentCount": {"min": 1}}]})),
            ] {
                let mut refused = document.clone();
                for summary in refused["summaries"].as_object_mut().unwrap().values_mut() {
                    for operation in summary["call"]["operations"]
                        .as_array_mut()
                        .into_iter()
                        .flatten()
                    {
                        if operation["protocol"] == json!(protocol) {
                            operation[field] = bad.clone();
                        }
                    }
                }
                assert!(
                    decode(&serde_json::to_vec(&refused).unwrap())
                        .and_then(|value| value.normalize())
                        .is_err(),
                    "{protocol}: {field}"
                );
            }
        }
    }

    /// The protocol golden with every `"protocol": from` replaced by `to`.
    fn protocols_rewritten(from: &str, to: &JsonValue) -> JsonValue {
        let mut document: JsonValue = serde_json::from_slice(PROTOCOLS).unwrap();
        for summary in document["summaries"].as_object_mut().unwrap().values_mut() {
            for operation in summary
                .get_mut("call")
                .and_then(|call| call.get_mut("operations"))
                .and_then(JsonValue::as_array_mut)
                .into_iter()
                .flatten()
            {
                if operation.get("protocol") == Some(&json!(from)) {
                    operation["protocol"] = to.clone();
                }
            }
        }
        document
    }

    /// `proposedClosures` labels a closure the document states, so it names a
    /// subset of `closed`.
    ///
    /// Decoding must keep the closure and the label together, refuse a label
    /// over a domain the document leaves open, refuse a duplicate the way
    /// `closed` is refused, refuse a domain no census can decide, and refuse a
    /// spelling outside the vocabulary instead of ignoring it.
    #[test]
    fn a_proposed_closure_labels_a_stated_closure_and_is_otherwise_refused() {
        let document = |call: &str| {
            format!(
                r#"{{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{{"name":"consumer","version":"1.0.0","integrity":"sha512:test","manifest":{{"path":"package.json","sha256":"{a}"}}}},"summaries":{{"fn":{{"shape":"callable","call":{call}}}}},"entrypoints":{{".":{{"artifact":{{"path":"dist/index.js","sha256":"{b}","closureSha256":"{c}"}},"declarations":{{"path":"dist/index.d.ts","sha256":"{d}"}},"exports":{{"run":"fn"}}}}}},"sidecars":{{}}}}"#,
                a = "a".repeat(64),
                b = "b".repeat(64),
                c = "c".repeat(64),
                d = "d".repeat(64),
            )
            .into_bytes()
        };

        // A closure over a *positive* operation, which is the shape a real
        // package produces and the corpus fixtures never do. `@corvu/utils`'
        // `contains` reads `.contains` off parameter 0, so its `reads` is
        // `Complete([read-0])` rather than an absence — and certifying that
        // package produced an accepted document with no closures at all, no
        // withheld records and no refusals (§ 14 of
        // `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
        //
        // This is the first half of that: does a complete positive survive the
        // decode/normalize round trip the certifier's candidate universe is
        // rebuilt from? The operation is copied from the real document.
        const POSITIVE_READ: &str = r#"{"closed":["reads"],"reads":["read-0"],"proposedClosures":["reads"],"operations":[{"at":{"event":"call","schedule":"same-stack"},"count":{"max":"many","min":0,"scope":"call"},"id":"read-0","inputs":[{"index":0,"kind":"parameter","path":["contains"]}],"kind":"read","tracking":"untracked","trigger":{"event":"call"}}]}"#;
        let positive = normalized(&document(POSITIVE_READ));
        let read_export = &positive.artifact_cases()[0].exports["run"];
        assert!(
            !read_export.claim_state(ClaimDomain::Reads).is_open(),
            "a closure over one positive read is a closure: {:?}",
            read_export.claim_state(ClaimDomain::Reads)
        );
        assert_eq!(
            read_export.call.proposed_closures(),
            &BTreeSet::from([ClaimDomain::Reads]),
            "and it is still labelled as proposed"
        );
        let reencoded = encode(&positive, &SidecarDigests::default(), false).unwrap();
        assert!(
            String::from_utf8_lossy(&reencoded).contains(r#""closed":["reads"]"#),
            "and it survives re-encoding: {}",
            String::from_utf8_lossy(&reencoded)
        );

        let contract = normalized(&document(
            r#"{"closed":["creates"],"creates":[],"proposedClosures":["creates"]}"#,
        ));
        let export = &contract.artifact_cases()[0].exports["run"];
        assert!(
            export
                .operation_claim(ClaimDomain::Creates)
                .unwrap()
                .proves_absence(),
            "the closure is the document's own claim"
        );
        assert_eq!(
            export.call.proposed_closures(),
            &BTreeSet::from([ClaimDomain::Creates]),
            "and it is labelled as proposed rather than reviewed"
        );
        // And back out again, byte for byte in both directions.
        let encoded = encode(&contract, &SidecarDigests::default(), false).unwrap();
        assert!(String::from_utf8_lossy(&encoded).contains(r#""proposedClosures":["creates"]"#));
        assert_eq!(normalized(&encoded), contract);

        for (call, needle) in [
            (r#"{"proposedClosures":["creates"]}"#, "states no closure"),
            (
                r#"{"closed":["creates"],"creates":[],"proposedClosures":["creates","creates"]}"#,
                "duplicate closed domain",
            ),
            // `writes`, not `reads`: `reads` became proposable on 2026-09-10
            // and this row needs a domain that still has no closure proof
            // mode. See `ClaimDomain::PROPOSABLE`.
            (
                r#"{"closed":["writes"],"writes":[],"proposedClosures":["writes"]}"#,
                "no closure proof mode",
            ),
        ] {
            let error = decode(&document(call))
                .and_then(|proposal| proposal.normalize())
                .expect_err(&format!("{call} must be refused"));
            assert!(
                error.to_string().contains(needle),
                "{call}: {error} does not name {needle:?}"
            );
        }

        assert!(
            decode(&document(r#"{"proposedClosures":["nonsense"]}"#)).is_err(),
            "an unknown domain spelling must be refused, not ignored"
        );
    }

    /// ADR 0139: a `result-access` item survives the round trip, puts the
    /// document in a digest family of its own, and is refused in any shape but
    /// its one.
    #[test]
    fn a_result_access_item_round_trips_in_its_own_digest_family() {
        let document = |operation: &str, callbacks: &str| {
            format!(
                r#"{{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{{"name":"consumer","version":"1.0.0","integrity":"sha512:test","manifest":{{"path":"package.json","sha256":"{a}"}}}},"summaries":{{"fn":{{"shape":"callable","call":{{"callbacks":{callbacks},"closed":["callbacks"],"operations":[{operation}]}}}}}},"entrypoints":{{".":{{"artifact":{{"path":"dist/index.js","sha256":"{b}","closureSha256":"{c}"}},"declarations":{{"path":"dist/index.d.ts","sha256":"{d}"}},"exports":{{"Keeper":"fn"}}}}}},"sidecars":{{}}}}"#,
                a = "a".repeat(64),
                b = "b".repeat(64),
                c = "c".repeat(64),
                d = "d".repeat(64),
            )
            .into_bytes()
        };
        const KEPT: &str = r#"{"at":{"event":"result-access","schedule":"external"},"count":{"max":"many","min":0,"scope":"trigger"},"id":"callback-0","kind":"invoke","owner":{"closed":["productions"],"productions":[],"source":"ambient-at-execution"},"tracking":"ambient-at-execution","trigger":{"event":"result-access"}}"#;
        const ITEM: &str = r#"[{"from":{"arg":1,"path":[]},"operation":"callback-0"}]"#;
        let kept = normalized(&document(KEPT, ITEM));
        let operation = &kept.artifact_cases()[0].exports["Keeper"].call.operations[0];
        assert!(operation.is_result_access(), "{operation:?}");
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert!(
            String::from_utf8_lossy(&encoded).contains("\"event\": \"result-access\""),
            "the encoder dropped the event"
        );
        assert_eq!(normalized(&encoded), kept);
        // Its own family: the same item at the external event is another
        // document with another digest.
        let external = normalized(&document(
            &KEPT
                .replace(
                    "\"result-access\",\"schedule\"",
                    "\"external-event\",\"schedule\"",
                )
                .replace(
                    "\"trigger\":{\"event\":\"result-access\"}",
                    "\"trigger\":{\"event\":\"external-event\"}",
                ),
            ITEM,
        ));
        assert_ne!(external.semantic_digest(), kept.semantic_digest());
        // Any other shape refuses.
        for (operation, callbacks, needle) in [
            (
                KEPT.replace("\"schedule\":\"external\"", "\"schedule\":\"queued\""),
                ITEM.to_owned(),
                "on an external schedule",
            ),
            (
                KEPT.replace(
                    "\"tracking\":\"ambient-at-execution\"",
                    "\"tracking\":\"untracked\"",
                ),
                ITEM.to_owned(),
                "ambient-at-execution and unconstrained",
            ),
            (
                KEPT.to_owned(),
                ITEM.replace("\"path\":[]", "\"path\":[\"handler\"]"),
                "exactly one callbacks item from a bare parameter",
            ),
        ] {
            let refused = decode(&document(&operation, &callbacks))
                .and_then(|decoded| decoded.normalize())
                .expect_err("the shape is refused");
            assert!(
                refused.to_string().contains(needle),
                "{needle:?}: {refused}"
            );
        }
        assert!(
            decode(&document(
                &KEPT.replace("result-access", "result-read"),
                ITEM
            ))
            .is_err(),
            "an unknown event spelling is refused, not ignored"
        );
    }

    #[test]
    fn callback_result_censuses_round_trip_and_refuse_unknown_closed_domains() {
        let original = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/reactive-ir/package-own-tracked-read-consumer/node_modules/reactive-package/solid-reactivity.json"
        ));
        let mut document: JsonValue = serde_json::from_slice(original).unwrap();
        let call = &mut document["summaries"]["summary-watch-status"]["call"];
        call["callbacks"] = json!([
            {"from":{"arg":0},"operation":"producer"},
            {"from":{"operation":"producer"},"operation":"use-result"}
        ]);
        call["operations"].as_array_mut().unwrap().extend([
            json!({"id":"producer","kind":"invoke","at":{"event":"call","schedule":"same-stack"},
                "tracking":"ambient-at-execution","owner":{"source":"ambient-at-execution"},"count":{"scope":"call","min":1,"max":1}}),
            json!({"id":"use-result","kind":"invoke","protocol":"coerce","at":{"event":"call","schedule":"same-stack"},
                "tracking":"ambient-at-execution","owner":{"source":"ambient-at-execution"},"count":{"scope":"call","min":0,"max":"many"}}),
        ]);
        call["edges"] = json!([{"kind":"data","from":"producer","to":"use-result"}]);
        call["callbackResults"] = json!([{"producer":"producer","shape":"unknown","uses":["use-result"],"closed":["uses"]}]);
        let kept = decode(&serde_json::to_vec(&document).unwrap())
            .unwrap()
            .normalize()
            .unwrap();
        let emitted = encode(
            &kept,
            &SidecarDigests {
                proof: None,
                probes: None,
            },
            false,
        )
        .unwrap();
        let again = decode(&emitted).unwrap().normalize().unwrap();
        assert_eq!(kept.semantic_digest(), again.semantic_digest());
        assert_eq!(
            again.artifact_cases()[0].exports["watchStatus"]
                .call
                .callback_results()
                .len(),
            1
        );
        document["summaries"]["summary-watch-status"]["call"]["callbackResults"][0]["closed"] =
            json!(["recursive"]);
        assert!(
            decode(&serde_json::to_vec(&document).unwrap())
                .and_then(|decoded| decoded.normalize())
                .is_err()
        );
    }

    #[test]
    fn strict_read_clearing_round_trips_and_refuses_other_contexts() {
        let document = |tracking: Option<&str>, strict_read: Option<&str>, kind: &str| {
            let mut operation = json!({"id":"read", "kind":kind,
                "trigger":{"event":"call"}, "at":{"event":"call","schedule":"same-stack"},
                "inputs":[{"kind":"reactive","role":"accessor"}]});
            if let Some(tracking) = tracking {
                operation["tracking"] = json!(tracking);
            }
            if let Some(strict_read) = strict_read {
                operation["strictRead"] = json!(strict_read);
            }
            serde_json::to_vec(&json!({"format":"solid-reactivity-contract", "schemaVersion":1,
                "semanticModelVersion":1, "package":{"name":"consumer","version":"1.0.0",
                    "integrity":"sha512:test","manifest":{"path":"package.json","sha256":"a".repeat(64)}},
                "summaries":{"fn":{"shape":"callable","call":{"closed":["reads"],
                    "reads":["read"], "operations":[operation]}}},
                "entrypoints":{".":{"artifact":{"path":"index.js","sha256":"b".repeat(64),
                    "closureSha256":"c".repeat(64)},"declarations":{"path":"index.d.ts","sha256":"d".repeat(64)},
                    "exports":{"make":"fn"}}},"sidecars":{}})).unwrap()
        };
        let plain = normalized(&document(Some("untracked"), None, "read"));
        let kept = normalized(&document(Some("untracked"), Some("cleared"), "read"));
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert!(String::from_utf8_lossy(&encoded).contains("strictRead"));
        assert_eq!(normalized(&encoded), kept);
        assert_ne!(plain.semantic_digest(), kept.semantic_digest());
        let encoded_plain = encode(&plain, &SidecarDigests::default(), true).unwrap();
        assert!(!String::from_utf8_lossy(&encoded_plain).contains("strictRead"));
        for tracking in [None, Some("tracked"), Some("ambient-at-execution")] {
            assert!(
                decode(&document(tracking, Some("cleared"), "read"))
                    .and_then(|decoded| decoded.normalize())
                    .is_err()
            );
        }
        assert!(decode(&document(Some("untracked"), Some("unknown"), "read")).is_err());
        let null = String::from_utf8(document(Some("untracked"), Some("cleared"), "read"))
            .unwrap()
            .replace(r#""strictRead":"cleared""#, r#""strictRead":null"#);
        assert!(decode(null.as_bytes()).is_err());
        assert!(
            decode(&document(Some("untracked"), Some("cleared"), "write"))
                .and_then(|decoded| decoded.normalize())
                .is_err()
        );
        assert!(
            solid_reactive_ir::contract_semantics::certification::proof_policy_2()
                .inspect_candidates(&kept)
                .is_err()
        );
    }

    #[test]
    fn lazy_getter_recipe_round_trips_and_refuses_certification() {
        let document = json!({
            "format": "solid-reactivity-contract", "schemaVersion": 1,
            "semanticModelVersion": 1,
            "package": {"name": "consumer", "version": "1.0.0", "integrity": "sha512:test",
                "manifest": {"path": "package.json", "sha256": "a".repeat(64)}},
            "summaries": {"fn": {"shape": "callable", "call": {
                "closed": ["returns"], "returns": ["return"], "operations": [{
                    "id": "return", "kind": "return", "trigger": {"event": "call"},
                    "at": {"event": "call", "schedule": "same-stack"},
                    "tracking": "untracked", "count": {"min": 1, "max": 1, "scope": "call"},
                    "output": {"kind": "lazy-getter-object", "keys": ["width", "height"]}
                }]
            }}},
            "entrypoints": {".": {
                "artifact": {"path": "dist/index.js", "sha256": "b".repeat(64), "closureSha256": "c".repeat(64)},
                "declarations": {"path": "dist/index.d.ts", "sha256": "d".repeat(64)},
                "exports": {"make": "fn"}
            }}, "sidecars": {}
        });
        let kept = normalized(&serde_json::to_vec(&document).unwrap());
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert!(String::from_utf8_lossy(&encoded).contains("lazy-getter-object"));
        assert_eq!(normalized(&encoded), kept);
        let Err(refusal) = solid_reactive_ir::contract_semantics::certification::proof_policy_2()
            .inspect_candidates(&kept)
        else {
            panic!("lazy recipe certified without a census");
        };
        assert!(refusal.to_string().contains("authored-only"));
    }

    /// ADR 0235: an `effectful-callable` member round-trips with its own
    /// call graph in the export's id namespace, names the export's resources,
    /// moves the digest, and is refused anywhere but directly inside a
    /// returned tuple or object.
    #[test]
    fn an_effectful_callable_member_round_trips_and_is_refused_elsewhere() {
        let document = |call: &str| {
            format!(
                r#"{{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{{"name":"consumer","version":"1.0.0","integrity":"sha512:test","manifest":{{"path":"package.json","sha256":"{a}"}}}},"summaries":{{"fn":{{"shape":"callable","call":{call}}}}},"entrypoints":{{".":{{"artifact":{{"path":"dist/index.js","sha256":"{b}","closureSha256":"{c}"}},"declarations":{{"path":"dist/index.d.ts","sha256":"{d}"}},"exports":{{"make":"fn"}}}}}},"sidecars":{{}}}}"#,
                a = "a".repeat(64),
                b = "b".repeat(64),
                c = "c".repeat(64),
                d = "d".repeat(64),
            )
            .into_bytes()
        };
        let at_call = r#""trigger":{"event":"call"},"at":{"event":"call","schedule":"same-stack"}"#;
        let member = |read_id: &str, resource: &str| {
            format!(
                r#"{{"kind":"effectful-callable","call":{{"closed":["reads","returns"],"reads":["{read_id}"],"returns":["start-return"],"operations":[{{"id":"{read_id}","kind":"read",{at_call},"tracking":"ambient-at-execution","owner":{{"source":"ambient-at-execution"}},"count":{{"min":1,"max":1,"scope":"call"}},"inputs":[{{"kind":"reactive","role":"accessor","resource":"{resource}"}}]}},{{"id":"start-return","kind":"return",{at_call},"tracking":"untracked","count":{{"min":0,"max":"many","scope":"call"}},"output":"undefined"}}]}}}}"#
            )
        };
        let outer = |output: &str| {
            format!(
                r#"{{"closed":["returns"],"returns":["return"],"resources":[{{"id":"running","kind":"reactive-source"}}],"operations":[{{"id":"return","kind":"return",{at_call},"tracking":"untracked","count":{{"min":0,"max":"many","scope":"call"}},"output":{output}}}]}}"#
            )
        };
        let tuple = |member: &str| {
            format!(
                r#"{{"kind":"tuple","closed":["items"],"items":[{{"kind":"reactive","role":"accessor","resource":"running"}},{member}]}}"#
            )
        };
        let kept = normalized(&document(&outer(&tuple(&member("start-read", "running")))));
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert!(String::from_utf8_lossy(&encoded).contains("effectful-callable"));
        assert_eq!(
            normalized(&encoded),
            kept,
            "the member survives the round trip"
        );
        // A member-only clearing selects the digest family recursively and
        // survives encoding, even when no outer operation states the field.
        let member_read = member("start-read", "running").replace(
            r#""tracking":"ambient-at-execution""#,
            r#""tracking":"untracked","strictRead":"cleared""#,
        );
        let cleared = normalized(&document(&outer(&tuple(&member_read))));
        let encoded_cleared = encode(&cleared, &SidecarDigests::default(), true).unwrap();
        assert_eq!(normalized(&encoded_cleared), cleared);
        let uncleared = normalized(&document(&outer(&tuple(
            &member_read.replace(r#","strictRead":"cleared""#, ""),
        ))));
        assert_ne!(uncleared.semantic_digest(), cleared.semantic_digest());
        let opaque = normalized(&document(&outer(&tuple(r#""callable""#))));
        assert_ne!(opaque.semantic_digest(), kept.semantic_digest());
        // No census proves what a member's call does: certification refuses.
        let Err(refused) = solid_reactive_ir::contract_semantics::certification::proof_policy_2()
            .inspect_candidates(&kept)
        else {
            panic!("certification refuses an effectful member");
        };
        assert!(refused.to_string().contains("not certifiable"), "{refused}");
        for (call, needle) in [
            // The whole output of a return.
            (
                outer(&member("start-read", "running")),
                "member of a returned tuple or object",
            ),
            // An operation input.
            (
                outer(&tuple(r#""callable""#)).replace(
                    r#""output":"#,
                    &format!(
                        r#""inputs":[{}],"output":"#,
                        member("start-read", "running")
                    ),
                ),
                "member of a returned tuple or object",
            ),
            // An operation id the export already uses.
            (
                outer(&tuple(&member("return", "running"))),
                "duplicate operation",
            ),
            // A resource neither it nor the export declares.
            (
                outer(&tuple(&member("start-read", "elsewhere"))),
                "missing resource",
            ),
        ] {
            let refused = decode(&document(&call))
                .and_then(|decoded| decoded.normalize())
                .expect_err("the shape is refused");
            assert!(
                refused.to_string().contains(needle),
                "{needle:?}: {refused}"
            );
        }
    }

    #[test]
    fn returned_callable_round_trip_refusal_and_namespace() {
        let at = json!({ "event": "call", "schedule": "same-stack" });
        let graph = |id: &str, resource: &str| {
            json!({
                "closed": ["reads", "returns", "callbacks", "creates"],
                "reads": [id], "returns": [], "callbacks": [], "creates": [],
                "operations": [{ "id": id, "kind": "read", "trigger": { "event": "call" },
                    "at": at, "tracking": "ambient-at-execution",
                    "owner": { "source": "ambient-at-execution" },
                    "count": { "min": 1, "max": 1, "scope": "call" },
                    "inputs": [{ "kind": "reactive", "role": "accessor", "resource": resource }] }]
            })
        };
        let whole = json!({ "kind": "returned-callable", "call": graph("invoke-read", "running"),
            "members": [{ "name": "clear", "value": { "kind": "effectful-callable",
                "call": graph("clear-read", "running") } }, { "name": "opaque", "value": "callable" }] });
        let document = |output: JsonValue| {
            let mut doc: JsonValue = serde_json::from_slice(MINIMAL).unwrap();
            doc["summaries"]["plain-value"] = json!({ "shape": "callable", "call": {
                "closed": ["returns"], "returns": ["return"],
                "resources": [{ "id": "running", "kind": "reactive-source" }],
                "operations": [{ "id": "return", "kind": "return", "trigger": { "event": "call" },
                    "at": at, "tracking": "untracked", "count": { "min": 0, "max": "many", "scope": "call" },
                    "output": output }]
            }});
            doc
        };
        let kept = normalized(&serde_json::to_vec(&document(whole.clone())).unwrap());
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert_eq!(normalized(&encoded), kept);
        let Err(refused) = solid_reactive_ir::contract_semantics::certification::proof_policy_2()
            .inspect_candidates(&kept)
        else {
            panic!("authored graphs are never certified")
        };
        assert!(refused.to_string().contains("not certifiable"));
        let absent = normalized(
            &serde_json::to_vec(&document(json!({ "kind": "returned-callable" }))).unwrap(),
        );
        let empty = normalized(
            &serde_json::to_vec(&document(
                json!({ "kind": "returned-callable", "call": {} }),
            ))
            .unwrap(),
        );
        assert_ne!(absent.semantic_digest(), empty.semantic_digest());
        for candidate in [&absent, &empty] {
            let encoded = encode(candidate, &SidecarDigests::default(), true).unwrap();
            assert_eq!(normalized(&encoded), *candidate);
            assert!(
                solid_reactive_ir::contract_semantics::certification::proof_policy_2()
                    .inspect_candidates(candidate)
                    .is_err()
            );
        }
        let mut duplicates = whole.clone();
        duplicates["members"][0]["value"]["call"] = graph("invoke-read", "running");
        let mut missing = whole.clone();
        missing["call"] = graph("invoke-read", "missing");
        let mut outer_collision = whole.clone();
        outer_collision["call"] = graph("return", "running");
        let mut unknown = whole.clone();
        unknown["captures"] = json!([]);
        let mut duplicate_members = whole.clone();
        duplicate_members["members"] = json!([{ "name": "clear", "value": "callable" }, { "name": "clear", "value": "unknown" }]);
        let mut sibling_resource = whole.clone();
        sibling_resource["call"]["resources"] =
            json!([{ "id": "private", "kind": "reactive-source" }]);
        sibling_resource["members"][0]["value"]["call"]["resources"] =
            json!([{ "id": "private", "kind": "reactive-source" }]);
        for output in [
            duplicates,
            missing,
            outer_collision,
            unknown,
            duplicate_members,
            sibling_resource,
            json!({ "kind": "tuple", "items": [whole.clone()], "closed": ["items"] }),
        ] {
            let bytes = serde_json::to_vec(&document(output)).unwrap();
            assert!(
                decode(&bytes)
                    .and_then(|decoded| decoded.normalize())
                    .is_err()
            );
        }
        let mut input = document(whole.clone());
        input["summaries"]["plain-value"]["call"]["operations"][0]["inputs"] =
            json!([whole.clone()]);
        assert!(
            decode(&serde_json::to_vec(&input).unwrap())
                .and_then(|decoded| decoded.normalize())
                .is_err()
        );
        let mut nested = whole;
        nested["call"] =
            document(json!({ "kind": "returned-callable" }))["summaries"]["plain-value"]["call"]
                .clone();
        assert!(
            decode(&serde_json::to_vec(&document(nested)).unwrap())
                .and_then(|decoded| decoded.normalize())
                .is_err()
        );
    }

    #[test]
    fn returned_captures_round_trip_and_refuse_ambiguous_or_dangling_identity() {
        let original = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/reactive-ir/package-returned-callable-consumer/node_modules/reactive-package/solid-reactivity.json"
        ));
        let mut document: JsonValue = serde_json::from_slice(original).unwrap();
        let summaries = document["summaries"].as_object_mut().unwrap();
        let summary = summaries
            .values_mut()
            .find(|summary| {
                summary["call"]["operations"]
                    .as_array()
                    .is_some_and(|operations| {
                        operations.iter().any(|operation| {
                            operation["output"]["kind"] == "returned-callable"
                                && operation["output"]["call"].is_object()
                        })
                    })
            })
            .unwrap();
        let graph = summary["call"]["operations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|operation| operation["output"]["call"].is_object())
            .unwrap();
        graph["output"]["call"]["captures"] = json!([{ "id": "callback", "from": { "arg": 0 } }]);
        graph["output"]["call"]["callbacks"] =
            json!([{ "from": { "capture": "callback" }, "operation": "invoke-capture" }]);
        graph["output"]["call"]["operations"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "invoke-capture", "kind": "invoke", "trigger": { "event": "call" },
                "at": { "event": "call", "schedule": "same-stack" },
                "tracking": "ambient-at-execution", "owner": { "source": "ambient-at-execution" },
                "count": { "min": 1, "max": 1, "scope": "call" }
            }));
        let kept = normalized(&serde_json::to_vec(&document).unwrap());
        assert_eq!(
            normalized(&encode(&kept, &SidecarDigests::default(), true).unwrap()),
            kept
        );
        let text = serde_json::to_string(&document).unwrap();
        for bad in [
            text.replace("\"capture\":\"callback\"", "\"capture\":\"missing\""),
            text.replace(
                "\"capture\":\"callback\"",
                "\"arg\":0,\"capture\":\"callback\"",
            ),
            text.replace(
                "\"from\":{\"arg\":0}",
                "\"from\":{\"resource\":\"missing\"}",
            ),
        ] {
            assert!(
                decode(bad.as_bytes())
                    .and_then(|proposal| proposal.normalize())
                    .is_err()
            );
        }
    }

    /// ADR 0153 part 3: `contextPremises` survives the round trip, puts the
    /// document in a digest family of its own, and refuses an empty or
    /// repeated name rather than ignoring it.
    #[test]
    fn context_premises_round_trip_in_their_own_digest_family() {
        let document = |premises: &str| {
            format!(
                r#"{{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{{"name":"consumer","version":"1.0.0","integrity":"sha512:test","manifest":{{"path":"package.json","sha256":"{a}"}}}},"summaries":{{"fn":{{"shape":"callable","call":{{"creates":[],"closed":["creates"]{premises}}}}}}},"entrypoints":{{".":{{"artifact":{{"path":"dist/index.js","sha256":"{b}","closureSha256":"{c}"}},"declarations":{{"path":"dist/index.d.ts","sha256":"{d}"}},"exports":{{"useThing":"fn"}}}}}},"sidecars":{{}}}}"#,
                a = "a".repeat(64),
                b = "b".repeat(64),
                c = "c".repeat(64),
                d = "d".repeat(64),
            )
            .into_bytes()
        };
        let plain = normalized(&document(""));
        let kept = normalized(&document(r#","contextPremises":["RouterContext"]"#));
        let premises = kept.artifact_cases()[0].exports["useThing"]
            .call
            .context_premises()
            .iter()
            .map(|premise| premise.export.as_str())
            .collect::<Vec<_>>();
        assert_eq!(premises, ["RouterContext"]);
        assert_ne!(kept.semantic_digest(), plain.semantic_digest());
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert!(
            String::from_utf8_lossy(&encoded).contains("\"contextPremises\""),
            "the encoder dropped the premise"
        );
        assert_eq!(normalized(&encoded), kept);
        // A document stating none writes none.
        let encoded = encode(&plain, &SidecarDigests::default(), true).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("contextPremises"));
        for premises in [
            r#","contextPremises":[""]"#,
            r#","contextPremises":["RouterContext","RouterContext"]"#,
        ] {
            assert!(
                decode(&document(premises))
                    .and_then(|decoded| decoded.normalize())
                    .is_err(),
                "{premises} is refused"
            );
        }
    }

    /// ADR 0153 item C: `accessorBounds` survives the round trip, puts the
    /// document in a digest family of its own, and refuses an empty or
    /// repeated source rather than ignoring it.
    #[test]
    fn accessor_bounds_round_trip_in_their_own_digest_family() {
        let document = |bounds: &str| {
            format!(
                r#"{{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{{"name":"consumer","version":"1.0.0","integrity":"sha512:test","manifest":{{"path":"package.json","sha256":"{a}"}}}},"summaries":{{"fn":{{"shape":"callable","call":{{"reads":[],"closed":["reads"]{bounds}}}}}}},"entrypoints":{{".":{{"artifact":{{"path":"dist/index.js","sha256":"{b}","closureSha256":"{c}"}},"declarations":{{"path":"dist/index.d.ts","sha256":"{d}"}},"exports":{{"plainSum":"fn"}}}}}},"sidecars":{{}}}}"#,
                a = "a".repeat(64),
                b = "b".repeat(64),
                c = "c".repeat(64),
                d = "d".repeat(64),
            )
            .into_bytes()
        };
        let plain = normalized(&document(""));
        let kept = normalized(&document(r#","accessorBounds":["./dist/index.js:10-15"]"#));
        assert_eq!(
            kept.artifact_cases()[0].exports["plainSum"]
                .call
                .accessor_bounds()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["./dist/index.js:10-15"]
        );
        assert_ne!(kept.semantic_digest(), plain.semantic_digest());
        let encoded = encode(&kept, &SidecarDigests::default(), true).unwrap();
        assert!(
            String::from_utf8_lossy(&encoded).contains("\"accessorBounds\""),
            "the encoder dropped the bound"
        );
        assert_eq!(normalized(&encoded), kept);
        let encoded = encode(&plain, &SidecarDigests::default(), true).unwrap();
        assert!(!String::from_utf8_lossy(&encoded).contains("accessorBounds"));
        for bounds in [
            r#","accessorBounds":[""]"#,
            r#","accessorBounds":["./a.js:1-2","./a.js:1-2"]"#,
        ] {
            assert!(
                decode(&document(bounds))
                    .and_then(|decoded| decoded.normalize())
                    .is_err(),
                "{bounds} is refused"
            );
        }
    }

    /// ADR 0114: `computations` round-trips by item only, puts the document in
    /// a digest family of its own, and is refused wherever it would be closed,
    /// empty, or missing its `compute` operation.
    #[test]
    fn computations_round_trip_by_item_only_in_their_own_digest_family() {
        use solid_reactive_ir::contract_semantics::KnowledgeState;

        let document = |call: &str| {
            format!(
                r#"{{"format":"solid-reactivity-contract","schemaVersion":1,"semanticModelVersion":1,"package":{{"name":"consumer","version":"1.0.0","integrity":"sha512:test","manifest":{{"path":"package.json","sha256":"{a}"}}}},"summaries":{{"fn":{{"shape":"callable","call":{call}}}}},"entrypoints":{{".":{{"artifact":{{"path":"dist/index.js","sha256":"{b}","closureSha256":"{c}"}},"declarations":{{"path":"dist/index.d.ts","sha256":"{d}"}},"exports":{{"run":"fn"}}}}}},"sidecars":{{}}}}"#,
                a = "a".repeat(64),
                b = "b".repeat(64),
                c = "c".repeat(64),
                d = "d".repeat(64),
            )
            .into_bytes()
        };
        // The generator's shape, copied from `signals-reexport-creates`.
        const COMPUTE: &str = r#"{"at":{"event":"call","schedule":"same-stack"},"count":{"max":"many","min":0,"scope":"call"},"id":"owner-requirement-0","kind":"compute","owner":{"requires":"required","requiresChildren":"required","source":"ambient-at-call"},"tracking":"untracked","trigger":{"event":"call"}}"#;

        let stated = normalized(&document(&format!(
            r#"{{"closed":["creates"],"creates":[],"computations":["owner-requirement-0"],"operations":[{COMPUTE}]}}"#
        )));
        let export = &stated.artifact_cases()[0].exports["run"];
        assert_eq!(
            export.claim_state(ClaimDomain::Computations),
            KnowledgeState::PartialPositive
        );
        let encoded = encode(&stated, &SidecarDigests::default(), false).unwrap();
        assert!(
            String::from_utf8_lossy(&encoded).contains(r#""computations":["owner-requirement-0"]"#),
            "{}",
            String::from_utf8_lossy(&encoded)
        );
        assert_eq!(normalized(&encoded), stated);
        let silent = normalized(&document(r#"{"closed":["creates"],"creates":[]}"#));
        assert_ne!(stated.semantic_digest(), silent.semantic_digest());

        for (call, why) in [
            (
                format!(
                    r#"{{"closed":["computations"],"computations":["owner-requirement-0"],"operations":[{COMPUTE}]}}"#
                ),
                "closed",
            ),
            (r#"{"computations":[]}"#.to_owned(), "empty"),
            (format!(r#"{{"operations":[{COMPUTE}]}}"#), "unlisted"),
        ] {
            assert!(
                decode(&document(&call))
                    .and_then(|proposal| proposal.normalize())
                    .is_err(),
                "a {why} computations domain must be refused: {call}"
            );
        }
    }

    #[test]
    fn encoder_keeps_sidecar_hashes_wire_only() {
        let expected = normalized(SIGNAL);
        let bytes = encode(
            &expected,
            &SidecarDigests {
                proof: Some(Digest::parse(format!("sha256:{}", "a".repeat(64))).unwrap()),
                probes: Some(Digest::parse(format!("sha256:{}", "b".repeat(64))).unwrap()),
            },
            false,
        )
        .unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.normalize().unwrap(), expected);
    }

    #[test]
    fn goldens_cover_every_local_knowledge_state() {
        use solid_reactive_ir::contract_semantics::{ClaimDomain, KnowledgeState};

        let minimal = normalized(MINIMAL);
        let minimal_export = minimal.artifact_cases()[0].exports.get("version").unwrap();
        assert_eq!(
            minimal_export.claim_state(ClaimDomain::Reads),
            KnowledgeState::Unknown
        );

        let signal = normalized(SIGNAL);
        let signal_export = signal.artifact_cases()[0]
            .exports
            .get("createSignal")
            .unwrap();
        assert_eq!(
            signal_export.claim_state(ClaimDomain::Reads),
            KnowledgeState::CompletePositive
        );
        assert_eq!(
            signal_export.claim_state(ClaimDomain::Callbacks),
            KnowledgeState::CompleteNegative
        );

        let conditional = normalized(CONDITIONAL);
        let effect = conditional.artifact_cases()[0]
            .exports
            .get("createOwnedEffect")
            .unwrap();
        assert_eq!(
            effect.call.resources[0].states.state(),
            KnowledgeState::PartialPositive
        );
    }

    #[test]
    fn composing_a_value_export_with_open_call_path_never_manufactures_function_effects() {
        // Regression for the ecosystem-wrapper refusal
        // `package contract value export .:IR cannot have function effects`
        // (@tanstack/solid-db -> IR, and its siblings @tanstack/solid-hotkeys
        // ALL_KEYS, @tanstack/solid-query dataTagErrorSymbol, @tanstack/solid-form
        // initialServerFormState, @tanstack/solid-query-persist-client
        // PERSISTER_KEY_PREFIX). Each republishes a plain value constant from a
        // dependency whose *proposal* still carries that export's call-path
        // domains as open (unresolved). Composing that proposal for the importing
        // package's generation projects the value export; opening its vacuous
        // call-path there manufactured function effects on a value export and
        // refused the parent. A non-callable value is never invoked, so its
        // call-path is vacuously empty; standalone generation already certifies
        // it effect-free and composition must agree.
        //
        // `MINIMAL`'s `version` export is this exact shape -- a plain (value)
        // export whose call-path knowledge is Unknown (open). Before the fix in
        // `project_accepted_export`, projecting it opened Reads/Callbacks/Returns
        // /Creates and `has_function_effects()` was true.
        use solid_reactive_ir::contract_semantics::proof::project_untrusted_proposal_for_generation;
        use solid_reactive_ir::contract_semantics::{AcceptedContractIndex, AcceptedContractInput};
        use solid_reactive_ir::project_accepted_export;

        let contract = normalized(MINIMAL);
        assert_eq!(
            contract.artifact_cases()[0]
                .exports
                .get("version")
                .unwrap()
                .claim_state(solid_reactive_ir::contract_semantics::ClaimDomain::Reads),
            solid_reactive_ir::contract_semantics::KnowledgeState::Unknown,
            "the fixture must keep `version`'s call-path open for the regression to bite",
        );
        let case_id = contract.artifact_cases()[0].id.clone();
        let accepted = project_untrusted_proposal_for_generation(contract, &case_id).unwrap();
        let index = AcceptedContractIndex::new([AcceptedContractInput {
            importer: "/pkg/index.js".into(),
            specifier: "dependency".into(),
            contract: accepted,
            artifact_identity: None,
        }])
        .unwrap();
        let used = index
            .resolve_name("/pkg/index.js", "dependency", "version")
            .unwrap();
        let projected = project_accepted_export(&used);
        assert_eq!(projected.kind, "value");
        assert!(
            !projected.has_function_effects(),
            "a non-callable value export's vacuous open call-path must not project as function effects: {projected:?}",
        );
        assert!(
            projected.open_claims.is_empty(),
            "no call-path domain may stay open on a non-callable value export: {:?}",
            projected.open_claims,
        );
    }

    #[test]
    fn wire_order_and_summary_names_do_not_change_semantic_identity() {
        let expected = normalized(MINIMAL);
        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        let summary = value["summaries"]
            .as_object_mut()
            .unwrap()
            .remove("plain-value")
            .unwrap();
        value["summaries"]
            .as_object_mut()
            .unwrap()
            .insert("renamed-wire-id".into(), summary);
        value["entrypoints"]["."]["exports"]["version"] =
            serde_json::Value::String("renamed-wire-id".into());
        let actual = normalized(&serde_json::to_vec(&value).unwrap());
        assert_eq!(expected.semantic_digest(), actual.semantic_digest());

        let expected = normalized(CONDITIONAL);
        value = serde_json::from_slice(CONDITIONAL).unwrap();
        value["entrypoints"]["."]["cases"]
            .as_array_mut()
            .unwrap()
            .reverse();
        let actual = normalized(&serde_json::to_vec(&value).unwrap());
        assert_eq!(expected.semantic_digest(), actual.semantic_digest());

        let expected = normalized(SIGNAL);
        value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["call"]["closed"]
            .as_array_mut()
            .unwrap()
            .reverse();
        let actual = normalized(&serde_json::to_vec(&value).unwrap());
        assert_eq!(expected.semantic_digest(), actual.semantic_digest());
    }

    #[test]
    fn local_closure_rejects_false_negative_proof() {
        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["call"] = serde_json::json!({"closed": ["reads"]});
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::DocumentDecode { .. })
        ));

        value["summaries"]["plain-value"]["call"] = serde_json::json!({"reads": []});
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::DocumentDecode { .. })
        ));
    }

    #[test]
    fn cross_field_validation_rejects_invalid_graphs_and_contradictions() {
        let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["call"]["edges"][0]["to"] =
            serde_json::Value::String("missing".into());
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::InvalidSemanticModel { .. })
        ));

        value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["shape"]["items"][0]["capabilities"] =
            serde_json::json!(["readable", "writable"]);
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::InvalidSemanticModel { .. })
        ));
    }

    #[test]
    fn schema_mechanics_and_limits_fail_closed() {
        for excluded in [
            "schemaStatus",
            "evidence",
            "generator",
            "trustStatus",
            "compilerFactsProtocol",
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
            value[excluded] = serde_json::Value::String("must-not-cross-the-boundary".into());
            assert!(matches!(
                decode(&serde_json::to_vec(&value).unwrap()),
                Err(ContractFailure::DocumentDecode { .. })
            ));
        }

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["entrypoints"]["."]["exports"]["version"] =
            serde_json::json!({"summary": "plain-value", "reads": []});
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap()),
            Err(ContractFailure::DocumentDecode { .. })
        ));

        value = serde_json::from_slice(MINIMAL).unwrap();
        value["package"]["manifest"]["path"] = serde_json::Value::String("../outside.json".into());
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::DocumentDecode { .. })
        ));

        let oversized = vec![b' '; MAX_DOCUMENT_BYTES + 1];
        assert!(matches!(
            decode(&oversized),
            Err(ContractFailure::DocumentTooLarge { .. })
        ));

        value = serde_json::from_slice(MINIMAL).unwrap();
        let mut nested = serde_json::json!({"kind": "unknown"});
        for _ in 0..MAX_RECURSIVE_DEPTH {
            nested = serde_json::json!({"kind": "promise", "value": nested});
        }
        value["summaries"]["plain-value"]["shape"] = nested.clone();
        normalized(&serde_json::to_vec(&value).unwrap());

        nested = serde_json::json!({"kind": "promise", "value": nested});
        value["summaries"]["plain-value"]["shape"] = nested;
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::DocumentDecode { .. })
        ));
    }

    #[test]
    fn collection_limits_are_checked_before_semantic_expansion() {
        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        let entrypoint = value["entrypoints"]["."].clone();
        let entrypoints = value["entrypoints"].as_object_mut().unwrap();
        entrypoints.clear();
        for index in 0..=MAX_ENTRYPOINTS {
            entrypoints.insert(format!("./entry-{index}"), entrypoint.clone());
        }
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::DocumentDecode { .. })
        ));

        value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["call"] = serde_json::json!({
            "operations": (0..=MAX_OPERATIONS)
                .map(|index| serde_json::json!({"id": format!("op-{index}"), "kind": "read"}))
                .collect::<Vec<_>>()
        });
        assert!(matches!(
            decode(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .normalize(),
            Err(ContractFailure::DocumentDecode { .. })
        ));
    }

    #[test]
    fn every_seeded_false_closure_mutation_is_detected() {
        let reject = |name: &str, value: serde_json::Value| {
            let bytes = serde_json::to_vec(&value).unwrap();
            let rejected = decode(&bytes)
                .and_then(DecodedContractDocument::normalize)
                .is_err();
            assert!(rejected, "seeded false-closure mutation {name:?} survived");
        };

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["call"] = serde_json::json!({
            "closed": ["reads"]
        });
        value["summaries"]["sibling"] = serde_json::json!({
            "shape": "plain",
            "call": {"closed": ["reads"], "reads": []}
        });
        reject("closure collection moved to a sibling summary", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["call"] = serde_json::json!({"reads": []});
        reject("empty open domain", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["entrypoints"]["."]["exports"]["version"] = serde_json::json!("missing");
        reject("dangling summary", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["shape"] =
            serde_json::json!({"kind": "promise", "summary": "plain-value"});
        reject("summary recursion smuggled through an unknown field", value);

        let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["call"]["operations"][0]["trigger"] =
            serde_json::json!({"operation": "read"});
        reject("operation trigger cycle", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["call"] = serde_json::json!({
            "resources": [
                {"id": "left", "kind": "reactive-source", "lifetime": {"kind": "resource", "resource": "right"}},
                {"id": "right", "kind": "reactive-source", "lifetime": {"kind": "resource", "resource": "left"}}
            ]
        });
        reject("resource lifetime cycle", value);

        let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["call"]["operations"][0]["resources"][0] =
            serde_json::json!("missing");
        reject("dangling resource", value);

        let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["call"]["edges"][0]["to"] = serde_json::json!("missing");
        reject("dangling operation", value);

        let mut value: serde_json::Value = serde_json::from_slice(SIGNAL).unwrap();
        value["summaries"]["signal-pair"]["shape"]["items"][0]["capabilities"] =
            serde_json::json!(["readable", "writable"]);
        reject("contradictory accessor capabilities", value);

        let mut value: serde_json::Value = serde_json::from_slice(CONDITIONAL).unwrap();
        let duplicate = value["summaries"]["owned-effect"]["call"]["cases"][0].clone();
        value["summaries"]["owned-effect"]["call"]["cases"]
            .as_array_mut()
            .unwrap()
            .insert(1, duplicate);
        reject("overlapping guard branches", value);

        let mut value: serde_json::Value = serde_json::from_slice(CONDITIONAL).unwrap();
        value["summaries"]["owned-effect"]["call"]["cases"][1]["otherwise"] =
            serde_json::json!(false);
        reject("uncovered remainder represented as complete", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["summaries"]["plain-value"]["call"] = serde_json::json!({
            "closed": ["states"],
            "reads": []
        });
        reject("misplaced closure domain", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        value["package"]["manifest"]["path"] = serde_json::json!("../outside.json");
        reject("package path traversal", value);

        let mut value: serde_json::Value = serde_json::from_slice(MINIMAL).unwrap();
        let entrypoint = value["entrypoints"]
            .as_object_mut()
            .unwrap()
            .remove(".")
            .unwrap();
        value["entrypoints"]
            .as_object_mut()
            .unwrap()
            .insert("../outside".into(), entrypoint);
        reject("entrypoint traversal", value);
    }

    #[test]
    fn seeded_decode_normalize_encode_fuzz_preserves_semantics_or_refuses() {
        let seeds = [MINIMAL, SIGNAL, CONDITIONAL];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut survivors = 0usize;
        for iteration in 0..512usize {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let seed = seeds[iteration % seeds.len()];
            let mut bytes = seed.to_vec();
            let edits = 1 + usize::try_from(state % 4).unwrap();
            for edit in 0..edits {
                state = state.rotate_left(11) ^ 0xa076_1d64_78bd_642f;
                let index = usize::try_from(state).unwrap_or(0) % bytes.len();
                let replacement = b"{}[]\",:0 "[(iteration + edit) % 9];
                bytes[index] = replacement;
            }

            let Ok(decoded) = decode(&bytes) else {
                continue;
            };
            let Ok(first) = decoded.normalize() else {
                continue;
            };
            survivors += 1;
            let encoded = encode(&first, &SidecarDigests::default(), false).unwrap();
            let second = normalized(&encoded);
            assert_eq!(
                first, second,
                "semantic drift at fuzz iteration {iteration}"
            );
            assert_eq!(
                encoded,
                encode(&second, &SidecarDigests::default(), false).unwrap(),
                "nondeterministic encoding at fuzz iteration {iteration}"
            );
        }
        assert!(
            survivors > 0,
            "mutation corpus did not exercise round-trip checks"
        );
    }

    #[test]
    fn checked_in_schema_pins_the_stable_envelope() {
        let schema: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../schema/solid-reactivity.schema.json"
        )))
        .unwrap();
        assert_eq!(
            schema["properties"]["format"]["const"],
            serde_json::json!(FORMAT)
        );
        assert_eq!(
            schema["properties"]["schemaVersion"]["const"],
            serde_json::json!(SCHEMA_VERSION)
        );
        assert_eq!(
            schema["properties"]["semanticModelVersion"]["const"],
            serde_json::json!(SEMANTIC_MODEL_VERSION)
        );
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
    }

    #[test]
    fn finite_wire_vocabularies_expand_without_fallback_categories() {
        let ids = IdScope::new("case", "export");
        let values = [
            serde_json::json!("unknown"),
            serde_json::json!("plain"),
            serde_json::json!({"kind": "parameter", "index": 0, "path": ["value"]}),
            serde_json::json!({"kind": "tuple", "closed": ["items"], "items": ["plain"]}),
            serde_json::json!({"kind": "array", "element": "plain", "length": {"min": 0, "max": "many"}}),
            serde_json::json!({"kind": "object", "closed": ["properties"], "properties": [{"name": "value", "value": "plain"}]}),
            serde_json::json!({"kind": "choice", "closed": ["alternatives"], "alternatives": ["plain", "callable"]}),
            serde_json::json!("callable"),
            serde_json::json!({"kind": "promise", "value": "plain"}),
            serde_json::json!({"kind": "async-iterable", "element": "plain"}),
            serde_json::json!({"kind": "reactive", "role": "accessor", "resource": "source", "closed": ["capabilities"], "capabilities": ["readable"]}),
            serde_json::json!({"kind": "store", "resource": "source", "closed": ["capabilities"], "capabilities": ["readable"]}),
            serde_json::json!({"kind": "action", "transition": "transition"}),
            serde_json::json!("component"),
            serde_json::json!({"kind": "cleanup", "resource": "cleanup"}),
            serde_json::json!("ref-application"),
            serde_json::json!({"kind": "server-function-reference", "resource": "server"}),
            serde_json::json!({"kind": "argument-array", "items": [0, 1]}),
            serde_json::json!({"kind": "argument-array", "items": []}),
            serde_json::json!({"kind": "invocation-result", "parameter": 0}),
            serde_json::json!("undefined"),
            serde_json::json!({"kind": "undefined"}),
            serde_json::json!({"kind": "described-callable", "reads": [], "returns": ["plain"]}),
            serde_json::json!({"kind": "described-callable", "reads": ["owned-signal"], "returns": []}),
        ];
        for value in values {
            let value: WireValue = serde_json::from_value(value).unwrap();
            expand_value(&value, &ids).unwrap();
        }
        // ADR 0115: an argument array with no `items` states nothing, and is
        // refused rather than read as `[]`.
        assert!(
            serde_json::from_value::<WireValue>(serde_json::json!({"kind": "argument-array"}))
                .is_err()
        );
        // ADR 0116: an invocation result that names no argument likewise.
        assert!(
            serde_json::from_value::<WireValue>(serde_json::json!({"kind": "invocation-result"}))
                .is_err()
        );
        // Item B round 2: `undefined` carries no field, and one is refused.
        assert!(
            serde_json::from_value::<WireValue>(
                serde_json::json!({"kind": "undefined", "parameter": 0})
            )
            .is_err()
        );
        // ADR 0145: both lists are required, and a read outside the reviewed
        // vocabulary is refused rather than dropped.
        for missing in [
            serde_json::json!({"kind": "described-callable", "returns": []}),
            serde_json::json!({"kind": "described-callable", "reads": []}),
        ] {
            assert!(serde_json::from_value::<WireValue>(missing).is_err());
        }
        let unknown: WireValue = serde_json::from_value(
            serde_json::json!({"kind": "described-callable", "reads": ["store"], "returns": []}),
        )
        .unwrap();
        assert!(expand_value(&unknown, &ids).is_err());
        let described = ValueShape::DescribedCallable(Box::new(
            solid_reactive_ir::contract_semantics::DescribedCall {
                reads: vec![solid_reactive_ir::contract_semantics::DescribedRead::OwnedSignal],
                returns: vec![ValueShape::Plain],
                callbacks: Vec::new(),
            },
        ));
        let compact =
            compact_value(&described, &CompactIds::new("case", "case", "export")).unwrap();
        assert_eq!(
            compact,
            serde_json::json!({"kind": "described-callable", "reads": ["owned-signal"], "returns": ["plain"]})
        );
        let wire: WireValue = serde_json::from_value(compact).unwrap();
        assert_eq!(expand_value(&wire, &ids).unwrap(), described);
        // ADR 0152: a nested item round-trips in the top-level item's shape,
        // its operation inline; absent, `callbacks` reads as `[]`, and an
        // inline operation carrying an id or a guard is refused.
        let piped = ValueShape::DescribedCallable(Box::new(
            solid_reactive_ir::contract_semantics::DescribedCall {
                reads: Vec::new(),
                returns: vec![ValueShape::InvocationResult { parameter: 1 }],
                callbacks: vec![
                    solid_reactive_ir::contract_semantics::DescribedCallback::same_stack_once(0),
                    solid_reactive_ir::contract_semantics::DescribedCallback::same_stack_once(1),
                ],
            },
        ));
        let compact = compact_value(&piped, &CompactIds::new("case", "case", "export")).unwrap();
        assert_eq!(
            compact["callbacks"][0],
            serde_json::json!({
                "from": {"arg": 0, "path": []},
                "operation": {
                    "kind": "invoke",
                    "trigger": {"event": "call"},
                    "at": {"event": "call", "schedule": "same-stack"},
                    "tracking": "ambient-at-execution",
                    "owner": {"source": "ambient-at-execution"},
                    "count": {"scope": "call", "min": 1, "max": 1}
                }
            })
        );
        let wire: WireValue = serde_json::from_value(compact.clone()).unwrap();
        assert_eq!(expand_value(&wire, &ids).unwrap(), piped);
        let mut with_id = compact;
        with_id["callbacks"][0]["operation"]["id"] = serde_json::json!("callback-0");
        assert!(serde_json::from_value::<WireValue>(with_id).is_err());
        // And it is emitted as the shorthand, which reads back as itself.
        assert_eq!(
            compact_value(
                &ValueShape::Undefined,
                &CompactIds::new("case", "case", "export")
            )
            .unwrap(),
            serde_json::json!("undefined")
        );

        let guard: WireGuard = serde_json::from_value(serde_json::json!({
            "all": [
                {"signature": "overload"},
                {"argumentCount": {"min": 1, "max": 2}},
                {"arg": 0, "path": [], "literal": true},
                {"arg": 0, "path": [], "kind": "callable"},
                {"arg": 0, "path": [], "property": "run", "callable": true},
                {"arg": 0, "tupleAlternative": 1},
                {"resultProtocol": "promise"},
                {"artifactCase": "artifact-case:known"}
            ]
        }))
        .unwrap();
        assert_eq!(expand_guard(guard, &ids).unwrap().0.len(), 8);

        for (kind, state) in [
            (WireResourceKind::Owner, WireResourceState::Active),
            (WireResourceKind::Owner, WireResourceState::Disposed),
            (WireResourceKind::Cleanup, WireResourceState::Installed),
            (WireResourceKind::Cleanup, WireResourceState::Disposed),
            (
                WireResourceKind::AsyncComputation,
                WireResourceState::Pending,
            ),
            (
                WireResourceKind::AsyncComputation,
                WireResourceState::Settled,
            ),
            (
                WireResourceKind::AsyncComputation,
                WireResourceState::Errored,
            ),
            (
                WireResourceKind::AsyncComputation,
                WireResourceState::Cancelled,
            ),
            (WireResourceKind::Transition, WireResourceState::Active),
            (WireResourceKind::Transition, WireResourceState::Settled),
            (WireResourceKind::Transition, WireResourceState::Reverted),
            (WireResourceKind::Response, WireResourceState::Uncommitted),
            (WireResourceKind::Response, WireResourceState::Committed),
            (WireResourceKind::Stream, WireResourceState::Unclaimed),
            (WireResourceKind::Stream, WireResourceState::Claimed),
        ] {
            expand_resource_state(kind, state).unwrap();
        }
    }
}
