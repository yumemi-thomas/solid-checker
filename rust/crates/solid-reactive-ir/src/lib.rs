//! The reactive IR: project indexes, interprocedural analysis, execution
//! roles, owner and reachability proofs, and the findings they produce.
//!
//! Module map (each row is a file or directory under `src/`). The analysis
//! modules reference each other cyclically (`execution_role`, `owners`,
//! `indexes`, `interproc`), so the crate is split inside, not across.
//! Directories hold a facade `foo.rs` with shared types plus child modules
//! re-exported through it; `foo/tests.rs` holds the unit tests.
//!
//! | Module | Responsibility |
//! | --- | --- |
//! | `pipeline` | Build entry points and the staged incremental pipeline. |
//! | `cache` | Cross-generation state: per-file contributions, fingerprints, reuse checks. |
//! | `symbols` | TypeScript symbol, alias-root and entity indexes, full and incremental. |
//! | `indexes` | `ProjectIndexes` and `SemanticLookup`, the semantic query surface. Children: completion, class_prototype, primitive_proof, callees, member_values, components. |
//! | `identity` | Compact symbol identities shared by the indexes. |
//! | `source_discovery` | Reactive sources, accessors and props reactivity. Children: returned_sources, discover_file, discover_sources, typed_sources, props_reactivity. |
//! | `static_rules`, `static_api`, `server_rules` | File-local rules run right after source discovery. |
//! | `local_access` | Per-file reads and writes of reactive values. |
//! | `interproc` | Interprocedural reactive-read analysis. Children: parameter_use, retained_literal, graph, obligations, return_escape, callback_chain, structured_returns, result_view, result_reads, reads. |
//! | `accessor_provenance` | Exact project-owned callable transport (a `#[path]` child of `interproc`). |
//! | `property_gets` | Value Gets instantiated at a package call (a `#[path]` child of `interproc`). |
//! | `execution_role` | Execution-role classification of a span. Children: events, callee_roles, named_callbacks, invocation_flow, unowned_event_literals and more. |
//! | `project_consumer` | Closed, read-only project callback-use summaries (a `#[path]` child of `execution_role`). |
//! | `owners` | Which computations need a reactive owner, and where one is missing. |
//! | `reachability` | Positive execution reachability and call multiplicity. |
//! | `hosts`, `callback_host` | Browser/server host reachability and callback invocation premises. |
//! | `reactive_analysis` | The coupled local/interprocedural stage. |
//! | `attribution` | Discharges parameter-member dispatch obligations in a closed program. |
//! | `cleanup`, `directives`, `effect_api`, `effect_execution` | Leaf-owner diagnostics, directive discovery, `createEffect` semantics. |
//! | `argument_values`, `callback_return`, `literal_member_slots`, `object_members`, `value_identity` | What arguments, returns, members and values prove. |
//! | `summary_semantics`, `runtime_semantics`, `runtime_configuration` | Summary vocabulary, audited runtime behaviour, assumed runtime conditions. |
//! | `package_boundary`, `boundary_defects`, `router_params` | Package-source boundary obligations and the router-parameters premise. |
//! | `findings`, `projection` | The finding model and its projection onto dialect rule catalogs. |
//! | `upstream_compat` | Retained eslint-plugin-solid 0.14.5 heuristics (see AGENTS.md before editing). |
//! | `shadow` | Shadow mode: a retained build is compared with a fresh one (`SOLID_CHECKER_IR_SHADOW`). |
//! | `development_feedback`, `timings` | Executed-feedback models and the stage-timing contract. |
#![forbid(unsafe_code)]

mod argument_values;
mod attribution;
mod boundary_defects;
mod cache;
mod callback_return;
mod cleanup;
mod development_feedback;
mod directives;
pub use development_feedback::{
    DevelopmentFile, DevelopmentFunction, DevelopmentOperation, ResultRelevance,
};
mod effect_api;
mod effect_execution;
mod effect_phase;
mod execution_role;
mod findings;
mod identity;
mod indexes;
mod interproc;
mod literal_member_slots;
mod local_access;
mod owners;
/// The crate's maps and sets hash spans, symbol ids and paths on every
/// lookup; Fx hashing is several times cheaper than SipHash for such keys.
/// No answer depends on iteration order: the default hasher already
/// randomized it on every run.
pub type HashMap<K, V> = std::collections::HashMap<K, V, rustc_hash::FxBuildHasher>;
/// See [`HashMap`].
pub type HashSet<T> = std::collections::HashSet<T, rustc_hash::FxBuildHasher>;

pub mod package_boundary;
mod pipeline;
mod projection;
mod reachability;
mod reactive_analysis;
pub mod route_components;
pub mod router_params;
mod runtime_configuration;
mod runtime_semantics;
pub mod shadow;
pub use runtime_configuration::RuntimeConfigurationPremise;
mod object_members;
mod server_rules;
mod source_discovery;
mod static_api;
mod static_rules;
pub mod summary_semantics;
mod symbols;
mod timings;
mod upstream_compat;
mod value_identity;

pub use owners::function_binding_name;

/// Whether `span` in `file` sits in an instance field initializer of a class,
/// directly or in a function written there: code that runs only when an
/// instance is constructed (review rounds 2-3 of 2026-10-19). The backend
/// demotes every violation anchored there to an uncertifiable result, whatever
/// rule placed it.
#[must_use]
pub fn site_in_instance_field(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
) -> bool {
    execution_role::inside_instance_field(file, span)
}
pub use pipeline::{AnalysisInputs, build, build_with_analysis_inputs_measured};

pub use upstream_compat::rule_options::{RuleOptions, RuleOverride};

pub mod callback_host;
pub mod hosts;

pub use findings::{
    DOCS_BASE_URL, EvidenceStep, Finding, RuleManifestIdentity, RuleMetadata, SolveTimings,
    assert_rules_have_documentation, direct_mutation_wording, finish_findings, rule_manifest_json,
    strict_read_evidence, strict_read_message, strict_read_message_with_compute,
    strict_read_related_locations,
};
pub use projection::{
    CatalogCapabilities, CatalogWording, FindingSeed, FindingWording, StaticDefectTerms,
    StaticDefectText, project_finding, project_findings, static_defect_text,
    suppress_findings_owned_by_enabled_rules,
};

use cache::{BuildCaches, BuildIdentity, IncrementalCacheState, RetainedBuild};
use pipeline::build_with_analysis_inputs_measured_incremental;

use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use execution_role::{
    NamedCallbackRoles, allowed_callback_spans, assigned_member_function_contains,
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
    /// is not a violation, and not a silent clean either: projection reports a
    /// read, write or action here as an uncertifiable obligation of its rule
    /// ([`ReactiveRead::is_uncertifiable`], [`ReactiveWrite::is_uncertifiable`],
    /// [`ActionInvocation::is_uncertifiable`]).
    Unknown,
    /// Proven to run only where no read, write or action rule reports, but in
    /// no single role: invocations in several non-reporting roles (ADR 0323's
    /// merge), or code past the strict-read window (after an unconditional
    /// `await`). Every consumer treats it as it treats [`Self::Unknown`],
    /// except projection, which reports nothing here: the absence of a breach
    /// is proven, only the role is not one.
    Unreported,
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
    /// What the backend read of the installed `@solidjs/router` (ADR 0324).
    /// No request or options document sets it, so it is skipped on the wire.
    #[serde(skip)]
    pub router_install: router_params::RouterInstallEvidence,
    /// Which installed copy of `solid-js` each analyzed file loads at runtime.
    /// Read from disk by the backend; a caller with no disk carries none.
    #[serde(skip)]
    pub package_instances: PackageInstanceEvidence,
}

/// Runtime package identity, decided by module resolution rather than by
/// declarations: TypeScript merges a second installed copy of the same package
/// at the same version into the first copy's declarations, while the runtime
/// loads both and each has its own scheduler queue.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageInstanceEvidence {
    /// The real directory of the `solid-js` that resolving `solid-js` from the
    /// file's directory finds (the nearest `node_modules/solid-js` up the
    /// tree, symlinks resolved), by the file's path. A file with no entry has
    /// no resolved copy, or was not resolved.
    pub solid_js_by_file: std::collections::BTreeMap<String, String>,
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

    /// Whether the selection names a server build of the Solid packages: a
    /// Node-family target, or server rendering. There an effect's apply never
    /// runs and `flush` is empty, so a rule about what the *client* runtime
    /// does with them claims nothing. No selection is the client default, as
    /// everywhere else the audited release is assumed.
    #[must_use]
    pub fn selects_server_build(&self) -> bool {
        matches!(
            self.rendering,
            Some(RuntimeRendering::StringSsr | RuntimeRendering::StreamingSsr)
        ) || self
            .selected_conditions()
            .iter()
            .any(|condition| matches!(condition.as_str(), "node" | "deno" | "worker"))
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
    /// artifact/guard resolver. Condition-resolution mechanics never reach the IR.
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

    /// Whether a read, write or action in this role is reported at all: a
    /// proven role that breaks the rule, or [`Self::Unknown`], where no fact
    /// rules the breach out and the finding is an uncertifiable obligation.
    #[must_use]
    pub const fn reports_unplaced(self) -> bool {
        matches!(self, Self::Unknown)
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
    /// and worded separately for that reason. Also used when an effect apply's
    /// defer options and compute dependencies leave its invocation unresolved.
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
    /// The read was attributed to this call through another function's
    /// interprocedural summary: the read runs inside that function's body, and
    /// whether it runs while the call does (not from a timer, listener,
    /// getter, returned accessor, effect compute or after an await) is not
    /// established by the summary. Measured on 48 real projects, these
    /// attributions were 73 false, 16 benign and 0 user-visible defects.
    #[serde(default, skip_serializing_if = "is_false")]
    pub summary_attributed: bool,
    /// Reactive origin supplied by project-owned accessor transport. This
    /// proves source identity for strict reads, not a synchronous condition
    /// dependency; ordinary summary hops must preserve the distinction.
    #[serde(default, skip_serializing_if = "is_false")]
    pub transported: bool,
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
    /// is possible but not guaranteed ([`Self::callback_invocation_unproven`]),
    /// or no fact places the read at all ([`ExecutionRole::Unknown`]).
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
            || self.execution == ExecutionRole::Unknown
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

impl ReactiveWrite {
    /// Whether a finding about this write is an uncertifiable obligation: no
    /// fact places the write ([`ExecutionRole::Unknown`]), so whether it runs
    /// under a children-capable owner is neither proven nor ruled out.
    #[must_use]
    pub fn is_uncertifiable(&self) -> bool {
        self.execution == ExecutionRole::Unknown
    }
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

impl ActionInvocation {
    /// Whether a finding about this invocation is an uncertifiable obligation,
    /// for [`ReactiveWrite::is_uncertifiable`]'s reason.
    #[must_use]
    pub fn is_uncertifiable(&self) -> bool {
        self.execution == ExecutionRole::Unknown
    }
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
    /// The operation may not happen at this call: the helper performs it on
    /// some completions only (ADR 0275). Projected as uncertifiable, like
    /// [`Self::uncertain`].
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
    /// A present observer's per-key tracking either creates a missing signal
    /// or reaches mandatory cleanup on the retained one. Both paths require
    /// a children/cleanup-capable owner; this states neither cache state.
    ObserverCacheTracking,
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
/// Named here rather than spelled twice because the projection and the
/// obligation attribution have to recognize the class. Producer:
/// `solid-reactive-ir/src/interproc/reads.rs`; consumers: `projection.rs` and
/// `attribution.rs`.
pub const EXPORTED_PARAMETER_MEMBER_DISPATCH: &str = "exported-parameter-member-dispatch";

/// ADR 0289: the `analysis_context` prefix of a dispatch obligation at a
/// project Get of a member that a package analyzed as source (ADR 0284)
/// defines as a reactive getter, where no proof carries the object to the
/// Get. Its `callee` is the Get's text. Producer: `interproc.rs`; consumer:
/// `projection.rs`, which words it.
pub const DEPENDENCY_SOURCE_GETTER: &str = "dependency-source-getter:";

/// ADR 0293: the `analysis_context` prefix of a dispatch obligation at an
/// untracked store Get whose strict-read warning depends on a key
/// solid-checker cannot decide; the rest is the reason. Its `callee` is the
/// Get's text. Producer: `local_access.rs`; consumer: `projection.rs`.
pub const STORE_KEY_UNDECIDED: &str = "store-key-undecided:";

/// ADR 0294: the `analysis_context` of a dispatch obligation at a call or
/// Get, in a role that reports an untracked read, of a value whose only
/// evidence of being a reactive source is a dialect type alias
/// (`Accessor<T>`, `Store<T>`) and whose implementation is not visible.
/// Its `callee` is the read's text. Producers: `local_access.rs`,
/// `static_rules.rs`; consumer: `projection.rs`.
pub const TYPE_ONLY_SOURCE: &str = "type-only-source";

/// The `analysis_context` of a dispatch obligation at a dialect primitive
/// used as a value (`const eff = createEffect`, `Reflect.apply(createEffect,
/// …)`): the calls it reaches are not followed. Its `callee` is the
/// reference's text. Producer: `static_rules.rs`; consumer: `projection.rs`.
pub const PRIMITIVE_VALUE: &str = "primitive-value";
/// ADR 0368 review: a project call the dependency-source program answers as
/// a call error and the project's own program does not
/// (`ProjectFacts::dependency_source_call_errors`).
pub const DEPENDENCY_SOURCE_CALL_ERROR: &str = "dependency-source-program-call-error";

/// ADR 0370: the `analysis_context` prefix of a dispatch obligation at a
/// proven source passed to an analyzed function whose parameter may reach
/// a callee nothing describes; the undescribed callee's text follows the
/// prefix. Its `callee` is the analyzed function's. Producer:
/// `upstream_compat/shared_reactivity.rs`; consumer: `projection.rs`.
pub const FORWARDED_TO_UNDESCRIBED_CALLEE: &str = "forwarded-to-undescribed-callee:";

/// The `analysis_context` of a dispatch obligation at a member call on the
/// result of a primitive that returns a getter (`createSignal(1).slice(0)`,
/// ADR 0338): the value it returns has no provenance the analysis can state.
/// Its `callee` is the primitive and its `member` the method. Producer:
/// `static_rules.rs`; consumer: `projection.rs`.
pub const PRIMITIVE_RESULT_METHOD: &str = "primitive-result-method";

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
    /// A call after an await whose callee reads `accessor` in its own body
    /// (`await load(); return byUser(posts)`): that read runs after the await,
    /// but nothing proves it runs on this call rather than on some paths, so
    /// the finding is an obligation.
    ReactiveReadAfterAwaitThroughCall {
        accessor: String,
        via: String,
    },
    ComponentReturnsConditionally,
    PackageBehaviorUnavailable {
        module: String,
        export: String,
        reexported: bool,
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
    /// that neither the AST nor dependency source proves.
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
    /// An event handler whose function the checker cannot place exactly: an
    /// `on…` attribute reads a member with no exact target, or an intrinsic
    /// element spreads an object it cannot resolve (ADR 0358). The handler may
    /// run owner-requiring work this analysis cannot see, so it is an obligation.
    HandlerTargetUnresolved {
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
    /// The package-behavior-unavailable family (`SC9005`): an import whose
    /// package behavior is not available from analyzed source.
    PackageBehaviorUnavailable,
    MissingEffectFunction,
    ReactiveSourceUncaptured,
    ReactiveDispatchUnresolved,
    ExpectedFunctionGotExpression,
    /// A handler the checker cannot place at an exact function, so the owner it
    /// runs under is unknown (ADR 0358). Its uncertain side is `missing-owner`.
    HandlerOwnerUnresolved,
    UncalledAccessor,
    DirectMutation,
    /// A value crossing the default server-function transport. Deliberately
    /// absent from [`StaticDefectKind::is_unresolved_obligation`]: the open
    /// transport proof is reported as uncertifiable, but it was never part of
    /// the `SC9xxx` obligation census the metrics read, and moving this rule off the static-violation channel did not change
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
            Self::PackageBehaviorUnavailable => "package-behavior-unavailable",
            Self::MissingEffectFunction => "missing-effect-function",
            Self::ReactiveSourceUncaptured => "reactive-source-uncaptured",
            Self::ReactiveDispatchUnresolved => "reactive-dispatch-unresolved",
            Self::ExpectedFunctionGotExpression => "expected-function-got-expression",
            Self::HandlerOwnerUnresolved => "handler-owner-unresolved",
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
            Self::ReactiveReadAfterAwait { .. }
            | Self::ReactiveReadAfterAwaitThroughCall { .. } => {
                StaticDefectFamily::ReactiveReadAfterAwait
            }
            Self::ComponentReturnsConditionally => {
                StaticDefectFamily::ComponentReturnsConditionally
            }
            Self::PackageBehaviorUnavailable { .. } => {
                StaticDefectFamily::PackageBehaviorUnavailable
            }
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
            Self::HandlerTargetUnresolved { .. } => StaticDefectFamily::HandlerOwnerUnresolved,
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
            Self::ReactiveReadAfterAwaitThroughCall { .. } => "ReactiveReadAfterAwaitThroughCall",
            Self::ComponentReturnsConditionally => "ComponentReturnsConditionally",
            Self::PackageBehaviorUnavailable { .. } => "PackageBehaviorUnavailable",
            Self::MissingEffectFunction => "MissingEffectFunction",
            Self::ReactiveSourceUncaptured { .. } => "ReactiveSourceUncaptured",
            Self::ReactiveDispatchUnresolved { .. } => "ReactiveDispatchUnresolved",
            Self::ReactiveCallbackUnresolved { .. } => "ReactiveCallbackUnresolved",
            Self::ResultAccessCallbackUnplaced { .. } => "ResultAccessCallbackUnplaced",
            Self::StructuredReturnUnresolved { .. } => "StructuredReturnUnresolved",
            Self::ReactiveHandlerRead { .. } => "ReactiveHandlerRead",
            Self::HandlerValueUnresolved { .. } => "HandlerValueUnresolved",
            Self::HandlerTargetUnresolved { .. } => "HandlerTargetUnresolved",
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
    /// uncertifiable class — rather than a proven violation. The metrics
    /// count them as unresolved; every consumer consults this so the answer
    /// cannot drift between them.
    #[must_use]
    pub fn is_unresolved_obligation(&self) -> bool {
        matches!(
            self,
            Self::PackageBehaviorUnavailable { .. }
                | Self::ReactiveSourceUncaptured { .. }
                | Self::ReactiveDispatchUnresolved { .. }
                | Self::ReactiveCallbackUnresolved { .. }
                | Self::ResultAccessCallbackUnplaced { .. }
                | Self::StructuredReturnUnresolved { .. }
        )
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
    /// The creation runs under an owner the analysis cannot decide
    /// (`runWithOwner(owner, …)` in the apply phase): an obligation, not a
    /// proven leak.
    #[serde(default)]
    pub uncertain: bool,
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
    /// ADR 0290: the operation is inside a package analyzed as source
    /// (ADR 0284), at this location, and this requirement stands at the
    /// project's call that runs it there, under the call's owner context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through_dependency: Option<Location>,
    pub report: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnerRequirementOperation {
    Effect,
    Cleanup,
    Boundary,
    SettledCleanup,
    /// `useContext(context)` with no owner: rc.13 throws `NoOwnerError` (ADR 0349).
    ContextRead,
    /// `useContext` under an owner of a context created with no default that a
    /// closed program never provides: rc.13 throws `ContextNotFoundError` (ADR 0349).
    ContextProvider,
}

impl OwnerRequirementOperation {
    #[must_use]
    pub(crate) fn from_internal(operation: &str) -> Self {
        match operation {
            "effect" => Self::Effect,
            "cleanup" => Self::Cleanup,
            "boundary" => Self::Boundary,
            "settled-cleanup" => Self::SettledCleanup,
            "context-read" => Self::ContextRead,
            "context-provider" => Self::ContextProvider,
            _ => panic!("owner analysis emitted unknown operation {operation:?}"),
        }
    }
}

const fn is_false(value: &bool) -> bool {
    !*value
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

pub use summary_semantics::{CallbackSchedule, CallbackSummary, ReturnSummary};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// Message-only observations, recomputed from the current merged reads.
    /// These apply sites also read the exact source in their paired compute.
    #[serde(skip)]
    pub apply_compute_reads: HashSet<Location>,
    #[serde(
        default,
        skip_serializing_if = "RuntimeConfigurationPremise::permits_proof"
    )]
    pub runtime_configuration: RuntimeConfigurationPremise,
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
    pub obligation_counts: ObligationCounts,
    /// Bodies of functions analyzed as components whose identity is not
    /// proven ([`site_in_undecided_component`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub undecided_component_regions: Vec<UndecidedRegion>,
}

/// The body of a function analyzed as a component because it returns JSX,
/// although a reference hands it to code that may call it as a plain function
/// (`items().map(Row)`, `{ badge: Row }`, a router config not read from
/// source). Solid runs a function as a component only when `createComponent`
/// calls it, so where the body runs is not proven. Callbacks a dialect
/// primitive places itself are carved out: they run in the primitive's
/// context however the function is entered.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndecidedRegion {
    pub body: Location,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placed_callbacks: Vec<Location>,
}

/// Whether the byte range `start..end` of `path` lies in one of `program`'s
/// [`UndecidedRegion`]s: no violation claimed there is proven.
pub fn site_in_undecided_component(program: &Program, path: &str, start: u64, end: u64) -> bool {
    let within = |location: &Location| {
        location.path.as_ref() == path && location.start_byte <= start && end <= location.end_byte
    };
    program
        .undecided_component_regions
        .iter()
        .any(|region| within(&region.body) && !region.placed_callbacks.iter().any(within))
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedCallbackObligation {
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
    pub location: Location,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BuildTimings {
    pub total: Duration,
    pub cache_lookup: Duration,
    /// Shadow mode ([`shadow`]): the fresh rebuild plus the comparison, which
    /// `total` includes. Zero when shadow mode is off.
    pub shadow: Duration,
    pub reused: bool,
    pub source_discovery_reused_files: u64,
    pub source_discovery_recomputed_files: u64,
    pub interprocedural_graph_reused_files: u64,
    pub interprocedural_graph_recomputed_files: u64,
    pub interprocedural_result_reused_files: u64,
    pub interprocedural_result_recomputed_files: u64,
    /// Deterministic work units of export aggregation (node visits per
    /// discovery round, leaf evaluations, exported properties examined): a
    /// count, so unlike the phase's wall time it does not depend on the
    /// machine. See `StructuredReturnDiscovery::work`.
    pub interprocedural_export_work: u64,
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
    pub boundary_defects: Duration,
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
    /// ADR 0266: one scan per Type Facts snapshot. The snapshot generation
    /// fixes the entity set the scan reads, so a repeated request for the
    /// same generation reuses it instead of rescanning every file.
    runtime_configuration: Option<(
        (solid_dialect::Version, String, u64),
        RuntimeConfigurationPremise,
    )>,
    /// Each file's part of that scan, reused across generations where the
    /// Type Facts delta proves it unchanged.
    runtime_configuration_scan: runtime_configuration::ScanCache,
    /// `None` reads `SOLID_CHECKER_IR_SHADOW` at the first build.
    shadow: Option<shadow::ShadowMode>,
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
        self.build_shared(facts, dialect)
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
        self.build_with_analysis_inputs_shared(
            facts,
            dialect,
            AnalysisInputs::empty(),
            &RuleOptions::default(),
        )
    }

    /// A build given the package boundary and host, both part of the cache
    /// identity.
    pub fn build_with_analysis_inputs_shared(
        &mut self,
        facts: &ProjectFacts,
        dialect: &dyn Dialect,
        inputs: AnalysisInputs<'_>,
        rule_options: &RuleOptions,
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        let total_started = Instant::now();
        let lookup_started = Instant::now();
        let premise_key = (
            dialect.version(),
            facts.project_id.clone(),
            facts.generation.get(),
        );
        let premise = match &self.runtime_configuration {
            Some((key, premise)) if *key == premise_key => premise.clone(),
            _ => {
                let premise = runtime_configuration::scan_retained(
                    facts,
                    dialect,
                    &mut self.runtime_configuration_scan,
                );
                self.runtime_configuration = Some((premise_key, premise.clone()));
                premise
            }
        };
        let identity = BuildIdentity {
            runtime_configuration: premise,
            dialect: dialect.version(),
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            boundary_fingerprints: inputs.cache_fingerprints(),
            rule_options: rule_options.clone(),
        };
        // A domain change drops every cache, so that build is fresh already.
        let domain_reset = self.caches.ensure_domain(
            identity.dialect,
            &identity.project_id,
            &identity.boundary_fingerprints,
            &identity.rule_options,
            &facts.runtime_symbol_redirects,
        );
        if domain_reset {
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
        let (program, mut timings) = build_with_analysis_inputs_measured_incremental(
            facts,
            dialect,
            inputs,
            rule_options,
            self.caches.for_build(),
            identity.runtime_configuration.clone(),
        )?;
        let shadow_mode = *self.shadow.get_or_insert_with(shadow::ShadowMode::from_env);
        let mut shadow_elapsed = Duration::ZERO;
        let program = if shadow_mode != shadow::ShadowMode::Off && !domain_reset {
            let shadow_started = Instant::now();
            let (fresh, _) = build_with_analysis_inputs_measured_incremental(
                facts,
                dialect,
                inputs,
                rule_options,
                BuildCaches::default(),
                runtime_configuration::scan(facts, dialect),
            )?;
            let program = if let Some(mismatch) = shadow::first_difference(&program, &fresh) {
                shadow::report(shadow_mode, &mismatch);
                // Fail closed: serve the fresh answer and distrust every
                // retained cache until the next build rebuilds them.
                self.caches.clear();
                self.runtime_configuration = None;
                fresh
            } else {
                program
            };
            shadow_elapsed = shadow_started.elapsed();
            program
        } else {
            program
        };
        let program = Arc::new(program);
        self.retained = Some(RetainedBuild {
            identity,
            program: Arc::clone(&program),
        });
        timings.total = total_started.elapsed();
        timings.cache_lookup = cache_lookup;
        timings.shadow = shadow_elapsed;
        Ok((program, timings))
    }

    /// Overrides `SOLID_CHECKER_IR_SHADOW` for this builder.
    pub fn set_shadow_mode(&mut self, mode: shadow::ShadowMode) {
        self.shadow = Some(mode);
    }

    pub fn clear(&mut self) {
        self.retained = None;
        self.runtime_configuration = None;
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
            && existing.transported == read.transported
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
    transported_edges: &HashSet<(usize, usize)>,
    guaranteed_edges: &HashSet<(usize, usize)>,
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
            let transported = transported_edges.contains(&(owner, target));
            let mut changed = false;
            for read in &propagated {
                let mut read = read.clone();
                read.transported |= transported;
                // ADR 0276: past this edge the row's own completion standing
                // is a hop below the caller, and the edge's is the caller's.
                read.conditional |= read.local;
                read.local = !guaranteed_edges.contains(&(owner, target));
                changed |= summaries[owner].push_unique(read);
            }
            if changed && !queued[owner] {
                queued[owner] = true;
                worklist.push_back(owner);
            }
        }
    }
}

fn callback_execution_word(execution: ExecutionRole) -> Option<&'static str> {
    match execution {
        // Unknown timing is an open obligation, never an inline promise. A consumer must not execute user code eagerly because the
        // producer lacked an execution proof.
        ExecutionRole::Unknown | ExecutionRole::Unreported => None,
        // A callback the compiler deleted has no execution timing to publish.
        // "inline" would be a promise that a consumer may run the callback
        // eagerly, and this role is evidence that nothing runs it at all — a
        // positive claim dead code cannot support. The summary carries no
        // execution word for it, exactly as for unproven timing.
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

fn push_callback_summary(callbacks: &mut Vec<CallbackSummary>, callback: CallbackSummary) {
    if !callbacks.contains(&callback) {
        callbacks.push(callback);
    }
}

fn function_indices_by_path<T>(functions: &[T]) -> HashMap<String, Vec<usize>>
where
    T: FunctionBoundary,
{
    let mut by_path = HashMap::<String, Vec<usize>>::default();
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
    let mut by_symbol = HashMap::default();
    let mut by_span = HashMap::default();
    let mut parameter_owner = HashMap::default();
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
    Other(Arc<str>),
}

impl PrimitiveName {
    /// Resolves a source-level name against the dialect's vocabulary.
    fn new(name: &str, dialect: &dyn Dialect) -> Self {
        Self::known(name, dialect).unwrap_or_else(|| Self::Other(name.into()))
    }

    /// [`Self::new`] for a symbol's name, sharing its text rather than
    /// copying it: most names a call resolves to are not primitives.
    fn from_symbol(name: &SymbolId, dialect: &dyn Dialect) -> Self {
        Self::known(name, dialect).unwrap_or_else(|| Self::Other(name.shared()))
    }

    fn known(name: &str, dialect: &dyn Dialect) -> Option<Self> {
        dialect
            .primitive(name)
            .and_then(|primitive| Some(Self::Known(primitive, dialect.name_of(primitive)?)))
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
    if let Some(symbol) = entities.at(path, span) {
        symbol_names
            .get(symbol)
            .map(|name| PrimitiveName::from_symbol(name, dialect))
            .or_else(|| {
                let property = static_callee?.rsplit('.').next()?;
                symbol_names
                    .get(format!("{symbol}::{property}").as_str())
                    .map(|name| PrimitiveName::from_symbol(name, dialect))
            })
    } else {
        None
    }
}

/// The dialect primitive a reference names, resolved through the binder: an
/// identifier whose declaration is a named import binding of a primitive, or a
/// non-computed member (`Solid.createEffect`, given by its own span or its
/// property's) of a namespace import binding. The answer is the symbol's at
/// the import binding, which the demand plan always resolves; an entity that
/// merely happens to sit at the reference proves nothing.
fn referenced_primitive(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    use solid_facts::ast::ImportKind;
    let ast = &file.ast;
    let import_binding = |declaration: Span| {
        ast.imports
            .iter()
            .flat_map(|import| &import.bindings)
            .find(|binding| binding.local.span == declaration)
    };
    let known = |name: Option<PrimitiveName>| name.filter(|name| name.primitive().is_some());
    if let Some(declaration) = ast.reference_declaration(span) {
        let binding = import_binding(declaration)?;
        if binding.kind == ImportKind::Namespace {
            return None;
        }
        return known(primitive_name(
            file.path.as_str(),
            declaration,
            None,
            entities,
            symbol_names,
            dialect,
        ));
    }
    let member = ast
        .members
        .iter()
        .find(|member| member.span == span || member.property == span)?;
    if ast.computed_members.contains(&member.span) {
        return None;
    }
    let declaration = ast.reference_declaration(ast.peel_ts_sugar_span(member.object))?;
    if import_binding(declaration)?.kind != ImportKind::Namespace {
        return None;
    }
    let namespace = entities.at(file.path.as_str(), declaration)?;
    let property = file.source_text(member.property)?;
    known(
        symbol_names
            .get(format!("{namespace}::{property}").as_str())
            .map(|name| PrimitiveName::from_symbol(name, dialect)),
    )
}

/// The primitive one concrete call denotes: [`primitive_name`] for its callee,
/// then the dialect's [`solid_dialect::Dialect::call_form`] for its options.
///
/// Every classifier that holds a call asks this rather than resolving the
/// callee alone, so a form the dialect distinguishes (2.0's
/// `dynamic(source, { static: true })`) is the same form in every pass. The
/// spelling stays the callee's: a form is a call of the export, not another
/// name.
/// [`jsx_primitive_name`], answered from `lookup`'s per-file memo under the
/// same exact-identity conditions as [`call_primitive_name_cached`]: the
/// element is the file's own fact, found by its place in that slice.
fn jsx_primitive_name_cached(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    let own_inputs = std::ptr::eq(entities, lookup.entities())
        && std::ptr::eq(symbol_names, lookup.symbol_names())
        && std::ptr::addr_eq(
            std::ptr::from_ref::<dyn Dialect>(dialect),
            std::ptr::from_ref::<dyn Dialect>(lookup.dialect),
        )
        && lookup
            .file_by_path(file.path.as_str())
            .is_some_and(|own| std::ptr::eq(own, file));
    let index = file
        .ast
        .jsx_elements
        .iter()
        .position(|own| std::ptr::eq(own, element));
    if own_inputs && let Some(index) = index {
        return lookup.primitives(file).jsx[index].clone();
    }
    jsx_primitive_name(file, element, entities, symbol_names, dialect)
}

/// [`call_primitive_name`], answered from `lookup`'s per-file memo (which
/// computes exactly it, once per call) when every input is the very one the
/// lookup was built from, compared by pointer, and computed directly
/// otherwise: a host analysis with its own entities, a file copy, a call
/// that is not the file's own fact. The answer is the same either way.
fn call_primitive_name_cached(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<PrimitiveName> {
    let own_inputs = std::ptr::eq(entities, lookup.entities())
        && std::ptr::eq(symbol_names, lookup.symbol_names())
        && std::ptr::addr_eq(
            std::ptr::from_ref::<dyn Dialect>(dialect),
            std::ptr::from_ref::<dyn Dialect>(lookup.dialect),
        )
        && lookup
            .file_by_path(file.path.as_str())
            .is_some_and(|own| std::ptr::eq(own, file));
    if own_inputs
        && let Some(index) = lookup.call_index(file, call.span)
        && file
            .ast
            .calls
            .get(index)
            .is_some_and(|own| std::ptr::eq(own, call))
    {
        return lookup.primitives(file).calls[index].clone();
    }
    call_primitive_name(file, call, entities, symbol_names, dialect)
}

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
    // A whole member span may carry its receiver's symbol, so
    // `createEffect.bind(x)` and `createEffect.call(this, …)` resolve to the
    // primitive while calling a method of it, with other arguments in other
    // slots. When the member's object is itself the primitive, the call is
    // that method's, not the primitive's. A namespace member
    // (`Solid.createEffect`), whose object is the namespace, keeps its answer.
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    // A member call on the *result* of a call (`createSignal(1).slice(1)`,
    // `createMemo(fn).toString()`) is that value's method, never the
    // primitive: the whole member span carries the inner callee's symbol.
    if let Some(member) = file.ast.member_at(callee)
        && file
            .ast
            .call_at(file.ast.peel_ts_sugar_span(member.object))
            .is_some()
    {
        return None;
    }
    if let Some(member) = file.ast.member_at(callee)
        && referenced_primitive(
            file,
            file.ast.peel_ts_sugar_span(member.object),
            entities,
            symbol_names,
            dialect,
        )
        .is_some()
    {
        return None;
    }
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
                .map(|name| PrimitiveName::from_symbol(name, dialect))
        })?
    })
}

fn location(path: impl Into<Arc<str>>, span: Span) -> Location {
    span.location(path)
}

#[cfg(test)]
mod descriptor_text_gate;
#[cfg(test)]
mod tests;
