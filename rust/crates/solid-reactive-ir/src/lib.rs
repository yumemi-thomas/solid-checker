mod attribution;
mod cache;
mod cleanup;
pub mod contract_semantics;
mod contracts;
mod creates_walk;
mod development_feedback;
mod directives;
pub use development_feedback::{
    DevelopmentFile, DevelopmentFunction, DevelopmentOperation, ResultRelevance,
};
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
pub mod returns_walk;
mod runtime_semantics;
mod server_rules;
mod source_discovery;
mod static_api;
mod static_rules;
mod symbols;
mod timings;
mod upstream_compat;
mod value_identity;

pub use attribution::ObligationReach;
pub use creates_walk::{CreatesDecline, CreatesDeclineKind, CreatesProposalWalk};
pub use owners::function_binding_name;
pub use pipeline::{build, build_with_accepted_contracts_measured};
pub use returns_walk::{
    ArgumentContainer, ReturnsDecline, described_callable_returns, literal_structural_returns,
    reading_callable_returns, value_completion, valueless_completion,
};

pub use upstream_compat::rule_options::{RuleOptions, RuleOverride};

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
    project_export_semantics, raised_function_export,
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
    /// The read sits in a callback a host API retains and may invoke on its
    /// invoker's stack -- an `addEventListener` listener, a `bind` bound
    /// argument, a `PromiseLike.then` callback, a Geolocation callback
    /// (`execution_role::host_callback_timing`). The host may invoke it inside
    /// the component body's strict-read window, or after it, and nothing here
    /// proves which. A third hole beside the other two, about the execution
    /// window rather than the reactive backing or the compiler census, and
    /// worded separately for that reason.
    #[serde(default, skip_serializing_if = "is_false")]
    pub host_callback_timing: bool,
    /// The read sits in a function literal handed to a project function or
    /// through a JSX prop whose consumer is not proven to invoke it during rendering
    /// (`execution_role::callee_callback_timing`): the literal is written in
    /// the component body but runs wherever the callee runs it -- during the
    /// call, from a closure the callee returns or stores, or never -- and the
    /// lexical position proves none of these. A fourth hole beside the other
    /// three, about which code invokes the read rather than when a host does,
    /// and worded separately for that reason.
    #[serde(default, skip_serializing_if = "is_false")]
    pub callee_callback_timing: bool,
    /// Generation-local closed project-consumer proof, used only by strict-read
    /// projection. It establishes no execution role, write legality, async-read
    /// behavior, ownership, or escape permission. Deserialization cannot supply
    /// this proof; the post-merge analysis recomputes it from current facts.
    #[serde(skip)]
    pub project_consumer_non_strict: bool,
    /// An accepted inline invocation may read an accessor passed by reference,
    /// but its call-scoped cardinality does not prove an invocation occurs.
    #[serde(default, skip_serializing_if = "is_false")]
    pub callback_invocation_unproven: bool,
    /// The read is the called package export reading reactive state of its
    /// own while it runs, as its contract's `reads` states. Solid warns about
    /// it at every use, so it is the package's implementation, not misuse at
    /// the call site.
    #[serde(default, skip_serializing_if = "is_false")]
    pub package_internal: bool,
    /// The read was attributed to this call through another function's
    /// interprocedural summary: the read runs inside that function's body, and
    /// whether it runs while the call does (not from a timer, listener,
    /// getter, returned accessor, effect compute or after an await) is not
    /// established by the summary. Measured on 48 real projects, these
    /// attributions were 73 false, 16 benign and 0 user-visible defects.
    #[serde(default, skip_serializing_if = "is_false")]
    pub summary_attributed: bool,
}

impl ReactiveRead {
    /// Whether a finding about this read is **uncertifiable** rather than a
    /// proven violation.
    ///
    /// Independent holes, any of which is enough: the reactive
    /// backing cannot be established because the component's callers cannot be
    /// enumerated ([`Self::uncertain`]), the execution context cannot be
    /// established because the compiler reported no census for the JSX region
    /// ([`Self::missing_jsx_census`]), the host may run the read's callback
    /// inside the strict-read window or after it
    /// ([`Self::host_callback_timing`]), or the read's function literal is
    /// handed to a project function not proven to invoke it during the call
    /// ([`Self::callee_callback_timing`]), or an accepted callback invocation
    /// is possible but not guaranteed ([`Self::callback_invocation_unproven`]).
    ///
    /// This is one predicate on purpose. The projection sets a finding's `kind`
    /// from it and each dialect's wording selects its hint from it; when the two
    /// disagreed, a finding could be published as a proven violation while
    /// carrying a proof-obligation hint, or the reverse.
    #[must_use]
    pub fn is_uncertifiable(&self) -> bool {
        self.uncertain
            || self.missing_jsx_census
            || self.host_callback_timing
            || self.callee_callback_timing
            || self.callback_invocation_unproven
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
    /// ADR 0179: the operation is a registration a package export's accepted
    /// contract states it makes on every call; [`Self::via`] names the export.
    #[serde(default, skip_serializing_if = "is_false")]
    pub through_contract: bool,
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
    /// The registration may not happen at this call: the accepted contract
    /// states it with `min: 0` (an early return the contract cannot express
    /// as a guard). Projected as uncertifiable, like [`Self::uncertain`].
    #[serde(default, skip_serializing_if = "is_false")]
    pub possible: bool,
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

/// Where a package-contract obligation was raised.
///
/// [`ContractDefectSite::Import`] messages name the package, the export and the
/// open claims and nothing else, so two of them differ only in which file did
/// the importing -- repetition a reader cannot act on separately.
/// [`ContractDefectSite::Argument`] messages are about one exact argument of one
/// exact call, where the site *is* the content: a descriptor absorbed by a rest
/// parameter and one observed through an `arguments` object are different facts
/// about different code, and must stay separate findings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContractDefectSite {
    Import,
    Argument,
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
        /// Where this was raised, which is what decides whether two of them are
        /// interchangeable. The producer knows; the projection must not have to
        /// infer it, because one `analysis_context` -- `unknown-contract-claims:
        /// callbacks` -- is emitted from both kinds of site.
        site: ContractDefectSite,
        /// The acceptance gate only: why an acceptance that exists for this
        /// package -- a project catalog entry -- was not admitted, as the
        /// backend's admission rule states it. Projected as one extra evidence
        /// step; it never changes the message, the severity or the collapse,
        /// and it is `None` wherever no such acceptance exists.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        admission_refusal: Option<String>,
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
    /// A callback the dialect says runs when the call's **returned object is
    /// read** ([`solid_dialect::Dialect::callback_runs_on_result_access`];
    /// rc.9's `omit(props, hidden)` predicate) is not proven free of reactive
    /// behaviour. It runs in whatever tracking scope and under whatever owner
    /// the reader has, which the call site does not decide, so neither a
    /// violation nor safety is provable. `analysis_context` names which proof
    /// is missing (see `result_access_callbacks` in `static_rules`).
    ResultAccessCallbackUnplaced {
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
    /// A value crosses the default server-function transport, which carries
    /// plain JSON. Which way it fails is proven here; the serializer that
    /// fixes it, and the module that installs it, are the dialect's to name.
    ServerFunctionRichArgument {
        transport: RichArgumentTransport,
    },
}

/// The named reasons a [`StaticDefectKind::ResultAccessCallbackUnplaced`]
/// carries in its `analysis_context`: which proof of an inert predicate is
/// missing.
///
/// The predicate's body performs a reactive operation the engine records (a
/// read, write, action invocation or async read), or references a reactive
/// source, setter, action or props binding.
pub(crate) const RESULT_ACCESS_REACTIVE_OPERATION: &str =
    "result-access-callback-reactive-operation";
/// The predicate's body calls something that does not resolve to a
/// standard-library declaration; whatever it does runs in the reader's scope.
pub(crate) const RESULT_ACCESS_OPAQUE_CALL: &str = "result-access-callback-opaque-call";
/// The value at the predicate position is potentially callable, but no body
/// for it is inspectable at this call: an import, a call result, a member, a
/// binding that is not a function literal.
pub(crate) const RESULT_ACCESS_BODY_UNRESOLVED: &str = "result-access-callback-body-unresolved";

/// How a server-function argument fails the default JSON transport.
///
/// Four proofs about different things, not one claim with a flag: the
/// argument's own resolved type, a value held by a closed object literal
/// reaching the call, a primitive JSON cannot encode, and a transport proof
/// that is open. A reader acts on the difference, and the last is an
/// obligation rather than a violation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "transport")]
pub enum RichArgumentTransport {
    ResolvedType {
        function: String,
        descriptor: String,
        member: String,
    },
    NestedValue {
        function: String,
    },
    NonJsonPrimitive {
        function: String,
    },
    Unresolved {
        reason: String,
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
    /// A value crossing the default server-function transport. Deliberately
    /// absent from [`StaticDefectKind::is_unresolved_obligation`]: the open
    /// transport proof is reported as uncertifiable, but it was never part of
    /// the `SC9xxx` obligation census the metrics and contract emission read,
    /// and moving this rule off the static-violation channel did not change
    /// what it counts.
    ServerFunctionRichArgument,
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
            Self::ServerFunctionRichArgument => "server-function-rich-argument",
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
            | Self::ResultAccessCallbackUnplaced { .. }
            | Self::StructuredReturnUnresolved { .. } => {
                StaticDefectFamily::ReactiveDispatchUnresolved
            }
            Self::ReactiveHandlerRead { .. } | Self::HandlerValueUnresolved { .. } => {
                StaticDefectFamily::ExpectedFunctionGotExpression
            }
            Self::UncalledAccessor { .. } => StaticDefectFamily::UncalledAccessor,
            Self::DirectMutation { .. } => StaticDefectFamily::DirectMutation,
            Self::ServerFunctionRichArgument { .. } => {
                StaticDefectFamily::ServerFunctionRichArgument
            }
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
            Self::ResultAccessCallbackUnplaced { .. } => "ResultAccessCallbackUnplaced",
            Self::StructuredReturnUnresolved { .. } => "StructuredReturnUnresolved",
            Self::ReactiveHandlerRead { .. } => "ReactiveHandlerRead",
            Self::HandlerValueUnresolved { .. } => "HandlerValueUnresolved",
            Self::UncalledAccessor { .. } => "UncalledAccessor",
            Self::DirectMutation { .. } => "DirectMutation",
            Self::ServerFunctionRichArgument { .. } => "ServerFunctionRichArgument",
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
                | Self::ResultAccessCallbackUnplaced { .. }
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
    /// The uncertainty comes from a callback that runs under its caller's
    /// owner only on its first run, and with no owner on every later run --
    /// 2.0 `createRenderEffect`'s apply. The first run is owned exactly as the
    /// call site is; whether a later run happens is a runtime fact.
    #[serde(default, skip_serializing_if = "is_false")]
    pub later_run_unowned: bool,
    /// The containing Solid 1 function is component-shaped only by a naming
    /// convention; JSX invocation and ordinary invocation imply different
    /// owner contexts.
    #[serde(default, skip_serializing_if = "is_false")]
    pub component_uncertain: bool,
    /// The operation runs after an `await` on every path through its async
    /// function's body: a promise continuation on an empty stack, where the
    /// dialect says no owner is current. Proven unowned whatever context the
    /// function was entered with.
    #[serde(default, skip_serializing_if = "is_false")]
    pub after_await: bool,
    /// The operation sits in a source-level JSX region for which the compiler
    /// emitted no execution census. The absence of an owner region is not a
    /// proof that a live operation runs unowned: the compiler may have deleted
    /// the expression or omitted a live lowering fact. Projection therefore
    /// reports an uncertifiable proof obligation rather than a violation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub missing_jsx_census: bool,
    /// ADR 0161: the requirement stands for a call of an accepted contract's
    /// export rather than for a dialect primitive the code calls itself. The
    /// generator never states such a site's registration as guaranteed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub through_contract: bool,
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
        crate::contracts::RETURNED_CALLABLE => {
            if !returned.label.is_empty()
                || returned.parameter.is_some()
                || !returned.elements.is_empty()
                || returned.properties.keys().any(String::is_empty)
            {
                return Err("a returned callable admits named members only");
            }
            for property in returned.properties.values() {
                validate_contract_return(property)?;
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
        // `merged-props` joins the parameter-carrying kinds rather than the
        // reactive leaves: its whole content is *which* argument it reaches
        // through to (ADR 0109), and a leaf's label would say nothing.
        "argument" | "callback-result" | "callback-result-function" | "merged-props" => {
            if returned.parameter.is_none()
                || !returned.label.is_empty()
                || !returned.elements.is_empty()
                || !returned.properties.is_empty()
            {
                return Err("a relational return requires a parameter only");
            }
        }
        // ADR 0234: an opaque member of a returned tuple or object.
        crate::contracts::OPAQUE_MEMBER => {
            if !returned.label.is_empty()
                || returned.parameter.is_some()
                || !returned.elements.is_empty()
                || !returned.properties.is_empty()
            {
                return Err("an opaque member carries nothing");
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
    /// The read sits in a callback a host API may invoke inside the component
    /// body's strict-read window or after it
    /// ([`ReactiveRead::host_callback_timing`]). Inside the window a pending
    /// read throws `PENDING_ASYNC_UNTRACKED_READ` (dev); after it, a plain
    /// `NotReadyError` (probed on rc.3 and rc.9), so SC5001 is a proof
    /// obligation rather than a proven throw.
    #[serde(default, skip_serializing_if = "is_false")]
    pub host_callback_timing: bool,
    /// The read's callback is handed to a callee whose invocation is not
    /// proven on the current stack ([`ReactiveRead::callee_callback_timing`]).
    /// The callee may defer the read or handle pending values, so SC5001 is
    /// an open obligation rather than a proven pending-read exception.
    #[serde(default, skip_serializing_if = "is_false")]
    pub callee_callback_timing: bool,
    /// Lexical nesting in a component does not prove this stored or returned
    /// callback executes during that component's strict-read window.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invocation_context_unproven: bool,
    /// The read is rendered by a function whose callers are not all in the
    /// analyzed project (an exported component, a route handed to a router, a
    /// lazy page): no `Loading` boundary was found above it, but the boundary
    /// that decides it may be written where the analysis cannot see. The
    /// missing-boundary findings are uncertifiable then, not proven.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mount_unresolved: bool,
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

/// ADR 0207: when an export invokes the `on…` members of one of its argument
/// values: the projected execution word of the item's operation, and that
/// operation's guard, kept whole so a consumer proves every atom or reads
/// nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHandlerPropsClaim {
    pub parameter: usize,
    pub path: Vec<String>,
    pub execution: String,
    pub guard: Vec<contract_semantics::GuardAtom>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContractExport {
    pub kind: String,
    pub reactive_reads: ContractClaim<Vec<ContractReactiveRead>>,
    pub returns: ContractClaim<Option<ContractReturn>>,
    pub callbacks: ContractClaim<Vec<ContractCallback>>,
    /// Exact argument slots an accepted ambient invocation may call inline
    /// during the export call. The value is true only for an unguarded,
    /// call-scoped lower bound of at least one. Internal projection only;
    /// callback-domain closure never strengthens a possible invocation.
    pub inline_accessor_invocations: BTreeMap<usize, bool>,
    pub owner_requirements: ContractClaim<Vec<ContractOwnerRequirement>>,
    /// ADR 0174: the guaranteed owner requirements still proven while
    /// [`Self::owner_requirements`] is `Open`. Opening a list says another item
    /// may exist; it does not disprove one this export's own body makes on
    /// every normal completion. The generator publishes them as items, and the
    /// list stays incomplete, so `creates` stays open. Empty whenever the claim
    /// is `Known`, which carries its own items. Generation only: never decoded
    /// from or encoded into a package-contract document.
    pub open_owner_requirements: Vec<ContractOwnerRequirement>,
    /// ADR 0178: the return this export's own body describes, kept while
    /// [`Self::returns`] is `Open` because an unresolved call opened it. An
    /// unknown call can add behavior; it does not replace the value a return
    /// statement hands back, and the certifier's census proves that value
    /// from the producer's trace or withdraws it. The generator publishes it
    /// as a positive item over an open domain. `None` whenever the claim is
    /// `Known`, which carries its own return. Generation only: never decoded
    /// from or encoded into a package-contract document.
    pub open_return: Option<ContractReturn>,
    /// ADR 0179: the owner registrations this export makes on its caller's
    /// owner, at the call and on the same stack: `ambient-at-call`,
    /// call-scoped. Inside a leaf owner each is a forbidden operation the
    /// runtime throws on. `guaranteed` when it happens on every call
    /// (`min >= 1`); otherwise it may not happen (`min: 0`) and the leaf
    /// finding is a proof obligation. A guarded one keeps its guard and
    /// counts only at a call where the guard holds (ADR 0223).
    /// Projected from an accepted document only; empty for a local summary.
    pub leaf_forbidden_operations: Vec<ContractOwnerRequirement>,
    /// ADR 0207: the accepted contract's `event-handler-props` items, each
    /// exhaustive for the `on…` members of one argument value. They stay out
    /// of [`Self::callbacks`], whose claim opens instead, so no consumer that
    /// reads rows by path mistakes one for a call of the argument itself.
    /// Projected from an accepted document only.
    pub event_handler_props: Vec<EventHandlerPropsClaim>,
    pub async_behavior: ContractClaim<String>,
    /// Wire-independent open domains retained when normalized partial
    /// knowledge is projected into the existing analysis indexes. This field
    /// is never decoded from or encoded into a package-contract document.
    pub open_claims: BTreeSet<contract_semantics::ClaimDomain>,
    /// Whether the *accepted* contract this summary was projected from closes
    /// `creates` with no item.
    ///
    /// Only [`crate::project_accepted_export`] sets it, and only from a
    /// normalized accepted document. `false` is the fail-closed default a
    /// locally generated summary keeps: a summary nothing projected states
    /// nothing about a dependency's `creates`, and the generator's proposal
    /// walk reads it that way.
    pub creates_closed_empty: bool,
    /// Whether the *accepted* contract this summary was projected from closes
    /// `returns` with no item (ADR 0143).
    ///
    /// Read from the accepted document for the same reason as
    /// [`Self::creates_closed_empty`]: the projection [`Self::returns`] is the
    /// consumer's single reactive leaf, and it reads a closed claim over exact
    /// outputs that name no leaf -- a `plain` return, an argument container --
    /// as `Known(None)`, exactly as it reads `returns: []`. Only the empty
    /// closure licenses a re-exporting package to state `returns: []` again.
    /// `false` is the fail-closed default every locally inferred summary keeps.
    pub returns_closed_empty: bool,
    /// The operations of the *accepted* contract's closed, non-empty `returns`
    /// claim, when a re-exporting package can state them again exactly (ADR
    /// 0170): every item a bare `return` ([`contract_semantics::Operation::is_bare_return`])
    /// whose output names no resource, operation or callable of the
    /// dependency's own -- `plain`, `undefined`, an argument, a fresh array of
    /// arguments, an invocation result, a props merge, an array of plains --
    /// in the accepted order.
    ///
    /// Empty is "nothing to restate", which is what every locally inferred
    /// summary keeps, what a projection of an open or empty `returns` keeps
    /// (the empty closure is [`Self::returns_closed_empty`]'s), and what a claim
    /// with any other item keeps: a partly restated enumeration would be a
    /// stronger claim than the dependency's.
    pub returns_restated: Vec<contract_semantics::Operation>,
    /// Whether the generator's own [`crate::CreatesProposalWalk`] found no call
    /// inside this export's implementation that a `creates: []` proposal would
    /// contradict.
    ///
    /// `false` is the default and the fail-closed answer: a summary no walk
    /// reached proposes nothing. This is a *proposal* input — the claim itself
    /// is proved, or refused, by the certifier's implementation census.
    pub creates_walk_clean: bool,
    /// Why that walk declined, when it did: every blocker reachable from this
    /// export's implementation span, lexically and through the resolved local
    /// call edges ([`crate::CreatesProposalWalk::declines_for`]).
    ///
    /// **Measurement, never evidence.** No claim is decided from it, it is
    /// never encoded into a package-contract document, and an empty list means
    /// only "no blocker was named" — for a summary no walk reached that is
    /// silence, not a clean walk, which is what `creates_walk_clean` says.
    pub creates_walk_declines: Vec<crate::CreatesDecline>,
    /// Whether the generator's valueless-completion walk cleared this export's
    /// implementation (ADR 0035): a block-bodied, non-`async`, non-generator
    /// function whose own body carries no `return` with an expression. A
    /// proposal input for `returns: []` and never a proof; `false` is
    /// "do not propose", including for a summary no walk reached.
    pub returns_walk_clean: bool,
    /// Whether that same walk declined *only* because this export's own body
    /// hands its caller a value, and no such completion is a literal that is an
    /// object on every run (ADR 0113, `returns_walk::value_completion`): a
    /// non-`async`, non-generator function with a value-carrying `return` or an
    /// expression body. It is the shape a `returns` closure over a primitive
    /// completion can describe, and a proposal input only: syntax cannot tell a
    /// primitive from an object in general, so the certifier's implementation
    /// census decides that from the producer's types. `false` is "do not
    /// propose", including for a summary no walk reached.
    pub returns_value_completion: bool,
    /// ADR 0172: literal container proposals; member behavior is census-owned.
    pub returns_literal_structures: Vec<contract_semantics::ValueShape>,
    /// ADR 0145's proposal input: when every value-carrying completion of this
    /// export's own body is a function or arrow literal, the call claims each
    /// literal's syntax does not already rule out, one per distinct shape
    /// (`returns_walk::described_callable_returns`). Empty is "do not
    /// propose", including for a summary no walk reached; the certifier's
    /// census decides every one of them from the producer's facts.
    pub returns_described_callables: Vec<contract_semantics::DescribedCall>,
    /// ADR 0146's proposal input: the described callables this export's
    /// completions would hand back if they read a signal it created
    /// (`returns_walk::reading_callable_returns`), which the generator proposes
    /// where its reactive analysis described the return as an accessor. Empty
    /// is "do not propose".
    pub returns_reading_callables: Vec<contract_semantics::DescribedCall>,
    /// Whether this export is a `const` binding of one identifier whose
    /// initializer is exactly a non-computed member access --
    /// `const entries = Object.entries` -- and whose summary no body produced.
    /// A proposal input only (ADR 0103, amended 2026-09-23): syntax cannot say
    /// which object the member belongs to, so the generator proposes the call
    /// domains ADR 0103's default-library alias census decides, `creates` and
    /// `callbacks` beside the `reads` it already proposes, and that census
    /// decides them from the producer's identity fact and its reviewed member
    /// table. `false` is "do not propose".
    pub member_alias_initializer: bool,
    /// The member access that initializer spells, `Object.keys`, when
    /// [`Self::member_alias_initializer`] holds: which reviewed row's return the
    /// generator proposes (the second 2026-09-24 amendment to ADR 0103). A
    /// spelling, never an identity -- the certifier's census decides the member
    /// from the producer's fact.
    pub member_alias_spelling: Option<String>,
    /// ADR 0115's proposal input: the argument containers this export's own
    /// completions hand back, when its syntax is nothing but conditionals over
    /// its whole parameters and array literals of them, with at least two
    /// distinct ones. Empty is "do not propose". The certifier decides it from
    /// the producer's own arms of each return.
    pub returns_argument_containers: Vec<ArgumentContainer>,
    /// The parameters this export's own body calls directly -- the callee is
    /// the parameter itself, a plain undefaulted binding written nowhere, and
    /// the call is written in the body of the function that declares it,
    /// outside any nested callable (ADR 0100). The
    /// interprocedural pass writes an `inline` callback row for exactly that
    /// shape and for a dialect primitive's inline position alike, and the wire
    /// does not tell them apart; this set does, so the generator proposes a
    /// described `callbacks` closure only for rows the implementation census
    /// can confirm site for site. A proposal input, never evidence: empty is
    /// "do not propose", and a summary no pass reached is empty.
    pub direct_callback_parameters: BTreeSet<usize>,
    /// ADR 0183: the parameters whose callback row is an owned computation
    /// (`tracked`, same-stack, owner `created`) invoked on every call. For a
    /// local summary, the generator's proposal input: such a row publishes
    /// `min: 1`, and the census proves it. For an accepted export, the rows
    /// the document states that way. Empty is "no lower bound".
    pub guaranteed_callback_parameters: BTreeSet<usize>,
    /// The parameters whose own value this export's own body reads a property
    /// of -- `v.length` with `v` the parameter itself, outside any nested
    /// callable, not in write position and not the callee of a call -- which
    /// the generator describes as a `get` item in `callbacks` (item A of
    /// ways-to-improve § 3.3). A proposal input in the same family as
    /// [`Self::direct_callback_parameters`]: never encoded, never evidence,
    /// empty is "describe nothing".
    pub direct_accessor_parameters: BTreeSet<usize>,
    /// The same, for a coercing operand (`a < b`, `` `${v}` ``, `+v`) that is
    /// the parameter's own identifier: a `coerce` item.
    pub direct_coerced_parameters: BTreeSet<usize>,
    /// `(parameter, path)` for a call this export's own body makes of a
    /// literal-keyed member of a parameter's own unwritten binding --
    /// `handler[0](…)`, `h["run"](…)`, outside any nested callable -- which the
    /// interprocedural pass writes as an `inline` callback row carrying that
    /// path (item B of ways-to-improve § 3.3). The member path's call item
    /// the implementation census can confirm site for site. A proposal input
    /// in the family of [`Self::direct_callback_parameters`]: never encoded,
    /// never evidence, empty is "do not propose".
    pub direct_member_callback_parameters: BTreeSet<(usize, Vec<String>)>,
    /// The parameters whose value, or a value reached through its members,
    /// this export iterates anywhere in its body (a `for…of`, a spread, an
    /// array pattern), and those bound by an array pattern in parameter
    /// position. The generator derives no `iterate` item, so it declines to
    /// propose a `callbacks` enumeration with non-call items beside any of
    /// these: it could not describe the enumeration whole. Never evidence.
    pub iterated_parameters: BTreeSet<usize>,
    /// ADR 0139: the parameters a class export's constructor keeps on the
    /// instance for later member calls, as the generator's byte walk
    /// ([`solid_facts::ast::retained_constructor_arguments`]) found them --
    /// each stored once as `this.<key> = p` in the constructor's own frame,
    /// with every other use a direct call there, and every read of the key a
    /// member call nothing at construction reaches. The generator describes
    /// each as a `result-access` item. A proposal input in the family of
    /// [`Self::direct_callback_parameters`]: never encoded, never evidence,
    /// empty is "do not propose"; the Type Facts producer's own census of the
    /// class decides the item.
    pub result_access_parameters: BTreeSet<usize>,
    /// ADR 0152: the export's argument slots whose callable the value this
    /// export returns invokes, on the stack of whoever invokes that value,
    /// exactly once per invocation -- read from an accepted contract whose
    /// closed `returns` is described callables every one of which names the
    /// slot, and whose closed `callbacks` invokes the slot at `result-access`
    /// and nowhere else. The consumer composes a callback written at such a
    /// slot with the proven invocations of the returned value
    /// (`execution_role::contract_returned_invoker_callback_role`). Empty for
    /// every locally inferred summary: never encoded, and a projection only.
    pub returned_invocations: BTreeSet<usize>,
    /// ADR 0235: what one call of each returned member does, by member key
    /// (a tuple index, or an object property name), for members an accepted
    /// contract states as `effectful-callable`. A destructured member's call
    /// is bound to its entry like a call of an export. Projection only.
    pub returned_member_effects: BTreeMap<String, ContractExport>,
    /// Projection only: one call of an exactly agreed whole returned function.
    pub returned_callable_effects: Option<Box<ContractExport>>,
    /// ADR 0109: the parameter whose reactivity a props merge this export
    /// returns carries, when the generator's own walk cleared the body
    /// ([`crate::returns_walk::MergedPropsReturns`]).
    ///
    /// A proposal input and never a proof: `None` is "do not propose",
    /// including for a summary no walk reached. The certifier re-derives every
    /// premise from the producer's control-flow and call censuses.
    pub merged_props_return: Option<usize>,
    /// The accepted dependency export this summary was *projected from*, when
    /// the public name is a cross-package re-export and nothing in this
    /// package declares it.
    ///
    /// Only [`crate::project_accepted_export`] sets it, from the exact
    /// accepted contract and export identity the projection resolved. `None`
    /// is the fail-closed default every locally inferred summary keeps, and it
    /// is never decoded from or encoded into a package-contract document.
    ///
    /// Re-emission reads it to decide *which* proposal filters apply. A
    /// projected summary has no local implementation, so the generator's own
    /// walks (`creates_walk_clean`, `returns_walk_clean`,
    /// `direct_callback_parameters`) are all silent about it — and silence is
    /// "do not propose". Applying them to an inherited summary therefore
    /// discards the dependency's certified closure for every domain, which is
    /// the defect `phase21/2026-09-15-closure-gap-plan.md` § 1 records. What
    /// replaces them is not a weaker filter but a different premise: the
    /// closure is the dependency's, and the certifier discharges it by
    /// composition from the dependency's receipt rather than by a census of
    /// bytes this artifact does not contain.
    pub inherited_from: Option<InheritedExportOrigin>,
    /// ADR 0153 part 3: the package exports whose contexts this export's
    /// claims assume no value from outside the package for, read from the
    /// accepted document. A consumer program that provides one of them loses
    /// every claim of the export at its import (`contracts.rs`,
    /// `provided_context_premises`). Empty for every summary that states none,
    /// and for every locally inferred summary.
    pub context_premises: Vec<String>,
}

/// The accepted dependency export a re-exported public name was projected
/// from: enough identity to name the claim in a plan sidecar and to attribute
/// a proposal to the contract that owns it.
///
/// **Measurement and attribution, never authority.** Nothing downstream
/// discharges a closure from these strings: the certifier rebinds the
/// re-export independently, from the parent's snapshot-verified runtime
/// binding and the dependency node's own plan, because a provenance field
/// travelling through a document is exactly the kind of self-report the
/// precision contract refuses to read as proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InheritedExportOrigin {
    pub package_name: String,
    pub package_version: String,
    pub artifact_case: String,
    pub semantic_digest: String,
    pub entrypoint: String,
    pub export: String,
}

impl ContractExport {
    /// An identified export whose runtime kind could not be established.
    /// This carries no negative or positive behavioral claims (ADR 0011).
    #[must_use]
    pub fn unknown_runtime_kind() -> Self {
        Self {
            kind: "unknown".into(),
            reactive_reads: ContractClaim::Open,
            returns: ContractClaim::Open,
            callbacks: ContractClaim::Open,
            owner_requirements: ContractClaim::Open,
            async_behavior: ContractClaim::Open,
            ..Self::default()
        }
    }

    /// Whether this summary is a projection of an accepted dependency export
    /// whose `domain` that dependency's contract **closes**.
    ///
    /// The consumer-side spelling of "closed" is the one
    /// [`crate::project_accepted_export`] writes: a `Known` claim whose domain
    /// is absent from `open_claims`, and for `creates` the domain's own
    /// closed-and-empty flag, because `project_owner_requirements` keeps only
    /// the obligation-imposing operations and a `creates` the dependency
    /// publishes need not survive it.
    ///
    /// `false` for every locally inferred summary, and for every domain a
    /// projection left open. It is the premise re-emission substitutes for the
    /// local proposal walks, never an addition to them: a summary that is both
    /// inherited and walked cannot exist, since a cross-package re-export has
    /// no local symbol for a walk to reach.
    #[must_use]
    pub fn inherited_closure(&self, domain: contract_semantics::ClaimDomain) -> bool {
        use contract_semantics::ClaimDomain;
        if self.inherited_from.is_none() {
            return false;
        }
        let closed = |claim_is_known: bool, domain: ClaimDomain| {
            claim_is_known && !self.open_claims.contains(&domain)
        };
        match domain {
            ClaimDomain::Creates => self.creates_closed_empty,
            // ADR 0143: the empty closure only. `Known(None)` is also the
            // projection of a closed claim over outputs that name no leaf, and
            // re-emitting that as `returns: []` states the dependency's export
            // yields no value -- false for every `plain` return.
            ClaimDomain::Returns => {
                self.returns_closed_empty && closed(!self.returns.is_open(), domain)
            }
            ClaimDomain::Reads => closed(!self.reactive_reads.is_open(), domain),
            ClaimDomain::Callbacks => closed(!self.callbacks.is_open(), domain),
            _ => false,
        }
    }

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

/// Where a composed reactive-read row was composed from: the export whose own
/// read this row is, and the ordinal of that read in *that* export's own list.
///
/// The ordinal is what makes the claim addressable: `normalize_export` names a
/// read operation `read-<ordinal>` over the same list in the same order, so
/// `ComposedReactiveRead { export: "createPolled", read: 0 }` names exactly
/// `createPolled:operation:read-0` and nothing else. A name alone would name a
/// set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComposedReactiveRead {
    pub export: String,
    pub read: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractReactiveRead {
    pub kind: String,
    pub label: String,
    pub parameter: Option<usize>,
    /// Exact access path from the parameter for a `parameter-member` read,
    /// when every access contributing to the row walks the same static
    /// properties. `parsed.modifiers.includes(m)` is `["modifiers",
    /// "includes"]`, not `["includes"]`: a consumer matches this as a *prefix*
    /// of the observed access, so naming only the last segment describes a
    /// property the parameter does not have and can never be witnessed.
    ///
    /// `None` where contributing accesses disagree, or where no segment could
    /// be named exactly. Older contracts omit it and remain valid, but only a
    /// named path can be runtime-probed without guessing which property to
    /// instrument.
    pub path: Option<Vec<String>>,
    /// The symbol of the summary node this row was composed from, when the
    /// read was discovered in *another* node and reached this one across a
    /// call edge. Unresolved provenance: an internal node identity, never
    /// published.
    ///
    /// [`contract_export_summaries`] resolves it to `composed_from` and clears
    /// it, so an emitted contract never carries it and a consumer can never
    /// read a provenance the aggregation could not name. It is a field rather
    /// than a side table because the per-node projection is cached and
    /// parallel: the node that discovers a read is knowable there, and the
    /// export it is published under is not.
    ///
    /// The symbol's own text rather than the interned identity, because this
    /// type crosses the crate boundary and the interner does not. It is only
    /// ever compared for equality against another symbol's text.
    pub composed_owner: Option<String>,
    /// The published provenance: this row's read is the named export's own,
    /// performed through this export's call to it.
    ///
    /// `None` is every case the aggregation could not name exactly — an owner
    /// that is not an export of this project, an owner exported under more
    /// than one name, an owner whose own read list does not carry a row with
    /// this row's identity, and a row the export performs itself. A consumer
    /// reads `None` as "no provenance stated" and falls back to requiring the
    /// export's own evidence; provenance may only ever *add* a discharge
    /// route.
    pub composed_from: Option<ComposedReactiveRead>,
}

/// When a `tracked` callback row runs, relative to the export returning.
///
/// `execution: "tracked"` is an *attribution* word: it says the runtime
/// subscribes the callback's reads, and says nothing about whether the export
/// has already run it. `inline` and `deferred` carry their schedule in the word
/// itself; `tracked` cannot, because 1.x `createMemo` runs its compute during
/// the creating call while 1.x `createEffect` queues it, and 2.0 disagrees with
/// 1.x on `createEffect`. The schedule is
/// [`solid_dialect::Dialect::tracked_callback_timing`]'s to answer, and it is
/// carried here rather than re-derived from the word — a consumer that reads
/// "queued" out of "tracked" publishes a promise the runtime breaks, which is
/// what `mergeProps` and `createMemo` rows did before this field existed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackSchedule {
    /// The callback has run by the time the export returns.
    SameStack,
    /// The export queued it; it has not run when the export returns.
    Queued,
    /// The runtime hands it to an external scheduler.
    External,
    /// The dialect states no timing for this slot, so no schedule word is
    /// honest. The emitted operation carries no execution point at all rather
    /// than a guessed one.
    Unestablished,
    /// ADR 0139, and only on a `deferred` row: the export keeps the callable
    /// only in the value it returns (for a construction, the instance), and
    /// it runs later, on the stack of code that invokes it through that value.
    /// The row is a deferred invocation in every pass that models one; the
    /// variant exists so an accepted contract's `result-access` item projects
    /// back, and is re-emitted, as exactly that item.
    ResultAccess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCallback {
    pub parameter: usize,
    pub execution: String,
    /// The schedule of a `tracked` row, where the producer established one.
    /// `None` is a producer that did not compute a schedule for this row and
    /// leaves the consumer's historical default in place; it is meaningless
    /// for `inline` and for `deferred`, whose word already carries the
    /// schedule -- except [`CallbackSchedule::ResultAccess`], which only a
    /// `deferred` row carries and which names ADR 0139's retention event.
    pub schedule: Option<CallbackSchedule>,
    /// Runtime arguments supplied when this callback is invoked. `null`
    /// preserves an unmodeled ordinary value at that position; a structured
    /// descriptor uses the same bounded accessor/store/tuple/object vocabulary
    /// as exported returns.
    pub arguments: Vec<Option<ContractReturn>>,
    /// The owner context in which the runtime invokes this callback. Missing
    /// means the package contract describes timing only; consumers must keep
    /// the existing fail-closed owner behavior for that callback.
    pub owner: Option<String>,
    /// Whether the callback is proven to run with no listener of the caller's
    /// current, defined per execution word. `true` publishes
    /// `tracking: "untracked"`; `false` publishes `ambient-at-execution`, which
    /// claims nothing about the listener and is the fail-closed answer.
    ///
    /// - `inline`: a `Detaching` wrapper (`untrack`, `createRoot`,
    ///   `runWithOwner`, a package row that states the same) stands between the
    ///   export and the callback. A bare `fn()` does not clear:
    ///   `@solid-primitives/utils`' `access` is `typeof v === "function" ? v() : v`
    ///   and its row once said `untracked` like `untrack`'s did.
    /// - `deferred`: the deferral is proven to run the callback on a fresh stack
    ///   or with the listener cleared -- a reviewed host queue
    ///   (`runtime_semantics::FRESH_STACK_SCHEDULERS`), a dialect deferred slot
    ///   the dialect states untracked, a clearing wrapper *inside* the deferral,
    ///   or a dependency's row that states one. "Runs after the export returns"
    ///   is not that proof: a returned closure (`safe`, `pipe`), a bound
    ///   function and an event listener all run on their caller's stack, inside
    ///   whatever computation that caller is in.
    /// - `tracked`: always `false`. The word is the attribution claim, and a
    ///   clearing wrapper outside a tracking one cannot undo its subscription.
    ///
    /// A bool rather than an enum because the state space is exactly
    /// (word x proven-or-not): no consumer distinguishes *why* a clearing was
    /// proven, and the one invalid pair, `tracked` with `true`, is excluded at
    /// both constructors -- the chain composition and
    /// [`ContractCallback::clears_tracking_from`].
    pub clears_tracking: bool,
    /// Which protocol of the caller's value this row invokes. `Call` is every
    /// row an analysis pass writes; the others arrive only from an accepted
    /// contract's non-call `invoke` items (a property read, iteration,
    /// coercion or `hasInstance` of the argument), and the generator's own
    /// derivation of them, and they are **not** inline invocations of a
    /// callable: no consumer pass may read one as a call of the argument. See
    /// [`ContractCallback::is_invocation`].
    pub protocol: contract_semantics::InvokeProtocol,
    /// The member of the argument at [`Self::parameter`] this row invokes,
    /// outwards from the argument: empty for the argument itself, `["0"]` for
    /// `handler[0](…)` (item B of ways-to-improve § 3.3). A row with a path is
    /// a call of *that member*, never of the argument: see
    /// [`ContractCallback::invokes_argument`] and
    /// [`ContractCallback::invokes_member`].
    pub path: Vec<String>,
}

impl ContractCallback {
    /// The clearing bit a contract operation's tracking word states for a row
    /// with this execution word: the exact inverse of the generator's mapping,
    /// so a row that round-trips through a document keeps its claim and a
    /// `tracked` row never acquires one. See [`Self::clears_tracking`].
    #[must_use]
    pub fn clears_tracking_from(execution: &str, tracking: contract_semantics::Tracking) -> bool {
        matches!(execution, "inline" | "deferred")
            && tracking == contract_semantics::Tracking::Untracked
    }

    /// Whether this row is an invocation of the argument *as a callable* --
    /// the only kind of row an interprocedural or owner pass models.
    ///
    /// A non-call row is kept in [`ContractExport::callbacks`] so a closed
    /// enumeration stays closed and re-emission republishes it; every pass
    /// that reads rows to build edges, invoked parameters, wrappers, owner
    /// edges or accessor arguments filters on this first. A property read or
    /// coercion of the caller's value runs that value's own traps at the call,
    /// on the caller's stack, in the caller's tracking context -- what the
    /// value's author wrote -- and today such a use of a non-callable argument
    /// raises no obligation at all, so ignoring the row loses nothing the
    /// consumer modelled.
    #[must_use]
    pub fn is_invocation(&self) -> bool {
        self.protocol == contract_semantics::InvokeProtocol::Call
    }

    /// Whether this row is ADR 0139's `result-access` item: a `deferred`
    /// invocation of a callable the export keeps only in the value it
    /// returns. Every pass that models invocations reads it as the deferred
    /// row it is; only projection and re-emission ask.
    #[must_use]
    pub fn is_result_access(&self) -> bool {
        self.execution == "deferred" && self.schedule == Some(CallbackSchedule::ResultAccess)
    }

    /// Whether this row calls the argument at [`Self::parameter`] itself: an
    /// invocation with an empty path. Every pass that reads a row as "the
    /// value passed here is called" -- a call-graph edge to the argument, an
    /// invoked parameter, a wrapper, an owner edge, an execution role for the
    /// argument -- asks this, not [`Self::is_invocation`]: a member-path row
    /// calls a member of the argument, and reading it as a call of the
    /// argument would fold `callHandler(e, handlerProp)` as a call of
    /// `handlerProp`.
    #[must_use]
    pub fn invokes_argument(&self) -> bool {
        self.is_invocation() && self.path.is_empty()
    }

    /// Whether this row calls a member of the argument, at [`Self::path`]
    /// (item B of ways-to-improve § 3.3). A consumer folds such a row only
    /// when the argument written at the slot resolves that member exactly.
    #[must_use]
    pub fn invokes_member(&self) -> bool {
        self.is_invocation() && !self.path.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractOwnerRequirement {
    pub operation: OwnerRequirementOperation,
    /// ADR 0161: whether every normal completion of one call performs an
    /// operation of this kind. From an accepted contract, a certified count
    /// with `min >= 1` on some operation of the kind; from the generator, a
    /// requirement whose site is a dialect primitive call the export makes on
    /// every completion. `false` is "may register": calling the export with no
    /// owner is then a proof obligation, never a proven violation.
    pub guaranteed: bool,
    /// The accepted operation's guard, when only some calls register: an
    /// argument-kind guard is evaluated at each call
    /// (`owners::owner_requirement_at_call`, ADR 0223), and any other guard
    /// leaves the registration possible, never guaranteed.
    pub guard: Option<crate::contract_semantics::Guard>,
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
        if name.is_empty() || !matches!(summary.kind.as_str(), "function" | "value" | "unknown") {
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
                    !read.label.is_empty() && read.parameter.is_none() && read.path.is_none()
                }
                // A stated path must name every one of its segments: an empty
                // segment names no property, and the spelling for "no segment
                // could be named" is an absent path, not a blank one.
                "parameter-member" => {
                    read.label.is_empty()
                        && read.parameter.is_some()
                        && read
                            .path
                            .as_ref()
                            .is_none_or(|path| path.iter().all(|segment| !segment.is_empty()))
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub development_feedback: Vec<development_feedback::DevelopmentFile>,
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
    /// Which call sites forbid *proposing* a closed `creates` domain.
    ///
    /// Deliberately not on the wire: it is generation-time input, and its
    /// [`Default`] refuses every span, so a deserialized `Program` proposes
    /// nothing rather than proposing everything.
    #[serde(skip)]
    pub creates_proposal_walk: CreatesProposalWalk,
    /// ADR 0109's proposal input: which parameter's reactivity a props merge
    /// returned by each function carries.
    ///
    /// Off the wire for the same reason as the walk above, and with the same
    /// fail-closed [`Default`]: an absent entry is "do not propose", so a
    /// deserialized `Program` proposes none of these rather than all of them.
    #[serde(skip)]
    pub merged_props_returns: returns_walk::MergedPropsReturns,
    /// ADR 0115's proposal input, off the wire and fail-closed the same way.
    #[serde(skip)]
    pub argument_container_returns: returns_walk::ArgumentContainerReturns,
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
        let external_contracts = contracts.external_packages();
        let contracts = external_contracts.as_ref();
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
///    call like any other; an unrecognised callee retains its spelling for
///    diagnostics without gaining built-in runtime semantics.
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

    /// The source spelling for messages, never for asking what a callee is.
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

/// The primitive one concrete call denotes: [`primitive_name`] for its callee,
/// then the dialect's [`solid_dialect::Dialect::call_form`] for its options.
///
/// Every classifier that holds a call asks this rather than resolving the
/// callee alone, so a form the dialect distinguishes (2.0's
/// `dynamic(source, { static: true })`) is the same form in every pass. The
/// spelling stays the callee's: a form is a call of the export, not another
/// name.
fn call_primitive_name(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    // A callee that is itself a call (`untrack(() => props.ref)?.(fn)`,
    // `factory()()`) is the *result* of that inner call. The compiler's entity
    // at its span answers with the inner callee's symbol, which would make the
    // outer call the primitive and hand its arguments to the primitive's
    // callback slots.
    if file
        .ast
        .call_at(file.ast.peel_ts_sugar_span(call.callee))
        .is_some()
    {
        return None;
    }
    let name = primitive_name(
        file.path.as_str(),
        call.callee,
        call.static_callee(&file.source),
        entities,
        symbol_names,
        dialect,
    )?;
    let PrimitiveName::Known(primitive, spelling) = name else {
        return Some(name);
    };
    let form = dialect.call_form(primitive, &|argument, key| {
        call_option_literal(file, call, argument, key)
    });
    Some(PrimitiveName::Known(form, spelling))
}

/// What `call`'s syntax proves about the boolean option `key` in its
/// `argument`-th argument. See [`solid_dialect::OptionLiteral`] for the four
/// answers; everything not proven is `Unknown`.
fn call_option_literal(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    key: &str,
) -> solid_dialect::OptionLiteral {
    use solid_dialect::OptionLiteral;
    use solid_facts::ast::{ArgumentValueKind, RuntimeValueKind};
    // A spread anywhere up to the slot moves arguments into it at runtime.
    if call
        .arguments
        .iter()
        .take(argument.saturating_add(1))
        .any(|candidate| candidate.spread)
    {
        return OptionLiteral::Unknown;
    }
    let Some(options) = call.arguments.get(argument) else {
        return OptionLiteral::Absent;
    };
    if matches!(
        options.value,
        ArgumentValueKind::Undefined | ArgumentValueKind::Null
    ) || options.runtime_value_kind == RuntimeValueKind::Nullish
    {
        return OptionLiteral::Absent;
    }
    if !options.exact_object_literal {
        return OptionLiteral::Unknown;
    }
    let named = |span: Span| file.source_text(span) == Some(key);
    // A later duplicate key wins at runtime, so only the last one decides.
    let Some(last) = options
        .property_names
        .iter()
        .copied()
        .filter(|span| named(*span))
        .max_by_key(|span| span.start)
    else {
        return OptionLiteral::Absent;
    };
    // `boolean_properties` records only unwrapped `key: true|false` literals,
    // so a key it does not carry has a value the syntax does not prove.
    match options
        .boolean_properties
        .iter()
        .find(|property| property.name == last)
    {
        Some(property) if property.value => OptionLiteral::True,
        Some(_) => OptionLiteral::False,
        None => OptionLiteral::Unknown,
    }
}

fn jsx_primitive_name(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    exact_jsx_primitive_name(file, element, entities, symbol_names, dialect).or_else(|| {
        // The spelling of an imported binding, for a tag the entity table does
        // not resolve. A tag that resolves to another symbol, a local `For`
        // shadowing the import, is that symbol and names no primitive.
        let tag = entities.at(file.path.as_str(), element.name.span);
        file.ast
            .imports
            .iter()
            .filter(|import| dialect.owns_module(&import.module))
            .flat_map(|import| &import.bindings)
            .find_map(|binding| {
                (binding.kind != solid_facts::ast::ImportKind::Namespace
                    && file.source_text(binding.local.span) == file.source_text(element.name.span)
                    && tag.is_none_or(|tag| {
                        entities.at(file.path.as_str(), binding.local.span) == Some(tag)
                    }))
                .then_some(binding.imported.as_deref())
                .flatten()
            })
            .map(|name| PrimitiveName::new(name, dialect))
    })
}

/// The dialect primitive a JSX tag names by symbol identity: its own resolved
/// entity, or a member of a dialect namespace import. Unlike
/// [`jsx_primitive_name`], a local binding that shadows an imported spelling
/// names nothing here.
fn exact_jsx_primitive_name(
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
}

fn location(path: impl Into<Arc<str>>, span: Span) -> Location {
    span.location(path)
}

/// Whether `declaration` is reactive state a package contract declares for
/// one of its exports (`<contract source>#<export>`), rather than a binding in
/// the project's own source.
pub(crate) fn contract_declared_state(declaration: &Location) -> bool {
    let path = declaration.path.as_ref();
    // ADR 0235: a returned member's effects (`<export location>[<key>]`).
    // When it runs is the caller's choice -- `start()` in a click handler
    // reads nothing untracked -- so its read is the call site's, not the
    // package's implementation.
    if path.ends_with(']') {
        return false;
    }
    path.starts_with("accepted:")
        || path
            .split_once('#')
            .is_some_and(|(source, _)| source.ends_with(".json"))
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
        // `flush` is 2.0 vocabulary; `batch` is 1.x's and 2.0 does not have
        // it, so the same spelling resolves in one vocabulary and not another.
        // This used to be shown against the 1.x dialect directly; with one
        // vocabulary it is shown the way a consumer would meet it.
        assert!(matches!(
            PrimitiveName::new("flush", &solid_dialect::Solid2),
            PrimitiveName::Known(Primitive::Flush, "flush")
        ));
        assert!(matches!(
            PrimitiveName::new("batch", &solid_dialect::Solid2),
            PrimitiveName::Other(_)
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
            owner: None,
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
            runtime_resolutions: None,
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

        let direct_members = vec![vec![crate::interproc::ParameterMemberInvocation {
            parameter: 0,
            path: vec!["of".into(), "values".into()],
            in_owner_body: true,
            primitive_builtin: false,
        }]];
        let direct_view = InterproceduralResultView {
            invoked_parameter_members: &direct_members,
            ..present_view
        };
        let direct_state = InterproceduralResultDependencyState::Function {
            name: nodes[0].name.clone(),
            summary: Vec::new(),
            invoked_parameters: Vec::new(),
            invoked_parameter_members: direct_members[0].clone(),
        };
        let mut captured_members = direct_members.clone();
        captured_members[0][0].in_owner_body = false;
        let captured_view = InterproceduralResultView {
            invoked_parameter_members: &captured_members,
            ..direct_view
        };
        assert!(direct_view.dependency_matches(&direct_state, &dependency));
        assert!(!captured_view.dependency_matches(&direct_state, &dependency));
    }
}
