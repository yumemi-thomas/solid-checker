mod cache;
mod cleanup;
mod contracts;
mod directives;
mod effect_api;
mod execution_role;
mod findings;
mod identity;
mod indexes;
mod interproc;
mod local_access;
mod owners;
mod pipeline;
mod projection;
mod reachability;
mod reactive_analysis;
mod runtime_semantics;
mod server_rules;
mod source_discovery;
mod static_api;
mod static_rules;
mod symbols;
mod timings;
mod upstream_compat;

pub use pipeline::{build, build_with_contracts, build_with_contracts_measured};

pub use upstream_compat::solid1x_options::{RuleOptions, RuleOverride, Solid1xRuleOptions};

pub use findings::{
    DOCS_BASE_URL, EvidenceStep, Finding, RuleManifestIdentity, RuleMetadata, SolveTimings,
    assert_rules_have_documentation, direct_mutation_wording, finish_findings, rule_manifest_json,
    strict_read_evidence, strict_read_message, strict_read_related_locations,
};
pub use projection::{
    CatalogCapabilities, CatalogWording, FindingSeed, FindingWording, PackageContractIssue,
    PackageContractIssueKind, StaticDefectTerms, StaticDefectText, project_finding,
    project_findings, static_defect_text, suppress_findings_owned_by_enabled_rules,
};

use cache::{BuildIdentity, IncrementalCacheState, RetainedBuild};
use pipeline::build_with_contracts_measured_incremental;

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use contracts::{
    ContractAnalysis, ContractGraph, ContractSemantics, contract_export_summaries,
    contract_export_summaries_incremental,
};
use execution_role::{
    NamedCallbackRoles, allowed_callback_spans, assigned_member_function_contains, execution_role,
    named_callback_roles, semantic_execution_role,
};
use identity::{SymbolId, SymbolInterner, SymbolName, symbol_id, symbol_name};
use indexes::{EntitySymbols, ProjectIndexes, SemanticLookup};
use interproc::{SummaryNode, SummaryRead, SummaryReads};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solid_dialect::{Dialect, Primitive};
use solid_facts::ProjectFacts;
use solid_facts::core::Span;
use thiserror::Error;
use typefacts::Location;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionRole {
    /// Neither compiler facts nor semantic facts classify this span. Unknown
    /// is not a violation: projection suppresses it instead of converting a
    /// missing fact into a user-facing claim.
    Unknown,
    /// One-shot module evaluation: reads are untracked, but writes do not run
    /// inside an owned tracking computation.
    ModuleInitialization,
    TrackedJsx,
    DeferredCallback,
    UntrackedCallback,
    EffectApply,
    EventCallback,
    DirectiveApply,
    UntrackedRendering,
}

impl ExecutionRole {
    /// Whether a reactive read in this role subscribes to nothing — the roles
    /// the strict-read rule reports in every dialect.
    #[must_use]
    pub const fn reports_untracked_read(self) -> bool {
        matches!(
            self,
            Self::ModuleInitialization
                | Self::UntrackedRendering
                | Self::UntrackedCallback
                | Self::EffectApply
        )
    }

    /// Whether a reactive write (or an action invocation) is allowed in this
    /// role: the imperative scopes that run outside the tracking phase.
    #[must_use]
    pub const fn reports_disallowed_write(self) -> bool {
        matches!(self, Self::TrackedJsx | Self::UntrackedRendering)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactiveRead {
    pub kind: Arc<str>,
    pub accessor: Arc<str>,
    pub location: Location,
    pub declaration: Location,
    pub execution: ExecutionRole,
    pub context: Arc<str>,
    pub via: Arc<str>,
    pub origin: Option<Location>,
    pub origin_context: Arc<str>,
    /// The read's reactive backing cannot be proven or ruled out — a
    /// component-props read whose callers the analyzer cannot enumerate
    /// (exported component, unresolvable references, call-site spreads).
    /// Projection reports it as an uncertifiable proof obligation rather
    /// than a proven violation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub uncertain: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactiveWrite {
    pub setter: Arc<str>,
    #[serde(default)]
    pub operation: ReactiveWriteOperation,
    pub source_kind: ReactiveSourceKind,
    pub location: Location,
    pub declaration: Location,
    pub execution: ExecutionRole,
    pub allowed_by_option: bool,
    pub context: Arc<str>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReactiveWriteOperation {
    #[default]
    Setter,
    Refresh,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionInvocation {
    pub action: Arc<str>,
    pub location: Location,
    pub declaration: Location,
    pub execution: ExecutionRole,
    pub context: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub location: Location,
    pub new_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    pub message: String,
    pub applicability: String,
    pub edits: Vec<TextEdit>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafOwnerOperation {
    pub kind: LeafOwnerOperationKind,
    pub owner: String,
    pub location: Location,
    pub fix: Option<Fix>,
    /// When set, this leaf owner only materializes if the owner call at this
    /// location executes under a live children-capable owner (2.0
    /// `onSettled`). The owner fixed point resolves the gate against the
    /// owner graph: an out-of-band call drops the operation, an unprovable
    /// call site marks it [`LeafOwnerOperation::uncertain`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_site_gate: Option<Location>,
    /// The gate could not be resolved (exported helper, conditional owner):
    /// the finding is projected as uncertifiable rather than a proven
    /// violation.
    #[serde(default)]
    pub uncertain: bool,
    /// The exactly-resolved helper the operation is reached through: the
    /// operation sits in the helper's synchronous extent and the helper is
    /// called from this leaf scope, so it executes here. The finding anchors
    /// at the helper call site — the call is what introduces the defect; the
    /// helper body may have other, legal callers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "primitive")]
pub enum LeafOwnerOperationKind {
    Cleanup,
    Flush,
    Primitive(String),
    /// The leaf owner receives a callback whose exact synchronous body is not
    /// available. It may contain any of the forbidden operations above, so
    /// the leaf scope is a proof obligation rather than a clean result.
    UnresolvedCallback,
}

impl LeafOwnerOperationKind {
    #[must_use]
    pub fn primitive(&self) -> &str {
        match self {
            Self::Cleanup => "onCleanup",
            Self::Flush => "flush",
            Self::Primitive(primitive) => primitive,
            Self::UnresolvedCallback => "unresolved leaf callback",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticViolation {
    pub id: String,
    pub rule: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
    pub location: Location,
    pub analysis_context: String,
    pub fixes: Vec<Fix>,
    /// A required runtime/configuration fact is unavailable. Projection keeps
    /// the rule identity and wording but emits an uncertifiable proof
    /// obligation rather than claiming a proven violation.
    #[serde(default)]
    pub uncertain: bool,
}

/// A version-independent defect proven by shared analysis.
///
/// Unlike [`StaticViolation`], this carries no external rule identity or
/// user-facing prose. Each dialect catalog projects the structured defect
/// into its own rule, message, and hint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticDefect {
    pub kind: StaticDefectKind,
    pub location: Location,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_context: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
    /// The defect's reactive premise cannot be proven or ruled out — a
    /// props-backed defect whose component's callers the analyzer cannot
    /// enumerate. Projection reports it as an uncertifiable proof obligation
    /// instead of a proven violation, mirroring the owner-requirement and
    /// leaf-operation escalations.
    #[serde(default)]
    pub uncertain: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum StaticDefectKind {
    ReactiveObjectDestructure {
        source: String,
        component_props: bool,
    },
    ReactiveReadAfterAwait {
        accessor: String,
    },
    ComponentReturnsConditionally,
    PackageContractExportMissing {
        module: String,
        export: String,
        reexported: bool,
    },
    /// A package export has different certified summaries for different
    /// conditional runtime targets. The current project analysis has no
    /// selected package condition, so applying any one variant would be a
    /// guess; keep the import uncertifiable until a human selects the target.
    PackageContractEnvironmentDependent {
        module: String,
        export: String,
        reexported: bool,
    },
    /// An exported callback reached an external call whose execution timing
    /// is not certified. The fields are deliberately data, not a guessed
    /// contract: the diagnostic can produce an editable stub while analysis
    /// remains blocked until the package author chooses the audited timing.
    UnknownCallbackExecution {
        package: String,
        entrypoint: String,
        function: String,
        parameter: usize,
        parameter_type: String,
        required_execution: String,
        contract_stub: String,
    },
    MissingEffectFunction,
    ReactiveSourceUncaptured {
        source: String,
        callee: String,
    },
    /// A type-correct call can reach more than one runtime implementation,
    /// and those implementations do not have one equivalent reactive-read
    /// summary. Silence would certify whichever implementation happened not
    /// to be selected by the analyzer.
    ReactiveDispatchUnresolved {
        callee: String,
        member: Option<String>,
    },
    /// An exact synchronous callback position is known, but the callback
    /// value's body is not an inspectable synchronous function literal. The
    /// enclosing operation is type-correct, so silence would certify behavior
    /// that neither the AST nor a contract proves.
    ReactiveCallbackUnresolved {
        callee: String,
    },
    /// An exported structured return contains a shorthand value whose exact
    /// binding cannot be joined to the analyzed project. Omitting the property
    /// would make a possibly-reactive return look inert.
    StructuredReturnUnresolved {
        function: String,
        property: String,
        reason: String,
    },
    ReactiveHandlerRead {
        attribute: String,
        expression: String,
    },
    /// A JSX attribute name TypeScript deliberately does not check is lowered
    /// as a native event listener, but the runtime value is either proven not
    /// callable or cannot be distinguished from a valid bound-handler pair.
    HandlerValueUnresolved {
        attribute: String,
        expression: String,
    },
    UncalledAccessor {
        name: String,
        position: String,
    },
    DirectMutation {
        name: String,
        target: DirectMutationTarget,
    },
}

impl StaticDefectKind {
    /// Whether this defect is an unresolved proof obligation — the `SC9xxx`
    /// uncertifiable class — rather than a proven violation. Contract
    /// emission refuses to describe a surface these are open against, and
    /// the metrics count them as unresolved; both consult this so the
    /// answer cannot drift between them.
    #[must_use]
    pub fn is_unresolved_obligation(&self) -> bool {
        matches!(
            self,
            Self::PackageContractExportMissing { .. }
                | Self::PackageContractEnvironmentDependent { .. }
                | Self::UnknownCallbackExecution { .. }
                | Self::ReactiveSourceUncaptured { .. }
                | Self::ReactiveDispatchUnresolved { .. }
                | Self::ReactiveCallbackUnresolved { .. }
                | Self::StructuredReturnUnresolved { .. }
        )
    }

    /// Whether contract emission refuses this obligation through
    /// [`Program::contract_generation_obligations`] rather than through the
    /// project-wide defect list.
    ///
    /// Those obligations carry the exported surface identity and the callee
    /// whose timing a contract author has to describe, so emission consults
    /// them only after resolving the requested entrypoint's exports.
    /// Refusing from the defect list first would ignore that filter and block
    /// every entrypoint over an obligation in an unrelated file. The metrics
    /// still count this class through [`Self::is_unresolved_obligation`].
    #[must_use]
    pub fn refused_through_generation_obligations(&self) -> bool {
        matches!(self, Self::UnknownCallbackExecution { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DirectMutationTarget {
    Props,
    Store,
    ReactiveValue,
    AccessorBinding,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimitiveCreation {
    pub primitive: String,
    pub location: Location,
    pub returned_closure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerRequirement {
    pub operation: OwnerRequirementOperation,
    pub location: Location,
    pub uncertain: bool,
    /// Allocation itself depends on a runtime fact not available to the
    /// analyzer (for example server-entry selection or spread arity).
    #[serde(default, skip_serializing_if = "is_false")]
    pub runtime_uncertain: bool,
    /// The containing exported function may be called with or without an
    /// owner, and its callers cannot be enumerated.
    #[serde(default, skip_serializing_if = "is_false")]
    pub caller_uncertain: bool,
    /// The uncertainty specifically comes from a nullable owner supplied to
    /// `runWithOwner`, rather than an exported function's unknown callers.
    #[serde(default, skip_serializing_if = "is_false")]
    pub conditional_owner: bool,
    /// The containing Solid 1 function is component-shaped only by a naming
    /// convention; JSX invocation and ordinary invocation imply different
    /// owner contexts.
    #[serde(default, skip_serializing_if = "is_false")]
    pub component_uncertain: bool,
    pub report: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnerRequirementOperation {
    Effect,
    Cleanup,
    Boundary,
    SettledCleanup,
}

impl OwnerRequirementOperation {
    #[must_use]
    pub(crate) fn from_internal(operation: &str) -> Self {
        match operation {
            "effect" => Self::Effect,
            "cleanup" => Self::Cleanup,
            "boundary" => Self::Boundary,
            "settled-cleanup" => Self::SettledCleanup,
            _ => panic!("owner analysis emitted unknown operation {operation:?}"),
        }
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
}

fn validate_contract_return(returned: &ContractReturn) -> Result<(), &'static str> {
    validate_claim_evidence(returned.evidence.as_ref())?;
    match returned.kind.as_str() {
        "accessor" | "store-path" => {
            if returned.label.is_empty() || returned.parameter.is_some() {
                return Err("a reactive leaf requires a label");
            }
            if !returned.elements.is_empty() || !returned.properties.is_empty() {
                return Err("a reactive leaf cannot contain elements or properties");
            }
        }
        "tuple" => {
            if !returned.label.is_empty()
                || returned.parameter.is_some()
                || returned.elements.is_empty()
                || !returned.properties.is_empty()
            {
                return Err("a tuple requires elements only");
            }
            for element in returned.elements.iter().flatten() {
                validate_contract_return(element)?;
            }
        }
        "object" => {
            if !returned.label.is_empty()
                || returned.parameter.is_some()
                || returned.properties.is_empty()
                || !returned.elements.is_empty()
                || returned.properties.keys().any(String::is_empty)
            {
                return Err("an object requires named properties only");
            }
            for property in returned.properties.values() {
                validate_contract_return(property)?;
            }
        }
        "argument" => {
            if returned.parameter.is_none()
                || !returned.label.is_empty()
                || !returned.elements.is_empty()
                || !returned.properties.is_empty()
            {
                return Err("an argument return requires a parameter only");
            }
        }
        _ => return Err("the return kind is unsupported"),
    }
    Ok(())
}

fn validate_claim_evidence(evidence: Option<&ContractClaimEvidence>) -> Result<(), &'static str> {
    let Some(evidence) = evidence else {
        return Ok(());
    };
    match evidence.kind.as_str() {
        "inferred" | "reviewed" => {
            if !evidence.modes.is_empty()
                || evidence.calls.is_some()
                || !evidence.package.is_empty()
                || !evidence.version.is_empty()
            {
                return Err(
                    "inferred or reviewed claim evidence cannot carry probe or inheritance details",
                );
            }
        }
        "probed" => {
            if evidence.modes.is_empty()
                || evidence.modes.iter().any(String::is_empty)
                || evidence.modes.iter().collect::<HashSet<_>>().len() != evidence.modes.len()
                || evidence.calls.is_none_or(|calls| calls == 0)
                || !evidence.package.is_empty()
                || !evidence.version.is_empty()
            {
                return Err(
                    "probed claim evidence requires unique modes and a positive call count",
                );
            }
        }
        "inherited-from" => {
            if evidence.package.is_empty()
                || evidence.version.is_empty()
                || !evidence.modes.is_empty()
                || evidence.calls.is_some()
            {
                return Err("inherited claim evidence requires an exact package and version");
            }
        }
        _ => return Err("claim evidence kind is unsupported"),
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsyncRead {
    pub accessor: Arc<str>,
    pub location: Location,
    pub declaration: Location,
    pub execution: ExecutionRole,
    pub leaf_owner: Option<Arc<str>>,
    pub under_loading: bool,
    /// The source's computation is proven async (returns a Promise or
    /// AsyncIterable). False only for rows that exist purely because the
    /// source declares a bare `ssrSource: "client"` (see
    /// [`AsyncRead::ssr_client_hole`]) — a client source can be fully
    /// synchronous.
    #[serde(default = "default_async_provenance")]
    pub async_provenance: bool,
    /// The source provably declares `loadingValue` (or a store-family
    /// `seedLoadingValue: true`): it is born committed, so its first flight
    /// never suspends readers and never trips a `Loading` boundary. Probed
    /// against rc.0: the exemption ends at the first real answer — later
    /// re-asks throw for untracked/leaf reads exactly like undeclared nodes,
    /// so SC5001/SC5002 stay reported with conditional wording while SC5003
    /// is suppressed.
    #[serde(default)]
    pub declared_loading: bool,
    /// An options argument exists on the source that the analyzer cannot
    /// read, so the loadingValue declaration can be neither proven nor
    /// refuted; SC5001 downgrades from proven violation to uncertifiable.
    #[serde(default)]
    pub options_opaque: bool,
    /// The source provably declares `ssrSource: "client"` with no
    /// `loadingValue`/`seedLoadingValue`, and the project server-renders:
    /// reading it during SSR outside a `Loading` boundary throws
    /// unconditionally (SC5005).
    #[serde(default)]
    pub ssr_client_hole: bool,
    /// The source is a proven bare `ssrSource: "client"` source, but whether
    /// the application server-renders cannot be decided from the analyzed
    /// project. SC5005 reports this as uncertifiable instead of treating a
    /// missing server-entry import as proof of CSR.
    #[serde(default)]
    pub server_rendering_unresolved: bool,
}

fn default_async_provenance() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageContract {
    pub schema_version: u32,
    pub package: ContractPackage,
    #[serde(default)]
    pub compiler_facts_protocol: u32,
    #[serde(default)]
    pub artifacts: ContractArtifacts,
    #[serde(default)]
    pub entrypoints: BTreeMap<String, ContractEntrypoint>,
    #[serde(default)]
    pub evidence: ContractEvidence,
    #[serde(skip)]
    pub contract_hash: String,
    #[serde(skip)]
    pub source_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractPackage {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub integrity: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractEntrypoint {
    #[serde(default)]
    pub exports: BTreeMap<String, ContractExport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractEvidence {
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generator: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractClaimEvidence {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<usize>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub package: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractExport {
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ContractClaimEvidence>,
    /// Complete summaries for conditional runtime targets whose behavior is
    /// not identical. The top-level summary remains the conservative union
    /// used by legacy readers; environment-unaware consumers must fail closed
    /// when this field is present rather than apply that union as a guess.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<ContractExportVariant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactive_reads: Vec<ContractReactiveRead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<ContractReturn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callbacks: Vec<ContractCallback>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub async_behavior: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractExportVariant {
    pub conditions: Vec<String>,
    pub summary: Box<ContractExport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractReactiveRead {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ContractClaimEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCallback {
    pub parameter: usize,
    pub execution: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ContractClaimEvidence>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractReturn {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ContractClaimEvidence>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Option<ContractReturn>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, ContractReturn>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractArtifacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<ContractArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<ContractArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractArtifact {
    pub path: String,
    pub hash: String,
}

impl PackageContract {
    /// Stable identity for every contract input that can affect analysis or
    /// diagnostics.
    ///
    /// Decoded contracts normally carry the hash of their source document;
    /// programmatically constructed contracts do not, so the canonical
    /// serialized model is included as a fallback. The source path remains
    /// part of the identity because it is observable in evidence locations.
    #[must_use]
    pub fn analysis_fingerprint(&self) -> [u8; 32] {
        fn field(hasher: &mut Sha256, value: &[u8]) {
            hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(value);
        }

        let mut hasher = Sha256::new();
        field(&mut hasher, self.source_path.as_bytes());
        field(&mut hasher, self.contract_hash.as_bytes());
        if self.contract_hash.is_empty() {
            let encoded = serde_json::to_vec(self)
                .expect("PackageContract contains only infallibly serializable fields");
            field(&mut hasher, &encoded);
        }
        hasher.finalize().into()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "package contract schema version {} is unsupported",
                self.schema_version
            ));
        }
        if self.compiler_facts_protocol != 1 {
            return Err(format!(
                "package contract compiler facts protocol {} is unsupported",
                self.compiler_facts_protocol
            ));
        }
        if self.package.name.is_empty() || self.package.version.is_empty() {
            return Err("package contract requires package.name and package.version".into());
        }
        if !self.package.integrity.is_empty() && !valid_sha512_integrity(&self.package.integrity) {
            return Err("package contract package.integrity is invalid".into());
        }
        if self.entrypoints.is_empty()
            || self.entrypoints.iter().any(|(name, entrypoint)| {
                (name != "." && !name.starts_with("./"))
                    || name == "./"
                    || entrypoint.exports.is_empty()
                    || entrypoint.conditions.iter().any(String::is_empty)
                    || entrypoint.conditions.iter().collect::<HashSet<_>>().len()
                        != entrypoint.conditions.len()
            })
        {
            return Err(
                "package contract entrypoints require exact package subpaths, exports, and unique nonempty conditions"
                    .into(),
            );
        }
        if !matches!(
            self.evidence.kind.as_str(),
            "generated" | "inferred" | "verified" | "reviewed" | "trusted" | "attested"
        ) {
            return Err(format!(
                "package contract evidence kind {:?} is unsupported",
                self.evidence.kind
            ));
        }
        for (name, artifact) in [
            ("declaration", self.artifacts.declaration.as_ref()),
            ("implementation", self.artifacts.implementation.as_ref()),
        ] {
            if let Some(artifact) = artifact
                && (artifact.path.is_empty() || !valid_sha256_hash(&artifact.hash))
            {
                return Err(format!("package contract {name} artifact is invalid"));
            }
        }
        for (entrypoint, exports) in self.export_maps() {
            for (name, summary) in exports {
                self.validate_export(entrypoint, name, summary)?;
            }
        }
        Ok(())
    }

    /// Whether every explicitly annotated claim has certifiable provenance.
    ///
    /// Legacy contracts have no row annotations and deliberately return true;
    /// their existing contract-level evidence gate remains authoritative.
    #[must_use]
    pub fn claims_are_certifiable(&self) -> bool {
        fn evidence_is_certifiable(evidence: Option<&ContractClaimEvidence>) -> bool {
            evidence.is_none_or(|evidence| {
                matches!(
                    evidence.kind.as_str(),
                    "probed" | "reviewed" | "inherited-from"
                )
            })
        }
        fn returned_is_certifiable(returned: &ContractReturn) -> bool {
            evidence_is_certifiable(returned.evidence.as_ref())
                && returned
                    .elements
                    .iter()
                    .flatten()
                    .all(returned_is_certifiable)
                && returned.properties.values().all(returned_is_certifiable)
        }

        fn export_is_certifiable(summary: &ContractExport) -> bool {
            evidence_is_certifiable(summary.evidence.as_ref())
                && summary
                    .reactive_reads
                    .iter()
                    .all(|read| evidence_is_certifiable(read.evidence.as_ref()))
                && summary
                    .callbacks
                    .iter()
                    .all(|callback| evidence_is_certifiable(callback.evidence.as_ref()))
                && summary.returns.as_ref().is_none_or(returned_is_certifiable)
                && summary
                    .variants
                    .iter()
                    .all(|variant| export_is_certifiable(&variant.summary))
        }

        self.entrypoints
            .values()
            .all(|entrypoint| entrypoint.exports.values().all(export_is_certifiable))
    }

    fn validate_export(
        &self,
        entrypoint: &str,
        name: &str,
        summary: &ContractExport,
    ) -> Result<(), String> {
        if name.is_empty() || !matches!(summary.kind.as_str(), "function" | "value") {
            return Err(format!(
                "package contract export {entrypoint}:{name} has unsupported kind {:?}",
                summary.kind
            ));
        }
        validate_claim_evidence(summary.evidence.as_ref()).map_err(|reason| {
            format!(
                "package contract export {entrypoint}:{name} has invalid claim evidence: {reason}"
            )
        })?;
        let mut variant_conditions = HashSet::new();
        for variant in &summary.variants {
            if variant.conditions.is_empty()
                || variant.conditions.iter().any(String::is_empty)
                || variant.conditions.iter().collect::<HashSet<_>>().len()
                    != variant.conditions.len()
                || !variant_conditions.insert(&variant.conditions)
            {
                return Err(format!(
                    "package contract export {entrypoint}:{name} has invalid conditional summary conditions"
                ));
            }
            if variant.summary.kind != summary.kind {
                return Err(format!(
                    "package contract export {entrypoint}:{name} has a conditional summary with kind {:?}, expected {:?}",
                    variant.summary.kind, summary.kind
                ));
            }
            self.validate_export(entrypoint, name, &variant.summary)?;
        }
        if summary.kind == "value"
            && (!summary.reactive_reads.is_empty()
                || summary.returns.is_some()
                || !summary.callbacks.is_empty()
                || !summary.async_behavior.is_empty())
        {
            return Err(format!(
                "package contract value export {entrypoint}:{name} cannot have function effects"
            ));
        }
        for read in &summary.reactive_reads {
            if !matches!(read.kind.as_str(), "accessor" | "store-path") || read.label.is_empty() {
                return Err(format!(
                    "package contract export {entrypoint}:{name} has an invalid reactive read"
                ));
            }
            validate_claim_evidence(read.evidence.as_ref()).map_err(|reason| {
                format!(
                    "package contract export {entrypoint}:{name} has invalid reactive-read evidence: {reason}"
                )
            })?;
        }
        if let Some(returned) = &summary.returns {
            validate_contract_return(returned).map_err(|reason| {
                format!(
                    "package contract export {entrypoint}:{name} has an invalid reactive return: {reason}"
                )
            })?;
        }
        if summary.callbacks.iter().any(|callback| {
            !matches!(
                callback.execution.as_str(),
                "inline" | "tracked" | "deferred"
            )
        }) {
            return Err(format!(
                "package contract export {entrypoint}:{name} has an invalid callback execution"
            ));
        }
        for callback in &summary.callbacks {
            validate_claim_evidence(callback.evidence.as_ref()).map_err(|reason| {
                format!(
                    "package contract export {entrypoint}:{name} has invalid callback evidence: {reason}"
                )
            })?;
        }
        if !summary.async_behavior.is_empty()
            && !matches!(
                summary.async_behavior.as_str(),
                "promise" | "async-iterable"
            )
        {
            return Err(format!(
                "package contract export {entrypoint}:{name} has unsupported async behavior {:?}",
                summary.async_behavior
            ));
        }
        Ok(())
    }

    /// The contract governing an imported module specifier, if any.
    ///
    /// A specifier matches a contract whose package name it equals or extends
    /// with a `/`-separated subpath. Scoped and nested package names can both
    /// prefix the same specifier (`@scope/pkg` and `@scope/pkg-extra`), so the
    /// longest matching name wins.
    pub fn for_module<'a>(contracts: &'a [Self], module: &str) -> Option<&'a Self> {
        contracts
            .iter()
            .filter(|contract| {
                module == contract.package.name
                    || module
                        .strip_prefix(&contract.package.name)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
            .max_by_key(|contract| contract.package.name.len())
    }

    pub fn exports_for_module(&self, module: &str) -> Option<&BTreeMap<String, ContractExport>> {
        let suffix = module.strip_prefix(&self.package.name)?;
        if !suffix.is_empty() && !suffix.starts_with('/') {
            return None;
        }
        let entrypoint = if suffix.is_empty() {
            "."
        } else {
            // Package export maps spell subpaths as "./foo".
            // `suffix` starts with '/', so prefixing '.' produces that form.
            return self
                .entrypoints
                .get(&format!(".{suffix}"))
                .map(|entry| &entry.exports);
        };
        self.entrypoints.get(entrypoint).map(|entry| &entry.exports)
    }

    pub fn root_exports(&self) -> &BTreeMap<String, ContractExport> {
        match self.entrypoints.get(".") {
            Some(entrypoint) => &entrypoint.exports,
            None => empty_contract_exports(),
        }
    }

    pub fn export_count(&self) -> usize {
        self.export_maps().map(|(_, exports)| exports.len()).sum()
    }

    fn export_maps(
        &self,
    ) -> Box<dyn Iterator<Item = (&str, &BTreeMap<String, ContractExport>)> + '_> {
        Box::new(
            self.entrypoints
                .iter()
                .map(|(name, entrypoint)| (name.as_str(), &entrypoint.exports)),
        )
    }
}

fn valid_sha256_hash(hash: &str) -> bool {
    hash.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn valid_sha512_integrity(integrity: &str) -> bool {
    integrity.strip_prefix("sha512-").is_some_and(|digest| {
        digest.len() == 88
            && digest.ends_with("==")
            && digest[..86]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
    })
}

fn empty_contract_exports() -> &'static BTreeMap<String, ContractExport> {
    static EMPTY: std::sync::OnceLock<BTreeMap<String, ContractExport>> =
        std::sync::OnceLock::new();
    EMPTY.get_or_init(BTreeMap::new)
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    pub reads: Vec<ReactiveRead>,
    pub writes: Vec<ReactiveWrite>,
    pub actions: Vec<ActionInvocation>,
    pub leaf_operations: Vec<LeafOwnerOperation>,
    pub static_violations: Vec<StaticViolation>,
    pub static_defects: Vec<StaticDefect>,
    pub directive_creations: Vec<PrimitiveCreation>,
    pub missing_owners: Vec<OwnerRequirement>,
    pub async_reads: Vec<AsyncRead>,
    pub contract_exports: Arc<BTreeMap<String, ContractExport>>,
    pub contract_generation_obligations: Vec<ContractGenerationObligation>,
    pub obligation_counts: ObligationCounts,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractGenerationObligation {
    pub function: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub function_identity: String,
    pub parameter: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub package: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parameter_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub required_execution: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contract_stub: String,
    pub location: Location,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BuildTimings {
    pub total: Duration,
    pub cache_lookup: Duration,
    pub reused: bool,
    pub source_discovery_reused_files: u64,
    pub source_discovery_recomputed_files: u64,
    pub typed_accessor_reused_files: u64,
    pub typed_accessor_recomputed_files: u64,
    pub interprocedural_graph_reused_files: u64,
    pub interprocedural_graph_recomputed_files: u64,
    pub interprocedural_result_reused_files: u64,
    pub interprocedural_result_recomputed_files: u64,
    pub typescript_indexes_reused: bool,
    pub reachability_reused: bool,
    pub reachability_reused_files: u64,
    pub reachability_recomputed_files: u64,
    pub local_accesses_reused: bool,
    pub local_access_reused_files: u64,
    pub local_access_recomputed_files: u64,
    pub interprocedural_reused: bool,
    pub owner_fixed_point_reused: bool,
    pub owner_reused_files: u64,
    pub owner_recomputed_files: u64,
    pub indexes_and_reachability: Duration,
    pub project_indexes: Duration,
    pub alias_and_entity_indexes: Duration,
    pub alias_roots: Duration,
    pub entity_symbols: Duration,
    pub symbol_name_indexes: Duration,
    pub contract_resolution: Duration,
    pub reachability: Duration,
    pub source_discovery: Duration,
    pub typed_accessors_and_prop_roots: Duration,
    pub prop_propagation_and_control_flow: Duration,
    pub static_prepass: Duration,
    pub local_and_interprocedural: Duration,
    pub local_reads_and_writes: Duration,
    pub interprocedural_summaries: Duration,
    pub interprocedural_graph: Duration,
    pub interprocedural_direct_summaries: Duration,
    pub interprocedural_direct_index: Duration,
    pub interprocedural_direct_references: Duration,
    pub interprocedural_typed_accessors: Duration,
    pub interprocedural_propagation: Duration,
    pub interprocedural_returned_direct: Duration,
    pub interprocedural_returned_delta: Duration,
    pub interprocedural_call_summary_delta: Duration,
    pub interprocedural_factory_propagation: Duration,
    pub interprocedural_results_and_exports: Duration,
    pub interprocedural_result_reads: Duration,
    pub interprocedural_export_summaries: Duration,
    pub leaf_and_cleanup: Duration,
    pub static_api: Duration,
    pub directives: Duration,
    pub owner_fixed_point: Duration,
    pub owner_fragment_build: Duration,
    pub owner_graph_assembly: Duration,
    pub owner_propagation: Duration,
    pub owner_requirement_emission: Duration,
    pub final_ordering: Duration,
}

/// Retains the last coherent Reactive IR generation behind the same build
/// interface used by fresh analysis. Cross-generation source discovery,
/// typed-accessor discovery, the symbolic interprocedural graph, and
/// dependency-validated result reads, local accesses, reachability, and owner
/// graph fragments are retained per file; propagated order-sensitive
/// summaries remain complete rebuilds.
#[derive(Default)]
pub struct IncrementalBuilder {
    retained: Option<RetainedBuild>,
    caches: IncrementalCacheState,
}

/// How much derived cross-generation state an idle retained session keeps.
///
/// The current coherent [`Program`] is always retained, so a repeated request
/// for the same generation remains a constant-time shared-pointer lookup.
/// These levels only control the intermediate indexes used to accelerate the
/// next changed generation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CacheRetention {
    /// Keep every incremental index for the lowest edit latency.
    #[default]
    Performance,
    /// Drop the largest low-cost-to-rebuild indexes.
    Balanced,
    /// Keep only the current coherent program.
    Compact,
}

impl IncrementalBuilder {
    pub fn build(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
    ) -> Result<(Program, BuildTimings), BuildError> {
        self.build_with_contracts(facts, dialect, &[])
    }

    /// Build a program behind shared ownership. This is the preferred service
    /// interface: retained generations are returned with an atomic reference
    /// increment instead of cloning every program table.
    pub fn build_shared(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        self.build_with_contracts_shared(facts, dialect, &[], &RuleOptions::default())
    }

    pub fn build_with_contracts(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
        contracts: &[PackageContract],
    ) -> Result<(Program, BuildTimings), BuildError> {
        self.build_with_contracts_shared(facts, dialect, contracts, &RuleOptions::default())
            .map(|(program, timings)| ((*program).clone(), timings))
    }

    /// Build a contract-aware program behind shared ownership.
    pub fn build_with_contracts_shared(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
        contracts: &[PackageContract],
        rule_options: &RuleOptions,
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        let total_started = Instant::now();
        let lookup_started = Instant::now();
        let identity = BuildIdentity {
            dialect: dialect.version(),
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: contracts
                .iter()
                .map(PackageContract::analysis_fingerprint)
                .collect(),
            rule_options: rule_options.clone(),
        };
        if self
            .caches
            .ensure_domain(identity.dialect, &identity.project_id, &identity.contracts)
        {
            self.retained = None;
        }
        let cache_lookup = lookup_started.elapsed();
        if let Some(retained) = &self.retained
            && retained.identity == identity
        {
            let program = Arc::clone(&retained.program);
            return Ok((
                program,
                BuildTimings {
                    total: total_started.elapsed(),
                    cache_lookup,
                    reused: true,
                    ..BuildTimings::default()
                },
            ));
        }
        let (program, mut timings) = build_with_contracts_measured_incremental(
            facts,
            dialect,
            contracts,
            rule_options,
            self.caches.for_build(),
        )?;
        let program = Arc::new(program);
        self.retained = Some(RetainedBuild {
            identity,
            program: Arc::clone(&program),
        });
        timings.total = total_started.elapsed();
        timings.cache_lookup = cache_lookup;
        Ok((program, timings))
    }

    pub fn clear(&mut self) {
        self.retained = None;
        self.caches.clear();
    }

    /// Applies the idle-memory policy without invalidating the current result.
    ///
    /// `Balanced` targets the three cache families that profiling found to
    /// account for most retained bytes per millisecond of recomputation.
    /// `Compact` releases every derived index while preserving the current
    /// generation and its source-discovery domain identity.
    pub fn retain_for_idle(&mut self, retention: CacheRetention) {
        self.caches.retain_for_idle(retention);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObligationCounts {
    pub strict_reads: usize,
    pub writes_and_actions: usize,
    pub factory_instances: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReactiveSourceKind {
    Accessor,
    Store,
}

#[derive(Clone)]
struct FunctionNode {
    path: String,
    span: Span,
    body: Span,
    name: Option<String>,
    symbol: Option<SymbolId>,
}

impl FunctionBoundary for FunctionNode {
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Span {
        self.body
    }
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("fact location offset does not fit Oxc span")]
    SpanWidth,
}

fn push_unique_summary_read(reads: &mut Vec<SummaryRead>, read: SummaryRead) {
    if !reads.iter().any(|existing| {
        existing.display == read.display
            && existing.origin == read.origin
            && existing.declaration == read.declaration
    }) {
        reads.push(read);
    }
}

fn propagate_returned_summary_deltas(summaries: &mut [SummaryReads], edges: &[(usize, usize)]) {
    let mut propagated_lengths = vec![0; edges.len()];
    for _ in 0..summaries.len() {
        let mut changed = false;
        for (edge_index, (owner, target)) in edges.iter().copied().enumerate() {
            let start = propagated_lengths[edge_index];
            let propagated = summaries[target].ordered[start..].to_vec();
            propagated_lengths[edge_index] = summaries[target].len();
            for read in propagated {
                changed |= summaries[owner].push_unique(read);
            }
        }
        if !changed {
            break;
        }
    }
}

fn propagate_summary_deltas(
    summaries: &mut [SummaryReads],
    reverse_edges: &[Vec<usize>],
    propagated_lengths: &mut [usize],
) {
    let mut queued = summaries
        .iter()
        .zip(propagated_lengths.iter())
        .map(|(summary, propagated)| summary.len() > *propagated)
        .collect::<Vec<_>>();
    let mut worklist = queued
        .iter()
        .enumerate()
        .filter_map(|(index, queued)| queued.then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(target) = worklist.pop_front() {
        queued[target] = false;
        let start = propagated_lengths[target];
        let propagated = summaries[target].ordered[start..].to_vec();
        propagated_lengths[target] = summaries[target].len();
        for owner in reverse_edges[target].iter().copied() {
            let mut changed = false;
            for read in &propagated {
                changed |= summaries[owner].push_unique(read.clone());
            }
            if changed && !queued[owner] {
                queued[owner] = true;
                worklist.push_back(owner);
            }
        }
    }
}

fn contract_callback_execution(execution: ExecutionRole) -> Option<&'static str> {
    match execution {
        // Unknown timing is a contract-generation obligation, never an inline
        // promise. A consumer must not execute user code eagerly because the
        // producer lacked an execution proof.
        ExecutionRole::Unknown => None,
        ExecutionRole::ModuleInitialization => Some("inline"),
        ExecutionRole::TrackedJsx => Some("tracked"),
        ExecutionRole::DeferredCallback | ExecutionRole::UntrackedCallback => Some("deferred"),
        ExecutionRole::EffectApply
        | ExecutionRole::EventCallback
        | ExecutionRole::DirectiveApply
        | ExecutionRole::UntrackedRendering => Some("inline"),
    }
}

fn push_contract_callback(callbacks: &mut Vec<ContractCallback>, callback: ContractCallback) {
    if !callbacks.contains(&callback) {
        callbacks.push(callback);
    }
}

fn function_indices_by_path<T>(functions: &[T]) -> HashMap<String, Vec<usize>>
where
    T: FunctionBoundary,
{
    let mut by_path = HashMap::<String, Vec<usize>>::new();
    for (index, function) in functions.iter().enumerate() {
        by_path
            .entry(function.path().to_owned())
            .or_default()
            .push(index);
    }
    by_path
}

fn functions_for_path<'a, T>(
    functions: &'a [T],
    by_path: &'a HashMap<String, Vec<usize>>,
    path: &str,
) -> impl Iterator<Item = (usize, &'a T)> + 'a {
    by_path
        .get(path)
        .into_iter()
        .flatten()
        .copied()
        .map(|index| (index, &functions[index]))
}

fn containing_function_indexed<T>(
    functions: &[T],
    by_path: &HashMap<String, Vec<usize>>,
    path: &str,
    span: Span,
) -> Option<usize>
where
    T: FunctionBoundary,
{
    by_path
        .get(path)?
        .iter()
        .copied()
        .filter(|index| functions[*index].body().contains(span))
        .min_by_key(|index| {
            let body = functions[*index].body();
            body.end - body.start
        })
}

fn containing_summary_function_indexed(
    functions: &[SummaryNode],
    by_path: &HashMap<String, Vec<usize>>,
    path: &str,
    span: Span,
) -> Option<usize> {
    containing_function_indexed(functions, by_path, path, span)
}

trait FunctionBoundary {
    fn path(&self) -> &str;
    fn body(&self) -> Span;
}

fn location_order(left: &Location, right: &Location) -> std::cmp::Ordering {
    (&left.path, left.start_byte, left.end_byte).cmp(&(
        &right.path,
        right.start_byte,
        right.end_byte,
    ))
}

/// A callee or JSX tag, resolved against the dialect's vocabulary.
///
/// Two things the engine needs and [`solid_dialect::Primitive`] alone cannot
/// give it, which is why this type exists:
///
/// 1. A name the dialect does not export is not an error. `useUser()` is a
///    call like any other, and the bundled contracts are keyed by *spelling*,
///    so an unrecognised callee has to keep the one it was written with.
/// 2. Even a recognised primitive is spelled into diagnostics and hints, and
///    the spelling is dialect-specific.
///
/// So the resolved primitive and the source spelling travel together. Ask
/// [`PrimitiveName::primitive`] the vocabulary questions and
/// [`PrimitiveName::as_str`] only the ones a human will read.
#[derive(Clone, Debug, Eq, PartialEq)]
enum PrimitiveName {
    /// A primitive this dialect exports, with the dialect's spelling for it.
    Known(Primitive, &'static str),
    /// A name this dialect does not export, carrying the spelling it was
    /// written with.
    Other(String),
}

impl PrimitiveName {
    /// Resolves a source-level name against the dialect's vocabulary.
    fn new(name: &str, dialect: &dyn Dialect) -> Self {
        match dialect
            .primitive(name)
            .and_then(|primitive| Some((primitive, dialect.name_of(primitive)?)))
        {
            Some((primitive, spelling)) => Self::Known(primitive, spelling),
            None => Self::Other(name.to_owned()),
        }
    }

    /// The dialect primitive this name denotes, or `None` when the dialect
    /// does not export it.
    fn primitive(&self) -> Option<Primitive> {
        match self {
            Self::Known(primitive, _) => Some(*primitive),
            Self::Other(_) => None,
        }
    }

    /// The source spelling. For messages and for the spelling-keyed bundled
    /// contract table -- never for asking what a callee *is*.
    fn as_str(&self) -> &str {
        match self {
            Self::Known(_, spelling) => spelling,
            Self::Other(name) => name,
        }
    }
}

/// The dialect primitive a resolved callee denotes.
///
/// The `Option<PrimitiveName>` the resolvers return already means "did this
/// callee resolve at all"; this flattens it with "and is it vocabulary the
/// dialect knows", which is the question nearly every classifier asks.
fn known_primitive(name: &Option<PrimitiveName>) -> Option<Primitive> {
    name.as_ref().and_then(PrimitiveName::primitive)
}

impl std::ops::Deref for PrimitiveName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq<&str> for PrimitiveName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl std::fmt::Display for PrimitiveName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn primitive_name(
    path: &str,
    span: Span,
    static_callee: Option<&str>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    let location = location(path, span);
    if let Some(symbol) = entities.get(&location) {
        symbol_names
            .get(symbol)
            .map(|name| PrimitiveName::new(name, dialect))
            .or_else(|| {
                let property = static_callee?.rsplit('.').next()?;
                symbol_names
                    .get(format!("{symbol}::{property}").as_str())
                    .map(|name| PrimitiveName::new(name, dialect))
            })
    } else {
        None
    }
}

fn jsx_primitive_name(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    primitive_name(
        file.path.as_str(),
        element.name.span,
        Some(file.source_text(element.name.span).unwrap_or_default()),
        entities,
        symbol_names,
        dialect,
    )
    .or_else(|| {
        let object = element.member_object?;
        let property = element.member_property?;
        let property_name = file.source_text(property)?;
        let object_symbol = entities.at(file.path.as_str(), object)?;
        let namespace_import = file.ast.imports.iter().any(|import| {
            dialect.owns_module(&import.module)
                && dialect
                    .namespace_import_primitives(&import.module)
                    .contains(&property_name)
                && import.bindings.iter().any(|binding| {
                    binding.kind == solid_facts::ast::ImportKind::Namespace
                        && entities.at(file.path.as_str(), binding.local.span)
                            == Some(object_symbol)
                })
        });
        namespace_import.then(|| {
            symbol_names
                .get(format!("{object_symbol}::{property_name}").as_str())
                .map(|name| PrimitiveName::new(name, dialect))
        })?
    })
    .or_else(|| {
        file.ast
            .imports
            .iter()
            .filter(|import| dialect.owns_module(&import.module))
            .flat_map(|import| &import.bindings)
            .find_map(|binding| {
                (binding.kind != solid_facts::ast::ImportKind::Namespace
                    && file.source_text(binding.local.span) == file.source_text(element.name.span))
                .then_some(binding.imported.as_deref())
                .flatten()
            })
            .map(|name| PrimitiveName::new(name, dialect))
    })
}

fn location(path: impl Into<Arc<str>>, span: Span) -> Location {
    span.location(path)
}

#[cfg(test)]
mod tests {
    use super::cache::{
        CachedTypeScriptIndexes, InterproceduralResultDependency,
        InterproceduralResultDependencyState, SourceDiscoveryIdentity,
        SourceDiscoveryTypeScriptDelta,
    };
    use solid_facts::TypeScriptTable;
    use solid_facts::core::SourceHash;
    use typefacts::{Declaration, EntityFact, FileFact, SourceDigest, SymbolFact};

    use super::interproc::InterproceduralResultView;
    use super::pipeline::{AnalysisWorkerLimit, parallel_slice_results};
    use super::source_discovery::source_discovery_identity_matches;
    use super::symbols::{
        alias_roots_and_source_declarations, entity_symbols, patch_typescript_indexes,
        references_for_sources, source_discovery_symbol_semantics, symbol_alias_targets,
        symbol_names, symbols_by_root,
    };
    use super::*;

    #[test]
    fn ordered_parallel_maps_respect_the_worker_budget() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let items = (0..512).collect::<Vec<_>>();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let parallel = {
            let _worker_limit = AnalysisWorkerLimit::enter(2);
            parallel_slice_results(&items, |item| {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                for _ in 0..32 {
                    std::thread::yield_now();
                }
                active.fetch_sub(1, Ordering::SeqCst);
                item * 2
            })
        };
        let sequential = {
            let _worker_limit = AnalysisWorkerLimit::enter(1);
            parallel_slice_results(&items, |item| item * 2)
        };

        assert_eq!(parallel, sequential);
        assert!(peak.load(Ordering::SeqCst) <= 2);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn primitive_names_resolve_through_the_dialect() {
        assert!(matches!(
            PrimitiveName::new("createEffect", &solid_dialect::Solid2),
            PrimitiveName::Known(Primitive::CreateEffect, "createEffect")
        ));
        // A name outside the dialect's vocabulary keeps its own spelling and
        // answers no vocabulary question.
        assert!(matches!(
            PrimitiveName::new("projectSpecificHelper", &solid_dialect::Solid2),
            PrimitiveName::Other(_)
        ));
        // The same spelling can be vocabulary in one dialect and not the
        // other: `flush` is 2.0-only, `batch` is 1.x-only.
        assert!(matches!(
            PrimitiveName::new("flush", &solid_dialect::Solid1x),
            PrimitiveName::Other(_)
        ));
        assert!(matches!(
            PrimitiveName::new("batch", &solid_dialect::Solid1x),
            PrimitiveName::Known(Primitive::Batch, "batch")
        ));
    }

    #[test]
    fn package_contract_validation_enforces_release_identity_and_surface() {
        let valid = PackageContract {
            schema_version: 1,
            package: ContractPackage {
                name: "reactive-package".into(),
                version: "1.0.0".into(),
                integrity: String::new(),
            },
            compiler_facts_protocol: 1,
            artifacts: ContractArtifacts::default(),
            entrypoints: BTreeMap::from([(
                ".".into(),
                ContractEntrypoint {
                    exports: BTreeMap::from([(
                        "createValue".into(),
                        ContractExport {
                            kind: "function".into(),
                            ..ContractExport::default()
                        },
                    )]),
                    conditions: Vec::new(),
                },
            )]),
            evidence: ContractEvidence {
                kind: "verified".into(),
                generator: String::new(),
            },
            contract_hash: String::new(),
            source_path: String::new(),
        };
        assert!(valid.validate().is_ok());

        let mut no_version = valid.clone();
        no_version.package.version.clear();
        assert!(no_version.validate().is_err());

        let mut no_entrypoints = valid.clone();
        no_entrypoints.entrypoints.clear();
        assert!(no_entrypoints.validate().is_err());

        let mut malformed_hash = valid;
        malformed_hash.artifacts.declaration = Some(ContractArtifact {
            path: "index.d.ts".into(),
            hash: "sha256:not-a-digest".into(),
        });
        assert!(malformed_hash.validate().is_err());
    }

    #[test]
    fn claim_evidence_is_optional_but_certification_rejects_inferred_rows() {
        let mut contract = PackageContract {
            schema_version: 1,
            package: ContractPackage {
                name: "reactive-package".into(),
                version: "1.0.0".into(),
                integrity: String::new(),
            },
            compiler_facts_protocol: 1,
            artifacts: ContractArtifacts::default(),
            entrypoints: BTreeMap::from([(
                ".".into(),
                ContractEntrypoint {
                    exports: BTreeMap::from([(
                        "createValue".into(),
                        ContractExport {
                            kind: "function".into(),
                            callbacks: vec![ContractCallback {
                                parameter: 0,
                                execution: "tracked".into(),
                                evidence: Some(ContractClaimEvidence {
                                    kind: "inferred".into(),
                                    ..ContractClaimEvidence::default()
                                }),
                            }],
                            ..ContractExport::default()
                        },
                    )]),
                    conditions: Vec::new(),
                },
            )]),
            evidence: ContractEvidence {
                kind: "verified".into(),
                generator: String::new(),
            },
            contract_hash: String::new(),
            source_path: String::new(),
        };
        assert!(contract.validate().is_ok());
        assert!(!contract.claims_are_certifiable());

        contract
            .entrypoints
            .get_mut(".")
            .unwrap()
            .exports
            .get_mut("createValue")
            .unwrap()
            .callbacks[0]
            .evidence = Some(ContractClaimEvidence {
            kind: "probed".into(),
            modes: vec!["browser".into(), "server".into()],
            calls: Some(2),
            ..ContractClaimEvidence::default()
        });
        assert!(contract.validate().is_ok());
        assert!(contract.claims_are_certifiable());

        let mut malformed = contract.clone();
        malformed
            .entrypoints
            .get_mut(".")
            .unwrap()
            .exports
            .get_mut("createValue")
            .unwrap()
            .callbacks[0]
            .evidence = Some(ContractClaimEvidence {
            kind: "inherited-from".into(),
            package: "solid-js".into(),
            ..ContractClaimEvidence::default()
        });
        assert!(malformed.validate().is_err());

        let mut conditional = contract.clone();
        conditional
            .entrypoints
            .get_mut(".")
            .unwrap()
            .exports
            .get_mut("createValue")
            .unwrap()
            .variants = vec![ContractExportVariant {
            conditions: vec!["browser".into()],
            summary: Box::new(ContractExport {
                kind: "function".into(),
                callbacks: vec![ContractCallback {
                    parameter: 0,
                    execution: "tracked".into(),
                    evidence: Some(ContractClaimEvidence {
                        kind: "probed".into(),
                        modes: vec!["client".into()],
                        calls: Some(2),
                        ..ContractClaimEvidence::default()
                    }),
                }],
                ..ContractExport::default()
            }),
        }];
        assert!(conditional.validate().is_ok());
        assert!(conditional.claims_are_certifiable());

        conditional
            .entrypoints
            .get_mut(".")
            .unwrap()
            .exports
            .get_mut("createValue")
            .unwrap()
            .variants[0]
            .conditions
            .clear();
        assert!(conditional.validate().is_err());
    }

    #[test]
    fn schema_one_accepts_structured_returns_and_rejects_mixed_shapes() {
        let leaf = ContractReturn {
            kind: "accessor".into(),
            label: "active".into(),
            ..ContractReturn::default()
        };
        let structured = ContractReturn {
            kind: "tuple".into(),
            elements: vec![
                Some(ContractReturn {
                    kind: "store-path".into(),
                    label: "query".into(),
                    ..ContractReturn::default()
                }),
                Some(ContractReturn {
                    kind: "object".into(),
                    properties: BTreeMap::from([("active".into(), leaf.clone())]),
                    ..ContractReturn::default()
                }),
            ],
            ..ContractReturn::default()
        };
        assert!(validate_contract_return(&structured).is_ok());

        let argument = ContractReturn {
            kind: "argument".into(),
            parameter: Some(0),
            ..ContractReturn::default()
        };
        assert!(validate_contract_return(&argument).is_ok());

        let mixed = ContractReturn {
            kind: "object".into(),
            label: "invalid".into(),
            properties: BTreeMap::from([("active".into(), leaf)]),
            ..ContractReturn::default()
        };
        assert!(validate_contract_return(&mixed).is_err());
    }

    fn summary_node(path: &str, span: Span, body: Span) -> SummaryNode {
        SummaryNode {
            path: path.into(),
            span,
            body,
            name: None,
            symbol: None,
            runtime_identity: String::new(),
            parameters: Vec::new(),
            exported: false,
            r#async: false,
        }
    }

    fn summary_read(symbol: &str, display: &str, start: u64) -> SummaryRead {
        SummaryRead {
            symbol: symbol.into(),
            display: display.into(),
            kind: Some("accessor".into()),
            declaration: Location {
                path: "fixture.tsx".into(),
                start_byte: start,
                end_byte: start + 1,
            },
            origin: Location {
                path: "fixture.tsx".into(),
                start_byte: start + 10,
                end_byte: start + 11,
            },
            origin_context: symbol.into(),
        }
    }

    fn declaration(name: &str, path: &str, start: u64) -> Declaration {
        Declaration {
            name: name.into(),
            kind: "const".into(),
            location: Location {
                path: path.into(),
                start_byte: start,
                end_byte: start + 1,
            },
        }
    }

    fn typescript_table(
        generation: u64,
        sources: Vec<SourceDigest>,
        entities: Vec<EntityFact>,
        symbols: Vec<SymbolFact>,
        files: Vec<FileFact>,
    ) -> TypeScriptTable {
        TypeScriptTable::from_parts(3, generation, "fixture", sources, entities, symbols, files)
    }

    fn empty_project(generation: u64) -> ProjectFacts {
        ProjectFacts {
            generation: solid_facts::core::Generation::new(generation).unwrap(),
            project_id: "fixture".into(),
            files: Vec::new(),
            typescript: typescript_table(
                generation,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
        }
    }

    #[test]
    fn projected_references_include_every_alias_member_without_retaining_unrelated_symbols() {
        let reference = |start| Location {
            path: "fixture.ts".into(),
            start_byte: start,
            end_byte: start + 1,
        };
        let table = typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![
                SymbolFact {
                    id: "root".into(),
                    alias_target: "".into(),
                    declarations: Vec::new().into(),
                    references: vec![reference(30)].into(),
                },
                SymbolFact {
                    id: "alias".into(),
                    alias_target: "root".into(),
                    declarations: Vec::new().into(),
                    references: vec![reference(10)].into(),
                },
                SymbolFact {
                    id: "unrelated".into(),
                    alias_target: "".into(),
                    declarations: Vec::new().into(),
                    references: vec![reference(20)].into(),
                },
            ],
            Vec::new(),
        );
        let interner = SymbolInterner::from_table(&table);
        let (aliases, _) = alias_roots_and_source_declarations(&table, &interner);
        let roots = symbols_by_root(&table, &aliases, &interner);
        let source = SymbolId::from("root");
        let projected = references_for_sources(&table, &roots, std::iter::once(&source));

        assert_eq!(
            projected[&source]
                .iter()
                .map(|location| location.start_byte)
                .collect::<Vec<_>>(),
            vec![10, 30]
        );
        assert_eq!(projected.len(), 1);
    }

    #[test]
    fn compact_source_identity_reuses_only_exact_typefacts_manifests() {
        let source_hash = SourceHash::of("source");
        let cached = SourceDiscoveryIdentity {
            source_hash: source_hash.clone(),
            symbols: vec![SymbolId::from("source-symbol")],
        };
        let exact = SourceDiscoveryTypeScriptDelta {
            entity_paths: HashSet::new(),
            file_paths: HashSet::new(),
            semantic_symbol_ids: HashSet::new(),
        };
        assert!(source_discovery_identity_matches(
            &cached,
            "fixture.ts",
            &source_hash,
            false,
            Some(&exact),
        ));
        assert!(!source_discovery_identity_matches(
            &cached,
            "fixture.ts",
            &source_hash,
            false,
            None,
        ));

        let affected = SourceDiscoveryTypeScriptDelta {
            entity_paths: HashSet::from(["fixture.ts".into()]),
            file_paths: HashSet::new(),
            semantic_symbol_ids: HashSet::new(),
        };
        assert!(!source_discovery_identity_matches(
            &cached,
            "fixture.ts",
            &source_hash,
            false,
            Some(&affected),
        ));
        let changed_symbol = SourceDiscoveryTypeScriptDelta {
            entity_paths: HashSet::new(),
            file_paths: HashSet::new(),
            semantic_symbol_ids: HashSet::from([SymbolId::from("source-symbol")]),
        };
        assert!(!source_discovery_identity_matches(
            &cached,
            "fixture.ts",
            &source_hash,
            false,
            Some(&changed_symbol),
        ));
    }

    fn typescript_index_cache(table: &TypeScriptTable) -> CachedTypeScriptIndexes {
        let interner = SymbolInterner::from_table(table);
        let (aliases, source_declarations) = alias_roots_and_source_declarations(table, &interner);
        CachedTypeScriptIndexes {
            symbol_alias_targets: symbol_alias_targets(table, &interner),
            symbols_by_root: symbols_by_root(table, &aliases, &interner),
            entities: entity_symbols(table, &aliases, &interner),
            symbol_names: symbol_names(table, &aliases, &interner, &solid_dialect::Solid2),
            source_discovery_symbol_semantics: source_discovery_symbol_semantics(table, &interner),
            source_discovery_delta: None,
            aliases,
            source_declarations,
            interner,
        }
    }

    #[test]
    fn incremental_builder_reuses_only_the_same_coherent_generation() {
        let first = empty_project(1);
        let fresh = build(&first, &solid_dialect::Solid2).unwrap();
        let mut incremental = IncrementalBuilder::default();

        let (initial, initial_timings) = incremental.build(&first, &solid_dialect::Solid2).unwrap();
        let (reused, reused_timings) = incremental.build(&first, &solid_dialect::Solid2).unwrap();
        let mut next_facts = empty_project(2);
        next_facts.typescript_changes = Some(solid_facts::TypeScriptChanges {
            unchanged: true,
            ..solid_facts::TypeScriptChanges::default()
        });
        let (next, next_timings) = incremental
            .build(&next_facts, &solid_dialect::Solid2)
            .unwrap();

        assert_eq!(initial, fresh);
        assert_eq!(reused, fresh);
        assert_eq!(next, fresh);
        assert!(!initial_timings.reused);
        assert!(reused_timings.reused);
        assert!(!next_timings.reused);
        assert!(next_timings.typescript_indexes_reused);
    }

    #[test]
    fn idle_retention_preserves_results_and_rebuilds_released_indexes() {
        let first = empty_project(1);
        let fresh = build(&first, &solid_dialect::Solid2).unwrap();
        let mut incremental = IncrementalBuilder::default();

        let (initial, _) = incremental.build(&first, &solid_dialect::Solid2).unwrap();
        incremental.retain_for_idle(CacheRetention::Balanced);
        let (same_generation, same_timings) =
            incremental.build(&first, &solid_dialect::Solid2).unwrap();
        incremental.retain_for_idle(CacheRetention::Compact);

        let mut next_facts = empty_project(2);
        next_facts.typescript_changes = Some(solid_facts::TypeScriptChanges {
            unchanged: true,
            ..solid_facts::TypeScriptChanges::default()
        });
        let (next, next_timings) = incremental
            .build(&next_facts, &solid_dialect::Solid2)
            .unwrap();

        assert_eq!(initial, fresh);
        assert_eq!(same_generation, fresh);
        assert_eq!(next, fresh);
        assert!(same_timings.reused);
        assert!(!next_timings.reused);
        assert!(!next_timings.typescript_indexes_reused);
    }

    #[test]
    fn shared_builder_reuses_the_program_allocation() {
        let facts = empty_project(1);
        let mut incremental = IncrementalBuilder::default();

        let (initial, initial_timings) = incremental
            .build_shared(&facts, &solid_dialect::Solid2)
            .unwrap();
        let (reused, reused_timings) = incremental
            .build_shared(&facts, &solid_dialect::Solid2)
            .unwrap();

        assert!(Arc::ptr_eq(&initial, &reused));
        assert!(!initial_timings.reused);
        assert!(reused_timings.reused);
    }

    #[test]
    fn source_declaration_index_skips_earlier_dts_only_symbols() {
        let table = typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![
                typefacts::SymbolFact {
                    id: "early".into(),
                    alias_target: "root".into(),
                    declarations: (vec![declaration("Accessor", "solid-js.d.ts", 1)]).into(),
                    references: (Vec::new()).into(),
                },
                typefacts::SymbolFact {
                    id: "later".into(),
                    alias_target: "root".into(),
                    declarations: (vec![
                        declaration("Accessor", "other.d.ts", 2),
                        declaration("sourceAccessor", "source.ts", 3),
                    ])
                    .into(),
                    references: (Vec::new()).into(),
                },
            ],
            Vec::new(),
        );

        let interner = SymbolInterner::from_table(&table);
        let (_, declarations) = alias_roots_and_source_declarations(&table, &interner);

        assert_eq!(declarations["root"].name, ("sourceAccessor").into());
        assert_eq!(declarations["root"].location.path, ("source.ts").into());
    }

    #[test]
    fn exact_index_patch_replaces_local_alias_ids_without_retargeting_the_graph() {
        let symbol = |id: &str, target: &str, declarations: Vec<Declaration>| SymbolFact {
            id: id.into(),
            alias_target: target.into(),
            declarations: declarations.into(),
            references: Vec::new().into(),
        };
        let entity = |symbol: &str, start: u64| EntityFact {
            location: Location {
                path: "fixture.ts".into(),
                start_byte: start,
                end_byte: start + 1,
            },
            symbol: symbol.into(),
            symbol_unresolved: false,
            type_descriptor: None,
            resolved_call: None,
            callability: None,
            runtime_value_domain: None,
            call_result_domain: None,
            constant_value: None,
            array_shape: None,
            tuple_shape: None,
            library_types: None,
            reference_space: None,
            runtime_identity: "".into(),
        };
        let old = typescript_table(
            1,
            Vec::new(),
            vec![entity("old-alias", 10)],
            vec![
                symbol("root", "", vec![declaration("root", "fixture.ts", 1)]),
                SymbolFact {
                    references: vec![Location {
                        path: "fixture.ts".into(),
                        start_byte: 10,
                        end_byte: 11,
                    }]
                    .into(),
                    ..symbol(
                        "old-alias",
                        "root",
                        vec![declaration("root", "fixture.d.ts", 2)],
                    )
                },
            ],
            Vec::new(),
        );
        let current = typescript_table(
            2,
            Vec::new(),
            vec![entity("new-alias", 12)],
            vec![
                symbol("root", "", vec![declaration("root", "fixture.ts", 3)]),
                SymbolFact {
                    references: vec![
                        Location {
                            path: "fixture.ts".into(),
                            start_byte: 13,
                            end_byte: 14,
                        },
                        Location {
                            path: "fixture.ts".into(),
                            start_byte: 12,
                            end_byte: 13,
                        },
                        Location {
                            path: "fixture.ts".into(),
                            start_byte: 12,
                            end_byte: 13,
                        },
                    ]
                    .into(),
                    ..symbol(
                        "new-alias",
                        "root",
                        vec![declaration("root", "fixture.d.ts", 4)],
                    )
                },
            ],
            Vec::new(),
        );
        let symbols_by_id = current
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect::<HashMap<_, _>>();
        let mut patched = typescript_index_cache(&old);
        let changes = solid_facts::TypeScriptChanges {
            unchanged: false,
            entity_paths: vec!["fixture.ts".into()],
            symbol_ids: vec!["new-alias".into(), "old-alias".into(), "root".into()],
            file_paths: Vec::new(),
        };

        assert!(
            patch_typescript_indexes(
                &mut patched,
                &current,
                &symbols_by_id,
                &solid_dialect::Solid2,
                &changes
            )
            .is_some()
        );
        let fresh = typescript_index_cache(&current);
        assert_eq!(patched.symbol_alias_targets, fresh.symbol_alias_targets);
        assert_eq!(patched.aliases, fresh.aliases);
        assert_eq!(patched.source_declarations, fresh.source_declarations);
        assert_eq!(patched.entities, fresh.entities);
        assert_eq!(patched.symbol_names, fresh.symbol_names);
        assert_eq!(
            patched.source_discovery_symbol_semantics,
            fresh.source_discovery_symbol_semantics
        );
        assert_eq!(
            patched
                .source_discovery_delta
                .as_ref()
                .unwrap()
                .semantic_symbol_ids,
            ["new-alias", "old-alias"]
                .into_iter()
                .map(SymbolId::from)
                .collect()
        );
    }

    #[test]
    fn exact_index_patch_does_not_treat_references_as_source_discovery_semantics() {
        let table = |reference_start| {
            typescript_table(
                1,
                Vec::new(),
                Vec::new(),
                vec![SymbolFact {
                    id: "root".into(),
                    alias_target: (String::new()).into(),
                    declarations: (vec![declaration("root", "fixture.ts", 1)]).into(),
                    references: (vec![Location {
                        path: "fixture.ts".into(),
                        start_byte: reference_start,
                        end_byte: reference_start + 1,
                    }])
                    .into(),
                }],
                Vec::new(),
            )
        };
        let old = table(10);
        let current = table(20);
        let symbols_by_id = current
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect::<HashMap<_, _>>();
        let mut patched = typescript_index_cache(&old);
        let changes = solid_facts::TypeScriptChanges {
            unchanged: false,
            entity_paths: Vec::new(),
            symbol_ids: vec!["root".into()],
            file_paths: Vec::new(),
        };

        assert!(
            patch_typescript_indexes(
                &mut patched,
                &current,
                &symbols_by_id,
                &solid_dialect::Solid2,
                &changes
            )
            .is_some()
        );
        assert!(
            patched
                .source_discovery_delta
                .as_ref()
                .unwrap()
                .semantic_symbol_ids
                .is_empty()
        );
    }

    #[test]
    fn exact_index_patch_does_not_treat_declaration_offsets_as_source_semantics() {
        let table = |start| {
            typescript_table(
                1,
                Vec::new(),
                Vec::new(),
                vec![SymbolFact {
                    id: "root".into(),
                    alias_target: (String::new()).into(),
                    declarations: (vec![declaration("root", "fixture.ts", start)]).into(),
                    references: (Vec::new()).into(),
                }],
                Vec::new(),
            )
        };
        let old = table(10);
        let current = table(30);
        let symbols_by_id = current
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect::<HashMap<_, _>>();
        let mut patched = typescript_index_cache(&old);
        let changes = solid_facts::TypeScriptChanges {
            unchanged: false,
            entity_paths: Vec::new(),
            symbol_ids: vec!["root".into()],
            file_paths: Vec::new(),
        };

        assert!(
            patch_typescript_indexes(
                &mut patched,
                &current,
                &symbols_by_id,
                &solid_dialect::Solid2,
                &changes
            )
            .is_some()
        );
        assert_eq!(
            patched.source_declarations,
            typescript_index_cache(&current).source_declarations
        );
        assert!(
            patched
                .source_discovery_delta
                .as_ref()
                .unwrap()
                .semantic_symbol_ids
                .is_empty(),
            "moving a declaration without changing its source semantics must not invalidate importers"
        );
    }

    #[test]
    fn exact_index_patch_does_not_invalidate_when_a_runtime_representative_moves_files() {
        let table = |path| {
            typescript_table(
                1,
                Vec::new(),
                Vec::new(),
                vec![SymbolFact {
                    id: "root".into(),
                    alias_target: (String::new()).into(),
                    declarations: (vec![declaration("createSignal", path, 10)]).into(),
                    references: (Vec::new()).into(),
                }],
                Vec::new(),
            )
        };
        let old = table("a.ts");
        let current = table("b.ts");
        let symbols_by_id = current
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect::<HashMap<_, _>>();
        let mut patched = typescript_index_cache(&old);
        let changes = solid_facts::TypeScriptChanges {
            unchanged: false,
            entity_paths: Vec::new(),
            symbol_ids: vec!["root".into()],
            file_paths: Vec::new(),
        };

        assert!(
            patch_typescript_indexes(
                &mut patched,
                &current,
                &symbols_by_id,
                &solid_dialect::Solid2,
                &changes
            )
            .is_some()
        );
        assert_eq!(
            patched.source_declarations["root"].location.path,
            ("b.ts").into(),
            "the current representative location must still be patched"
        );
        assert!(
            patched
                .source_discovery_delta
                .as_ref()
                .unwrap()
                .semantic_symbol_ids
                .is_empty(),
            "choosing another runtime declaration for the same root must not invalidate importers"
        );
    }

    #[test]
    fn exact_index_patch_invalidates_a_root_when_an_alias_changes_its_source_declaration() {
        let symbol = |id: &str, target: &str, declarations| SymbolFact {
            id: id.into(),
            alias_target: target.into(),
            declarations,
            references: (Vec::new()).into(),
        };
        let old = typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![
                symbol("root", "", (Vec::new()).into()),
                symbol(
                    "old-alias",
                    "root",
                    (vec![declaration("root", "fixture.d.ts", 1)]).into(),
                ),
            ],
            Vec::new(),
        );
        let current = typescript_table(
            2,
            Vec::new(),
            Vec::new(),
            vec![
                symbol("root", "", (Vec::new()).into()),
                symbol(
                    "new-alias",
                    "root",
                    (vec![declaration("root", "fixture.ts", 1)]).into(),
                ),
            ],
            Vec::new(),
        );
        let symbols_by_id = current
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect::<HashMap<_, _>>();
        let mut patched = typescript_index_cache(&old);
        let changes = solid_facts::TypeScriptChanges {
            unchanged: false,
            entity_paths: Vec::new(),
            symbol_ids: vec!["new-alias".into(), "old-alias".into()],
            file_paths: Vec::new(),
        };

        assert!(
            patch_typescript_indexes(
                &mut patched,
                &current,
                &symbols_by_id,
                &solid_dialect::Solid2,
                &changes
            )
            .is_some()
        );
        assert!(
            patched
                .source_discovery_delta
                .as_ref()
                .unwrap()
                .semantic_symbol_ids
                .contains("root")
        );
        assert_eq!(
            patched.source_declarations,
            typescript_index_cache(&current).source_declarations
        );
    }

    #[test]
    fn exact_index_patch_rejects_alias_retargeting() {
        let table = |target: &str| {
            typescript_table(
                1,
                Vec::new(),
                Vec::new(),
                vec![
                    SymbolFact {
                        id: "root-a".into(),
                        alias_target: (String::new()).into(),
                        declarations: (Vec::new()).into(),
                        references: (Vec::new()).into(),
                    },
                    SymbolFact {
                        id: "root-b".into(),
                        alias_target: (String::new()).into(),
                        declarations: (Vec::new()).into(),
                        references: (Vec::new()).into(),
                    },
                    SymbolFact {
                        id: "alias".into(),
                        alias_target: target.into(),
                        declarations: (Vec::new()).into(),
                        references: (Vec::new()).into(),
                    },
                ],
                Vec::new(),
            )
        };
        let old = table("root-a");
        let current = table("root-b");
        let symbols_by_id = current
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect::<HashMap<_, _>>();
        let mut patched = typescript_index_cache(&old);
        let changes = solid_facts::TypeScriptChanges {
            unchanged: false,
            entity_paths: Vec::new(),
            symbol_ids: vec!["alias".into()],
            file_paths: Vec::new(),
        };

        assert!(
            patch_typescript_indexes(
                &mut patched,
                &current,
                &symbols_by_id,
                &solid_dialect::Solid2,
                &changes
            )
            .is_none()
        );
        assert_eq!(patched.aliases, typescript_index_cache(&old).aliases);
    }

    #[test]
    fn summary_containment_selects_innermost_function() {
        let nodes = vec![
            summary_node(
                "fixture.tsx",
                Span { start: 0, end: 100 },
                Span { start: 10, end: 90 },
            ),
            summary_node(
                "fixture.tsx",
                Span { start: 20, end: 60 },
                Span { start: 30, end: 50 },
            ),
        ];
        let by_path = function_indices_by_path(&nodes);

        assert_eq!(
            containing_summary_function_indexed(
                &nodes,
                &by_path,
                "fixture.tsx",
                Span { start: 35, end: 40 },
            ),
            Some(1)
        );
    }

    #[test]
    fn summary_membership_keeps_first_writer_and_ordered_insertions() {
        let mut reads = SummaryReads::default();
        let first = summary_read("first", "signal", 1);
        let mut duplicate = first.clone();
        duplicate.symbol = "second".into();
        duplicate.kind = Some("store-path".into());
        duplicate.origin_context = "different".into();

        assert!(reads.push_unique(first));
        assert!(!reads.push_unique(duplicate));
        reads.insert(0, summary_read("typed", "typed accessor", 2));

        assert_eq!(reads.len(), 2);
        assert_eq!(reads[0].symbol, "typed");
        assert_eq!(reads[1].symbol, "first");
        assert_eq!(reads[1].kind.as_deref(), Some("accessor"));
        assert_eq!(reads[1].origin_context, "first");
    }

    #[test]
    fn returned_summary_deltas_preserve_fixed_edge_order() {
        let mut summaries = vec![
            SummaryReads::default(),
            SummaryReads::default(),
            SummaryReads::default(),
        ];
        summaries[1].push(summary_read("one", "one", 1));
        summaries[2].push(summary_read("two", "two", 2));

        propagate_returned_summary_deltas(&mut summaries, &[(0, 1), (1, 2), (0, 2)]);

        assert_eq!(
            summaries[0]
                .iter()
                .map(|read| read.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
        assert_eq!(
            summaries[1]
                .iter()
                .map(|read| read.symbol.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }

    #[test]
    fn missing_result_dependency_invalidates_when_a_function_appears() {
        let dependency = InterproceduralResultDependency::Symbol("helper".into());
        let mut node = summary_node(
            "fixture.tsx",
            Span { start: 0, end: 10 },
            Span { start: 2, end: 9 },
        );
        node.symbol = Some("helper".into());
        let nodes = vec![node];
        let indexes = HashMap::from([(("fixture.tsx".into(), nodes[0].span), 0)]);
        let summaries = vec![SummaryReads::default()];
        let invoked_parameters = vec![Vec::new()];
        let invoked_parameter_members = vec![Vec::new()];
        let returned_bindings = HashMap::new();
        let missing = InterproceduralResultDependencyState::Missing;

        let missing_by_symbol = HashMap::new();
        let missing_view = InterproceduralResultView {
            nodes: &nodes,
            indexes: &indexes,
            by_symbol: &missing_by_symbol,
            summaries: &summaries,
            invoked_parameters: &invoked_parameters,
            invoked_parameter_members: &invoked_parameter_members,
            returned_bindings: &returned_bindings,
        };
        assert!(missing_view.dependency_matches(&missing, &dependency));

        let by_symbol = HashMap::from([("helper".into(), 0)]);
        let present_view = InterproceduralResultView {
            by_symbol: &by_symbol,
            ..missing_view
        };
        assert!(!present_view.dependency_matches(&missing, &dependency));
    }
}
