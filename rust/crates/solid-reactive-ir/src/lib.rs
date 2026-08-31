mod attribution;
mod cache;
mod cleanup;
pub mod contract_semantics;
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

pub use attribution::ObligationReach;
pub use owners::function_binding_name;
pub use pipeline::{build, build_with_accepted_contracts_measured};

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
use pipeline::build_with_accepted_contracts_measured_incremental;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use contracts::{
    ContractAnalysis, ContractGraph, ContractSemantics, contract_export_summaries,
    contract_export_summaries_incremental,
};
pub use contracts::{
    ExportKindProof, export_kind_proof, export_kind_proof_from_entity, project_accepted_export,
    raised_function_export,
};
use execution_role::{
    NamedCallbackRoles, allowed_callback_spans, assigned_member_function_contains, execution_role,
    named_callback_roles, semantic_execution_role,
};
use identity::{SymbolId, SymbolInterner, SymbolName, symbol_id, symbol_name};
use indexes::{EntitySymbols, ProjectIndexes, SemanticLookup};
use interproc::{SummaryNode, SummaryRead, SummaryReads};
use serde::{Deserialize, Serialize};
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
    /// The compiler deleted this code: a `Value(Elided)` site, projected as a
    /// [`solid_facts::compiler::ExecutionMap::discarded_regions`] entry.
    ///
    /// Not a weaker [`Self::UntrackedRendering`] — the opposite of it. An
    /// untracked-rendering read executes once and then goes stale; a discarded
    /// read does not execute, so "sees the current value once and never
    /// updates" is false in all three clauses, and so is every claim built on
    /// it (a write that never runs is not a render-phase write, an action that
    /// never runs is not invoked in the wrong phase).
    ///
    /// It certifies nothing either. Silence here means the compiler deleted the
    /// operation: a discarded region satisfies no reactive reader, establishes
    /// no owner, and settles no value.
    DiscardedRendering,
}

/// Explicit runtime evidence supplied by the host integration. An empty
/// value means that the project has not selected a runtime; source heuristics
/// may still contribute facts, but they cannot discharge a condition-specific
/// package summary or prove CSR/SSR.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeEnvironment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<RuntimeTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<RuntimeBuild>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendering: Option<RuntimeRendering>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub conditions: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub framework_transforms: BTreeSet<String>,
    /// Whether the analyzed project is the whole program.
    ///
    /// This is evidence the analyzer cannot derive, in the same class as
    /// [`Self::rendering`]: nothing inside a tsconfig proves that nothing
    /// outside it imports from the tsconfig. Left unset, every exported symbol
    /// is assumed reachable by callers this build cannot see, which is why an
    /// exported component's props and an exported helper's owner stay proof
    /// obligations however completely the project itself is analyzed.
    ///
    /// Selecting [`ProgramBoundary::Closed`] asserts that the analyzed files
    /// are the entire program. It does **not** license guessing: the caller
    /// set must still be enumerated exactly, every reference must still
    /// resolve to a use the analyzer understands, and a missing reference list
    /// is still the absence of a fact. All it removes is the assumption that
    /// an *additional*, unseen caller exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_boundary: Option<ProgramBoundary>,
}

/// Whether callers outside the analyzed project may exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgramBoundary {
    /// The default. An exported symbol may be imported by code this build
    /// cannot see.
    Open,
    /// The analyzed files are the whole program; an export reaches no caller
    /// outside them.
    Closed,
}

impl RuntimeEnvironment {
    /// Whether the user has asserted that the analyzed project is the whole
    /// program. Absent selection is [`ProgramBoundary::Open`], never closed:
    /// a build that was never told stays fail-closed.
    #[must_use]
    pub const fn program_is_closed(&self) -> bool {
        matches!(self.program_boundary, Some(ProgramBoundary::Closed))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeTarget {
    Browser,
    Node,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeBuild {
    Development,
    Production,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeRendering {
    Csr,
    StringSsr,
    StreamingSsr,
}

/// The export-map conditions that name a host runtime. At most one of them
/// describes any single environment, which is what makes them the one
/// dimension of an entrypoint's recorded condition union a consumer can read
/// as scope rather than as alternatives.
const HOST_TARGET_CONDITIONS: &[&str] = &["browser", "node", "deno", "worker"];

impl RuntimeEnvironment {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .conditions
            .iter()
            .chain(self.framework_transforms.iter())
            .any(String::is_empty)
        {
            return Err("runtime conditions and framework transforms must be nonempty".into());
        }
        if matches!(self.rendering, Some(RuntimeRendering::Csr))
            && self.target == Some(RuntimeTarget::Node)
        {
            return Err("CSR cannot be selected with the node runtime".into());
        }
        if matches!(
            self.rendering,
            Some(RuntimeRendering::StringSsr | RuntimeRendering::StreamingSsr)
        ) && self.target == Some(RuntimeTarget::Browser)
        {
            return Err("SSR cannot be selected with the browser runtime".into());
        }
        let selected = self.selected_conditions();
        for (label, alternatives) in [
            ("runtime target", HOST_TARGET_CONDITIONS),
            ("build mode", &["development", "production"][..]),
            (
                "rendering mode",
                &["csr", "string-ssr", "streaming-ssr"][..],
            ),
        ] {
            let present = alternatives
                .iter()
                .filter(|condition| selected.contains(**condition))
                .copied()
                .collect::<Vec<_>>();
            if present.len() > 1 {
                return Err(format!(
                    "runtime selection contains contradictory {label} conditions: {}",
                    present.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// Exact runtime premises used by native rules and passed to the backend
    /// artifact/guard resolver. Contract branch mechanics never reach the IR.
    #[must_use]
    pub fn selected_conditions(&self) -> BTreeSet<String> {
        let mut conditions = self.conditions.clone();
        if let Some(target) = self.target {
            conditions.insert(
                match target {
                    RuntimeTarget::Browser => "browser",
                    RuntimeTarget::Node => "node",
                }
                .into(),
            );
        }
        if let Some(build) = self.build {
            conditions.insert(
                match build {
                    RuntimeBuild::Development => "development",
                    RuntimeBuild::Production => "production",
                }
                .into(),
            );
        }
        if let Some(rendering) = self.rendering {
            conditions.insert(
                match rendering {
                    RuntimeRendering::Csr => "csr",
                    RuntimeRendering::StringSsr => "string-ssr",
                    RuntimeRendering::StreamingSsr => "streaming-ssr",
                }
                .into(),
            );
        }
        conditions.extend(self.framework_transforms.iter().cloned());
        conditions
    }
}

impl ExecutionRole {
    /// Whether a reactive read in this role subscribes to nothing — the roles
    /// the strict-read rule reports in every dialect.
    ///
    /// [`Self::DiscardedRendering`] is deliberately absent. The rule's claim is
    /// that the read *happens* and then never updates; in a discarded region it
    /// does not happen, in either compiler's output, so the honest answer is
    /// silence rather than a violation or an uncertifiable obligation. Nothing
    /// is missing here — the compiler reported on the region and said the code
    /// is gone.
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
    ///
    /// [`Self::DiscardedRendering`] is absent for the same reason as above: a
    /// write the compiler deleted runs in no phase at all, so it is neither a
    /// tracked-phase write nor a render-phase one.
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
    /// The untracked-rendering role rests on a *missing* compiler fact: the
    /// narrowest JSX region containing the read carries no census entry, so
    /// the producer never reported how — or whether — that expression is
    /// lowered. Distinct from [`Self::uncertain`], which is uncertainty about
    /// the reactive backing rather than about the execution context, and
    /// worded separately for that reason. Projection reports either as
    /// uncertifiable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub missing_jsx_census: bool,
}

impl ReactiveRead {
    /// Whether a finding about this read is **uncertifiable** rather than a
    /// proven violation.
    ///
    /// Two independent holes, either of which is enough: the reactive
    /// backing cannot be established because the component's callers cannot be
    /// enumerated ([`Self::uncertain`]), the execution context cannot be
    /// established because the compiler reported no census for the JSX region
    /// ([`Self::missing_jsx_census`]).
    ///
    /// This is one predicate on purpose. The projection sets a finding's `kind`
    /// from it and each dialect's wording selects its hint from it; when the two
    /// disagreed, a finding could be published as a proven violation while
    /// carrying a proof-obligation hint, or the reverse.
    #[must_use]
    pub fn is_uncertifiable(&self) -> bool {
        self.uncertain || self.missing_jsx_census
    }
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
/// [`StaticDefect::analysis_context`] of the obligation an exported helper
/// raises when it invokes a member supplied by one of its own parameters.
///
/// Named here rather than spelled twice because contract emission has to
/// recognize the class in order to ask whether the published
/// `parameter-member` reactive-read row already carries the same uncertainty
/// — a question only that class raises. Producer:
/// `solid-reactive-ir/src/interproc.rs`; consumer:
/// `solid-facts-backend/src/main.rs`.
pub const EXPORTED_PARAMETER_MEMBER_DISPATCH: &str = "exported-parameter-member-dispatch";

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
    /// contract: the diagnostic identifies the exact open semantic leaf while
    /// analysis remains blocked until that leaf is proved and receipted.
    UnknownCallbackExecution {
        package: String,
        entrypoint: String,
        function: String,
        parameter: usize,
        parameter_type: String,
        required_execution: String,
        claim_context: String,
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

/// The finding family a [`StaticDefectKind`] projects to.
///
/// Two consumers need the same grouping and must not drift apart:
///
/// - every dialect rule catalog maps a defect kind to its own rule name
///   (`rust/dialects/solid-v{1,2}/rules/src/lib.rs`), and the *grouping* of
///   kinds is identical across dialects even though the rule names are not;
/// - the analysis pipeline deduplicates a static defect once per family,
///   path, and offset, because obligations discovered by different semantic
///   routes share one identity space.
///
/// Keeping the grouping in one exhaustive `match` — [`StaticDefectKind::family`]
/// — means a newly added defect kind is a compile error in both consumers
/// instead of silently falling into a catch-all arm and deduplicating against
/// an unrelated finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StaticDefectFamily {
    ReactiveObjectDestructure,
    ReactiveReadAfterAwait,
    ComponentReturnsConditionally,
    /// The contract-incomplete family: an import whose package contract does
    /// not describe the surface the project uses.
    PackageContractIncomplete,
    /// Projects to the same dialect rule as
    /// [`Self::PackageContractIncomplete`] but keeps its own dedup identity.
    /// A contract-generation obligation and a contract-incomplete consumer
    /// obligation are two separate claims about the same import, and a call
    /// site can produce both at one start byte; sharing an identity would let
    /// whichever the walk pushed first swallow the other.
    UnknownCallbackExecution,
    MissingEffectFunction,
    ReactiveSourceUncaptured,
    ReactiveDispatchUnresolved,
    ExpectedFunctionGotExpression,
    UncalledAccessor,
    DirectMutation,
}

impl StaticDefectFamily {
    /// The stable dedup identity for this family, used as the key of the
    /// draft's one-diagnostic-per-(identity, path, offset) set.
    #[must_use]
    pub const fn dedup_identity(self) -> &'static str {
        match self {
            Self::ReactiveObjectDestructure => "no-destructure",
            Self::ReactiveReadAfterAwait => "reactive-read-after-await",
            Self::ComponentReturnsConditionally => "components-return-once",
            Self::PackageContractIncomplete => "package-contract-incomplete",
            Self::UnknownCallbackExecution => "unknown-callback-execution",
            Self::MissingEffectFunction => "missing-effect-function",
            Self::ReactiveSourceUncaptured => "reactive-source-uncaptured",
            Self::ReactiveDispatchUnresolved => "reactive-dispatch-unresolved",
            Self::ExpectedFunctionGotExpression => "expected-function-got-expression",
            Self::UncalledAccessor => "uncalled-accessor",
            Self::DirectMutation => "no-direct-mutation",
        }
    }
}

impl StaticDefectKind {
    /// The finding family this defect kind projects to.
    ///
    /// This match is deliberately exhaustive with no catch-all arm: it is the
    /// single place the kind-to-finding grouping is written down, and both the
    /// dialect rule projection and the pipeline's dedup identity read it.
    #[must_use]
    pub const fn family(&self) -> StaticDefectFamily {
        match self {
            Self::ReactiveObjectDestructure { .. } => StaticDefectFamily::ReactiveObjectDestructure,
            Self::ReactiveReadAfterAwait { .. } => StaticDefectFamily::ReactiveReadAfterAwait,
            Self::ComponentReturnsConditionally => {
                StaticDefectFamily::ComponentReturnsConditionally
            }
            Self::PackageContractExportMissing { .. }
            | Self::PackageContractEnvironmentDependent { .. } => {
                StaticDefectFamily::PackageContractIncomplete
            }
            Self::UnknownCallbackExecution { .. } => StaticDefectFamily::UnknownCallbackExecution,
            Self::MissingEffectFunction => StaticDefectFamily::MissingEffectFunction,
            Self::ReactiveSourceUncaptured { .. } => StaticDefectFamily::ReactiveSourceUncaptured,
            Self::ReactiveDispatchUnresolved { .. }
            | Self::ReactiveCallbackUnresolved { .. }
            | Self::StructuredReturnUnresolved { .. } => {
                StaticDefectFamily::ReactiveDispatchUnresolved
            }
            Self::ReactiveHandlerRead { .. } | Self::HandlerValueUnresolved { .. } => {
                StaticDefectFamily::ExpectedFunctionGotExpression
            }
            Self::UncalledAccessor { .. } => StaticDefectFamily::UncalledAccessor,
            Self::DirectMutation { .. } => StaticDefectFamily::DirectMutation,
        }
    }

    /// The defect kind's own name, for the machine-readable channels that must
    /// name the exact obligation rather than its finding family.
    ///
    /// Exhaustive with no catch-all: a new kind is a compile error here rather
    /// than a silently mislabelled attribution note on a review plan.
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::ReactiveObjectDestructure { .. } => "ReactiveObjectDestructure",
            Self::ReactiveReadAfterAwait { .. } => "ReactiveReadAfterAwait",
            Self::ComponentReturnsConditionally => "ComponentReturnsConditionally",
            Self::PackageContractExportMissing { .. } => "PackageContractExportMissing",
            Self::PackageContractEnvironmentDependent { .. } => {
                "PackageContractEnvironmentDependent"
            }
            Self::UnknownCallbackExecution { .. } => "UnknownCallbackExecution",
            Self::MissingEffectFunction => "MissingEffectFunction",
            Self::ReactiveSourceUncaptured { .. } => "ReactiveSourceUncaptured",
            Self::ReactiveDispatchUnresolved { .. } => "ReactiveDispatchUnresolved",
            Self::ReactiveCallbackUnresolved { .. } => "ReactiveCallbackUnresolved",
            Self::StructuredReturnUnresolved { .. } => "StructuredReturnUnresolved",
            Self::ReactiveHandlerRead { .. } => "ReactiveHandlerRead",
            Self::HandlerValueUnresolved { .. } => "HandlerValueUnresolved",
            Self::UncalledAccessor { .. } => "UncalledAccessor",
            Self::DirectMutation { .. } => "DirectMutation",
        }
    }

    /// The dedup identity this defect carries into the draft's static-defect
    /// table. Derived from [`Self::family`] so it can never disagree with the
    /// finding kind the dialects project.
    #[must_use]
    pub const fn dedup_identity(&self) -> &'static str {
        self.family().dedup_identity()
    }

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
    /// The operation sits in a source-level JSX region for which the compiler
    /// emitted no execution census. The absence of an owner region is not a
    /// proof that a live operation runs unowned: the compiler may have deleted
    /// the expression or omitted a live lowering fact. Projection therefore
    /// reports an uncertifiable proof obligation rather than a violation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub missing_jsx_census: bool,
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
        "argument" | "callback-result" | "callback-result-function" => {
            if returned.parameter.is_none()
                || !returned.label.is_empty()
                || !returned.elements.is_empty()
                || !returned.properties.is_empty()
            {
                return Err("a relational return requires a parameter only");
            }
        }
        _ => return Err("the return kind is unsupported"),
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

/// Generator-local package summary accumulator. It is never decoded from or
/// encoded to a public contract document; `solid-facts-backend` immediately
/// normalizes it into the semantic model.
#[derive(Clone, Debug)]
pub struct PackageContract {
    pub package: ContractPackage,
    pub entrypoints: BTreeMap<String, ContractEntrypoint>,
    /// Diagnostic-only provenance for projected accepted semantics or local
    /// generation. It has no wire meaning.
    pub source_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPackage {
    pub name: String,
    pub version: String,
    pub integrity: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContractEntrypoint {
    pub exports: BTreeMap<String, ContractExport>,
}

/// Generator-local knowledge for one inferred summary domain.
///
/// This is not a wire type. `Open` means the producer has not established an
/// exhaustive answer; normalization maps it to the exact stable-v1 open
/// semantic leaf. No sentinel object is serialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractClaim<T> {
    Open,
    Known(T),
}

impl<T: Default> Default for ContractClaim<T> {
    fn default() -> Self {
        Self::Known(T::default())
    }
}

impl<T> From<T> for ContractClaim<T> {
    fn from(value: T) -> Self {
        Self::Known(value)
    }
}

impl<T> ContractClaim<T> {
    #[must_use]
    pub fn known(&self) -> Option<&T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Open => None,
        }
    }

    #[must_use]
    pub fn known_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Open => None,
        }
    }

    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContractExport {
    pub kind: String,
    pub reactive_reads: ContractClaim<Vec<ContractReactiveRead>>,
    pub returns: ContractClaim<Option<ContractReturn>>,
    pub callbacks: ContractClaim<Vec<ContractCallback>>,
    pub owner_requirements: ContractClaim<Vec<ContractOwnerRequirement>>,
    pub async_behavior: ContractClaim<String>,
    /// Wire-independent open domains retained when normalized partial
    /// knowledge is projected into the existing analysis indexes. This field
    /// is never decoded from or encoded into a package-contract document.
    pub open_claims: BTreeSet<contract_semantics::ClaimDomain>,
}

impl ContractExport {
    /// Whether this summary contains any domain that only a runtime function
    /// may carry. A `value` export with one of these domains is internally
    /// inconsistent even when the domain is open: absence of proof is not a
    /// value-side effect summary.
    #[must_use]
    pub fn has_function_effects(&self) -> bool {
        self.reactive_reads.is_open()
            || self
                .reactive_reads
                .known()
                .is_some_and(|reads| !reads.is_empty())
            || self.returns.is_open()
            || self.returns.known().is_some_and(Option::is_some)
            || self.callbacks.is_open()
            || self
                .callbacks
                .known()
                .is_some_and(|callbacks| !callbacks.is_empty())
            || self.owner_requirements.is_open()
            || self
                .owner_requirements
                .known()
                .is_some_and(|requirements| !requirements.is_empty())
            || self.async_behavior.is_open()
            || self
                .async_behavior
                .known()
                .is_some_and(|behavior| !behavior.is_empty())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractReactiveRead {
    pub kind: String,
    pub label: String,
    pub parameter: Option<usize>,
    /// Exact invoked property for a `parameter-member` read when every path
    /// contributing to the row names the same static member. Older contracts
    /// omit it and remain valid, but only a named member can be runtime-probed
    /// without guessing which property to instrument.
    pub member: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCallback {
    pub parameter: usize,
    pub execution: String,
    /// Runtime arguments supplied when this callback is invoked. `null`
    /// preserves an unmodeled ordinary value at that position; a structured
    /// descriptor uses the same bounded accessor/store/tuple/object vocabulary
    /// as exported returns.
    pub arguments: Vec<Option<ContractReturn>>,
    /// The owner context in which the runtime invokes this callback. Missing
    /// means the package contract describes timing only; consumers must keep
    /// the existing fail-closed owner behavior for that callback.
    pub owner: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractOwnerRequirement {
    pub operation: OwnerRequirementOperation,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ContractReturn {
    pub kind: String,
    pub label: String,
    pub parameter: Option<usize>,
    pub elements: Vec<Option<ContractReturn>>,
    pub properties: BTreeMap<String, ContractReturn>,
}

impl PackageContract {
    /// Validate the generator-local accumulator before normalization.
    pub fn validate(&self) -> Result<(), String> {
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
            })
        {
            return Err(
                "inferred contract entrypoints require exact package subpaths and exports".into(),
            );
        }
        for (entrypoint, exports) in self.export_maps() {
            for (name, summary) in exports {
                self.validate_export(entrypoint, name, summary)?;
            }
        }
        Ok(())
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
        if summary.kind == "value" && summary.has_function_effects() {
            return Err(format!(
                "package contract value export {entrypoint}:{name} cannot have function effects"
            ));
        }
        for read in summary.reactive_reads.known().into_iter().flatten() {
            let valid = match read.kind.as_str() {
                "accessor" | "store-path" => {
                    !read.label.is_empty() && read.parameter.is_none() && read.member.is_none()
                }
                "parameter-member" => {
                    read.label.is_empty()
                        && read.parameter.is_some()
                        && read.member.as_ref().is_none_or(|member| !member.is_empty())
                }
                _ => false,
            };
            if !valid {
                return Err(format!(
                    "package contract export {entrypoint}:{name} has an invalid reactive read"
                ));
            }
        }
        if let Some(returned) = summary.returns.known().and_then(Option::as_ref) {
            validate_contract_return(returned).map_err(|reason| {
                format!(
                    "package contract export {entrypoint}:{name} has an invalid reactive return: {reason}"
                )
            })?;
        }
        if summary
            .callbacks
            .known()
            .into_iter()
            .flatten()
            .any(|callback| {
                !matches!(
                    callback.execution.as_str(),
                    "inline" | "tracked" | "deferred"
                )
            })
        {
            return Err(format!(
                "package contract export {entrypoint}:{name} has an invalid callback execution"
            ));
        }
        for callback in summary.callbacks.known().into_iter().flatten() {
            if callback.owner.as_deref().is_some_and(|owner| {
                !matches!(
                    owner,
                    "inherited" | "created" | "unowned" | "conditional" | "leaf"
                )
            }) {
                return Err(format!(
                    "package contract export {entrypoint}:{name} has an invalid callback owner"
                ));
            }
            for argument in callback.arguments.iter().flatten() {
                validate_contract_return(argument).map_err(|reason| {
                    format!(
                        "package contract export {entrypoint}:{name} has an invalid callback argument: {reason}"
                    )
                })?;
            }
        }
        if let Some(async_behavior) = summary.async_behavior.known()
            && !async_behavior.is_empty()
            && !matches!(async_behavior.as_str(), "promise" | "async-iterable")
        {
            return Err(format!(
                "package contract export {entrypoint}:{name} has unsupported async behavior {:?}",
                async_behavior
            ));
        }
        Ok(())
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
    #[serde(skip)]
    pub contract_exports: Arc<BTreeMap<String, ContractExport>>,
    pub contract_generation_obligations: Vec<ContractGenerationObligation>,
    /// Which project functions can reach each unresolved proof obligation.
    ///
    /// Contract emission attributes an open claim to exactly the exports
    /// that can reach the obligation; see [`ObligationReach`]. Empty when the
    /// build produced no unresolved obligation, and empty for an obligation
    /// whose location is outside every function body.
    pub obligation_reach: Vec<ObligationReach>,
    pub obligation_counts: ObligationCounts,
    /// How many import and `export … from` declarations a contract named and
    /// the attested resolution then bound or refused.
    #[serde(default)]
    pub contract_binding: ContractBindingCounts,
}

/// How contract binding answered across the program's declarations.
///
/// A refusal is deliberately silent in the findings — the import becomes
/// uncertifiable on the rules' own terms — but silent is not the same as
/// invisible. A defect in the span join, in the attestation scope, or in a
/// host's specifier offsets degrades contract coverage toward nothing without
/// an error, and this is what makes that countable: `refused` above zero on a
/// project whose contracts are supposed to apply is the signal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractBindingCounts {
    /// Declarations whose specifier a contract named and the resolution
    /// confirmed.
    pub bound: usize,
    /// Declarations whose specifier a contract named and the resolution
    /// refused.
    pub refused: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractGenerationObligation {
    pub function: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub function_symbol: String,
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
    pub claim_context: String,
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
        self.build_with_accepted_contracts_shared(
            facts,
            dialect,
            &contract_semantics::AcceptedContractIndex::default(),
            &RuleOptions::default(),
        )
        .map(|(program, timings)| ((*program).clone(), timings))
    }

    /// Build a program behind shared ownership. This is the preferred service
    /// interface: retained generations are returned with an atomic reference
    /// increment instead of cloning every program table.
    pub fn build_shared(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        self.build_with_accepted_contracts_shared(
            facts,
            dialect,
            &contract_semantics::AcceptedContractIndex::default(),
            &RuleOptions::default(),
        )
    }

    /// Build from receipt-validated normalized semantics. The accepted index
    /// supplies the complete cache identity, including exact import/artifact
    /// bindings and the proof policy that authorized every closed claim.
    pub fn build_with_accepted_contracts_shared(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
        contracts: &contract_semantics::AcceptedContractIndex,
        rule_options: &RuleOptions,
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        let total_started = Instant::now();
        let lookup_started = Instant::now();
        let identity = BuildIdentity {
            dialect: dialect.version(),
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: vec![contracts.cache_fingerprint()],
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
            return Ok((
                Arc::clone(&retained.program),
                BuildTimings {
                    total: total_started.elapsed(),
                    cache_lookup,
                    reused: true,
                    ..BuildTimings::default()
                },
            ));
        }
        let (program, mut timings) = build_with_accepted_contracts_measured_incremental(
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
        // A callback the compiler deleted has no execution timing to publish.
        // "inline" would be a promise that a consumer may run the callback
        // eagerly, and this role is evidence that nothing runs it at all — a
        // positive claim dead code cannot support. The contract carries no
        // execution for it, exactly as for unproven timing.
        ExecutionRole::DiscardedRendering => None,
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

struct FunctionLookup {
    by_symbol: HashMap<SymbolId, usize>,
    by_span: HashMap<Span, usize>,
    parameter_owner: HashMap<SymbolId, (usize, usize)>,
}

fn function_lookup_for_path(
    functions: &[SummaryNode],
    by_path: &HashMap<String, Vec<usize>>,
    path: &str,
) -> FunctionLookup {
    let mut by_symbol = HashMap::new();
    let mut by_span = HashMap::new();
    let mut parameter_owner = HashMap::new();
    for (index, function) in functions_for_path(functions, by_path, path) {
        by_span.entry(function.span).or_insert(index);
        if let Some(symbol) = &function.symbol {
            by_symbol.entry(symbol.clone()).or_insert(index);
        }
        for (parameter, symbol) in function.parameters.iter().enumerate() {
            parameter_owner
                .entry(symbol.clone())
                .or_insert((index, parameter));
        }
    }
    FunctionLookup {
        by_symbol,
        by_span,
        parameter_owner,
    }
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

fn items_by_containing_function<'a, T, U>(
    functions: &[T],
    by_path: &HashMap<String, Vec<usize>>,
    items: impl IntoIterator<Item = (&'a str, &'a U)>,
    span: impl Fn(&U) -> Span,
) -> Vec<Vec<&'a U>>
where
    T: FunctionBoundary,
{
    let mut buckets = vec![Vec::new(); functions.len()];
    for (path, item) in items {
        if let Some(owner) = containing_function_indexed(functions, by_path, path, span(item)) {
            buckets[owner].push(item);
        }
    }
    buckets
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
    use std::collections::HashSet;

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
    fn runtime_environment_requires_exact_noncontradictory_selection() {
        let mut environment = RuntimeEnvironment {
            target: Some(RuntimeTarget::Browser),
            build: Some(RuntimeBuild::Production),
            rendering: Some(RuntimeRendering::Csr),
            conditions: BTreeSet::from(["import".into()]),
            framework_transforms: BTreeSet::from(["use-server".into()]),
            program_boundary: None,
        };
        assert!(environment.validate().is_ok());
        assert_eq!(
            environment.selected_conditions(),
            BTreeSet::from([
                "browser".into(),
                "import".into(),
                "production".into(),
                "csr".into(),
                "use-server".into()
            ])
        );
        // The program boundary is a build-wide premise, not a package export
        // condition. Artifact/guard selection happens before the normalized
        // accepted index reaches the analyzer.
        environment.program_boundary = Some(ProgramBoundary::Closed);
        assert!(environment.validate().is_ok());
        assert!(!environment.selected_conditions().contains("closed"));
        assert!(environment.program_is_closed());
        environment.program_boundary = Some(ProgramBoundary::Open);
        assert!(!environment.program_is_closed());
        environment.program_boundary = None;
        assert!(!environment.program_is_closed());
        // Resolver-only condition labels do not get synthesized here.
        assert!(!environment.selected_conditions().contains("default"));

        environment.target = Some(RuntimeTarget::Node);
        assert!(environment.validate().is_err());
        environment.target = Some(RuntimeTarget::Browser);
        environment.conditions.insert(String::new());
        assert!(environment.validate().is_err());
        environment.conditions.remove("");
        environment.conditions.insert("node".into());
        assert!(environment.validate().is_err());
        environment.conditions.remove("node");
        environment.conditions.insert("development".into());
        assert!(environment.validate().is_err());
    }

    #[test]
    fn inferred_structured_returns_reject_mixed_shapes() {
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

        let callback_result = ContractReturn {
            kind: "callback-result".into(),
            parameter: Some(0),
            ..ContractReturn::default()
        };
        assert!(validate_contract_return(&callback_result).is_ok());

        let callback_result_function = ContractReturn {
            kind: "callback-result-function".into(),
            parameter: Some(0),
            ..ContractReturn::default()
        };
        assert!(validate_contract_return(&callback_result_function).is_ok());

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
            resolved_imports: None,
            runtime_symbol_redirects: HashMap::new(),
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
        let (aliases, _) = alias_roots_and_source_declarations(&table, &interner, &HashMap::new());
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
    fn exact_runtime_redirects_replace_declaration_alias_roots() {
        let table = typescript_table(
            1,
            Vec::new(),
            Vec::new(),
            vec![
                SymbolFact {
                    id: "import-alias".into(),
                    alias_target: "declaration".into(),
                    declarations: Vec::new().into(),
                    references: Vec::new().into(),
                },
                SymbolFact {
                    id: "declaration".into(),
                    alias_target: "".into(),
                    declarations: Vec::new().into(),
                    references: Vec::new().into(),
                },
                SymbolFact {
                    id: "runtime".into(),
                    alias_target: "".into(),
                    declarations: Vec::new().into(),
                    references: Vec::new().into(),
                },
            ],
            Vec::new(),
        );
        let interner = SymbolInterner::from_table(&table);
        let redirects = HashMap::from([("declaration".into(), "runtime".into())]);
        let (aliases, _) = alias_roots_and_source_declarations(&table, &interner, &redirects);

        assert_eq!(aliases["import-alias"].as_str(), "runtime");
        assert_eq!(aliases["declaration"].as_str(), "runtime");
        assert_eq!(aliases["runtime"].as_str(), "runtime");
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
        let (aliases, source_declarations) =
            alias_roots_and_source_declarations(table, &interner, &HashMap::new());
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
        let (_, declarations) =
            alias_roots_and_source_declarations(&table, &interner, &HashMap::new());

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
            constructability: None,
            runtime_binding_kind: None,
            runtime_value_domain: None,
            primitive_value_domain: typefacts::PrimitiveValueDomain::default(),
            primitive_literal_candidates: None,
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
    fn containing_function_buckets_assign_each_item_to_one_innermost_owner() {
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
        let items = [
            ("fixture.tsx", Span { start: 35, end: 40 }),
            ("fixture.tsx", Span { start: 70, end: 75 }),
            (
                "fixture.tsx",
                Span {
                    start: 110,
                    end: 115,
                },
            ),
        ];

        let buckets = items_by_containing_function(
            &nodes,
            &by_path,
            items.iter().map(|(path, span)| (*path, span)),
            |span| *span,
        );

        assert_eq!(buckets[0], vec![&items[1].1]);
        assert_eq!(buckets[1], vec![&items[0].1]);
    }

    #[test]
    fn function_lookup_preserves_first_symbol_and_parameter_owner_for_a_path() {
        let mut first = summary_node(
            "fixture.tsx",
            Span { start: 0, end: 40 },
            Span { start: 10, end: 30 },
        );
        first.symbol = Some("first-function".into());
        first.parameters = vec!["shared-parameter".into()];
        let mut second = summary_node(
            "fixture.tsx",
            Span { start: 50, end: 90 },
            Span { start: 60, end: 80 },
        );
        second.symbol = Some("second-function".into());
        second.parameters = vec!["shared-parameter".into(), "second-parameter".into()];
        let nodes = vec![first, second];
        let by_path = function_indices_by_path(&nodes);

        let lookup = function_lookup_for_path(&nodes, &by_path, "fixture.tsx");

        assert_eq!(lookup.by_symbol.get("first-function"), Some(&0));
        assert_eq!(lookup.by_symbol.get("second-function"), Some(&1));
        assert_eq!(lookup.by_span.get(&Span { start: 0, end: 40 }), Some(&0));
        assert_eq!(lookup.by_span.get(&Span { start: 50, end: 90 }), Some(&1));
        assert_eq!(
            lookup.parameter_owner.get("shared-parameter"),
            Some(&(0, 0))
        );
        assert_eq!(
            lookup.parameter_owner.get("second-parameter"),
            Some(&(1, 1))
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
