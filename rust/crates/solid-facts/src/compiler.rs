//! Typed, validated Solid compiler execution facts.
//!
//! The controlled Oxc Solid compiler emits this model from original source.
//! This crate deliberately contains no compiler implementation and no AST:
//! it is the stable boundary consumed by the Rust analysis engine.

use crate::core::{SourceHash, Span};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::Arc;
use thiserror::Error;

/// The compiler-facts wire version. [`ExecutionMap::validate`] refuses a map
/// that does not carry exactly this value.
///
/// Protocol 2 replaced producer-shaped compatibility arrays with one deeply
/// normalized semantic operation model. The arrays remain serialized only as
/// deterministic compatibility projections and validation rejects any
/// disagreement. A future producer or cache boundary must negotiate a new
/// protocol rather than defaulting missing semantic domains to empty.
pub const COMPILER_FACTS_PROTOCOL: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerOptions {
    pub module_name: String,
    pub generate: String,
    #[serde(default)]
    pub hydratable: bool,
    #[serde(default)]
    pub dev: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_wrapper: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_conditionals: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_marker: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub built_ins: Vec<String>,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            module_name: "dom".into(),
            generate: "dom".into(),
            hydratable: false,
            dev: false,
            effect_wrapper: None,
            wrap_conditionals: None,
            static_marker: None,
            built_ins: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisRequest {
    pub compiler_facts_protocol: u32,
    pub path: String,
    pub source: Arc<str>,
    pub source_hash: SourceHash,
    pub compiler_options: CompilerOptions,
}

impl AnalysisRequest {
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        source: impl Into<Arc<str>>,
        mut options: CompilerOptions,
    ) -> Self {
        let source = source.into();
        options.built_ins.sort();
        options.built_ins.dedup();
        Self {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            path: path.into(),
            source_hash: SourceHash::of(source.as_ref()),
            source,
            compiler_options: options,
        }
    }
}

/// The checker's view of a Solid JSX compiler.
///
/// Each Solid dialect supplies its own adapter: Solid 2 wraps the compiler in
/// `solid/packages/compiler`, while Solid 1 wraps its pinned legacy compiler.
/// Infrastructure only sees this trait, so producer vocabulary never leaks
/// across the dialect seam.
pub trait CompilerFactsProvider {
    fn analyze(&mut self, request: &AnalysisRequest)
    -> Result<ExecutionMap, CompilerProviderError>;
}

/// What a [`CompilerFactsProvider`] can fail with, kept free of any single
/// compiler's error types so adapters stay interchangeable.
#[derive(Debug, Error)]
pub enum CompilerProviderError {
    #[error("the Solid compiler returned no semantic trace")]
    MissingExecutionMap,
    #[error("native Solid compiler facts error: {0}")]
    Native(String),
    #[error("compiler facts error: {0}")]
    Facts(#[from] CompilerFactsError),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SidecarResponse {
    pub ok: bool,
    #[serde(default)]
    pub execution_map: Option<ExecutionMap>,
    #[serde(default)]
    pub measurement: Option<Measurement>,
    #[serde(default)]
    pub error: Option<SidecarError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Measurement {
    pub computation_ns: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionMap {
    pub compiler_facts_protocol: u32,
    pub source_hash: SourceHash,
    pub semantic_model: CompilerSemanticModel,
    #[serde(default)]
    pub tracked_regions: Vec<ExecutionRegion>,
    #[serde(default)]
    pub untracked_regions: Vec<ExecutionRegion>,
    /// Regions the compiler **deleted**: the value is censused, decided, and
    /// then emitted nowhere.
    ///
    /// Deliberately not part of [`Self::untracked_regions`], which is a claim
    /// about code that *executes* — once, at render, outside any tracking
    /// scope. A discarded region executes zero times, so every claim an
    /// untracked region licenses is false of it: the read does not see a stale
    /// value, the write does not run in the render phase, the callback is never
    /// attached. It is equally not a hole (see `missing_jsx_census` in
    /// solid-reactive-ir): the compiler reported on this JSX and said the code
    /// is gone.
    ///
    /// A discarded region proves no *positive* claim either. Nothing here is
    /// evidence that a reader is satisfied, that an owner was established, or
    /// that a value settles — dead code establishes nothing.
    #[serde(default)]
    pub discarded_regions: Vec<ExecutionRegion>,
    #[serde(default)]
    pub ownership_regions: Vec<OwnershipRegion>,
    #[serde(default)]
    pub callback_roles: Vec<CallbackRole>,
    #[serde(default)]
    pub jsx_operations: Vec<JsxOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionRegion {
    pub span: Span,
    pub reason: RegionReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionReason {
    JsxChild,
    JsxAttribute,
    ComponentGetter,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnershipRegion {
    pub span: Span,
    pub kind: OwnershipRegionKind,
}

/// The owner state the compiler proves for a source region.
///
/// Known states are closed and typed. An unfamiliar serialized spelling is
/// retained as `Unknown`, but protocol-2 compatibility projection validation
/// prevents it from overriding the normalized semantic operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnershipRegionKind {
    Owned,
    Unowned,
    Leaf,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallbackRole {
    pub span: Span,
    pub role: CallbackRoleKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallbackRoleKind {
    EventHandler,
    Render,
    Deferred,
    DirectiveApply,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JsxOperation {
    pub span: Span,
    pub kind: String,
}

/// The protocol-2 compiler domain. Consumers reason over these operations;
/// the older region/role arrays above are deterministic compatibility views
/// derived by this module and validated against this model.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerSemanticModel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<CompilerProducerIdentity>,
    pub source_operations_complete: bool,
    pub generated_operations_complete: bool,
    pub operations: Vec<CompilerOperation>,
    pub generated_operations: Vec<GeneratedCompilerOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerProducerIdentity {
    pub dialect: String,
    pub trace_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_revision: Option<String>,
    pub implementation_revision: String,
    pub output_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_map_sha256: Option<String>,
    pub configuration_sha256: String,
    pub identity_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerOperationKind {
    JsxChild,
    NativeAttribute,
    NativeSpread,
    ComponentProperty,
    ComponentSpread,
    ComponentChild,
    EventHandler,
    Ref,
    ControlFlowRender,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerOperation {
    pub id: String,
    pub span: Span,
    pub kind: CompilerOperationKind,
    pub execution: CompilerExecutionSemantics,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilerExecutionSemantics {
    pub disposition: CompilerExecutionDisposition,
    pub trigger: CompilerExecutionTrigger,
    pub schedule: CompilerExecutionSchedule,
    pub tracking: CompilerTrackingRelation,
    pub cardinality: CompilerExecutionCardinality,
    pub owner: CompilerOwnerRelation,
    pub generated_operations: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerExecutionDisposition {
    Unknown,
    Discarded,
    EagerOnce,
    Deferred,
    ReactiveRerun,
    EventTriggered,
    RefFactory,
    RefApplication,
    ComponentPropertyGetter,
    ControlFlowRender,
    SsrEvaluation,
    SsrRenderCallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerExecutionTrigger {
    Unknown,
    None,
    Render,
    Dependency,
    Event,
    RefApplication,
    Caller,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerExecutionSchedule {
    Unknown,
    None,
    Inline,
    Render,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerTrackingRelation {
    Unknown,
    None,
    Tracked,
    Untracked,
    Inherited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerExecutionCardinality {
    Never,
    ZeroOrOne,
    ExactlyOnce,
    ZeroOrMore,
    OneOrMore,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompilerOwnerRelation {
    None,
    AmbientAtTransformSite,
    AmbientAtGeneratedInvocation,
    CapturedGeneratedOwner,
    CreatedGeneratedOwner,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedCompilerOperationKind {
    Effect,
    Insert,
    Memo,
    Scope,
    ComponentInvocation,
    DeferredCallback,
    DelegatedEvent,
    RefApplication,
    SsrClaim,
    RuntimeWrapper,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedCompilerOperation {
    pub id: String,
    pub source_id: String,
    pub source_span: Span,
    pub kind: GeneratedCompilerOperationKind,
    pub trigger: CompilerExecutionTrigger,
    pub schedule: CompilerExecutionSchedule,
    pub tracking: CompilerTrackingRelation,
    pub cardinality: CompilerExecutionCardinality,
    pub owner: CompilerOwnerRelation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver_span: Option<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapper: Option<String>,
}

/// Adapter input. Dialects translate only producer-owned vocabulary into this
/// shape; validation, legacy-decision expansion, compatibility projection,
/// canonicalization, and proof checks live below in one module.
pub struct CompilerTraceInput {
    pub producer: CompilerProducerInput,
    pub source_operations_complete: bool,
    pub generated_operations_complete: bool,
    pub operations: Vec<CompilerOperationInput>,
    pub generated_operations: Vec<GeneratedCompilerOperation>,
}

pub struct CompilerProducerInput {
    pub dialect: String,
    pub trace_version: u32,
    pub package_version: Option<String>,
    pub upstream_revision: Option<String>,
    pub implementation_revision: String,
    pub source_sha256: String,
    pub output: String,
    pub source_map: Option<String>,
    pub claimed_output_sha256: Option<String>,
    pub claimed_source_map_sha256: Option<String>,
    pub configuration_json: String,
    pub identity_complete: bool,
}

pub struct CompilerOperationInput {
    pub id: String,
    pub span: Span,
    pub kind: CompilerOperationKind,
    pub evidence: CompilerOperationEvidence,
}

impl CompilerOperationInput {
    #[must_use]
    pub fn legacy(
        span: Span,
        kind: CompilerOperationKind,
        decision: LegacyCompilerDecision,
        default_effect_owner: bool,
    ) -> Self {
        Self {
            id: format!(
                "s:{}:{}:{}",
                span.start,
                span.end,
                operation_kind_name(kind)
            ),
            span,
            kind,
            evidence: CompilerOperationEvidence::Legacy {
                decision,
                default_effect_owner,
            },
        }
    }
}

pub enum CompilerOperationEvidence {
    Rich(CompilerExecutionSemantics),
    Legacy {
        decision: LegacyCompilerDecision,
        default_effect_owner: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyCompilerDecision {
    EagerOnce,
    ReactiveRerun,
    CallerContext,
    Elided,
    LaterEvent,
    LaterRender,
    RefApply,
}

#[derive(Debug, Error)]
pub enum CompilerFactsError {
    #[error("invalid compiler facts JSON: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("compiler facts protocol {0} is unsupported")]
    Protocol(u32),
    #[error("compiler source hash {actual} does not match {expected}")]
    SourceHash {
        expected: SourceHash,
        actual: SourceHash,
    },
    #[error("invalid {category} span at index {index}: {source}")]
    Span {
        category: &'static str,
        index: usize,
        #[source]
        source: crate::core::FactIdentityError,
    },
    #[error("{category} facts are not in canonical order at index {index}")]
    Order {
        category: &'static str,
        index: usize,
    },
    #[error("JSX operation kind is empty at index {0}")]
    EmptyOperationKind(usize),
    #[error("compiler producer identity is invalid: {0}")]
    ProducerIdentity(String),
    #[error("compiler operation id is empty at index {0}")]
    EmptyCompilerOperationId(usize),
    #[error("compiler operation id {0:?} is duplicated")]
    DuplicateCompilerOperationId(String),
    #[error("generated compiler operation id is empty at index {0}")]
    EmptyGeneratedOperationId(usize),
    #[error("generated compiler operation id {0:?} is duplicated")]
    DuplicateGeneratedOperationId(String),
    #[error(
        "compiler operation {operation:?} references missing generated operation {generated:?}"
    )]
    MissingGeneratedOperation {
        operation: String,
        generated: String,
    },
    #[error("generated compiler operation {0:?} has contradictory execution semantics")]
    ContradictoryGeneratedOperation(String),
    #[error("compiler compatibility projection disagrees with semantic operations")]
    CompatibilityProjection,
}

impl ExecutionMap {
    pub fn normalize(input: CompilerTraceInput, source: &str) -> Result<Self, CompilerFactsError> {
        let expected_source = SourceHash::of(source);
        let expected_digest = expected_source
            .as_str()
            .strip_prefix("sha256:")
            .expect("SourceHash is canonical");
        if input.producer.source_sha256 != expected_digest {
            return Err(CompilerFactsError::SourceHash {
                expected: expected_source,
                actual: SourceHash::parse(format!("sha256:{}", input.producer.source_sha256))
                    .map_err(|error| CompilerFactsError::ProducerIdentity(error.to_string()))?,
            });
        }
        let output_sha256 = format!("{:x}", Sha256::digest(input.producer.output.as_bytes()));
        let source_map_sha256 = input
            .producer
            .source_map
            .as_deref()
            .map(|map| format!("{:x}", Sha256::digest(map.as_bytes())));
        if let Some(claimed) = &input.producer.claimed_output_sha256 {
            validate_digest("claimed output", claimed)?;
            if claimed != &output_sha256 {
                return Err(CompilerFactsError::ProducerIdentity(
                    "claimed output digest does not match generated output".into(),
                ));
            }
        }
        if input.producer.claimed_source_map_sha256 != source_map_sha256 {
            return Err(CompilerFactsError::ProducerIdentity(
                "claimed source-map digest does not match generated source map".into(),
            ));
        }
        if input.producer.dialect.trim().is_empty()
            || input.producer.implementation_revision.trim().is_empty()
        {
            return Err(CompilerFactsError::ProducerIdentity(
                "dialect and implementation revision must be non-empty".into(),
            ));
        }
        if input.producer.identity_complete
            && (input.producer.package_version.is_none()
                || input.producer.upstream_revision.is_none())
        {
            return Err(CompilerFactsError::ProducerIdentity(
                "a complete identity requires package and upstream revisions".into(),
            ));
        }
        let configuration: serde_json::Value =
            serde_json::from_str(&input.producer.configuration_json).map_err(|error| {
                CompilerFactsError::ProducerIdentity(format!("configuration is not JSON: {error}"))
            })?;
        if !configuration.is_object() {
            return Err(CompilerFactsError::ProducerIdentity(
                "configuration must be a JSON object".into(),
            ));
        }
        let configuration_sha256 = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&configuration).expect("a decoded JSON value always serializes")
            )
        );

        let operations = input
            .operations
            .into_iter()
            .map(|operation| CompilerOperation {
                id: operation.id,
                span: operation.span,
                kind: operation.kind,
                execution: match operation.evidence {
                    CompilerOperationEvidence::Rich(execution) => execution,
                    CompilerOperationEvidence::Legacy {
                        decision,
                        default_effect_owner,
                    } => legacy_execution(operation.kind, decision, default_effect_owner),
                },
            })
            .collect::<Vec<_>>();
        let semantic_model = CompilerSemanticModel {
            producer: Some(CompilerProducerIdentity {
                dialect: input.producer.dialect,
                trace_version: input.producer.trace_version,
                package_version: input.producer.package_version,
                upstream_revision: input.producer.upstream_revision,
                implementation_revision: input.producer.implementation_revision,
                output_sha256,
                source_map_sha256,
                configuration_sha256,
                identity_complete: input.producer.identity_complete,
            }),
            source_operations_complete: input.source_operations_complete,
            generated_operations_complete: input.generated_operations_complete,
            operations,
            generated_operations: input.generated_operations,
        };
        let projection = compatibility_projection(&semantic_model);
        let map = Self {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: SourceHash::of(source),
            semantic_model,
            tracked_regions: projection.tracked_regions,
            untracked_regions: projection.untracked_regions,
            discarded_regions: projection.discarded_regions,
            ownership_regions: projection.ownership_regions,
            callback_roles: projection.callback_roles,
            jsx_operations: projection.jsx_operations,
        };
        map.validate(source)?;
        Ok(map)
    }

    #[must_use]
    pub fn inert(source_hash: SourceHash) -> Self {
        Self {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash,
            semantic_model: CompilerSemanticModel {
                producer: None,
                source_operations_complete: true,
                generated_operations_complete: true,
                operations: Vec::new(),
                generated_operations: Vec::new(),
            },
            tracked_regions: Vec::new(),
            untracked_regions: Vec::new(),
            discarded_regions: Vec::new(),
            ownership_regions: Vec::new(),
            callback_roles: Vec::new(),
            jsx_operations: Vec::new(),
        }
    }

    pub fn from_json(encoded: &str, source: &str) -> Result<Self, CompilerFactsError> {
        let facts: Self = serde_json::from_str(encoded)?;
        facts.validate(source)?;
        Ok(facts)
    }

    pub fn validate(&self, source: &str) -> Result<(), CompilerFactsError> {
        if self.compiler_facts_protocol != COMPILER_FACTS_PROTOCOL {
            return Err(CompilerFactsError::Protocol(self.compiler_facts_protocol));
        }
        let expected = SourceHash::of(source);
        if self.source_hash != expected {
            return Err(CompilerFactsError::SourceHash {
                expected,
                actual: self.source_hash.clone(),
            });
        }
        validate_semantic_model(&self.semantic_model, source.len())?;
        validate_spanned(
            "tracked regions",
            &self.tracked_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "untracked regions",
            &self.untracked_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "discarded regions",
            &self.discarded_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "ownership regions",
            &self.ownership_regions,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "callback roles",
            &self.callback_roles,
            source.len(),
            |value| value.span,
        )?;
        validate_spanned(
            "JSX operations",
            &self.jsx_operations,
            source.len(),
            |value| value.span,
        )?;
        if let Some(index) = self
            .jsx_operations
            .iter()
            .position(|operation| operation.kind.trim().is_empty())
        {
            return Err(CompilerFactsError::EmptyOperationKind(index));
        }
        let projection = compatibility_projection(&self.semantic_model);
        if self.tracked_regions != projection.tracked_regions
            || self.untracked_regions != projection.untracked_regions
            || self.discarded_regions != projection.discarded_regions
            || self.ownership_regions != projection.ownership_regions
            || self.callback_roles != projection.callback_roles
            || self.jsx_operations != projection.jsx_operations
        {
            return Err(CompilerFactsError::CompatibilityProjection);
        }
        Ok(())
    }

    /// Whether some region or role fact decides `candidate`.
    ///
    /// A discarded region counts: "the compiler deleted this" is a decision
    /// about the site, not an absence of one. Leaving it out would make
    /// [`Self::uncovered_jsx_expressions`] report every deleted value as an
    /// unclassified JSX expression, which the dialect adapters turn into a hard
    /// refusal of the whole file.
    #[must_use]
    pub fn classifies(&self, candidate: Span) -> bool {
        self.tracked_regions
            .iter()
            .any(|fact| fact.span.contains(candidate))
            || self
                .untracked_regions
                .iter()
                .any(|fact| fact.span.contains(candidate))
            || self
                .discarded_regions
                .iter()
                .any(|fact| fact.span.contains(candidate))
            || self
                .callback_roles
                .iter()
                .any(|fact| fact.span.contains(candidate))
            || self.jsx_operations.iter().any(|fact| {
                matches!(
                    fact.kind.as_str(),
                    "component-property" | "component-spread" | "component-child"
                ) && fact.span.contains(candidate)
            })
            || self
                .semantic_model
                .operations
                .iter()
                .any(|operation| operation.span.contains(candidate))
    }

    #[must_use]
    pub fn uncovered_jsx_expressions(&self) -> Vec<Span> {
        self.jsx_operations
            .iter()
            .filter(|operation| {
                operation.kind == "jsx-expression" && !self.classifies(operation.span)
            })
            .map(|operation| operation.span)
            .collect()
    }

    #[must_use]
    pub fn seed_spans(&self) -> Vec<Span> {
        let mut spans = self
            .callback_roles
            .iter()
            .map(|fact| fact.span)
            .chain(self.jsx_operations.iter().map(|fact| fact.span))
            .collect::<Vec<_>>();
        spans.sort_unstable();
        spans.dedup();
        spans
    }
}

#[derive(Default)]
struct CompatibilityProjection {
    tracked_regions: Vec<ExecutionRegion>,
    untracked_regions: Vec<ExecutionRegion>,
    discarded_regions: Vec<ExecutionRegion>,
    ownership_regions: Vec<OwnershipRegion>,
    callback_roles: Vec<CallbackRole>,
    jsx_operations: Vec<JsxOperation>,
}

fn compatibility_projection(model: &CompilerSemanticModel) -> CompatibilityProjection {
    let mut projection = CompatibilityProjection::default();
    for operation in &model.operations {
        let span = operation.span;
        let reason = operation_region_reason(operation.kind);
        projection.jsx_operations.push(JsxOperation {
            span,
            kind: operation_jsx_kind(operation.kind).to_string(),
        });
        match operation.execution.disposition {
            CompilerExecutionDisposition::ReactiveRerun
                if operation.execution.tracking == CompilerTrackingRelation::Tracked =>
            {
                projection
                    .tracked_regions
                    .push(ExecutionRegion { span, reason });
            }
            CompilerExecutionDisposition::Discarded => {
                projection
                    .discarded_regions
                    .push(ExecutionRegion { span, reason });
            }
            CompilerExecutionDisposition::EagerOnce
                if operation.kind == CompilerOperationKind::ComponentChild =>
            {
                // The child value is created once, but a function value's body
                // is invoked by the receiving component. Keep the source
                // evaluation in the rich operation while preserving the
                // callback-body compatibility role used by current IR.
                projection.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Deferred,
                });
            }
            CompilerExecutionDisposition::EagerOnce
                if operation.kind != CompilerOperationKind::ComponentChild
                    && operation.execution.tracking == CompilerTrackingRelation::Untracked =>
            {
                projection
                    .untracked_regions
                    .push(ExecutionRegion { span, reason });
            }
            CompilerExecutionDisposition::RefFactory
                if operation.execution.tracking == CompilerTrackingRelation::Untracked
                    && operation.execution.cardinality
                        == CompilerExecutionCardinality::ExactlyOnce =>
            {
                projection
                    .untracked_regions
                    .push(ExecutionRegion { span, reason });
            }
            CompilerExecutionDisposition::Deferred
            | CompilerExecutionDisposition::ComponentPropertyGetter => {
                projection.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Deferred,
                });
            }
            CompilerExecutionDisposition::EventTriggered => {
                projection.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::EventHandler,
                });
            }
            CompilerExecutionDisposition::RefApplication => {
                projection.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::DirectiveApply,
                });
            }
            CompilerExecutionDisposition::ControlFlowRender => {
                projection.callback_roles.push(CallbackRole {
                    span,
                    role: CallbackRoleKind::Render,
                });
            }
            CompilerExecutionDisposition::Unknown
            | CompilerExecutionDisposition::EagerOnce
            | CompilerExecutionDisposition::ReactiveRerun
            | CompilerExecutionDisposition::RefFactory
            | CompilerExecutionDisposition::SsrEvaluation
            | CompilerExecutionDisposition::SsrRenderCallback => {}
        }
        if operation.execution.disposition == CompilerExecutionDisposition::RefFactory
            && operation
                .execution
                .generated_operations
                .iter()
                .any(|generated_id| {
                    model.generated_operations.iter().any(|generated| {
                        generated.id == *generated_id
                            && generated.kind == GeneratedCompilerOperationKind::RefApplication
                    })
                })
        {
            // A two-phase ref expression has two independent truths: the
            // factory expression evaluates eagerly while rendering, and its
            // returned function is later invoked once per ref application.
            // Preserve both compatibility views until the IR consumes the
            // normalized operation graph directly.
            projection.callback_roles.push(CallbackRole {
                span,
                role: CallbackRoleKind::DirectiveApply,
            });
        }
        if operation.execution.disposition == CompilerExecutionDisposition::ReactiveRerun
            && operation.execution.owner == CompilerOwnerRelation::CreatedGeneratedOwner
        {
            projection.ownership_regions.push(OwnershipRegion {
                span,
                kind: OwnershipRegionKind::Owned,
            });
        }
    }
    projection.tracked_regions.sort_by_key(|fact| fact.span);
    projection.untracked_regions.sort_by_key(|fact| fact.span);
    projection.discarded_regions.sort_by_key(|fact| fact.span);
    projection.ownership_regions.sort_by_key(|fact| fact.span);
    projection.callback_roles.sort_by_key(|fact| fact.span);
    projection.jsx_operations.sort_by_key(|fact| fact.span);
    projection.tracked_regions.dedup();
    projection.untracked_regions.dedup();
    projection.discarded_regions.dedup();
    projection.ownership_regions.dedup();
    projection.callback_roles.dedup();
    projection.jsx_operations.dedup();
    projection
}

fn legacy_execution(
    kind: CompilerOperationKind,
    decision: LegacyCompilerDecision,
    default_effect_owner: bool,
) -> CompilerExecutionSemantics {
    let (disposition, trigger, schedule, tracking, cardinality, owner) = match decision {
        LegacyCompilerDecision::Elided => (
            CompilerExecutionDisposition::Discarded,
            CompilerExecutionTrigger::None,
            CompilerExecutionSchedule::None,
            CompilerTrackingRelation::None,
            CompilerExecutionCardinality::Never,
            CompilerOwnerRelation::None,
        ),
        LegacyCompilerDecision::ReactiveRerun => (
            CompilerExecutionDisposition::ReactiveRerun,
            CompilerExecutionTrigger::Dependency,
            CompilerExecutionSchedule::Render,
            CompilerTrackingRelation::Tracked,
            CompilerExecutionCardinality::OneOrMore,
            if default_effect_owner {
                CompilerOwnerRelation::CreatedGeneratedOwner
            } else {
                CompilerOwnerRelation::Unknown
            },
        ),
        LegacyCompilerDecision::CallerContext => (
            if kind == CompilerOperationKind::ComponentProperty {
                CompilerExecutionDisposition::ComponentPropertyGetter
            } else {
                CompilerExecutionDisposition::Deferred
            },
            CompilerExecutionTrigger::Caller,
            CompilerExecutionSchedule::Deferred,
            CompilerTrackingRelation::Inherited,
            CompilerExecutionCardinality::Unknown,
            CompilerOwnerRelation::AmbientAtGeneratedInvocation,
        ),
        LegacyCompilerDecision::EagerOnce if kind == CompilerOperationKind::ComponentChild => (
            CompilerExecutionDisposition::Deferred,
            CompilerExecutionTrigger::Caller,
            CompilerExecutionSchedule::Deferred,
            CompilerTrackingRelation::Inherited,
            CompilerExecutionCardinality::Unknown,
            CompilerOwnerRelation::AmbientAtGeneratedInvocation,
        ),
        LegacyCompilerDecision::EagerOnce => (
            CompilerExecutionDisposition::EagerOnce,
            CompilerExecutionTrigger::Render,
            CompilerExecutionSchedule::Inline,
            CompilerTrackingRelation::Untracked,
            CompilerExecutionCardinality::ExactlyOnce,
            CompilerOwnerRelation::AmbientAtTransformSite,
        ),
        LegacyCompilerDecision::LaterEvent => (
            CompilerExecutionDisposition::EventTriggered,
            CompilerExecutionTrigger::Event,
            CompilerExecutionSchedule::Deferred,
            CompilerTrackingRelation::Untracked,
            CompilerExecutionCardinality::ZeroOrMore,
            CompilerOwnerRelation::None,
        ),
        LegacyCompilerDecision::LaterRender => (
            CompilerExecutionDisposition::ControlFlowRender,
            CompilerExecutionTrigger::Caller,
            CompilerExecutionSchedule::Render,
            CompilerTrackingRelation::Untracked,
            CompilerExecutionCardinality::ZeroOrMore,
            CompilerOwnerRelation::AmbientAtGeneratedInvocation,
        ),
        LegacyCompilerDecision::RefApply => (
            CompilerExecutionDisposition::RefApplication,
            CompilerExecutionTrigger::RefApplication,
            CompilerExecutionSchedule::Render,
            CompilerTrackingRelation::Untracked,
            CompilerExecutionCardinality::ZeroOrMore,
            CompilerOwnerRelation::None,
        ),
    };
    CompilerExecutionSemantics {
        disposition,
        trigger,
        schedule,
        tracking,
        cardinality,
        owner,
        generated_operations: Vec::new(),
    }
}

fn validate_semantic_model(
    model: &CompilerSemanticModel,
    source_len: usize,
) -> Result<(), CompilerFactsError> {
    if model.producer.is_none() {
        if !model.operations.is_empty() || !model.generated_operations.is_empty() {
            return Err(CompilerFactsError::ProducerIdentity(
                "operations require a producer identity".into(),
            ));
        }
        return Ok(());
    }
    let producer = model.producer.as_ref().expect("checked above");
    if producer.dialect.trim().is_empty() || producer.implementation_revision.trim().is_empty() {
        return Err(CompilerFactsError::ProducerIdentity(
            "dialect and implementation revision must be non-empty".into(),
        ));
    }
    validate_digest("output", &producer.output_sha256)?;
    validate_digest("configuration", &producer.configuration_sha256)?;
    if let Some(source_map) = &producer.source_map_sha256 {
        validate_digest("source map", source_map)?;
    }
    if producer.identity_complete
        && (producer.package_version.is_none() || producer.upstream_revision.is_none())
    {
        return Err(CompilerFactsError::ProducerIdentity(
            "a complete identity requires package and upstream revisions".into(),
        ));
    }

    validate_spanned(
        "compiler operations",
        &model.operations,
        source_len,
        |operation| operation.span,
    )?;
    validate_spanned(
        "generated compiler operations",
        &model.generated_operations,
        source_len,
        |operation| operation.source_span,
    )?;
    let mut operation_ids = BTreeSet::new();
    for (index, operation) in model.operations.iter().enumerate() {
        if operation.id.trim().is_empty() {
            return Err(CompilerFactsError::EmptyCompilerOperationId(index));
        }
        if !operation_ids.insert(operation.id.as_str()) {
            return Err(CompilerFactsError::DuplicateCompilerOperationId(
                operation.id.clone(),
            ));
        }
        validate_execution(operation)?;
    }
    let mut generated_ids = BTreeSet::new();
    for (index, operation) in model.generated_operations.iter().enumerate() {
        if operation.id.trim().is_empty() {
            return Err(CompilerFactsError::EmptyGeneratedOperationId(index));
        }
        if !generated_ids.insert(operation.id.as_str()) {
            return Err(CompilerFactsError::DuplicateGeneratedOperationId(
                operation.id.clone(),
            ));
        }
        if operation.source_id.trim().is_empty() {
            return Err(CompilerFactsError::ProducerIdentity(format!(
                "generated operation {} has no source identity",
                operation.id
            )));
        }
        validate_generated_execution(operation)?;
        if let Some(receiver) = operation.receiver_span {
            receiver
                .validate(source_len)
                .map_err(|source| CompilerFactsError::Span {
                    category: "generated compiler receiver spans",
                    index,
                    source,
                })?;
        }
    }
    for operation in &model.operations {
        for generated in &operation.execution.generated_operations {
            if !generated_ids.contains(generated.as_str()) {
                return Err(CompilerFactsError::MissingGeneratedOperation {
                    operation: operation.id.clone(),
                    generated: generated.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_execution(operation: &CompilerOperation) -> Result<(), CompilerFactsError> {
    let execution = &operation.execution;
    let valid = match execution.disposition {
        CompilerExecutionDisposition::Unknown => true,
        CompilerExecutionDisposition::Discarded => {
            execution.trigger == CompilerExecutionTrigger::None
                && execution.schedule == CompilerExecutionSchedule::None
                && execution.tracking == CompilerTrackingRelation::None
                && execution.cardinality == CompilerExecutionCardinality::Never
                && execution.owner == CompilerOwnerRelation::None
        }
        CompilerExecutionDisposition::ReactiveRerun => {
            execution.trigger == CompilerExecutionTrigger::Dependency
                && execution.schedule == CompilerExecutionSchedule::Render
                && execution.tracking == CompilerTrackingRelation::Tracked
                && execution.cardinality == CompilerExecutionCardinality::OneOrMore
        }
        CompilerExecutionDisposition::EventTriggered => {
            execution.trigger == CompilerExecutionTrigger::Event
                && execution.schedule == CompilerExecutionSchedule::Deferred
                && execution.tracking == CompilerTrackingRelation::Untracked
                && execution.cardinality == CompilerExecutionCardinality::ZeroOrMore
        }
        CompilerExecutionDisposition::ControlFlowRender => {
            execution.schedule == CompilerExecutionSchedule::Render
                && execution.tracking == CompilerTrackingRelation::Untracked
        }
        CompilerExecutionDisposition::RefApplication => {
            execution.trigger == CompilerExecutionTrigger::RefApplication
                && execution.tracking == CompilerTrackingRelation::Untracked
        }
        CompilerExecutionDisposition::RefFactory => matches!(
            execution.cardinality,
            CompilerExecutionCardinality::ExactlyOnce | CompilerExecutionCardinality::ZeroOrOne
        ),
        CompilerExecutionDisposition::SsrEvaluation => {
            execution.schedule == CompilerExecutionSchedule::Render
                && execution.tracking == CompilerTrackingRelation::Inherited
                && execution.cardinality == CompilerExecutionCardinality::ExactlyOnce
        }
        CompilerExecutionDisposition::SsrRenderCallback => {
            execution.schedule == CompilerExecutionSchedule::Render
                && execution.tracking == CompilerTrackingRelation::Inherited
                && execution.cardinality == CompilerExecutionCardinality::ExactlyOnce
        }
        CompilerExecutionDisposition::EagerOnce
        | CompilerExecutionDisposition::Deferred
        | CompilerExecutionDisposition::ComponentPropertyGetter => true,
    };
    if valid {
        Ok(())
    } else {
        Err(CompilerFactsError::ProducerIdentity(format!(
            "operation {} has contradictory execution semantics",
            operation.id
        )))
    }
}

fn validate_generated_execution(
    operation: &GeneratedCompilerOperation,
) -> Result<(), CompilerFactsError> {
    let valid = match operation.kind {
        GeneratedCompilerOperationKind::Effect | GeneratedCompilerOperationKind::Memo => {
            operation.trigger == CompilerExecutionTrigger::Dependency
                && operation.schedule == CompilerExecutionSchedule::Render
                && operation.tracking == CompilerTrackingRelation::Tracked
                && operation.cardinality == CompilerExecutionCardinality::OneOrMore
                && operation.owner == CompilerOwnerRelation::CreatedGeneratedOwner
        }
        GeneratedCompilerOperationKind::Insert => {
            operation.trigger == CompilerExecutionTrigger::Render
                && operation.schedule == CompilerExecutionSchedule::Render
                && operation.tracking == CompilerTrackingRelation::Tracked
                && operation.cardinality == CompilerExecutionCardinality::OneOrMore
                && operation.owner == CompilerOwnerRelation::CreatedGeneratedOwner
        }
        GeneratedCompilerOperationKind::Scope => {
            operation.trigger == CompilerExecutionTrigger::Caller
                && operation.schedule == CompilerExecutionSchedule::Render
                && operation.tracking == CompilerTrackingRelation::Inherited
                && operation.cardinality == CompilerExecutionCardinality::Unknown
                && operation.owner == CompilerOwnerRelation::CapturedGeneratedOwner
        }
        GeneratedCompilerOperationKind::ComponentInvocation => {
            operation.trigger == CompilerExecutionTrigger::Render
                && operation.schedule == CompilerExecutionSchedule::Inline
                && operation.tracking == CompilerTrackingRelation::Untracked
                && operation.cardinality == CompilerExecutionCardinality::ExactlyOnce
                && operation.owner == CompilerOwnerRelation::Unknown
        }
        GeneratedCompilerOperationKind::DeferredCallback => {
            operation.trigger == CompilerExecutionTrigger::Caller
                && operation.schedule == CompilerExecutionSchedule::Deferred
                && operation.tracking == CompilerTrackingRelation::Inherited
                && operation.cardinality == CompilerExecutionCardinality::Unknown
                && operation.owner == CompilerOwnerRelation::AmbientAtGeneratedInvocation
        }
        GeneratedCompilerOperationKind::DelegatedEvent => {
            operation.trigger == CompilerExecutionTrigger::Event
                && operation.schedule == CompilerExecutionSchedule::Deferred
                && operation.tracking == CompilerTrackingRelation::Untracked
                && operation.cardinality == CompilerExecutionCardinality::ZeroOrMore
                && operation.owner == CompilerOwnerRelation::None
        }
        GeneratedCompilerOperationKind::RefApplication => {
            operation.trigger == CompilerExecutionTrigger::RefApplication
                && operation.schedule == CompilerExecutionSchedule::Render
                && operation.tracking == CompilerTrackingRelation::Untracked
                && operation.cardinality == CompilerExecutionCardinality::ZeroOrMore
                && operation.owner == CompilerOwnerRelation::None
        }
        GeneratedCompilerOperationKind::SsrClaim => {
            operation.trigger == CompilerExecutionTrigger::Render
                && operation.schedule == CompilerExecutionSchedule::Render
                && operation.tracking == CompilerTrackingRelation::Inherited
                && operation.cardinality == CompilerExecutionCardinality::ZeroOrOne
                && operation.owner == CompilerOwnerRelation::AmbientAtGeneratedInvocation
        }
        GeneratedCompilerOperationKind::RuntimeWrapper => {
            operation.trigger == CompilerExecutionTrigger::Unknown
                && operation.schedule == CompilerExecutionSchedule::Unknown
                && operation.tracking == CompilerTrackingRelation::Unknown
                && operation.cardinality == CompilerExecutionCardinality::Unknown
                && operation.owner == CompilerOwnerRelation::Unknown
        }
    };
    if valid {
        Ok(())
    } else {
        Err(CompilerFactsError::ContradictoryGeneratedOperation(
            operation.id.clone(),
        ))
    }
}

fn validate_digest(label: &str, digest: &str) -> Result<(), CompilerFactsError> {
    if digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(CompilerFactsError::ProducerIdentity(format!(
            "{label} SHA-256 digest is not canonical"
        )))
    }
}

fn operation_region_reason(kind: CompilerOperationKind) -> RegionReason {
    match kind {
        CompilerOperationKind::NativeAttribute | CompilerOperationKind::NativeSpread => {
            RegionReason::JsxAttribute
        }
        CompilerOperationKind::ComponentProperty
        | CompilerOperationKind::ComponentSpread
        | CompilerOperationKind::ComponentChild => RegionReason::ComponentGetter,
        _ => RegionReason::JsxChild,
    }
}

fn operation_jsx_kind(kind: CompilerOperationKind) -> &'static str {
    match kind {
        CompilerOperationKind::JsxChild => "jsx-expression",
        CompilerOperationKind::NativeAttribute | CompilerOperationKind::NativeSpread => {
            "dynamic-attribute"
        }
        CompilerOperationKind::ComponentProperty => "component-property",
        CompilerOperationKind::ComponentSpread => "component-spread",
        CompilerOperationKind::ComponentChild => "component-child",
        CompilerOperationKind::EventHandler => "event-listener",
        CompilerOperationKind::Ref => "directive-apply",
        CompilerOperationKind::ControlFlowRender => "control-flow-render",
    }
}

fn operation_kind_name(kind: CompilerOperationKind) -> &'static str {
    match kind {
        CompilerOperationKind::JsxChild => "jsx-child",
        CompilerOperationKind::NativeAttribute => "native-attribute",
        CompilerOperationKind::NativeSpread => "native-spread",
        CompilerOperationKind::ComponentProperty => "component-property",
        CompilerOperationKind::ComponentSpread => "component-spread",
        CompilerOperationKind::ComponentChild => "component-child",
        CompilerOperationKind::EventHandler => "event-handler",
        CompilerOperationKind::Ref => "ref",
        CompilerOperationKind::ControlFlowRender => "control-flow-render",
    }
}

fn validate_spanned<T>(
    category: &'static str,
    values: &[T],
    source_len: usize,
    get_span: impl Fn(&T) -> Span,
) -> Result<(), CompilerFactsError> {
    for (index, value) in values.iter().enumerate() {
        let current = get_span(value);
        current
            .validate(source_len)
            .map_err(|source| CompilerFactsError::Span {
                category,
                index,
                source,
            })?;
        if index > 0 && get_span(&values[index - 1]) > current {
            return Err(CompilerFactsError::Order { category, index });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn producer(source: &str) -> CompilerProducerInput {
        let source_hash = SourceHash::of(source);
        CompilerProducerInput {
            dialect: "test".into(),
            trace_version: 1,
            package_version: None,
            upstream_revision: None,
            implementation_revision: "test-revision".into(),
            source_sha256: source_hash.as_str().strip_prefix("sha256:").unwrap().into(),
            output: "output".into(),
            source_map: None,
            claimed_output_sha256: None,
            claimed_source_map_sha256: None,
            configuration_json: "{}".into(),
            identity_complete: false,
        }
    }

    fn facts(source: &str, kind: CompilerOperationKind) -> ExecutionMap {
        ExecutionMap::normalize(
            CompilerTraceInput {
                producer: producer(source),
                source_operations_complete: true,
                generated_operations_complete: false,
                operations: vec![CompilerOperationInput::legacy(
                    Span::new(0, 5),
                    kind,
                    LegacyCompilerDecision::ReactiveRerun,
                    true,
                )],
                generated_operations: Vec::new(),
            },
            source,
        )
        .unwrap()
    }

    fn encoded(source: &str) -> String {
        serde_json::to_string(&facts(source, CompilerOperationKind::JsxChild)).unwrap()
    }

    #[test]
    fn validates_execution_map_and_completeness() {
        let source = "value";
        let facts = ExecutionMap::from_json(&encoded(source), source).unwrap();
        assert_eq!(facts.seed_spans(), vec![Span::new(0, 5)]);
    }

    #[test]
    fn rejects_a_compatibility_projection_that_disagrees_with_operations() {
        let source = "value";
        let mut facts = ExecutionMap::from_json(&encoded(source), source).unwrap();
        facts.tracked_regions.clear();
        assert!(matches!(
            facts.validate(source),
            Err(CompilerFactsError::CompatibilityProjection)
        ));
    }

    #[test]
    fn all_component_value_operations_cover_jsx_expressions() {
        for kind in [
            CompilerOperationKind::ComponentProperty,
            CompilerOperationKind::ComponentSpread,
            CompilerOperationKind::ComponentChild,
        ] {
            let source = "value";
            let facts = facts(source, kind);
            assert!(facts.classifies(Span::new(1, 2)), "kind {kind:?}");
        }
    }

    #[test]
    fn unfamiliar_ownership_kinds_cannot_override_protocol_two_operations() {
        let source = "value";
        let encoded = encoded(source).replace(r#""kind":"owned""#, r#""kind":"future-state""#);
        assert!(ExecutionMap::from_json(&encoded, source).is_err());
    }

    #[test]
    fn rejects_stale_source() {
        assert!(matches!(
            ExecutionMap::from_json(&encoded("value"), "other"),
            Err(CompilerFactsError::SourceHash { .. })
        ));
    }

    #[test]
    fn normalization_recomputes_and_checks_generated_output_identity() {
        let source = "value";
        let mut producer = producer(source);
        producer.claimed_output_sha256 = Some("0".repeat(64));
        let error = ExecutionMap::normalize(
            CompilerTraceInput {
                producer,
                source_operations_complete: true,
                generated_operations_complete: true,
                operations: Vec::new(),
                generated_operations: Vec::new(),
            },
            source,
        )
        .expect_err("a producer cannot certify bytes it did not emit");
        assert!(matches!(error, CompilerFactsError::ProducerIdentity(_)));
    }

    #[test]
    fn generated_operation_references_are_foreign_key_checked() {
        let source = "value";
        let error = ExecutionMap::normalize(
            CompilerTraceInput {
                producer: producer(source),
                source_operations_complete: true,
                generated_operations_complete: true,
                operations: vec![CompilerOperationInput {
                    id: "source:0".into(),
                    span: Span::new(0, 5),
                    kind: CompilerOperationKind::JsxChild,
                    evidence: CompilerOperationEvidence::Rich(CompilerExecutionSemantics {
                        disposition: CompilerExecutionDisposition::Unknown,
                        trigger: CompilerExecutionTrigger::Unknown,
                        schedule: CompilerExecutionSchedule::Unknown,
                        tracking: CompilerTrackingRelation::Unknown,
                        cardinality: CompilerExecutionCardinality::Unknown,
                        owner: CompilerOwnerRelation::Unknown,
                        generated_operations: vec!["missing:0".into()],
                    }),
                }],
                generated_operations: Vec::new(),
            },
            source,
        )
        .expect_err("a source operation cannot cite invented generated code");
        assert!(matches!(
            error,
            CompilerFactsError::MissingGeneratedOperation { .. }
        ));
    }

    #[test]
    fn uncertainty_is_local_to_each_execution_axis() {
        let source = "value";
        let facts = ExecutionMap::normalize(
            CompilerTraceInput {
                producer: producer(source),
                source_operations_complete: true,
                generated_operations_complete: true,
                operations: vec![CompilerOperationInput {
                    id: "source:0".into(),
                    span: Span::new(0, 5),
                    kind: CompilerOperationKind::Ref,
                    evidence: CompilerOperationEvidence::Rich(CompilerExecutionSemantics {
                        disposition: CompilerExecutionDisposition::RefFactory,
                        trigger: CompilerExecutionTrigger::Render,
                        schedule: CompilerExecutionSchedule::Inline,
                        tracking: CompilerTrackingRelation::Untracked,
                        cardinality: CompilerExecutionCardinality::ZeroOrOne,
                        owner: CompilerOwnerRelation::Unknown,
                        generated_operations: Vec::new(),
                    }),
                }],
                generated_operations: Vec::new(),
            },
            source,
        )
        .expect("an unknown owner must not erase known timing or tracking");
        let execution = &facts.semantic_model.operations[0].execution;
        assert_eq!(execution.schedule, CompilerExecutionSchedule::Inline);
        assert_eq!(execution.tracking, CompilerTrackingRelation::Untracked);
        assert_eq!(execution.owner, CompilerOwnerRelation::Unknown);
        assert!(facts.untracked_regions.is_empty());
    }

    #[test]
    fn contradictory_generated_operation_axes_are_refused() {
        let source = "value";
        let error = ExecutionMap::normalize(
            CompilerTraceInput {
                producer: producer(source),
                source_operations_complete: true,
                generated_operations_complete: false,
                operations: Vec::new(),
                generated_operations: vec![GeneratedCompilerOperation {
                    id: "g0".into(),
                    source_id: "s:0:5:event".into(),
                    source_span: Span::new(0, 5),
                    kind: GeneratedCompilerOperationKind::DelegatedEvent,
                    trigger: CompilerExecutionTrigger::Event,
                    schedule: CompilerExecutionSchedule::Inline,
                    tracking: CompilerTrackingRelation::Untracked,
                    cardinality: CompilerExecutionCardinality::ZeroOrMore,
                    owner: CompilerOwnerRelation::None,
                    receiver_span: None,
                    group_id: None,
                    wrapper: Some("delegated".into()),
                }],
            },
            source,
        )
        .expect_err("an event operation cannot claim inline scheduling");
        assert!(matches!(
            error,
            CompilerFactsError::ContradictoryGeneratedOperation(_)
        ));
    }

    #[test]
    fn component_child_creation_keeps_its_deferred_body_projection() {
        let source = "value";
        let facts = ExecutionMap::normalize(
            CompilerTraceInput {
                producer: producer(source),
                source_operations_complete: true,
                generated_operations_complete: false,
                operations: vec![CompilerOperationInput {
                    id: "source:0".into(),
                    span: Span::new(0, 5),
                    kind: CompilerOperationKind::ComponentChild,
                    evidence: CompilerOperationEvidence::Rich(CompilerExecutionSemantics {
                        disposition: CompilerExecutionDisposition::EagerOnce,
                        trigger: CompilerExecutionTrigger::Render,
                        schedule: CompilerExecutionSchedule::Inline,
                        tracking: CompilerTrackingRelation::Untracked,
                        cardinality: CompilerExecutionCardinality::ExactlyOnce,
                        owner: CompilerOwnerRelation::AmbientAtTransformSite,
                        generated_operations: Vec::new(),
                    }),
                }],
                generated_operations: Vec::new(),
            },
            source,
        )
        .expect("component child facts");
        assert_eq!(
            facts.callback_roles,
            vec![CallbackRole {
                span: Span::new(0, 5),
                role: CallbackRoleKind::Deferred,
            }]
        );
        assert!(facts.untracked_regions.is_empty());
    }
}
