mod cleanup;
mod contracts;
mod directives;
mod execution_role;
mod identity;
mod indexes;
mod interproc;
mod reachability;
mod runtime_semantics;
mod static_api;
mod symbols;

use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use cleanup::{cleanup_returns_for_file, function_returns_cleanup, leaf_owner_operations_for_file};
use contracts::{
    ContractAnalysis, ContractGraph, ContractSemantics, ResolvedContracts,
    contract_export_summaries, contract_export_summaries_incremental, resolve_contract_imports,
};
use directives::{DirectiveCreationCollector, is_created_primitive, push_directive_creation};
use execution_role::{
    allowed_callback_spans, argument_references_callback_symbol, assigned_member_function_contains,
    async_execution_role, control_flow_execution_role, execution_role, function_symbol,
    named_callback_execution_role, read_analysis_context, semantic_execution_role,
};
use identity::{SymbolId, SymbolInterner, SymbolName, symbol_id, symbol_name};
use indexes::{CachedAstFileIndex, EntitySymbols, ProjectIndexes, SemanticLookup};
use interproc::{
    InterproceduralContext, InterproceduralResult, InterproceduralTimings, SummaryNode,
    SummaryRead, SummaryReads,
};
use reachability::{
    ReachabilityInputs, ReachabilityState, ReachabilityTopology, reachable_call_multiplicity,
    reachable_call_multiplicity_incremental,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solid_facts::{FileFacts, ProjectFacts};
use solid_facts_core::{SourceHash, SourcePath, Span};
use static_api::StaticApiContext;
use symbols::{
    add_solid_import_names, alias_roots_and_source_declarations, async_symbol_root, entity_symbols,
    patch_typescript_indexes, references_for_sources, source_discovery_symbol_semantics,
    symbol_alias_targets, symbol_names, symbols_by_root,
};
use thiserror::Error;
use typefacts::{Declaration, Location, ResolvedCallValidity};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionRole {
    TrackedJsx,
    DeferredCallback,
    EffectApply,
    EventCallback,
    DirectiveApply,
    UntrackedRendering,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactiveWrite {
    pub setter: Arc<str>,
    pub location: Location,
    pub declaration: Location,
    pub execution: ExecutionRole,
    pub allowed_by_option: bool,
    pub context: Arc<str>,
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
    pub primitive: String,
    pub owner: String,
    pub location: Location,
    pub fix: Option<Fix>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidCleanupReturn {
    pub primitive: String,
    pub location: Location,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedCleanupReturn {
    pub primitive: String,
    pub location: Location,
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
    pub operation: String,
    pub location: Location,
    pub uncertain: bool,
    pub report: bool,
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
pub struct ContractExport {
    #[serde(default)]
    pub kind: String,
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
#[serde(deny_unknown_fields)]
pub struct ContractReactiveRead {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractCallback {
    pub parameter: usize,
    pub execution: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractReturn {
    pub kind: String,
    #[serde(default)]
    pub label: String,
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
        }
        if let Some(returned) = &summary.returns
            && (!matches!(returned.kind.as_str(), "accessor" | "store-path")
                || returned.label.is_empty())
        {
            return Err(format!(
                "package contract export {entrypoint}:{name} has an invalid reactive return"
            ));
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
    pub invalid_cleanup_returns: Vec<InvalidCleanupReturn>,
    pub unresolved_cleanup_returns: Vec<UnresolvedCleanupReturn>,
    pub static_violations: Vec<StaticViolation>,
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
    pub parameter: usize,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildIdentity {
    project_id: String,
    generation: u64,
    contracts: Vec<String>,
}

struct RetainedBuild {
    identity: BuildIdentity,
    program: Arc<Program>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDiscoveryIdentity {
    source_hash: SourceHash,
    symbols: Vec<SymbolId>,
}

#[derive(Clone, Default)]
struct SourceDiscoveryContribution {
    accessors: Vec<(SymbolId, (SymbolId, Location))>,
    accessor_origins: Vec<(SymbolId, (SymbolId, SymbolId, Location))>,
    setters: Vec<(SymbolId, (SymbolId, Location, bool))>,
    actions: Vec<(SymbolId, (SymbolId, Location))>,
    source_kinds: Vec<(SymbolId, ReactiveSourceKind)>,
    source_primitives: Vec<(SymbolId, SymbolName)>,
    source_phases: Vec<(SymbolId, u8)>,
    returned_source_symbols: Vec<SymbolId>,
    summary_source_symbols: Vec<SymbolId>,
    source_owned_write: Vec<(SymbolId, bool)>,
    async_sources: Vec<SymbolId>,
    contracted_accessor_symbols: Vec<SymbolId>,
}

struct CachedSourceDiscovery {
    identity: SourceDiscoveryIdentity,
    contribution: SourceDiscoveryContribution,
}

#[derive(Clone)]
struct TypedAccessorContribution {
    owner: Span,
    read: SummaryRead,
}

struct CachedTypedAccessors {
    contributions: Vec<TypedAccessorContribution>,
}

#[derive(Clone)]
enum InterproceduralGraphTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
}

#[derive(Clone, Default)]
struct InterproceduralGraphContribution {
    direct_reads: Vec<(Span, SummaryRead)>,
    edges: Vec<(Span, InterproceduralGraphTarget)>,
    invoked_parameters: Vec<(Span, usize)>,
    callbacks: Vec<(Span, ContractCallback)>,
    callback_forwardings: Vec<(Span, InterproceduralGraphTarget, usize, usize)>,
    contract_generation_obligations: Vec<(Span, ContractGenerationObligation)>,
    returned_bindings: Vec<(SymbolId, SymbolId)>,
    factory_calls: Vec<(Span, SymbolId)>,
}

struct CachedInterproceduralGraph {
    nodes: Vec<SummaryNode>,
    contribution: InterproceduralGraphContribution,
    compiler: Arc<solid_facts::solid_compiler_facts::ExecutionMap>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum InterproceduralResultDependency {
    Symbol(SymbolId),
    InlineFunction(String, Span),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InterproceduralResultDependencyState {
    Missing,
    Function {
        name: Option<String>,
        summary: Vec<SummaryRead>,
        invoked_parameters: Vec<usize>,
    },
    Returned(Vec<SummaryRead>),
    Inline(Vec<SummaryRead>),
}

struct CachedInterproceduralResultFile {
    dependencies: HashSet<InterproceduralResultDependency>,
    reads: Vec<ReactiveRead>,
    compiler: Arc<solid_facts::solid_compiler_facts::ExecutionMap>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedReactiveSource {
    symbol: SymbolId,
    display: SymbolId,
    declaration: Location,
    phase: u8,
}

#[derive(Default)]
struct CachedInterproceduralResults {
    dependency_states:
        HashMap<InterproceduralResultDependency, InterproceduralResultDependencyState>,
    dependency_users: HashMap<InterproceduralResultDependency, usize>,
    files: HashMap<SourcePath, CachedInterproceduralResultFile>,
    reactive_sources: Option<Arc<Vec<CachedReactiveSource>>>,
    contract_exports: CachedContractExports,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ContractNodeKey {
    path: String,
    ordinal: usize,
}

#[derive(Clone, Default, Eq, PartialEq)]
struct ContractExportFragment {
    direct: Vec<(String, ContractExport)>,
    syntax: Vec<(String, ContractExport, bool)>,
    dependencies: HashSet<ContractNodeKey>,
}

#[derive(Default)]
struct CachedContractExports {
    nodes: HashMap<ContractNodeKey, ContractExport>,
    files: HashMap<String, ContractExportFragment>,
    aggregate: Option<Arc<BTreeMap<String, ContractExport>>>,
}

type SourceDiscoverySymbolFingerprint = [u8; 32];

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDiscoveryDeclarationSemantics {
    name: String,
    kind: String,
    runtime: bool,
}

fn source_discovery_declaration_semantic(
    declaration: &Declaration,
) -> SourceDiscoveryDeclarationSemantics {
    SourceDiscoveryDeclarationSemantics {
        name: declaration.name.to_string(),
        kind: declaration.kind.to_string(),
        runtime: !declaration.location.path.ends_with(".d.ts"),
    }
}

fn source_discovery_symbol_fingerprint(
    alias_target: &str,
    declarations: &[Declaration],
) -> SourceDiscoverySymbolFingerprint {
    fn field(hasher: &mut Sha256, value: &str) {
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(value.as_bytes());
    }

    let mut hasher = Sha256::new();
    field(&mut hasher, alias_target);
    hasher.update(
        u64::try_from(declarations.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for declaration in declarations {
        field(&mut hasher, declaration.name.as_ref());
        field(&mut hasher, declaration.kind.as_ref());
        hasher.update([u8::from(!declaration.location.path.ends_with(".d.ts"))]);
    }
    hasher.finalize().into()
}

struct SourceDiscoveryTypeScriptDelta {
    entity_paths: HashSet<String>,
    file_paths: HashSet<String>,
    semantic_symbol_ids: HashSet<SymbolId>,
}

struct CachedTypeScriptIndexes {
    interner: SymbolInterner,
    symbol_alias_targets: HashMap<SymbolId, SymbolId>,
    aliases: HashMap<SymbolId, SymbolId>,
    symbols_by_root: HashMap<SymbolId, Vec<SymbolId>>,
    source_declarations: HashMap<SymbolId, Declaration>,
    entities: EntitySymbols,
    symbol_names: HashMap<SymbolId, SymbolName>,
    source_discovery_symbol_semantics: HashMap<SymbolId, SourceDiscoverySymbolFingerprint>,
    source_discovery_delta: Option<SourceDiscoveryTypeScriptDelta>,
}

fn build_typescript_indexes(
    table: &solid_facts::TypeScriptTable,
    parallel: bool,
) -> (CachedTypeScriptIndexes, Duration, Duration) {
    let interner = SymbolInterner::from_table(table);
    let aliases_started = Instant::now();
    let (aliases, source_declarations) = alias_roots_and_source_declarations(table, &interner);
    let aliases_elapsed = aliases_started.elapsed();
    let (
        (entities, entities_elapsed),
        symbol_names,
        symbol_alias_targets,
        symbols_by_root,
        source_discovery_symbol_semantics,
    ) = if parallel {
        std::thread::scope(|scope| {
            let entities = scope.spawn(|| {
                let started = Instant::now();
                let entities = entity_symbols(table, &aliases, &interner);
                (entities, started.elapsed())
            });
            let names = scope.spawn(|| symbol_names(table, &aliases, &interner));
            let targets = scope.spawn(|| symbol_alias_targets(table, &interner));
            let roots = scope.spawn(|| symbols_by_root(table, &aliases, &interner));
            let semantics = scope.spawn(|| source_discovery_symbol_semantics(table, &interner));
            (
                entities.join().expect("entity index worker panicked"),
                names.join().expect("symbol name index worker panicked"),
                targets.join().expect("symbol target index worker panicked"),
                roots.join().expect("symbol root index worker panicked"),
                semantics
                    .join()
                    .expect("source discovery semantics worker panicked"),
            )
        })
    } else {
        let entities_started = Instant::now();
        let entities = entity_symbols(table, &aliases, &interner);
        let entities_elapsed = entities_started.elapsed();
        (
            (entities, entities_elapsed),
            symbol_names(table, &aliases, &interner),
            symbol_alias_targets(table, &interner),
            symbols_by_root(table, &aliases, &interner),
            source_discovery_symbol_semantics(table, &interner),
        )
    };
    (
        CachedTypeScriptIndexes {
            interner,
            symbol_alias_targets,
            aliases,
            symbols_by_root,
            source_declarations,
            entities,
            symbol_names,
            source_discovery_symbol_semantics,
            source_discovery_delta: None,
        },
        aliases_elapsed,
        entities_elapsed,
    )
}

struct CachedReachability {
    inputs: HashMap<String, (SourceHash, Arc<solid_ast_facts::AstFacts>)>,
    files: HashMap<SourcePath, CachedReachabilityFile>,
    calls: HashMap<Location, usize>,
    multiplicity_by_path: HashMap<String, Vec<usize>>,
    function_symbols: HashSet<SymbolId>,
}

#[derive(Clone)]
enum ReachabilityTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
}

#[derive(Clone)]
struct ReachabilityEdge {
    owner: Option<Span>,
    target: ReachabilityTarget,
}

struct CachedReachabilityFile {
    identity: SourceDiscoveryIdentity,
    compiler: Arc<solid_facts::solid_compiler_facts::ExecutionMap>,
    functions: Vec<FunctionNode>,
    roots: Vec<ReachabilityTarget>,
    edges: Vec<ReachabilityEdge>,
    callback_edges: Vec<(Option<Span>, Vec<ReachabilityTarget>)>,
    call_owners: Vec<Option<Span>>,
    call_owner_indices: Vec<Option<usize>>,
    topology: ReachabilityTopology,
}

type LateStageSourceFingerprint = [u8; 32];

#[derive(Clone)]
struct LateStageFileInput {
    source_hash: SourceHash,
    ast: Arc<solid_ast_facts::AstFacts>,
    compiler: Arc<solid_facts::solid_compiler_facts::ExecutionMap>,
    source_fingerprint: LateStageSourceFingerprint,
}

#[derive(Clone, Default)]
struct LocalAccessResult {
    reads: Vec<Arc<ReactiveRead>>,
    writes: Vec<Arc<ReactiveWrite>>,
    action_invocations: Vec<Arc<ActionInvocation>>,
    async_reads: Vec<Arc<AsyncRead>>,
    strict_read_obligations: usize,
    write_action_obligations: HashSet<(&'static str, String, u64, u64)>,
}

struct LocalAccessBuild {
    result: LocalAccessResult,
    reused: bool,
    reused_files: u64,
    recomputed_files: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalAccessSymbolState {
    accessor: Option<(SymbolId, Location)>,
    accessor_origin: Option<(SymbolId, SymbolId, Location)>,
    setter: Option<(SymbolId, Location, bool)>,
    action: Option<(SymbolId, Location)>,
    source_primitive: Option<SymbolId>,
    async_source: bool,
    contract_reads: Option<Vec<(String, String, Location, String)>>,
    source_kind: Option<ReactiveSourceKind>,
    prop_source: Option<(SymbolId, Location)>,
    source_declaration: Option<Declaration>,
    symbol_name: Option<SymbolId>,
}

struct CachedLocalAccessFile {
    source_hash: SourceHash,
    compiler: Arc<solid_facts::solid_compiler_facts::ExecutionMap>,
    dependencies: HashSet<SymbolId>,
    call_multiplicities: Vec<(Location, Option<usize>)>,
    contribution: LocalAccessResult,
}

#[derive(Default)]
struct CachedLocalAccesses {
    aggregate: Option<LocalAccessResult>,
    files: HashMap<SourcePath, CachedLocalAccessFile>,
    dependency_states: HashMap<SymbolId, LocalAccessSymbolState>,
    prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
}

struct CachedLateStages {
    inputs: HashMap<SourcePath, LateStageFileInput>,
    local_accesses: CachedLocalAccesses,
    interprocedural: Option<InterproceduralResult>,
    missing_owners: Option<Vec<OwnerRequirement>>,
    owner_files: HashMap<SourcePath, CachedOwnerFile>,
}

fn same_reachability_ast(
    previous: &solid_ast_facts::AstFacts,
    current: &solid_ast_facts::AstFacts,
) -> bool {
    previous.schema == current.schema
        && previous.source.path == current.source.path
        && previous.calls == current.calls
        && previous.bindings == current.bindings
        && previous.functions == current.functions
        && previous.imports == current.imports
        && previous.exports == current.exports
        && previous.identifiers == current.identifiers
        && previous.awaits == current.awaits
        && previous.returns == current.returns
        && previous.jsx_elements == current.jsx_elements
        && previous.members == current.members
        && previous.spreads == current.spreads
        && previous.conditional_tests == current.conditional_tests
}

fn same_compiler_semantics(
    previous: &solid_facts::solid_compiler_facts::ExecutionMap,
    current: &solid_facts::solid_compiler_facts::ExecutionMap,
) -> bool {
    previous.compiler_facts_protocol == current.compiler_facts_protocol
        && previous.tracked_regions == current.tracked_regions
        && previous.untracked_regions == current.untracked_regions
        && previous.ownership_regions == current.ownership_regions
        && previous.callback_roles == current.callback_roles
        && previous.jsx_operations == current.jsx_operations
}

fn late_stage_source_fingerprint(file: &solid_facts::FileFacts) -> LateStageSourceFingerprint {
    fn optional_bool(hasher: &mut Sha256, value: Option<bool>) {
        hasher.update([match value {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        }]);
    }

    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(file.ast.calls.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for call in &file.ast.calls {
        let callee = usize::try_from(call.callee.start)
            .ok()
            .zip(usize::try_from(call.callee.end).ok())
            .and_then(|(start, end)| file.source.get(start..end));
        optional_bool(&mut hasher, callee.map(|_| false));
        if let Some(callee) = callee {
            hasher.update(
                u64::try_from(callee.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            hasher.update(callee.as_bytes());
        }
    }
    hasher.update(
        u64::try_from(file.ast.bindings.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for binding in &file.ast.bindings {
        let shape = binding.call_initializer.and_then(|initializer| {
            file.ast
                .call_at(initializer)
                .map(|call| go_binding_pattern_accepts_call(file.source.as_ref(), binding, call))
        });
        optional_bool(&mut hasher, shape);
    }
    let returned_functions = file.ast.returns.iter().filter_map(|returned| {
        (returned.value == solid_ast_facts::ReturnValueKind::Function)
            .then_some(returned.argument)
            .flatten()
    });
    for argument in returned_functions {
        hasher.update([u8::from(go_returned_arrow_pattern_accepts(
            file.source.as_ref(),
            argument,
        ))]);
    }
    hasher.finalize().into()
}

fn late_stage_inputs_match(cache: &CachedLateStages, facts: &ProjectFacts) -> bool {
    cache.inputs.len() == facts.files.len()
        && facts.files.iter().all(|file| {
            cache
                .inputs
                .get(file.path.as_str())
                .is_some_and(|previous| {
                    previous.source_hash == file.source_hash
                        || same_reachability_ast(&previous.ast, &file.ast)
                            && (Arc::ptr_eq(&previous.compiler, &file.compiler)
                                || same_compiler_semantics(&previous.compiler, &file.compiler))
                            && previous.source_fingerprint == late_stage_source_fingerprint(file)
                })
        })
}

fn current_late_stage_inputs(facts: &ProjectFacts) -> HashMap<SourcePath, LateStageFileInput> {
    facts
        .files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                LateStageFileInput {
                    source_hash: file.source_hash.clone(),
                    ast: file.ast.clone(),
                    compiler: file.compiler.clone(),
                    source_fingerprint: late_stage_source_fingerprint(file),
                },
            )
        })
        .collect()
}

fn refresh_late_stage_inputs(
    inputs: &mut HashMap<SourcePath, LateStageFileInput>,
    facts: &ProjectFacts,
) {
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    inputs.retain(|path, _| current_paths.contains(path.as_str()));
    for file in &facts.files {
        let unchanged = inputs
            .get(file.path.as_str())
            .is_some_and(|input| input.source_hash == file.source_hash);
        if unchanged {
            continue;
        }
        inputs.insert(
            file.path.clone(),
            LateStageFileInput {
                source_hash: file.source_hash.clone(),
                ast: file.ast.clone(),
                compiler: file.compiler.clone(),
                source_fingerprint: late_stage_source_fingerprint(file),
            },
        );
    }
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
    ast_indexes: HashMap<SourcePath, CachedAstFileIndex>,
    source_discovery: HashMap<SourcePath, CachedSourceDiscovery>,
    typed_accessors: HashMap<SourcePath, CachedTypedAccessors>,
    interprocedural_graph: HashMap<SourcePath, CachedInterproceduralGraph>,
    interprocedural_results: CachedInterproceduralResults,
    typescript_indexes: Option<CachedTypeScriptIndexes>,
    reachability: Option<CachedReachability>,
    late_stages: Option<CachedLateStages>,
    source_discovery_domain: Option<(String, Vec<String>)>,
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

#[derive(Default)]
struct BuildCaches<'a> {
    ast_indexes: Option<&'a mut HashMap<SourcePath, CachedAstFileIndex>>,
    source_discovery: Option<&'a mut HashMap<SourcePath, CachedSourceDiscovery>>,
    typed_accessors: Option<&'a mut HashMap<SourcePath, CachedTypedAccessors>>,
    interprocedural_graph: Option<&'a mut HashMap<SourcePath, CachedInterproceduralGraph>>,
    interprocedural_results: Option<&'a mut CachedInterproceduralResults>,
    typescript_indexes: Option<&'a mut Option<CachedTypeScriptIndexes>>,
    reachability: Option<&'a mut Option<CachedReachability>>,
    late_stages: Option<&'a mut Option<CachedLateStages>>,
}

impl IncrementalBuilder {
    pub fn build(&mut self, facts: &ProjectFacts) -> Result<(Program, BuildTimings), BuildError> {
        self.build_with_contracts(facts, &[])
    }

    /// Build a program behind shared ownership. This is the preferred service
    /// interface: retained generations are returned with an atomic reference
    /// increment instead of cloning every program table.
    pub fn build_shared(
        &mut self,
        facts: &ProjectFacts,
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        self.build_with_contracts_shared(facts, &[])
    }

    pub fn build_with_contracts(
        &mut self,
        facts: &ProjectFacts,
        contracts: &[PackageContract],
    ) -> Result<(Program, BuildTimings), BuildError> {
        self.build_with_contracts_shared(facts, contracts)
            .map(|(program, timings)| ((*program).clone(), timings))
    }

    /// Build a contract-aware program behind shared ownership.
    pub fn build_with_contracts_shared(
        &mut self,
        facts: &ProjectFacts,
        contracts: &[PackageContract],
    ) -> Result<(Arc<Program>, BuildTimings), BuildError> {
        let total_started = Instant::now();
        let lookup_started = Instant::now();
        let identity = BuildIdentity {
            project_id: facts.project_id.clone(),
            generation: facts.generation.get(),
            contracts: contracts
                .iter()
                .map(|contract| format!("{contract:?}"))
                .collect(),
        };
        let source_discovery_domain = (identity.project_id.clone(), identity.contracts.clone());
        if self.source_discovery_domain.as_ref() != Some(&source_discovery_domain) {
            self.ast_indexes.clear();
            self.source_discovery.clear();
            self.typed_accessors.clear();
            self.interprocedural_graph.clear();
            self.interprocedural_results = CachedInterproceduralResults::default();
            self.typescript_indexes = None;
            self.reachability = None;
            self.late_stages = None;
            self.source_discovery_domain = Some(source_discovery_domain);
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
            contracts,
            BuildCaches {
                ast_indexes: Some(&mut self.ast_indexes),
                source_discovery: Some(&mut self.source_discovery),
                typed_accessors: Some(&mut self.typed_accessors),
                interprocedural_graph: Some(&mut self.interprocedural_graph),
                interprocedural_results: Some(&mut self.interprocedural_results),
                typescript_indexes: Some(&mut self.typescript_indexes),
                reachability: Some(&mut self.reachability),
                late_stages: Some(&mut self.late_stages),
            },
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
        self.ast_indexes.clear();
        self.source_discovery.clear();
        self.typed_accessors.clear();
        self.interprocedural_graph.clear();
        self.interprocedural_results = CachedInterproceduralResults::default();
        self.typescript_indexes = None;
        self.reachability = None;
        self.late_stages = None;
        self.source_discovery_domain = None;
    }

    /// Applies the idle-memory policy without invalidating the current result.
    ///
    /// `Balanced` targets the three cache families that profiling found to
    /// account for most retained bytes per millisecond of recomputation.
    /// `Compact` releases every derived index while preserving the current
    /// generation and its source-discovery domain identity.
    pub fn retain_for_idle(&mut self, retention: CacheRetention) {
        if retention == CacheRetention::Performance {
            return;
        }

        self.interprocedural_graph.clear();
        self.interprocedural_results = CachedInterproceduralResults::default();
        self.typescript_indexes = None;
        self.reachability = None;

        if retention == CacheRetention::Compact {
            self.ast_indexes.clear();
            self.source_discovery.clear();
            self.typed_accessors.clear();
            self.late_stages = None;
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObligationCounts {
    pub strict_reads: usize,
    pub writes_and_actions: usize,
    pub factory_instances: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReactiveSourceKind {
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

fn source_discovery_identity(
    file: &FileFacts,
    indexes: &ProjectIndexes<'_>,
) -> SourceDiscoveryIdentity {
    let mut symbol_ids = HashSet::<SymbolId>::new();
    for entity in indexes.entities_for_path(file.path.as_str()) {
        if !entity.symbol.is_empty() {
            symbol_ids.insert(symbol_id(entity.symbol.as_ref()));
        }
        if let Some(call) = &entity.resolved_call
            && call.validity == ResolvedCallValidity::Valid
            && !call.target.is_empty()
        {
            symbol_ids.insert(symbol_id(call.target.as_ref()));
        }
    }
    let mut pending = symbol_ids.iter().cloned().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        let Some(symbol) = indexes.symbols_by_id.get(id.as_str()) else {
            continue;
        };
        if !symbol.alias_target().is_empty() && symbol_ids.insert(symbol_id(symbol.alias_target()))
        {
            pending.push(symbol_id(symbol.alias_target()));
        }
    }
    let mut symbols = symbol_ids.into_iter().collect::<Vec<_>>();
    symbols.sort_unstable();
    SourceDiscoveryIdentity {
        source_hash: file.source_hash.clone(),
        symbols,
    }
}

fn source_discovery_identity_matches(
    cached: &SourceDiscoveryIdentity,
    path: &str,
    source_hash: &SourceHash,
    typescript_unchanged: bool,
    typescript_delta: Option<&SourceDiscoveryTypeScriptDelta>,
) -> bool {
    if &cached.source_hash != source_hash {
        return false;
    }
    if typescript_unchanged {
        return true;
    }
    if let Some(delta) = typescript_delta {
        if delta.entity_paths.contains(path) || delta.file_paths.contains(path) {
            return false;
        }
        if delta.semantic_symbol_ids.is_empty() {
            return true;
        }
        return cached
            .symbols
            .iter()
            .all(|symbol| !delta.semantic_symbol_ids.contains(symbol.as_str()));
    }
    false
}

fn discover_file_sources(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    ast_index: &CachedAstFileIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    resolved_contracts: &ResolvedContracts,
    bundled_returns: &HashMap<SymbolId, ContractReturn>,
) -> SourceDiscoveryContribution {
    let mut result = SourceDiscoveryContribution::default();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = ast_index.call_by_span(initializer) else {
            continue;
        };
        let contracted = entities
            .get(&location(file.path.shared(), call.callee))
            .and_then(|symbol| resolved_contracts.by_symbol.get(symbol));
        if let Some(contracted) = contracted
            && let Some(contracted_return) = contracted.summary.returns.as_ref()
        {
            if let Some(name) = binding.names.first() {
                let declaration = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&declaration) {
                    result.accessors.push((
                        symbol.clone(),
                        (
                            symbol_id(file.source_text(name.span).unwrap_or_default()),
                            contracted.contract_location.clone(),
                        ),
                    ));
                    result.contracted_accessor_symbols.push(symbol.clone());
                    result.accessor_origins.push((
                        symbol.clone(),
                        (
                            symbol_id(&contracted_return.label),
                            symbol_id(&contracted.local_name),
                            contracted.contract_location.clone(),
                        ),
                    ));
                    result.source_kinds.push((
                        symbol.clone(),
                        if contracted_return.kind == "store-path" {
                            ReactiveSourceKind::Store
                        } else {
                            ReactiveSourceKind::Accessor
                        },
                    ));
                }
            }
            continue;
        }
        let primitive = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
        );
        if primitive.as_deref() == Some("action") {
            if let Some(name) = binding.names.first() {
                let location = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&location) {
                    result.actions.push((
                        symbol.clone(),
                        (
                            symbol_id(file.source_text(name.span).unwrap_or_default()),
                            location,
                        ),
                    ));
                }
            }
            continue;
        }
        if primitive.as_deref() == Some("dynamic") {
            if let Some(name) = binding.names.first() {
                let declaration = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&declaration) {
                    result
                        .source_primitives
                        .push((symbol.clone(), "dynamic".into()));
                    if call
                        .arguments
                        .first()
                        .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
                    {
                        result.async_sources.push(symbol.clone());
                    }
                }
            }
            continue;
        }
        if !matches!(
            primitive.as_deref(),
            Some(
                "createSignal"
                    | "createMemo"
                    | "createStore"
                    | "createProjection"
                    | "createOptimistic"
                    | "createOptimisticStore"
            )
        ) && !primitive
            .as_deref()
            .is_some_and(|primitive| bundled_returns.contains_key(primitive))
        {
            continue;
        }
        let source_name = if binding.shape == solid_ast_facts::BindingShape::Array {
            binding.array_slots.first().and_then(Option::as_ref)
        } else {
            binding.names.first()
        };
        if let Some(name) = source_name {
            let declaration = location(file.path.shared(), name.span);
            if let Some(symbol) = entities.get(&declaration) {
                result.accessors.push((
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration,
                    ),
                ));
                let go_returned_source = binding.shape == solid_ast_facts::BindingShape::Array
                    && matches!(primitive.as_deref(), Some("createSignal" | "createStore"))
                    && go_binding_pattern_accepts_call(file.source.as_ref(), binding, call);
                result.source_phases.push((
                    symbol.clone(),
                    if go_returned_source && primitive.as_deref() == Some("createStore") {
                        2
                    } else if go_returned_source {
                        0
                    } else {
                        1
                    },
                ));
                if go_returned_source {
                    result.returned_source_symbols.push(symbol.clone());
                    result.summary_source_symbols.push(symbol.clone());
                }
                if binding.shape != solid_ast_facts::BindingShape::Array
                    && primitive
                        .as_deref()
                        .is_some_and(|primitive| bundled_returns.contains_key(primitive))
                {
                    result.summary_source_symbols.push(symbol.clone());
                }
                if let Some(primitive) = primitive.as_deref()
                    && let Some(returned) = bundled_returns.get(primitive)
                {
                    result.accessor_origins.push((
                        symbol.clone(),
                        (
                            symbol_id(&returned.label),
                            primitive.into(),
                            Location {
                                path: format!("bundled://solid-js.json#{primitive}").into(),
                                start_byte: 0,
                                end_byte: 0,
                            },
                        ),
                    ));
                }
                result.source_kinds.push((
                    symbol.clone(),
                    if primitive
                        .as_deref()
                        .and_then(|primitive| bundled_returns.get(primitive))
                        .is_some_and(|returned| returned.kind == "store-path")
                        || matches!(
                            primitive.as_deref(),
                            Some("createStore" | "createOptimisticStore" | "createProjection")
                        )
                    {
                        ReactiveSourceKind::Store
                    } else {
                        ReactiveSourceKind::Accessor
                    },
                ));
                if let Some(primitive) = primitive.as_deref() {
                    result
                        .source_primitives
                        .push((symbol.clone(), primitive.into()));
                }
                result
                    .source_owned_write
                    .push((symbol.clone(), call.owned_write_option));
                if call
                    .arguments
                    .first()
                    .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
                {
                    result.async_sources.push(symbol.clone());
                }
            }
        }
        if primitive.as_deref() != Some("createMemo")
            && let Some(name) = if binding.shape == solid_ast_facts::BindingShape::Array {
                binding.array_slots.get(1).and_then(Option::as_ref)
            } else {
                binding.names.get(1)
            }
        {
            let declaration = location(file.path.shared(), name.span);
            if let Some(symbol) = entities.get(&declaration) {
                result.setters.push((
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration,
                        call.owned_write_option,
                    ),
                ));
            }
        }
    }
    result
}

struct SourceDiscoveryMergeTarget<'a> {
    accessors: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    accessor_origins: &'a mut HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    setters: &'a mut HashMap<SymbolId, (SymbolId, Location, bool)>,
    actions: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: &'a mut HashMap<SymbolId, ReactiveSourceKind>,
    source_primitives: &'a mut HashMap<SymbolId, SymbolId>,
    source_phases: &'a mut HashMap<SymbolId, u8>,
    returned_source_symbols: &'a mut HashSet<SymbolId>,
    summary_source_symbols: &'a mut HashSet<SymbolId>,
    source_owned_write: &'a mut HashMap<SymbolId, bool>,
    async_sources: &'a mut HashSet<SymbolId>,
    contracted_accessor_symbols: &'a mut HashSet<SymbolId>,
}

#[derive(Default)]
struct SourceDiscoveryAggregate {
    accessors: HashMap<SymbolId, (SymbolId, Location)>,
    accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    setters: HashMap<SymbolId, (SymbolId, Location, bool)>,
    actions: HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    source_primitives: HashMap<SymbolId, SymbolId>,
    source_phases: HashMap<SymbolId, u8>,
    returned_source_symbols: HashSet<SymbolId>,
    summary_source_symbols: HashSet<SymbolId>,
    source_owned_write: HashMap<SymbolId, bool>,
    async_sources: HashSet<SymbolId>,
    contracted_accessor_symbols: HashSet<SymbolId>,
}

impl SourceDiscoveryAggregate {
    fn merge(&mut self, contribution: &SourceDiscoveryContribution) {
        merge_source_discovery(
            contribution,
            SourceDiscoveryMergeTarget {
                accessors: &mut self.accessors,
                accessor_origins: &mut self.accessor_origins,
                setters: &mut self.setters,
                actions: &mut self.actions,
                source_kinds: &mut self.source_kinds,
                source_primitives: &mut self.source_primitives,
                source_phases: &mut self.source_phases,
                returned_source_symbols: &mut self.returned_source_symbols,
                summary_source_symbols: &mut self.summary_source_symbols,
                source_owned_write: &mut self.source_owned_write,
                async_sources: &mut self.async_sources,
                contracted_accessor_symbols: &mut self.contracted_accessor_symbols,
            },
        );
    }

    fn append_to(self, target: SourceDiscoveryMergeTarget<'_>) {
        target.accessors.extend(self.accessors);
        target.accessor_origins.extend(self.accessor_origins);
        target.setters.extend(self.setters);
        target.actions.extend(self.actions);
        target.source_kinds.extend(self.source_kinds);
        target.source_primitives.extend(self.source_primitives);
        target.source_phases.extend(self.source_phases);
        target
            .returned_source_symbols
            .extend(self.returned_source_symbols);
        target
            .summary_source_symbols
            .extend(self.summary_source_symbols);
        target.source_owned_write.extend(self.source_owned_write);
        target.async_sources.extend(self.async_sources);
        target
            .contracted_accessor_symbols
            .extend(self.contracted_accessor_symbols);
    }
}

fn merge_source_discovery(
    contribution: &SourceDiscoveryContribution,
    target: SourceDiscoveryMergeTarget<'_>,
) {
    target
        .accessors
        .extend(contribution.accessors.iter().cloned());
    target
        .accessor_origins
        .extend(contribution.accessor_origins.iter().cloned());
    target.setters.extend(contribution.setters.iter().cloned());
    target.actions.extend(contribution.actions.iter().cloned());
    target
        .source_kinds
        .extend(contribution.source_kinds.iter().cloned());
    target
        .source_primitives
        .extend(contribution.source_primitives.iter().cloned());
    target
        .source_phases
        .extend(contribution.source_phases.iter().cloned());
    target
        .returned_source_symbols
        .extend(contribution.returned_source_symbols.iter().cloned());
    target
        .summary_source_symbols
        .extend(contribution.summary_source_symbols.iter().cloned());
    target
        .source_owned_write
        .extend(contribution.source_owned_write.iter().cloned());
    target
        .async_sources
        .extend(contribution.async_sources.iter().cloned());
    target
        .contracted_accessor_symbols
        .extend(contribution.contracted_accessor_symbols.iter().cloned());
}

fn extend_source_discovery_symbols(
    symbols: &mut HashSet<SymbolId>,
    contribution: &SourceDiscoveryContribution,
) {
    symbols.extend(
        contribution
            .accessors
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .accessor_origins
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .setters
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .actions
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .source_kinds
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .source_primitives
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(contribution.async_sources.iter().cloned());
}

pub fn build(facts: &ProjectFacts) -> Result<Program, BuildError> {
    build_with_contracts(facts, &[])
}

pub fn build_measured(facts: &ProjectFacts) -> Result<(Program, BuildTimings), BuildError> {
    build_with_contracts_measured(facts, &[])
}

pub fn build_with_contracts(
    facts: &ProjectFacts,
    contracts: &[PackageContract],
) -> Result<Program, BuildError> {
    build_with_contracts_measured(facts, contracts).map(|(program, _)| program)
}

pub fn build_with_contracts_measured(
    facts: &ProjectFacts,
    contracts: &[PackageContract],
) -> Result<(Program, BuildTimings), BuildError> {
    build_with_contracts_measured_incremental(facts, contracts, BuildCaches::default())
}

/// Owned reactive-source facts produced by the source-discovery stage and
/// consumed by the later interprocedural, static, and owner stages.
struct SourceDiscovery {
    accessors: HashMap<SymbolId, (SymbolId, Location)>,
    accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    setters: HashMap<SymbolId, (SymbolId, Location, bool)>,
    actions: HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    source_primitives: HashMap<SymbolId, SymbolId>,
    source_phases: HashMap<SymbolId, u8>,
    returned_source_symbols: HashSet<SymbolId>,
    summary_source_symbols: HashSet<SymbolId>,
    source_owned_write: HashMap<SymbolId, bool>,
    async_sources: HashSet<SymbolId>,
    contract_reads: HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    contract_callbacks: HashMap<SymbolId, Vec<ContractCallback>>,
    contract_returns: HashMap<SymbolId, (ContractReturn, Location)>,
    contracted_accessor_symbols: HashSet<SymbolId>,
    prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
    bundled_returns: HashMap<SymbolId, ContractReturn>,
    retained_source_paths: HashSet<String>,
    changed_source_symbols: HashSet<SymbolId>,
}

/// The stable, read-mostly environment threaded through every pipeline stage:
/// project facts, prebuilt indexes, resolved contracts, and the semantic lookup.
#[derive(Clone, Copy)]
struct StageContext<'a> {
    facts: &'a ProjectFacts,
    project_indexes: &'a ProjectIndexes<'a>,
    typescript_indexes: &'a CachedTypeScriptIndexes,
    entities: &'a EntitySymbols,
    source_declarations: &'a HashMap<SymbolId, Declaration>,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    semantic_lookup: &'a SemanticLookup<'a>,
    resolved_contracts: &'a ResolvedContracts,
    contracts: &'a [PackageContract],
}

// The final `finish_stage!` resets the stage timer for symmetry; that last write
// is intentionally unused because the stage ends here.
#[allow(unused_assignments)]
fn discover_sources(
    ctx: &StageContext<'_>,
    source_discovery_cache: Option<&mut HashMap<SourcePath, CachedSourceDiscovery>>,
    typescript_unchanged: bool,
    build_timings: &mut BuildTimings,
    emit_timings: bool,
) -> SourceDiscovery {
    let StageContext {
        facts,
        project_indexes,
        typescript_indexes,
        entities,
        source_declarations,
        symbol_names,
        semantic_lookup,
        resolved_contracts,
        contracts,
    } = *ctx;
    let mut stage_started = Instant::now();
    macro_rules! finish_stage {
        ($field:ident, $name:literal) => {{
            let elapsed = stage_started.elapsed();
            build_timings.$field = elapsed;
            if emit_timings {
                eprintln!(
                    "{{\"reactiveIrStage\":\"{}\",\"elapsedNs\":{}}}",
                    $name,
                    elapsed.as_nanos()
                );
            }
            stage_started = Instant::now();
        }};
    }
    let mut accessors = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let bundled_returns = contracts
        .iter()
        .find(|contract| contract.package.name == "solid-js")
        .map(|contract| {
            contract
                .root_exports()
                .iter()
                .filter_map(|(name, summary)| {
                    summary
                        .returns
                        .clone()
                        .map(|returned| (symbol_id(name), returned))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let mut accessor_origins = HashMap::<SymbolId, (SymbolId, SymbolId, Location)>::new();
    let mut setters = HashMap::<SymbolId, (SymbolId, Location, bool)>::new();
    let mut actions = HashMap::<SymbolId, (SymbolId, Location)>::new();
    let mut source_kinds = HashMap::<SymbolId, ReactiveSourceKind>::new();
    let mut source_primitives = HashMap::<SymbolId, SymbolId>::new();
    let mut source_phases = HashMap::<SymbolId, u8>::new();
    let mut returned_source_symbols = HashSet::<SymbolId>::new();
    let mut summary_source_symbols = HashSet::<SymbolId>::new();
    let mut source_owned_write = HashMap::<SymbolId, bool>::new();
    let mut async_sources = HashSet::<SymbolId>::new();
    let mut contract_reads = HashMap::<SymbolId, Vec<(String, String, Location, String)>>::new();
    let mut contract_callbacks = HashMap::<SymbolId, Vec<ContractCallback>>::new();
    let mut contract_returns = HashMap::<SymbolId, (ContractReturn, Location)>::new();
    let mut contracted_accessor_symbols = HashSet::<SymbolId>::new();

    for contracted in &resolved_contracts.bindings {
        if !contracted.summary.reactive_reads.is_empty() {
            contract_reads.insert(
                contracted.symbol.clone(),
                contracted
                    .summary
                    .reactive_reads
                    .iter()
                    .map(|read| {
                        (
                            format!("{}.{}", contracted.package_name, contracted.imported_name),
                            contracted.local_name.clone(),
                            contracted.contract_location.clone(),
                            read.kind.clone(),
                        )
                    })
                    .collect(),
            );
        }
        contract_callbacks.insert(
            contracted.symbol.clone(),
            contracted.summary.callbacks.clone(),
        );
        if let Some(returned) = &contracted.summary.returns {
            contract_returns.insert(
                contracted.symbol.clone(),
                (returned.clone(), contracted.contract_location.clone()),
            );
            source_kinds.insert(
                contracted.symbol.clone(),
                if returned.kind == "store-path" {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                },
            );
        }
    }

    let mut retained_source_paths = HashSet::<String>::new();
    let mut changed_source_symbols = HashSet::<SymbolId>::new();
    match source_discovery_cache {
        None => {
            for file in &facts.files {
                for binding in &file.ast.bindings {
                    let Some(initializer) = binding.call_initializer else {
                        continue;
                    };
                    let Some(call) = project_indexes
                        .ast_files_by_path
                        .get(file.path.as_str())
                        .and_then(|index| index.call_by_span(initializer))
                    else {
                        continue;
                    };
                    let contracted = entities
                        .get(&location(file.path.shared(), call.callee))
                        .and_then(|symbol| resolved_contracts.by_symbol.get(symbol));
                    if let Some(contracted) = contracted
                        && let Some(contracted_return) = contracted.summary.returns.as_ref()
                    {
                        let source_name = binding.names.first();
                        if let Some(name) = source_name {
                            let declaration = location(file.path.shared(), name.span);
                            if let Some(symbol) = entities.get(&declaration) {
                                accessors.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                                        contracted.contract_location.clone(),
                                    ),
                                );
                                contracted_accessor_symbols.insert(symbol.clone());
                                accessor_origins.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(&contracted_return.label),
                                        symbol_id(&contracted.local_name),
                                        contracted.contract_location.clone(),
                                    ),
                                );
                                source_kinds.insert(
                                    symbol.clone(),
                                    if contracted_return.kind == "store-path" {
                                        ReactiveSourceKind::Store
                                    } else {
                                        ReactiveSourceKind::Accessor
                                    },
                                );
                            }
                        }
                        continue;
                    }
                    let primitive = primitive_name(
                        file.path.as_str(),
                        call.callee,
                        call.static_callee(&file.source),
                        entities,
                        symbol_names,
                    );
                    if primitive.as_deref() == Some("action") {
                        if let Some(name) = binding.names.first() {
                            let location = location(file.path.shared(), name.span);
                            if let Some(symbol) = entities.get(&location) {
                                actions.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                                        location,
                                    ),
                                );
                            }
                        }
                        continue;
                    }
                    if primitive.as_deref() == Some("dynamic") {
                        if let Some(name) = binding.names.first() {
                            let declaration = location(file.path.shared(), name.span);
                            if let Some(symbol) = entities.get(&declaration) {
                                source_primitives.insert(symbol.clone(), "dynamic".into());
                                if call.arguments.first().is_some_and(|argument| {
                                    computation_is_async(semantic_lookup, file, argument.span)
                                }) {
                                    async_sources.insert(symbol.clone());
                                }
                            }
                        }
                        continue;
                    }
                    if !matches!(
                        primitive.as_deref(),
                        Some(
                            "createSignal"
                                | "createMemo"
                                | "createStore"
                                | "createProjection"
                                | "createOptimistic"
                                | "createOptimisticStore"
                        )
                    ) && !primitive
                        .as_deref()
                        .is_some_and(|primitive| bundled_returns.contains_key(primitive))
                    {
                        continue;
                    }
                    let source_name = if binding.shape == solid_ast_facts::BindingShape::Array {
                        binding.array_slots.first().and_then(Option::as_ref)
                    } else {
                        binding.names.first()
                    };
                    if let Some(name) = source_name {
                        let declaration = location(file.path.shared(), name.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            accessors.insert(
                                symbol.clone(),
                                (
                                    symbol_id(file.source_text(name.span).unwrap_or_default()),
                                    declaration,
                                ),
                            );
                            let go_returned_source = binding.shape
                                == solid_ast_facts::BindingShape::Array
                                && matches!(
                                    primitive.as_deref(),
                                    Some("createSignal" | "createStore")
                                )
                                && go_binding_pattern_accepts_call(
                                    file.source.as_ref(),
                                    binding,
                                    call,
                                );
                            source_phases.insert(
                                symbol.clone(),
                                if go_returned_source && primitive.as_deref() == Some("createStore")
                                {
                                    2
                                } else if go_returned_source {
                                    0
                                } else {
                                    1
                                },
                            );
                            if go_returned_source {
                                returned_source_symbols.insert(symbol.clone());
                                summary_source_symbols.insert(symbol.clone());
                            }
                            if binding.shape != solid_ast_facts::BindingShape::Array
                                && primitive.as_deref().is_some_and(|primitive| {
                                    bundled_returns.contains_key(primitive)
                                })
                            {
                                summary_source_symbols.insert(symbol.clone());
                            }
                            if let Some(primitive) = primitive.as_deref()
                                && let Some(returned) = bundled_returns.get(primitive)
                            {
                                accessor_origins.insert(
                                    symbol.clone(),
                                    (
                                        symbol_id(&returned.label),
                                        primitive.into(),
                                        Location {
                                            path: format!("bundled://solid-js.json#{primitive}")
                                                .into(),
                                            start_byte: 0,
                                            end_byte: 0,
                                        },
                                    ),
                                );
                            }
                            source_kinds.insert(
                                symbol.clone(),
                                if primitive
                                    .as_deref()
                                    .and_then(|primitive| bundled_returns.get(primitive))
                                    .is_some_and(|returned| returned.kind == "store-path")
                                    || matches!(
                                        primitive.as_deref(),
                                        Some(
                                            "createStore"
                                                | "createOptimisticStore"
                                                | "createProjection"
                                        )
                                    )
                                {
                                    ReactiveSourceKind::Store
                                } else {
                                    ReactiveSourceKind::Accessor
                                },
                            );
                            if let Some(primitive) = primitive.as_deref() {
                                source_primitives.insert(symbol.clone(), primitive.into());
                            }
                            source_owned_write.insert(symbol.clone(), call.owned_write_option);
                            if call.arguments.first().is_some_and(|argument| {
                                computation_is_async(semantic_lookup, file, argument.span)
                            }) {
                                async_sources.insert(symbol.clone());
                            }
                        }
                    }
                    if primitive.as_deref() != Some("createMemo")
                        && let Some(name) = if binding.shape == solid_ast_facts::BindingShape::Array
                        {
                            binding.array_slots.get(1).and_then(Option::as_ref)
                        } else {
                            binding.names.get(1)
                        }
                    {
                        let declaration = location(file.path.shared(), name.span);
                        if let Some(symbol) = entities.get(&declaration) {
                            setters.insert(
                                symbol.clone(),
                                (
                                    symbol_id(file.source_text(name.span).unwrap_or_default()),
                                    declaration,
                                    call.owned_write_option,
                                ),
                            );
                        }
                    }
                }
            }
        }
        Some(cache) => {
            let current_paths = facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache.retain(|path, _| current_paths.contains(path.as_str()));
            let reusable_paths = facts
                .files
                .iter()
                .filter_map(|file| {
                    cache
                        .get(file.path.as_str())
                        .is_some_and(|cached| {
                            source_discovery_identity_matches(
                                &cached.identity,
                                file.path.as_str(),
                                &file.source_hash,
                                typescript_unchanged,
                                typescript_indexes.source_discovery_delta.as_ref(),
                            )
                        })
                        .then_some(file.path.as_str())
                })
                .collect::<HashSet<_>>();
            let recomputed = facts
                .files
                .iter()
                .filter(|file| !reusable_paths.contains(file.path.as_str()))
                .collect::<Vec<_>>();
            let discovered = parallel_slice_results(&recomputed, |file| {
                (
                    source_discovery_identity(file, project_indexes),
                    discover_file_sources(
                        semantic_lookup,
                        file,
                        project_indexes
                            .ast_files_by_path
                            .get(file.path.as_str())
                            .expect("project index contains every source file"),
                        entities,
                        symbol_names,
                        resolved_contracts,
                        &bundled_returns,
                    ),
                )
            });
            let mut discovered = discovered.into_iter();
            for file in &facts.files {
                if reusable_paths.contains(file.path.as_str()) {
                    build_timings.source_discovery_reused_files += 1;
                    retained_source_paths.insert(file.path.to_string());
                    continue;
                }
                let (identity, contribution) = discovered
                    .next()
                    .expect("recomputed source path has a fresh contribution");
                build_timings.source_discovery_recomputed_files += 1;
                if let Some(cached) = cache.get(file.path.as_str()) {
                    extend_source_discovery_symbols(
                        &mut changed_source_symbols,
                        &cached.contribution,
                    );
                }
                extend_source_discovery_symbols(&mut changed_source_symbols, &contribution);
                cache.insert(
                    file.path.clone(),
                    CachedSourceDiscovery {
                        identity,
                        contribution,
                    },
                );
            }
            debug_assert!(discovered.next().is_none());
            let cache = &*cache;
            for aggregate in parallel_file_chunk_results(&facts.files, |files| {
                let mut aggregate = SourceDiscoveryAggregate::default();
                for file in files {
                    if let Some(cached) = cache.get(file.path.as_str()) {
                        aggregate.merge(&cached.contribution);
                    }
                }
                aggregate
            }) {
                aggregate.append_to(SourceDiscoveryMergeTarget {
                    accessors: &mut accessors,
                    accessor_origins: &mut accessor_origins,
                    setters: &mut setters,
                    actions: &mut actions,
                    source_kinds: &mut source_kinds,
                    source_primitives: &mut source_primitives,
                    source_phases: &mut source_phases,
                    returned_source_symbols: &mut returned_source_symbols,
                    summary_source_symbols: &mut summary_source_symbols,
                    source_owned_write: &mut source_owned_write,
                    async_sources: &mut async_sources,
                    contracted_accessor_symbols: &mut contracted_accessor_symbols,
                });
            }
        }
    }
    finish_stage!(source_discovery, "source-discovery");
    for entity in facts.typescript.entities() {
        let Some(descriptor) = &entity.type_descriptor else {
            continue;
        };
        if descriptor.origin_module != "solid-js".into() {
            continue;
        }
        let Some(symbol) = entities.get(&entity.location) else {
            continue;
        };
        if resolved_contracts.by_symbol.contains_key(symbol) {
            continue;
        }
        let declaration = source_declarations.get(symbol);
        let (name, local_location) = declaration.map_or_else(
            || ("accessor".into(), entity.location.clone()),
            |declaration| (declaration.name.clone(), declaration.location.clone()),
        );
        let declaration_location = descriptor
            .alias_declarations
            .iter()
            .find(|declaration| matches!(declaration.name.as_ref(), "Accessor" | "Setter"))
            .map_or(local_location, |declaration| declaration.location.clone());
        accessors
            .entry(symbol.clone())
            .or_insert((symbol_id(name.as_ref()), declaration_location));
        source_phases.entry(symbol.clone()).or_insert(1);
    }
    for file in &facts.files {
        for element in &file.ast.jsx_elements {
            let primitive = jsx_primitive_name(file, element, entities, symbol_names);
            let keyed = element
                .boolean_properties
                .iter()
                .find(|property| file.source_text(property.name) == Some("keyed"))
                .map(|property| property.value);
            let custom_key = keyed.is_none()
                && element
                    .properties
                    .iter()
                    .any(|property| file.source_text(*property) == Some("keyed"));
            let parameter_indices: &[usize] = match (primitive.as_deref(), keyed) {
                (Some("Show" | "Match"), Some(true)) => &[],
                (Some("Show" | "Match"), _) => &[0],
                (Some("For"), _) if custom_key => &[0, 1],
                (Some("For"), Some(false)) => &[0],
                (Some("For"), _) => &[1],
                _ => &[],
            };
            if parameter_indices.is_empty() {
                continue;
            }
            for function in file.ast.functions.iter().filter(|function| {
                element.span.contains(function.span)
                    && !file.ast.functions.iter().any(|outer| {
                        outer.span != function.span
                            && element.span.contains(outer.span)
                            && outer.span.contains(function.span)
                    })
            }) {
                for index in parameter_indices {
                    let Some(parameter) = function
                        .parameters
                        .get(*index)
                        .and_then(|parameter| parameter.names.first())
                    else {
                        continue;
                    };
                    let declaration = location(file.path.shared(), parameter.span);
                    if let Some(symbol) = entities.get(&declaration) {
                        accessors.entry(symbol.clone()).or_insert((
                            symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                            declaration,
                        ));
                    }
                }
            }
        }
    }
    for file in &facts.files {
        for call in &file.ast.calls {
            if !primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
            )
            .is_some_and(|primitive| {
                matches!(primitive.as_str(), "createEffect" | "createRenderEffect")
            }) {
                continue;
            }
            let Some(compute) = call.arguments.first().and_then(|argument| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| argument.span.contains(function.span))
                    .max_by_key(|function| function.span.end - function.span.start)
            }) else {
                continue;
            };
            let returned = compute.expression_return.as_ref().or_else(|| {
                file.ast.returns.iter().find(|returned| {
                    compute.body.contains(returned.span)
                        && containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == compute.span)
                })
            });
            let Some(source_symbol) = returned
                .and_then(|returned| {
                    entities
                        .get(&location(file.path.shared(), returned.span))
                        .or_else(|| {
                            (returned.value == solid_ast_facts::ReturnValueKind::Identifier)
                                .then_some(returned.span)
                                .and_then(|span| file.source_text(span))
                                .and_then(|name| {
                                    source_declarations
                                        .iter()
                                        .find_map(|(symbol, declaration)| {
                                            (declaration.name == name.into()
                                                && declaration.location.path
                                                    == file.path.as_str().into())
                                            .then_some(symbol)
                                        })
                                })
                        })
                })
                .filter(|symbol| {
                    source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store)
                        || matches!(
                            source_primitives.get(*symbol).map(SymbolId::as_str),
                            Some("createStore" | "createOptimisticStore")
                        )
                })
            else {
                continue;
            };
            let Some(apply) = call.arguments.get(1).and_then(|argument| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| argument.span.contains(function.span))
                    .max_by_key(|function| function.span.end - function.span.start)
            }) else {
                continue;
            };
            let Some(parameter) = apply
                .parameters
                .first()
                .and_then(|parameter| parameter.names.first())
            else {
                continue;
            };
            let parameter_location = location(file.path.shared(), parameter.span);
            let Some(parameter_symbol) = entities.get(&parameter_location) else {
                continue;
            };
            let (display, declaration) =
                accessors.get(source_symbol).cloned().unwrap_or_else(|| {
                    (
                        symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                        location(file.path.shared(), parameter.span),
                    )
                });
            accessors.insert(parameter_symbol.clone(), (display, declaration));
            source_kinds.insert(parameter_symbol.clone(), ReactiveSourceKind::Store);
        }
    }
    loop {
        let mut setter_aliases = Vec::new();
        let mut action_aliases = Vec::new();
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let Some(source_symbol) =
                    binding
                        .initializer_identifier
                        .as_ref()
                        .and_then(|identifier| {
                            entities.get(&location(file.path.shared(), identifier.span))
                        })
                else {
                    continue;
                };
                let setter = setters.get(source_symbol).cloned();
                let action = actions.get(source_symbol).cloned();
                if setter.is_none() && action.is_none() {
                    continue;
                }
                for name in &binding.names {
                    let declaration = location(file.path.shared(), name.span);
                    let Some(symbol) = entities.get(&declaration) else {
                        continue;
                    };
                    if let Some((_, source, owned_write)) = &setter
                        && !setters.contains_key(symbol)
                    {
                        setter_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                                *owned_write,
                            ),
                        ));
                    }
                    if let Some((_, source)) = &action
                        && !actions.contains_key(symbol)
                    {
                        action_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                            ),
                        ));
                    }
                }
            }
        }
        if setter_aliases.is_empty() && action_aliases.is_empty() {
            break;
        }
        setters.extend(setter_aliases);
        actions.extend(action_aliases);
    }
    let mut prop_sources = HashMap::<SymbolId, (SymbolId, Location)>::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            if !function_binding_name(file, function)
                .and_then(|name| {
                    file.source_text(name.span)
                        .unwrap_or_default()
                        .chars()
                        .next()
                })
                .is_some_and(char::is_uppercase)
            {
                continue;
            }
            let Some(parameter) = function
                .parameters
                .first()
                .filter(|parameter| parameter.shape == solid_ast_facts::BindingShape::Identifier)
                .and_then(|parameter| parameter.names.first())
            else {
                continue;
            };
            let declaration = location(file.path.shared(), parameter.span);
            if let Some(symbol) = entities.get(&declaration) {
                prop_sources.insert(
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                        declaration,
                    ),
                );
            }
        }
    }
    finish_stage!(
        typed_accessors_and_prop_roots,
        "typed-accessors-and-prop-roots"
    );
    loop {
        let mut changed = false;
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let source = binding
                    .initializer_identifier
                    .as_ref()
                    .and_then(|identifier| {
                        entities.get(&location(file.path.shared(), identifier.span))
                    })
                    .and_then(|symbol| prop_sources.get(symbol))
                    .cloned()
                    .or_else(|| {
                        let initializer = binding.call_initializer?;
                        let call = file.ast.call_at(initializer)?;
                        let primitive = primitive_name(
                            file.path.as_str(),
                            call.callee,
                            call.static_callee(&file.source),
                            entities,
                            symbol_names,
                        );
                        if primitive.as_deref() != Some("merge") {
                            return None;
                        }
                        call.arguments.iter().find_map(|argument| {
                            entities
                                .get(&location(file.path.shared(), argument.span))
                                .and_then(|symbol| prop_sources.get(symbol))
                                .cloned()
                        })
                    });
                let Some((_, declaration)) = source else {
                    continue;
                };
                for name in &binding.names {
                    let binding_location = location(file.path.shared(), name.span);
                    if let Some(symbol) = entities.get(&binding_location)
                        && !prop_sources.contains_key(symbol)
                    {
                        prop_sources.insert(
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                declaration.clone(),
                            ),
                        );
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    finish_stage!(
        prop_propagation_and_control_flow,
        "prop-propagation-and-control-flow"
    );
    SourceDiscovery {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        returned_source_symbols,
        summary_source_symbols,
        source_owned_write,
        async_sources,
        contract_reads,
        contract_callbacks,
        contract_returns,
        contracted_accessor_symbols,
        prop_sources,
        bundled_returns,
        retained_source_paths,
        changed_source_symbols,
    }
}

fn build_with_contracts_measured_incremental(
    facts: &ProjectFacts,
    contracts: &[PackageContract],
    caches: BuildCaches<'_>,
) -> Result<(Program, BuildTimings), BuildError> {
    let BuildCaches {
        ast_indexes: ast_indexes_cache,
        source_discovery: source_discovery_cache,
        typed_accessors: typed_accessor_cache,
        interprocedural_graph: interprocedural_graph_cache,
        interprocedural_results: interprocedural_result_cache,
        typescript_indexes: typescript_indexes_cache,
        reachability: mut reachability_cache,
        late_stages: mut late_stage_cache,
    } = caches;
    let emit_timings = std::env::var_os("SOLID_CHECKER_TIMINGS").is_some();
    let total_started = Instant::now();
    let mut stage_started = Instant::now();
    let mut build_timings = BuildTimings::default();
    macro_rules! finish_stage {
        ($field:ident, $name:literal) => {{
            let elapsed = stage_started.elapsed();
            build_timings.$field = elapsed;
            if emit_timings {
                eprintln!(
                    "{{\"reactiveIrStage\":\"{}\",\"elapsedNs\":{}}}",
                    $name,
                    elapsed.as_nanos()
                );
            }
            stage_started = Instant::now();
        }};
    }
    let substage_started = Instant::now();
    let owned_ast_indexes;
    let ast_indexes = if let Some(cache) = ast_indexes_cache {
        let current_paths = facts
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
        cache.retain(|path, _| current_paths.contains(path.as_str()));
        for file in &facts.files {
            if cache
                .get(file.path.as_str())
                .is_some_and(|index| Arc::ptr_eq(&index.ast, &file.ast))
            {
                continue;
            }
            cache.insert(file.path.clone(), CachedAstFileIndex::new(file));
        }
        &*cache
    } else {
        owned_ast_indexes = facts
            .files
            .iter()
            .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
            .collect::<HashMap<_, _>>();
        &owned_ast_indexes
    };
    let project_indexes = ProjectIndexes::new(facts, ast_indexes);
    build_timings.project_indexes = substage_started.elapsed();
    let typescript_unchanged = facts
        .typescript_changes
        .as_ref()
        .is_some_and(|changes| changes.unchanged);
    let late_stages_reusable = typescript_unchanged
        && late_stage_cache
            .as_deref()
            .and_then(Option::as_ref)
            .is_some_and(|cache| late_stage_inputs_match(cache, facts));
    if let Some(cache) = late_stage_cache.as_deref_mut() {
        if late_stages_reusable {
            if let Some(retained) = cache.as_mut() {
                for file in &facts.files {
                    if let Some(input) = retained.inputs.get_mut(file.path.as_str()) {
                        input.source_hash.clone_from(&file.source_hash);
                    }
                }
            }
        } else if let Some(retained) = cache.as_mut() {
            refresh_late_stage_inputs(&mut retained.inputs, facts);
            retained.local_accesses.aggregate = None;
            retained.interprocedural = None;
            retained.missing_owners = None;
        } else {
            *cache = Some(CachedLateStages {
                inputs: current_late_stage_inputs(facts),
                local_accesses: CachedLocalAccesses::default(),
                interprocedural: None,
                missing_owners: None,
                owner_files: HashMap::new(),
            });
        }
    }
    let owned_typescript_indexes;
    let typescript_indexes = if let Some(cache) = typescript_indexes_cache {
        let patch_timings = (!typescript_unchanged)
            .then(|| {
                cache.as_mut().and_then(|cached| {
                    facts.typescript_changes.as_ref().and_then(|changes| {
                        patch_typescript_indexes(
                            cached,
                            &facts.typescript,
                            &project_indexes.symbols_by_id,
                            changes,
                        )
                    })
                })
            })
            .flatten();
        let indexes_patched = patch_timings.is_some();
        if let Some((alias_roots, entity_symbols)) = patch_timings {
            build_timings.alias_roots = alias_roots;
            build_timings.entity_symbols = entity_symbols;
            build_timings.alias_and_entity_indexes = alias_roots + entity_symbols;
        }
        if (!typescript_unchanged && !indexes_patched) || cache.is_none() {
            let substage_started = Instant::now();
            let (indexes, alias_roots, entity_symbols) =
                build_typescript_indexes(&facts.typescript, facts.files.len() >= 256);
            build_timings.alias_roots = alias_roots;
            build_timings.entity_symbols = entity_symbols;
            build_timings.alias_and_entity_indexes = substage_started.elapsed();
            *cache = Some(indexes);
        } else {
            build_timings.typescript_indexes_reused = true;
        }
        cache.as_ref().expect("TypeScript indexes initialized")
    } else {
        let substage_started = Instant::now();
        let (indexes, alias_roots, entity_symbols) =
            build_typescript_indexes(&facts.typescript, facts.files.len() >= 256);
        build_timings.alias_roots = alias_roots;
        build_timings.entity_symbols = entity_symbols;
        build_timings.alias_and_entity_indexes = substage_started.elapsed();
        owned_typescript_indexes = indexes;
        &owned_typescript_indexes
    };
    let aliases = &typescript_indexes.aliases;
    let source_declarations = &typescript_indexes.source_declarations;
    let entities = &typescript_indexes.entities;
    let substage_started = Instant::now();
    let mut symbol_names = typescript_indexes.symbol_names.clone();
    add_solid_import_names(facts, entities, &mut symbol_names);
    build_timings.symbol_name_indexes = substage_started.elapsed();
    let substage_started = Instant::now();
    let mut resolved_contracts = resolve_contract_imports(facts, contracts, entities);
    build_timings.contract_resolution = substage_started.elapsed();
    let semantic_lookup = SemanticLookup::new(facts, ast_indexes, entities, &symbol_names);
    let semantic_lookup = &semantic_lookup;
    // Source discovery does not inspect missing exports, and the static prepass
    // owns them after the two independent index passes complete.
    let mut static_violations = std::mem::take(&mut resolved_contracts.missing_exports);
    let mut owned_reachable_calls = None;
    let source_discovery = std::thread::scope(|scope| {
        let shared_worker_limit = analysis_worker_limit_for_lanes(2);
        let source_context = StageContext {
            facts,
            project_indexes: &project_indexes,
            typescript_indexes,
            entities,
            source_declarations,
            symbol_names: &symbol_names,
            semantic_lookup,
            resolved_contracts: &resolved_contracts,
            contracts,
        };
        let source_discovery_handle = scope.spawn(move || {
            let _worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
            let mut timings = BuildTimings::default();
            let sources = discover_sources(
                &source_context,
                source_discovery_cache,
                typescript_unchanged,
                &mut timings,
                emit_timings,
            );
            (sources, timings)
        });
        let reachability_worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
        if let Some(cache) = reachability_cache.as_deref_mut() {
            let can_reuse = typescript_unchanged
                && cache.as_ref().is_some_and(|cached| {
                    cached.inputs.len() == facts.files.len()
                        && facts.files.iter().all(|file| {
                            cached.inputs.get(file.path.as_str()).is_some_and(
                                |(source_hash, ast)| {
                                    source_hash == &file.source_hash
                                        || same_reachability_ast(ast, &file.ast)
                                },
                            )
                        })
                });
            if can_reuse {
                let cached = cache.as_mut().expect("checked retained reachability");
                for file in &facts.files {
                    if let Some((source_hash, _)) = cached.inputs.get_mut(file.path.as_str()) {
                        source_hash.clone_from(&file.source_hash);
                    }
                    if let Some(retained_file) = cached.files.get_mut(file.path.as_str()) {
                        retained_file
                            .identity
                            .source_hash
                            .clone_from(&file.source_hash);
                    }
                }
                build_timings.reachability_reused = true;
            } else {
                let substage_started = Instant::now();
                let cached = cache.get_or_insert_with(|| CachedReachability {
                    inputs: HashMap::new(),
                    files: HashMap::new(),
                    calls: HashMap::new(),
                    multiplicity_by_path: HashMap::new(),
                    function_symbols: HashSet::new(),
                });
                let (reused_files, recomputed_files) = reachable_call_multiplicity_incremental(
                    ReachabilityInputs {
                        facts,
                        indexes: &project_indexes,
                        entities,
                        symbol_names: &symbol_names,
                        lookup: semantic_lookup,
                        typescript_unchanged,
                        typescript_delta: typescript_indexes.source_discovery_delta.as_ref(),
                    },
                    ReachabilityState {
                        files: &mut cached.files,
                        multiplicity_by_path: &mut cached.multiplicity_by_path,
                        calls: &mut cached.calls,
                        function_symbols: &mut cached.function_symbols,
                    },
                );
                build_timings.reachability = substage_started.elapsed();
                build_timings.reachability_reused_files = reused_files;
                build_timings.reachability_recomputed_files = recomputed_files;
                cached.inputs = facts
                    .files
                    .iter()
                    .map(|file| {
                        (
                            file.path.to_string(),
                            (file.source_hash.clone(), file.ast.clone()),
                        )
                    })
                    .collect();
            }
        } else {
            let substage_started = Instant::now();
            owned_reachable_calls = Some(reachable_call_multiplicity(
                facts,
                &project_indexes,
                entities,
                &symbol_names,
                semantic_lookup,
            ));
            build_timings.reachability = substage_started.elapsed();
        }
        drop(reachability_worker_limit);
        finish_stage!(indexes_and_reachability, "indexes-and-reachability");
        let (source_discovery, discovery_timings) = source_discovery_handle
            .join()
            .expect("parallel source discovery worker panicked");
        build_timings.source_discovery = discovery_timings.source_discovery;
        build_timings.source_discovery_reused_files =
            discovery_timings.source_discovery_reused_files;
        build_timings.source_discovery_recomputed_files =
            discovery_timings.source_discovery_recomputed_files;
        build_timings.typed_accessors_and_prop_roots =
            discovery_timings.typed_accessors_and_prop_roots;
        build_timings.prop_propagation_and_control_flow =
            discovery_timings.prop_propagation_and_control_flow;
        source_discovery
    });
    let reachable_calls = if let Some(cache) = reachability_cache {
        &cache.as_ref().expect("reachability initialized").calls
    } else {
        owned_reachable_calls
            .as_ref()
            .expect("owned reachability initialized")
    };
    let SourceDiscovery {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        returned_source_symbols,
        summary_source_symbols,
        source_owned_write,
        async_sources,
        contract_reads,
        contract_callbacks,
        contract_returns,
        contracted_accessor_symbols,
        prop_sources,
        bundled_returns,
        retained_source_paths,
        changed_source_symbols,
    } = source_discovery;
    // discover_sources owns its own stage timers; re-anchor this function's
    // timer so finish_stage!(static_prepass) measures only the prepass loops.
    // The read keeps the previous stage's macro reset from becoming a dead
    // store.
    let _ = stage_started;
    stage_started = Instant::now();

    let mut leaf_operations = Vec::new();
    let mut invalid_cleanup_returns = Vec::new();
    let mut unresolved_cleanup_returns = Vec::new();
    for file in &facts.files {
        for span in file.compiler.uncovered_jsx_expressions() {
            static_violations.push(StaticViolation {
                id: "SC9004".into(),
                rule: "execution-map-incomplete".into(),
                message:
                    "the Solid compiler did not classify this JSX expression as tracked, untracked, or a callback; without an execution role, solid-checker cannot certify any reactive read inside it"
                        .into(),
                hint: "Simplify the expression: hoist complex logic into a createMemo and interpolate the accessor. If this persists on plain JSX, re-run with fresh compiler facts and report the pattern as a solid-checker issue.".into(),
                location: location(file.path.shared(), span),
                analysis_context: String::new(),
                fixes: vec![],
            });
        }
    }
    let mut directive_creations = Vec::new();
    let mut missing_owners = Vec::new();
    let mut seen_static = HashSet::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            if function_binding_name(file, function)
                .and_then(|name| {
                    file.source_text(name.span)
                        .unwrap_or_default()
                        .chars()
                        .next()
                })
                .is_some_and(char::is_uppercase)
                && let Some(parameter) = function
                    .parameters
                    .first()
                    .filter(|parameter| parameter.shape == solid_ast_facts::BindingShape::Object)
            {
                let location = location(file.path.shared(), parameter.pattern);
                if seen_static.insert((
                    "component-props-destructure",
                    location.path.clone(),
                    location.start_byte,
                )) {
                    static_violations.push(StaticViolation {
                        id: "SC1003".into(),
                        rule: "component-props-destructure".into(),
                        message: "destructuring props unwraps each property once at component setup; the bindings are frozen values, and the component never updates when the parent passes new props".into(),
                        hint: "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use omit(props, ...keys) and merge(defaults, props) instead of destructuring.".into(),
                        location,
                        analysis_context: function_binding_name(file, function)
                            .map_or_else(String::new, |name| file.source_text(name.span).unwrap_or_default().to_owned()),
                        fixes: component_props_parameter_fix(
                            facts,
                            file,
                            function,
                            parameter,
                            entities,
                        )
                        .into_iter()
                        .collect(),
                    });
                }
            }
        }
        for binding in &file.ast.bindings {
            if binding.shape != solid_ast_facts::BindingShape::Object {
                continue;
            }
            let props = binding
                .initializer_identifier
                .as_ref()
                .and_then(|identifier| entities.get(&location(file.path.shared(), identifier.span)))
                .is_some_and(|symbol| prop_sources.contains_key(symbol));
            if props {
                let location = location(file.path.shared(), binding.pattern);
                if seen_static.insert((
                    "component-props-destructure",
                    location.path.clone(),
                    location.start_byte,
                )) {
                    static_violations.push(StaticViolation {
                        id: "SC1003".into(),
                        rule: "component-props-destructure".into(),
                        message: "destructuring props unwraps each property once at component setup; the bindings are frozen values, and the component never updates when the parent passes new props".into(),
                        hint: "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use omit(props, ...keys) and merge(defaults, props) instead of destructuring.".into(),
                        location,
                        analysis_context: enclosing_function_label(file, binding.pattern),
                        fixes: vec![],
                    });
                }
            }
        }
    }
    for typescript_file in facts.typescript.files() {
        for function in typescript_file.async_functions.iter() {
            for call in &function.calls_after_await {
                let Some(symbol) = entities.get(call) else {
                    continue;
                };
                let Some((name, _)) = accessors.get(symbol) else {
                    continue;
                };
                let ast_call = facts
                    .files
                    .iter()
                    .find(|file| *file.path.as_str() == *call.path)
                    .and_then(|file| {
                        file.ast
                            .calls
                            .iter()
                            .find(|candidate| {
                                u64::from(candidate.callee.start) == call.start_byte
                                    && u64::from(candidate.callee.end) == call.end_byte
                            })
                            .map(|candidate| (file, candidate))
                    });
                let display = ast_call
                    .and_then(|(file, candidate)| candidate.static_callee(&file.source))
                    .unwrap_or(name);
                let diagnostic_location = Location {
                    path: call.path.clone(),
                    start_byte: call.start_byte,
                    end_byte: call.end_byte.saturating_add(1),
                };
                let function_symbol = async_symbol_root(
                    aliases
                        .get(function.symbol.as_ref())
                        .map_or(function.symbol.as_ref(), SymbolId::as_str),
                    &facts.typescript,
                );
                let Some(analysis_context) = facts.files.iter().find_map(|file| {
                    file.ast.calls.iter().find_map(|candidate| {
                        let argument = candidate.arguments.first()?;
                        let lexical = *file.path.as_str() == *function.expression.path
                            && argument.span.contains(Span::new(
                                u32::try_from(function.expression.start_byte).ok()?,
                                u32::try_from(function.expression.end_byte).ok()?,
                            ));
                        let semantic = entities
                            .get(&location(file.path.shared(), argument.span))
                            .is_some_and(|symbol| {
                                async_symbol_root(symbol, &facts.typescript) == function_symbol
                            });
                        if !lexical && !semantic {
                            return None;
                        }
                        let primitive = primitive_name(
                            file.path.as_str(),
                            candidate.callee,
                            candidate.static_callee(&file.source),
                            entities,
                            &symbol_names,
                        )?;
                        matches!(
                            primitive.as_str(),
                            "createMemo"
                                | "createEffect"
                                | "createRenderEffect"
                                | "createProjection"
                                | "createSignal"
                                | "createStore"
                                | "createOptimistic"
                                | "createOptimisticStore"
                        )
                        .then(|| format!("{primitive} async computation"))
                    })
                }) else {
                    continue;
                };
                if seen_static.insert((
                    "reactive-read-after-await",
                    call.path.clone(),
                    call.start_byte,
                )) {
                    static_violations.push(StaticViolation {
                        id: "SC1002".into(),
                        rule: "reactive-read-after-await".into(),
                        message: format!(
                            "reactive accessor {display:?} is read after an await; dependency tracking ends at the first await, so this read registers no dependency and the computation never re-runs when {display:?} changes"
                        ),
                        hint: "Read reactive values before the first await and carry the results through the async work. If the value must stay live after the await, split the read into its own synchronous computation.".into(),
                        location: diagnostic_location,
                        analysis_context,
                        fixes: vec![],
                    });
                }
            }
        }
    }
    finish_stage!(static_prepass, "static-prepass");
    let local_access_context = LocalAccessContext {
        facts,
        lookup: semantic_lookup,
        entities,
        symbol_names: &symbol_names,
        reachable_calls,
        accessors: &accessors,
        accessor_origins: &accessor_origins,
        setters: &setters,
        actions: &actions,
        source_primitives: &source_primitives,
        async_sources: &async_sources,
        source_declarations,
        contract_reads: &contract_reads,
        source_kinds: &source_kinds,
        prop_sources: &prop_sources,
    };
    let cached_interprocedural = late_stages_reusable
        .then(|| {
            late_stage_cache
                .as_deref()
                .and_then(Option::as_ref)
                .and_then(|cache| cache.interprocedural.as_ref())
                .cloned()
        })
        .flatten();
    let references_by_source = if cached_interprocedural.is_some() {
        HashMap::new()
    } else {
        references_for_sources(
            &facts.typescript,
            &typescript_indexes.symbols_by_root,
            accessors.keys(),
        )
    };
    let local_access_cache = late_stage_cache
        .as_deref_mut()
        .and_then(Option::as_mut)
        .map(|cache| &mut cache.local_accesses);
    let overlap_late_stages = cached_interprocedural.is_none() && facts.files.len() >= 256;
    let interprocedural_context = InterproceduralContext {
        facts,
        project_indexes: &project_indexes,
        accessors: &accessors,
        contracted_accessor_symbols: &contracted_accessor_symbols,
        returned_source_symbols: &returned_source_symbols,
        summary_source_symbols: &summary_source_symbols,
        source_phases: &source_phases,
        source_kinds: &source_kinds,
        contract_reads: &contract_reads,
        contract_callbacks: &contract_callbacks,
        contract_returns: &contract_returns,
        bundled_returns: &bundled_returns,
        source_primitives: &source_primitives,
        entities,
        references_by_source: &references_by_source,
        symbol_names: &symbol_names,
        changed_semantic_symbols: typescript_indexes
            .source_discovery_delta
            .as_ref()
            .map(|delta| &delta.semantic_symbol_ids),
        retained_source_paths: &retained_source_paths,
        lookup: semantic_lookup,
    };
    let run_local_access = || {
        local_access_context.build(
            local_access_cache,
            LocalAccessReuse {
                aggregate_reusable: late_stages_reusable,
                typescript_unchanged,
                source_discovery_delta: typescript_indexes.source_discovery_delta.as_ref(),
                changed_source_symbols: &changed_source_symbols,
                retained_source_paths: &retained_source_paths,
                global_async_context_unchanged: late_stages_reusable,
            },
        )
    };
    let (local_access, interprocedural, local_access_elapsed, interprocedural_elapsed, reused) =
        std::thread::scope(|scope| {
            if let Some(mut cached) = cached_interprocedural {
                let local_started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = local_started.elapsed();
                cached.timings = InterproceduralTimings::default();
                return (local_access, cached, local_elapsed, Duration::ZERO, true);
            }
            if overlap_late_stages {
                let shared_worker_limit = analysis_worker_limit_for_lanes(2);
                let interprocedural = scope.spawn(move || {
                    let _worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
                    let started = Instant::now();
                    let result = interprocedural_context.build(
                        typed_accessor_cache,
                        interprocedural_graph_cache,
                        interprocedural_result_cache,
                    );
                    (result, started.elapsed())
                });
                let local_worker_limit = AnalysisWorkerLimit::enter(shared_worker_limit);
                let local_started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = local_started.elapsed();
                drop(local_worker_limit);
                let (interprocedural, interprocedural_elapsed) = interprocedural
                    .join()
                    .expect("parallel interprocedural analysis worker panicked");
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    interprocedural_elapsed,
                    false,
                )
            } else {
                let local_started = Instant::now();
                let local_access = run_local_access();
                let local_elapsed = local_started.elapsed();
                let interprocedural_started = Instant::now();
                let interprocedural = interprocedural_context.build(
                    typed_accessor_cache,
                    interprocedural_graph_cache,
                    interprocedural_result_cache,
                );
                (
                    local_access,
                    interprocedural,
                    local_elapsed,
                    interprocedural_started.elapsed(),
                    false,
                )
            }
        });
    build_timings.local_reads_and_writes = local_access_elapsed;
    build_timings.interprocedural_summaries = interprocedural_elapsed;
    build_timings.interprocedural_reused = reused;
    let local_and_interprocedural_elapsed = stage_started.elapsed();
    build_timings.local_and_interprocedural = local_and_interprocedural_elapsed;
    if emit_timings {
        eprintln!(
            "{{\"reactiveIrStage\":\"local-reads-and-writes\",\"elapsedNs\":{}}}",
            local_access_elapsed.as_nanos()
        );
        eprintln!(
            "{{\"reactiveIrStage\":\"interprocedural-summaries\",\"elapsedNs\":{}}}",
            interprocedural_elapsed.as_nanos()
        );
        eprintln!(
            "{{\"reactiveIrStage\":\"local-and-interprocedural\",\"elapsedNs\":{}}}",
            local_and_interprocedural_elapsed.as_nanos()
        );
    }
    stage_started = Instant::now();
    if !reused && let Some(cache) = late_stage_cache.as_deref_mut().and_then(Option::as_mut) {
        cache.interprocedural = Some(interprocedural.clone());
    }
    build_timings.local_accesses_reused = local_access.reused;
    build_timings.local_access_reused_files = local_access.reused_files;
    build_timings.local_access_recomputed_files = local_access.recomputed_files;
    let LocalAccessResult {
        reads,
        writes,
        action_invocations,
        async_reads,
        mut strict_read_obligations,
        mut write_action_obligations,
    } = local_access.result;
    let mut reads = reads
        .into_iter()
        .map(|read| (*read).clone())
        .collect::<Vec<_>>();
    let mut writes = writes
        .into_iter()
        .map(|write| (*write).clone())
        .collect::<Vec<_>>();
    let mut action_invocations = action_invocations
        .into_iter()
        .map(|action| (*action).clone())
        .collect::<Vec<_>>();
    let mut async_reads = async_reads
        .into_iter()
        .map(|read| (*read).clone())
        .collect::<Vec<_>>();
    build_timings.interprocedural_graph = interprocedural.timings.graph;
    build_timings.interprocedural_direct_summaries = interprocedural.timings.direct_summaries;
    build_timings.interprocedural_direct_index = interprocedural.timings.direct_index;
    build_timings.interprocedural_direct_references = interprocedural.timings.direct_references;
    build_timings.interprocedural_typed_accessors = interprocedural.timings.typed_accessors;
    build_timings.interprocedural_propagation = interprocedural.timings.propagation;
    build_timings.interprocedural_returned_direct = interprocedural.timings.returned_direct;
    build_timings.interprocedural_returned_delta = interprocedural.timings.returned_delta;
    build_timings.interprocedural_call_summary_delta = interprocedural.timings.call_summary_delta;
    build_timings.interprocedural_factory_propagation = interprocedural.timings.factory_propagation;
    build_timings.interprocedural_results_and_exports = interprocedural.timings.results_and_exports;
    build_timings.interprocedural_result_reads = interprocedural.timings.result_reads;
    build_timings.interprocedural_export_summaries = interprocedural.timings.export_summaries;
    build_timings.typed_accessor_reused_files = interprocedural.timings.typed_accessor_reused_files;
    build_timings.typed_accessor_recomputed_files =
        interprocedural.timings.typed_accessor_recomputed_files;
    build_timings.interprocedural_graph_reused_files = interprocedural.timings.graph_reused_files;
    build_timings.interprocedural_graph_recomputed_files =
        interprocedural.timings.graph_recomputed_files;
    build_timings.interprocedural_result_reused_files = interprocedural.timings.result_reused_files;
    build_timings.interprocedural_result_recomputed_files =
        interprocedural.timings.result_recomputed_files;
    strict_read_obligations += interprocedural.reads.len();
    reads.extend(interprocedural.reads.iter().cloned());
    for file in &facts.files {
        for function in &file.ast.functions {
            let Some(name) = function_binding_name(file, function).or(function.name.as_ref())
            else {
                continue;
            };
            if !file
                .source_text(name.span)
                .unwrap_or_default()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
            {
                continue;
            }
            let mut direct_returns = file
                .ast
                .returns
                .iter()
                .filter(|returned| {
                    function.body.contains(returned.span)
                        && containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                })
                .collect::<Vec<_>>();
            if let Some(returned) = &function.expression_return {
                direct_returns.push(returned);
            }
            for test in file.ast.conditional_tests.iter().filter(|test| {
                function.body.contains(**test)
                    && containing_ast_function(&file.ast, **test)
                        .is_some_and(|owner| owner.span == function.span)
            }) {
                let reactive = reads.iter().any(|read| {
                    read.location.path == file.path.as_str().into()
                        && u64::from(test.start) <= read.location.start_byte
                        && read.location.end_byte <= u64::from(test.end)
                });
                let conditional_return = direct_returns.iter().any(|returned| {
                    returned.control_tests.contains(test)
                        || (returned.conditional
                            && returned
                                .argument
                                .is_some_and(|argument| argument.contains(*test)))
                });
                if reactive && conditional_return {
                    let location = location(file.path.shared(), *test);
                    if seen_static.insert((
                        "component-returns-conditionally",
                        location.path.clone(),
                        location.start_byte,
                    )) {
                        static_violations.push(StaticViolation {
                            id: "SC1004".into(),
                            rule: "component-returns-conditionally".into(),
                            message: "this component's return value depends on a reactive condition, but a component body runs once; whichever branch is taken at setup renders forever, and the condition is never re-evaluated".into(),
                            hint: "Return a single JSX tree and move the branch into it: wrap the alternatives in <Show when={...} fallback={...}> (or <Switch>/<Match> for multiple cases), or use a ternary inside JSX where it stays tracked.".into(),
                            location,
                            analysis_context: file.source_text(name.span).unwrap_or_default().to_owned(),
                            fixes: vec![],
                        });
                    }
                }
            }
        }
    }
    let contract_exports = interprocedural.exports;
    let mut contract_generation_obligations =
        interprocedural.contract_generation_obligations.to_vec();
    leaf_operations.extend(
        parallel_file_results(&facts.files, |file| {
            leaf_owner_operations_for_file(file, entities, &symbol_names)
        })
        .into_iter()
        .flatten(),
    );
    for (invalid, unresolved) in parallel_file_results(&facts.files, |file| {
        cleanup_returns_for_file(semantic_lookup, file, &symbol_names)
    }) {
        invalid_cleanup_returns.extend(invalid);
        unresolved_cleanup_returns.extend(unresolved);
    }
    finish_stage!(leaf_and_cleanup, "leaf-and-cleanup");
    let static_api = StaticApiContext {
        lookup: semantic_lookup,
        entities,
        symbol_names: &symbol_names,
        source_kinds: &source_kinds,
        source_owned_write: &source_owned_write,
        accessors: &accessors,
        reachable_calls,
    };
    for result in parallel_file_results(&facts.files, |file| static_api.check_file(file)) {
        static_violations.extend(result.violations);
        writes.extend(result.writes);
        write_action_obligations.extend(result.write_action_obligations);
    }
    finish_stage!(static_api, "static-api");
    let mut seen_directive_creations = HashSet::new();
    for file in &facts.files {
        for call in &file.ast.calls {
            let role = execution_role(&file.compiler, call.callee, &[]);
            if role == ExecutionRole::DirectiveApply
                && let Some(primitive) = primitive_name(
                    file.path.as_str(),
                    call.callee,
                    call.static_callee(&file.source),
                    entities,
                    &symbol_names,
                )
                .filter(|primitive| is_created_primitive(primitive))
            {
                push_directive_creation(
                    &mut directive_creations,
                    &mut seen_directive_creations,
                    primitive.to_string(),
                    file.path.as_str(),
                    call.callee,
                    false,
                );
            }
        }
        for callback in &file.compiler.callback_roles {
            if callback.role != solid_facts::solid_compiler_facts::CallbackRoleKind::DirectiveApply
            {
                continue;
            }
            for call in file
                .ast
                .calls
                .iter()
                .filter(|call| callback.span.contains(call.span))
            {
                if let Some((target_file, target)) =
                    semantic_lookup.function_called_at(file.path.as_str(), call.callee)
                {
                    DirectiveCreationCollector::new(
                        semantic_lookup,
                        &symbol_names,
                        &mut directive_creations,
                        &mut seen_directive_creations,
                    )
                    .collect_returned(target_file, target);
                }
            }
        }
    }
    finish_stage!(directives, "directives");
    let cached_missing_owners = late_stages_reusable
        .then(|| {
            late_stage_cache
                .as_deref()
                .and_then(Option::as_ref)
                .and_then(|cache| cache.missing_owners.as_ref())
                .cloned()
        })
        .flatten();
    if let Some(cached) = cached_missing_owners {
        missing_owners = cached;
        build_timings.owner_fixed_point_reused = true;
        build_timings.owner_reused_files = u64::try_from(facts.files.len()).unwrap_or(u64::MAX);
    } else {
        if let Some(cache) = late_stage_cache.and_then(Option::as_mut) {
            let (requirements, timings) = find_missing_owners_incremental(
                facts,
                semantic_lookup,
                &project_indexes,
                &symbol_names,
                &retained_source_paths,
                &mut cache.owner_files,
                &mut build_timings,
            );
            missing_owners.extend(requirements);
            build_timings.owner_fragment_build = timings.fragment_build;
            build_timings.owner_graph_assembly = timings.graph_assembly;
            build_timings.owner_propagation = timings.propagation;
            build_timings.owner_requirement_emission = timings.requirement_emission;
            cache.missing_owners = Some(missing_owners.clone());
        } else {
            missing_owners.extend(find_missing_owners(
                facts,
                semantic_lookup,
                &project_indexes,
                &symbol_names,
            ));
            build_timings.owner_recomputed_files =
                u64::try_from(facts.files.len()).unwrap_or(u64::MAX);
        }
    }
    finish_stage!(owner_fixed_point, "owner-fixed-point");
    reads.sort_by(|left, right| {
        (
            &left.location.path,
            left.location.start_byte,
            left.location.end_byte,
        )
            .cmp(&(
                &right.location.path,
                right.location.start_byte,
                right.location.end_byte,
            ))
    });
    writes.sort_by(|left, right| location_order(&left.location, &right.location));
    action_invocations.sort_by(|left, right| location_order(&left.location, &right.location));
    invalid_cleanup_returns.sort_by(|left, right| location_order(&left.location, &right.location));
    unresolved_cleanup_returns
        .sort_by(|left, right| location_order(&left.location, &right.location));
    static_violations.sort_by(|left, right| location_order(&left.location, &right.location));
    directive_creations.sort_by(|left, right| location_order(&left.location, &right.location));
    missing_owners.sort_by(|left, right| location_order(&left.location, &right.location));
    async_reads.sort_by(|left, right| location_order(&left.location, &right.location));
    contract_generation_obligations
        .sort_by(|left, right| location_order(&left.location, &right.location));
    finish_stage!(final_ordering, "final-ordering");
    let _ = stage_started;
    build_timings.total = total_started.elapsed();
    Ok((
        Program {
            reads,
            writes,
            actions: action_invocations,
            leaf_operations,
            invalid_cleanup_returns,
            unresolved_cleanup_returns,
            static_violations,
            directive_creations,
            missing_owners,
            async_reads,
            contract_exports,
            contract_generation_obligations,
            obligation_counts: ObligationCounts {
                strict_reads: strict_read_obligations,
                writes_and_actions: write_action_obligations.len(),
                factory_instances: interprocedural.factory_instances,
            },
        },
        build_timings,
    ))
}

fn parallel_file_results<R, F>(files: &[FileFacts], analyze: F) -> Vec<R>
where
    R: Send,
    F: Fn(&FileFacts) -> R + Sync,
{
    parallel_slice_results(files, analyze)
}

thread_local! {
    static ANALYSIS_WORKER_LIMIT: Cell<usize> = const { Cell::new(usize::MAX) };
}

struct AnalysisWorkerLimit {
    previous: usize,
}

impl AnalysisWorkerLimit {
    fn enter(limit: usize) -> Self {
        let previous = ANALYSIS_WORKER_LIMIT.replace(limit.max(1));
        Self { previous }
    }
}

impl Drop for AnalysisWorkerLimit {
    fn drop(&mut self) {
        ANALYSIS_WORKER_LIMIT.set(self.previous);
    }
}

fn available_analysis_workers() -> usize {
    let available = std::thread::available_parallelism().map_or(1, usize::from);
    ANALYSIS_WORKER_LIMIT.with(|limit| available.min(limit.get()))
}

fn analysis_worker_limit_for_lanes(lanes: usize) -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .div_ceil(lanes.max(1))
}

fn parallel_slice_results<T, R, F>(items: &[T], analyze: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let workers = available_analysis_workers().min(items.len());
    if workers <= 1 || items.len() < 256 {
        return items.iter().map(analyze).collect();
    }
    let chunk_size = items.len().div_ceil(workers);
    std::thread::scope(|scope| {
        let handles = items
            .chunks(chunk_size)
            .map(|chunk| {
                let analyze = &analyze;
                scope.spawn(move || chunk.iter().map(analyze).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .expect("parallel Reactive IR analysis worker panicked")
            })
            .collect()
    })
}

fn parallel_file_chunk_results<R, F>(files: &[FileFacts], analyze: F) -> Vec<R>
where
    R: Send,
    F: Fn(&[FileFacts]) -> R + Sync,
{
    let workers = available_analysis_workers().min(files.len());
    if workers <= 1 || files.len() < 256 {
        return vec![analyze(files)];
    }
    let chunk_size = files.len().div_ceil(workers);
    std::thread::scope(|scope| {
        files
            .chunks(chunk_size)
            .map(|chunk| {
                let analyze = &analyze;
                scope.spawn(move || analyze(chunk))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .expect("parallel Reactive IR analysis worker panicked")
            })
            .collect()
    })
}

fn component_props_parameter_fix(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    function: &solid_ast_facts::FunctionFact,
    parameter: &solid_ast_facts::BindingFact,
    entities: &EntitySymbols,
) -> Option<Fix> {
    let pattern_start = usize::try_from(parameter.pattern.start).ok()?;
    let pattern_end = usize::try_from(parameter.pattern.end).ok()?;
    let pattern = file.source.get(pattern_start..pattern_end)?;
    if !pattern.starts_with('{') || !pattern.ends_with('}') || parameter.names.is_empty() {
        return None;
    }
    let mut cursor = pattern_start + 1;
    for name in &parameter.names {
        let start = usize::try_from(name.span.start).ok()?;
        let end = usize::try_from(name.span.end).ok()?;
        if start < cursor || end > pattern_end {
            return None;
        }
        if !file.source.as_bytes()[cursor..start]
            .iter()
            .all(|byte| byte.is_ascii_whitespace() || *byte == b',')
            || file.source.get(start..end)? != file.source_text(name.span)?
        {
            return None;
        }
        cursor = end;
    }
    if !file.source.as_bytes()[cursor..pattern_end - 1]
        .iter()
        .all(|byte| byte.is_ascii_whitespace() || *byte == b',')
    {
        return None;
    }

    let used_names = file
        .ast
        .identifiers
        .iter()
        .filter(|identifier| function.body.contains(identifier.span))
        .filter_map(|identifier| file.source_text(identifier.span))
        .collect::<HashSet<_>>();
    let parameter_name = (1..)
        .map(|suffix| {
            if suffix == 1 {
                "props".into()
            } else {
                format!("props{suffix}")
            }
        })
        .find(|candidate| !used_names.contains(candidate.as_str()))?;
    let mut edits = vec![TextEdit {
        location: location(file.path.shared(), parameter.pattern),
        new_text: parameter_name.clone(),
    }];
    let mut body_references = 0;
    for name in &parameter.names {
        let declaration = location(file.path.shared(), name.span);
        let symbol = entities.get(&declaration)?;
        let symbol = facts
            .typescript
            .symbols()
            .find(|candidate| candidate.id() == symbol.as_str())?;
        for reference in symbol.references() {
            if reference.path != file.path.as_str().into() {
                return None;
            }
            let span = Span::new(
                u32::try_from(reference.start_byte).ok()?,
                u32::try_from(reference.end_byte).ok()?,
            );
            if parameter.pattern.contains(span) {
                continue;
            }
            if !function.body.contains(span)
                || (!matches!(
                    execution_role(&file.compiler, span, &[]),
                    ExecutionRole::TrackedJsx
                ) && !file.compiler.jsx_operations.iter().any(|operation| {
                    operation.kind == "jsx-expression" && operation.span.contains(span)
                }))
            {
                return None;
            }
            let start = usize::try_from(reference.start_byte).ok()?;
            let end = usize::try_from(reference.end_byte).ok()?;
            if file.source.get(start..end)? != file.source_text(name.span)? {
                return None;
            }
            body_references += 1;
            edits.push(TextEdit {
                location: reference.clone(),
                new_text: format!(
                    "{parameter_name}.{}",
                    file.source_text(name.span).unwrap_or_default()
                ),
            });
        }
    }
    if body_references == 0 {
        return None;
    }
    edits.sort_by_key(|edit| edit.location.start_byte);
    Some(Fix {
        message: "Keep component props reactive: read via props.<name> instead of destructuring"
            .into(),
        applicability: "safe".into(),
        edits,
    })
}

fn containing_ast_function(
    ast: &solid_ast_facts::AstFacts,
    span: Span,
) -> Option<&solid_ast_facts::FunctionFact> {
    ast.functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
}

const OWNER_CONTEXT_OWNED: u8 = 1;
const OWNER_CONTEXT_UNOWNED: u8 = 2;
const OWNER_CONTEXT_LEAF: u8 = 4;

#[derive(Clone, Copy)]
enum OwnerEdgeKind {
    Preserve,
    Owned,
    Unowned,
    Leaf,
}

#[derive(Clone)]
struct OwnerNode {
    path: String,
    span: Span,
    body: Span,
    name: Option<String>,
    symbol: Option<SymbolId>,
    exported: bool,
}

impl FunctionBoundary for OwnerNode {
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Span {
        self.body
    }
}

struct OwnerFileIndex {
    call_primitives: Vec<Option<PrimitiveName>>,
    providing_regions: Vec<Span>,
}

#[derive(Clone)]
enum OwnerTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
}

#[derive(Clone)]
struct SymbolicOwnerEdge {
    source: Option<Span>,
    target: OwnerTarget,
    kind: OwnerEdgeKind,
}

#[derive(Clone)]
struct OwnerRequirementCandidate {
    operation: &'static str,
    operation_span: Span,
    owner: Option<Span>,
    report_mask: u8,
    allow_uncertain: bool,
    settled_target: Option<OwnerTarget>,
}

struct CachedOwnerFile {
    source_hash: SourceHash,
    compiler: Arc<solid_facts::solid_compiler_facts::ExecutionMap>,
    nodes: Vec<OwnerNode>,
    edges: Vec<SymbolicOwnerEdge>,
    requirements: Vec<OwnerRequirementCandidate>,
}

#[derive(Clone, Copy, Default)]
struct OwnerIncrementalTimings {
    fragment_build: Duration,
    graph_assembly: Duration,
    propagation: Duration,
    requirement_emission: Duration,
}

fn find_missing_owners(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> Vec<OwnerRequirement> {
    let entities = lookup.entities();
    let owner_file_indexes = facts
        .files
        .iter()
        .map(|file| {
            let call_primitives = file
                .ast
                .calls
                .iter()
                .map(|call| {
                    primitive_name(
                        file.path.as_str(),
                        call.callee,
                        call.static_callee(&file.source),
                        entities,
                        symbol_names,
                    )
                })
                .collect::<Vec<_>>();
            let providing_regions = file
                .ast
                .calls
                .iter()
                .zip(&call_primitives)
                .filter_map(|(call, primitive)| {
                    let argument = match primitive.as_deref() {
                        Some(
                            "createRoot" | "createMemo" | "createEffect" | "createRenderEffect"
                            | "createProjection" | "createSignal" | "createStore",
                        ) => 0,
                        Some("runWithOwner") => 1,
                        _ => return None,
                    };
                    call.arguments.get(argument).and_then(|argument| {
                        matches!(
                            argument.value,
                            solid_ast_facts::ArgumentValueKind::Identifier
                                | solid_ast_facts::ArgumentValueKind::Function
                                | solid_ast_facts::ArgumentValueKind::AsyncFunction
                        )
                        .then_some(argument.span)
                    })
                })
                .collect();
            OwnerFileIndex {
                call_primitives,
                providing_regions,
            }
        })
        .collect::<Vec<_>>();
    let mut nodes = Vec::new();
    for file in &facts.files {
        for function in &file.ast.functions {
            let symbol = function.name.as_ref().and_then(|name| {
                entities
                    .get(&location(file.path.shared(), name.span))
                    .cloned()
            });
            let exported =
                indexes
                    .typescript_file(file.path.as_str())
                    .is_some_and(|typescript_file| {
                        typescript_file.functions.iter().any(|candidate| {
                            candidate.exported
                                && candidate.body.start_byte == u64::from(function.body.start)
                                && candidate.body.end_byte == u64::from(function.body.end)
                        })
                    })
                    || file.ast.exports.iter().any(|export| {
                        export.span.contains(function.span)
                            && !file.ast.functions.iter().any(|candidate| {
                                candidate.span != function.span
                                    && export.span.contains(candidate.span)
                                    && candidate.span.contains(function.span)
                            })
                    });
            nodes.push(OwnerNode {
                path: file.path.to_string(),
                span: function.span,
                body: function.body,
                name: function
                    .name
                    .as_ref()
                    .or_else(|| function_binding_name(file, function))
                    .map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
                symbol,
                exported,
            });
        }
    }
    let nodes_by_path = function_indices_by_path(&nodes);
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut contexts = vec![0_u8; nodes.len()];
    let mut edges = Vec::<(usize, usize, OwnerEdgeKind)>::new();
    for (index, node) in nodes.iter().enumerate() {
        if node
            .name
            .as_deref()
            .and_then(|name| name.chars().next())
            .is_some_and(char::is_uppercase)
        {
            contexts[index] |= OWNER_CONTEXT_OWNED;
        }
        if node.exported
            && node.name.is_some()
            && !node
                .name
                .as_deref()
                .and_then(|name| name.chars().next())
                .is_some_and(char::is_uppercase)
        {
            contexts[index] |= OWNER_CONTEXT_UNOWNED;
        }
    }
    for (file_index, file) in facts.files.iter().enumerate() {
        for (call_index, call) in file.ast.calls.iter().enumerate() {
            let owner =
                containing_function_indexed(&nodes, &nodes_by_path, file.path.as_str(), call.span);
            if let Some(target_index) = entities
                .get(&location(file.path.shared(), call.callee))
                .and_then(|symbol| by_symbol.get(symbol))
                .copied()
            {
                if let Some(owner) = owner {
                    edges.push((owner, target_index, OwnerEdgeKind::Preserve));
                } else {
                    contexts[target_index] |= OWNER_CONTEXT_UNOWNED;
                }
            }
            let callback_roles: &[(usize, OwnerEdgeKind)] =
                match owner_file_indexes[file_index].call_primitives[call_index].as_deref() {
                    Some(
                        "createRoot"
                        | "createMemo"
                        | "createSignal"
                        | "createStore"
                        | "createProjection"
                        | "createOptimistic"
                        | "createOptimisticStore",
                    ) => &[(0, OwnerEdgeKind::Owned)],
                    Some("createEffect" | "createRenderEffect") => {
                        &[(0, OwnerEdgeKind::Owned), (1, OwnerEdgeKind::Unowned)]
                    }
                    Some("createTrackedEffect" | "onSettled") => &[(0, OwnerEdgeKind::Leaf)],
                    _ => &[],
                };
            for (argument_index, edge_kind) in callback_roles {
                let Some(argument) = call.arguments.get(*argument_index) else {
                    continue;
                };
                let Some(target_index) = owner_callback_index(
                    &nodes,
                    &nodes_by_path,
                    &by_symbol,
                    file,
                    argument.span,
                    entities,
                ) else {
                    continue;
                };
                if let Some(owner) = owner {
                    edges.push((owner, target_index, *edge_kind));
                } else {
                    contexts[target_index] |= owner_edge_context(*edge_kind, OWNER_CONTEXT_UNOWNED);
                }
            }
        }
        for callback in &file.compiler.callback_roles {
            if !matches!(
                callback.role,
                solid_facts::solid_compiler_facts::CallbackRoleKind::EventHandler
                    | solid_facts::solid_compiler_facts::CallbackRoleKind::DirectiveApply
            ) {
                continue;
            }
            if let Some(index) = owner_callback_index(
                &nodes,
                &nodes_by_path,
                &by_symbol,
                file,
                callback.span,
                entities,
            ) {
                contexts[index] |= OWNER_CONTEXT_UNOWNED;
            }
        }
    }
    let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, target, kind) in edges {
        outgoing[source].push((target, kind));
    }
    let mut queued = contexts
        .iter()
        .map(|context| *context != 0)
        .collect::<Vec<_>>();
    let mut worklist = queued
        .iter()
        .enumerate()
        .filter_map(|(index, queued)| queued.then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(source) = worklist.pop_front() {
        queued[source] = false;
        for (target, kind) in outgoing[source].iter().copied() {
            let propagated = owner_edge_context(kind, contexts[source]);
            let next = contexts[target] | propagated;
            if next != contexts[target] {
                contexts[target] = next;
                if !queued[target] {
                    queued[target] = true;
                    worklist.push_back(target);
                }
            }
        }
    }

    let mut requirements = Vec::new();
    let mut seen = HashSet::new();
    for (file_index, file) in facts.files.iter().enumerate() {
        for (call_index, call) in file.ast.calls.iter().enumerate() {
            let primitive = owner_file_indexes[file_index].call_primitives[call_index].as_deref();
            let context = owner_context_at(
                &nodes,
                &nodes_by_path,
                &contexts,
                file.path.as_str(),
                call.span,
            );
            let root_owned = inside_owner_providing_region(
                &owner_file_indexes[file_index].providing_regions,
                call.span,
            );
            let operation = match primitive {
                Some("createEffect" | "createTrackedEffect") if !root_owned => {
                    Some(("effect", context & OWNER_CONTEXT_UNOWNED != 0))
                }
                Some("onCleanup") if !root_owned => Some((
                    "cleanup",
                    context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                )),
                Some("onSettled")
                    if !root_owned
                        && call.arguments.first().is_some_and(|argument| {
                            owner_callback_index(
                                &nodes,
                                &nodes_by_path,
                                &by_symbol,
                                file,
                                argument.span,
                                entities,
                            )
                            .and_then(|index| {
                                let node = &nodes[index];
                                let callback_file = facts
                                    .files
                                    .iter()
                                    .find(|candidate| candidate.path.as_str() == node.path)?;
                                let callback = callback_file
                                    .ast
                                    .functions
                                    .iter()
                                    .find(|candidate| candidate.span == node.span)?;
                                Some(function_returns_cleanup(lookup, callback_file, callback))
                            })
                            .unwrap_or(false)
                        }) =>
                {
                    Some((
                        "settled-cleanup",
                        context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                    ))
                }
                _ => None,
            };
            if let Some((operation, report)) = operation {
                let uncertain = containing_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    file.path.as_str(),
                    call.span,
                )
                .is_some_and(|index| {
                    nodes[index].exported
                        && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                        && !nodes[index]
                            .name
                            .as_deref()
                            .and_then(|name| name.chars().next())
                            .is_some_and(char::is_uppercase)
                });
                let operation_span = if operation == "settled-cleanup" {
                    call.arguments
                        .first()
                        .map_or(call.callee, |argument| argument.span)
                } else {
                    call.callee
                };
                push_owner_requirement(
                    &mut requirements,
                    &mut seen,
                    operation,
                    file.path.as_str(),
                    operation_span,
                    uncertain,
                    report,
                );
            }
        }
        for element in &file.ast.jsx_elements {
            let boundary = primitive_name(
                file.path.as_str(),
                element.name.span,
                Some(file.source_text(element.name.span).unwrap_or_default()),
                entities,
                symbol_names,
            );
            if boundary.as_deref() != Some("Loading") {
                continue;
            }
            let context = owner_context_at(
                &nodes,
                &nodes_by_path,
                &contexts,
                file.path.as_str(),
                element.span,
            );
            if inside_owner_providing_region(
                &owner_file_indexes[file_index].providing_regions,
                element.span,
            ) {
                continue;
            }
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                "boundary",
                file.path.as_str(),
                Span::new(element.span.start, element.name.span.end),
                false,
                context & OWNER_CONTEXT_UNOWNED != 0,
            );
        }
    }
    requirements
}

fn discover_owner_file(
    file: &solid_facts::FileFacts,
    indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> CachedOwnerFile {
    let call_primitives = file
        .ast
        .calls
        .iter()
        .map(|call| {
            primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
            )
        })
        .collect::<Vec<_>>();
    let providing_regions = file
        .ast
        .calls
        .iter()
        .zip(&call_primitives)
        .filter_map(|(call, primitive)| {
            let argument = match primitive.as_deref() {
                Some(
                    "createRoot" | "createMemo" | "createEffect" | "createRenderEffect"
                    | "createProjection" | "createSignal" | "createStore",
                ) => 0,
                Some("runWithOwner") => 1,
                _ => return None,
            };
            call.arguments.get(argument).and_then(|argument| {
                matches!(
                    argument.value,
                    solid_ast_facts::ArgumentValueKind::Identifier
                        | solid_ast_facts::ArgumentValueKind::Function
                        | solid_ast_facts::ArgumentValueKind::AsyncFunction
                )
                .then_some(argument.span)
            })
        })
        .collect::<Vec<_>>();
    let nodes = file
        .ast
        .functions
        .iter()
        .map(|function| {
            let symbol = function
                .name
                .as_ref()
                .or_else(|| function_binding_name(file, function))
                .and_then(|name| {
                    entities
                        .get(&location(file.path.shared(), name.span))
                        .cloned()
                });
            let exported =
                indexes
                    .typescript_file(file.path.as_str())
                    .is_some_and(|typescript_file| {
                        typescript_file.functions.iter().any(|candidate| {
                            candidate.exported
                                && candidate.body.start_byte == u64::from(function.body.start)
                                && candidate.body.end_byte == u64::from(function.body.end)
                        })
                    })
                    || file.ast.exports.iter().any(|export| {
                        export.span.contains(function.span)
                            && !file.ast.functions.iter().any(|candidate| {
                                candidate.span != function.span
                                    && export.span.contains(candidate.span)
                                    && candidate.span.contains(function.span)
                            })
                    });
            OwnerNode {
                path: file.path.to_string(),
                span: function.span,
                body: function.body,
                name: function
                    .name
                    .as_ref()
                    .map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
                symbol,
                exported,
            }
        })
        .collect::<Vec<_>>();
    let nodes_by_path = function_indices_by_path(&nodes);
    let owner_at = |span| {
        containing_function_indexed(&nodes, &nodes_by_path, file.path.as_str(), span)
            .map(|index| nodes[index].span)
    };
    let callback_target = |argument: Span| {
        nodes_by_path
            .get(file.path.as_str())
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| argument.contains(nodes[*index].span))
            .max_by_key(|index| nodes[*index].span.end - nodes[*index].span.start)
            .map(|index| OwnerTarget::LocalSpan(nodes[index].span))
            .or_else(|| {
                entities
                    .get(&location(file.path.shared(), argument))
                    .cloned()
                    .map(OwnerTarget::Symbol)
            })
    };
    let mut edges = Vec::new();
    let mut requirements = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let owner = owner_at(call.span);
        if let Some(symbol) = entities.get(&location(file.path.shared(), call.callee)) {
            edges.push(SymbolicOwnerEdge {
                source: owner,
                target: OwnerTarget::Symbol(symbol.clone()),
                kind: OwnerEdgeKind::Preserve,
            });
        }
        let callback_roles: &[(usize, OwnerEdgeKind)] = match call_primitives[call_index].as_deref()
        {
            Some(
                "createRoot"
                | "createMemo"
                | "createSignal"
                | "createStore"
                | "createProjection"
                | "createOptimistic"
                | "createOptimisticStore"
                | "dynamic",
            ) => &[(0, OwnerEdgeKind::Owned)],
            Some("createEffect" | "createRenderEffect") => {
                &[(0, OwnerEdgeKind::Owned), (1, OwnerEdgeKind::Unowned)]
            }
            Some("createTrackedEffect" | "onSettled") => &[(0, OwnerEdgeKind::Leaf)],
            _ => &[],
        };
        for (argument_index, kind) in callback_roles {
            if let Some(target) = call
                .arguments
                .get(*argument_index)
                .and_then(|argument| callback_target(argument.span))
            {
                edges.push(SymbolicOwnerEdge {
                    source: owner,
                    target,
                    kind: *kind,
                });
            }
        }
        if inside_owner_providing_region(&providing_regions, call.span) {
            continue;
        }
        let operation = match call_primitives[call_index].as_deref() {
            Some("createEffect" | "createTrackedEffect") => {
                Some(("effect", OWNER_CONTEXT_UNOWNED, None, call.callee))
            }
            Some("onCleanup") => Some((
                "cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                None,
                call.callee,
            )),
            Some("onSettled") => Some((
                "settled-cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                call.arguments
                    .first()
                    .and_then(|argument| callback_target(argument.span)),
                call.arguments
                    .first()
                    .map_or(call.callee, |argument| argument.span),
            )),
            _ => None,
        };
        if let Some((operation, report_mask, settled_target, operation_span)) = operation {
            requirements.push(OwnerRequirementCandidate {
                operation,
                operation_span,
                owner,
                report_mask,
                allow_uncertain: true,
                settled_target,
            });
        }
    }
    for callback in &file.compiler.callback_roles {
        if matches!(
            callback.role,
            solid_facts::solid_compiler_facts::CallbackRoleKind::EventHandler
                | solid_facts::solid_compiler_facts::CallbackRoleKind::DirectiveApply
        ) && let Some(target) = callback_target(callback.span)
        {
            edges.push(SymbolicOwnerEdge {
                source: None,
                target,
                kind: OwnerEdgeKind::Unowned,
            });
        }
    }
    for element in &file.ast.jsx_elements {
        let boundary = primitive_name(
            file.path.as_str(),
            element.name.span,
            Some(file.source_text(element.name.span).unwrap_or_default()),
            entities,
            symbol_names,
        );
        if boundary.as_deref() == Some("Loading")
            && !inside_owner_providing_region(&providing_regions, element.span)
        {
            requirements.push(OwnerRequirementCandidate {
                operation: "boundary",
                operation_span: Span::new(element.span.start, element.name.span.end),
                owner: owner_at(element.span),
                report_mask: OWNER_CONTEXT_UNOWNED,
                allow_uncertain: false,
                settled_target: None,
            });
        }
    }
    CachedOwnerFile {
        source_hash: file.source_hash.clone(),
        compiler: file.compiler.clone(),
        nodes,
        edges,
        requirements,
    }
}

fn resolve_owner_target(
    path: &str,
    target: &OwnerTarget,
    nodes_by_span: &HashMap<String, HashMap<Span, usize>>,
    by_symbol: &HashMap<SymbolId, usize>,
) -> Option<usize> {
    match target {
        OwnerTarget::Symbol(symbol) => by_symbol.get(symbol).copied(),
        OwnerTarget::LocalSpan(span) => nodes_by_span
            .get(path)
            .and_then(|nodes| nodes.get(span))
            .copied(),
    }
}

fn find_missing_owners_incremental(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    indexes: &ProjectIndexes<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    retained_source_paths: &HashSet<String>,
    cache: &mut HashMap<SourcePath, CachedOwnerFile>,
    build_timings: &mut BuildTimings,
) -> (Vec<OwnerRequirement>, OwnerIncrementalTimings) {
    let entities = lookup.entities();
    let total_started = Instant::now();
    let current_paths = facts
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<HashSet<_>>();
    cache.retain(|path, _| current_paths.contains(path.as_str()));
    let mut recomputed = Vec::new();
    for file in &facts.files {
        if retained_source_paths.contains(file.path.as_str())
            && let Some(cached) = cache.get(file.path.as_str())
            && cached.source_hash == file.source_hash
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
        {
            build_timings.owner_reused_files += 1;
            continue;
        }
        recomputed.push(file);
        build_timings.owner_recomputed_files += 1;
    }
    for (path, discovered) in parallel_slice_results(&recomputed, |file| {
        (
            file.path.clone(),
            discover_owner_file(file, indexes, entities, symbol_names),
        )
    }) {
        cache.insert(path, discovered);
    }
    let fragment_build = total_started.elapsed();

    let graph_started = Instant::now();
    let mut nodes = Vec::new();
    for file in &facts.files {
        if let Some(fragment) = cache.get(file.path.as_str()) {
            nodes.extend(fragment.nodes.iter().cloned());
        }
    }
    let mut nodes_by_span = HashMap::<String, HashMap<Span, usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        nodes_by_span
            .entry(node.path.clone())
            .or_default()
            .insert(node.span, index);
    }
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut contexts = vec![0_u8; nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        if node
            .name
            .as_deref()
            .and_then(|name| name.chars().next())
            .is_some_and(char::is_uppercase)
        {
            contexts[index] |= OWNER_CONTEXT_OWNED;
        }
        if node.exported
            && node.name.is_some()
            && !node
                .name
                .as_deref()
                .and_then(|name| name.chars().next())
                .is_some_and(char::is_uppercase)
        {
            contexts[index] |= OWNER_CONTEXT_UNOWNED;
        }
    }
    let mut outgoing = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for edge in &fragment.edges {
            let Some(target) =
                resolve_owner_target(file.path.as_str(), &edge.target, &nodes_by_span, &by_symbol)
            else {
                continue;
            };
            if let Some(source) = edge.source.and_then(|span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(&span))
                    .copied()
            }) {
                outgoing[source].push((target, edge.kind));
            } else {
                contexts[target] |= owner_edge_context(edge.kind, OWNER_CONTEXT_UNOWNED);
            }
        }
    }
    let graph_assembly = graph_started.elapsed();

    let propagation_started = Instant::now();
    let mut queued = contexts
        .iter()
        .map(|context| *context != 0)
        .collect::<Vec<_>>();
    let mut worklist = queued
        .iter()
        .enumerate()
        .filter_map(|(index, queued)| queued.then_some(index))
        .collect::<VecDeque<_>>();
    while let Some(source) = worklist.pop_front() {
        queued[source] = false;
        for (target, kind) in outgoing[source].iter().copied() {
            let propagated = owner_edge_context(kind, contexts[source]);
            let next = contexts[target] | propagated;
            if next != contexts[target] {
                contexts[target] = next;
                if !queued[target] {
                    queued[target] = true;
                    worklist.push_back(target);
                }
            }
        }
    }
    let propagation = propagation_started.elapsed();

    let requirements_started = Instant::now();
    let mut requirements = Vec::new();
    let mut seen = HashSet::new();
    for file in &facts.files {
        let Some(fragment) = cache.get(file.path.as_str()) else {
            continue;
        };
        for candidate in &fragment.requirements {
            if candidate.operation == "settled-cleanup" {
                let returns_cleanup = candidate
                    .settled_target
                    .as_ref()
                    .and_then(|target| {
                        resolve_owner_target(file.path.as_str(), target, &nodes_by_span, &by_symbol)
                    })
                    .and_then(|index| {
                        let node = &nodes[index];
                        let callback_file = indexes.files_by_path.get(node.path.as_str())?;
                        let callback = callback_file
                            .ast
                            .functions
                            .iter()
                            .find(|function| function.span == node.span)?;
                        Some(function_returns_cleanup(lookup, callback_file, callback))
                    })
                    .unwrap_or(false);
                if !returns_cleanup {
                    continue;
                }
            }
            let owner_index = candidate.owner.and_then(|span| {
                nodes_by_span
                    .get(file.path.as_str())
                    .and_then(|nodes| nodes.get(&span))
                    .copied()
            });
            let context = owner_index.map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index]);
            let uncertain = candidate.allow_uncertain
                && owner_index.is_some_and(|index| {
                    nodes[index].exported
                        && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                        && !nodes[index]
                            .name
                            .as_deref()
                            .and_then(|name| name.chars().next())
                            .is_some_and(char::is_uppercase)
                });
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                candidate.operation,
                file.path.as_str(),
                candidate.operation_span,
                uncertain,
                context & candidate.report_mask != 0,
            );
        }
    }
    let requirement_emission = requirements_started.elapsed();
    (
        requirements,
        OwnerIncrementalTimings {
            fragment_build,
            graph_assembly,
            propagation,
            requirement_emission,
        },
    )
}

fn inside_owner_providing_region(providing_regions: &[Span], span: Span) -> bool {
    providing_regions
        .iter()
        .any(|argument| argument.contains(span))
}

fn owner_callback_index(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    by_symbol: &HashMap<SymbolId, usize>,
    file: &solid_facts::FileFacts,
    argument: Span,
    entities: &EntitySymbols,
) -> Option<usize> {
    nodes_by_path
        .get(file.path.as_str())
        .into_iter()
        .flatten()
        .copied()
        .filter(|index| argument.contains(nodes[*index].span))
        // The argument's function can itself contain nested callbacks or a
        // returned closure. Select the outermost contained function: choosing
        // the smallest one assigns the caller's owner semantics to a nested
        // callback and leaves the actual argument unclassified.
        .max_by_key(|index| nodes[*index].span.end - nodes[*index].span.start)
        .or_else(|| {
            entities
                .get(&location(file.path.shared(), argument))
                .and_then(|symbol| by_symbol.get(symbol))
                .copied()
        })
}

const fn owner_edge_context(kind: OwnerEdgeKind, source: u8) -> u8 {
    match kind {
        OwnerEdgeKind::Preserve => source,
        OwnerEdgeKind::Owned => OWNER_CONTEXT_OWNED,
        OwnerEdgeKind::Unowned => OWNER_CONTEXT_UNOWNED,
        OwnerEdgeKind::Leaf => OWNER_CONTEXT_LEAF,
    }
}

fn owner_context_at(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    contexts: &[u8],
    path: &str,
    span: Span,
) -> u8 {
    containing_function_indexed(nodes, nodes_by_path, path, span)
        .map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index])
}

fn push_owner_requirement(
    requirements: &mut Vec<OwnerRequirement>,
    seen: &mut HashSet<(String, u64, u64, String)>,
    operation: &str,
    path: &str,
    span: Span,
    uncertain: bool,
    report: bool,
) {
    let location = location(path, span);
    if seen.insert((
        location.path.to_string(),
        location.start_byte,
        location.end_byte,
        operation.into(),
    )) {
        requirements.push(OwnerRequirement {
            operation: operation.into(),
            location,
            uncertain,
            report,
        });
    }
}
fn containing_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> Option<String> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if index != 0 {
                return None;
            }
            let owner = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
            )?;
            matches!(owner.as_str(), "onSettled" | "createTrackedEffect").then(|| owner.to_string())
        })
}

fn read_is_under_loading(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    let entities = lookup.entities();
    if file
        .ast
        .jsx_containing(span)
        .any(|element| jsx_element_is_loading(file, element, entities, symbol_names))
    {
        return true;
    }
    if file.ast.jsx_containing(span).any(|element| {
        jsx_target_function(lookup, file, element).is_some_and(|(target_file, target)| {
            target_file.ast.jsx_within(target.body).any(|candidate| {
                jsx_element_is_loading(target_file, candidate, entities, symbol_names)
            })
        })
    }) {
        return true;
    }
    let Some(owner) = file
        .ast
        .functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
    else {
        return false;
    };
    // For call sites whose target matched (file, owner), the "wrapper" the
    // second branch resolves is the owner itself, so the caller scan
    // distributes into: a Loading-wrapped call site exists, or any call site
    // exists and the owner's own body renders a Loading element.
    let call_sites = lookup.jsx_call_site_loading(file.path.as_str(), owner.span);
    call_sites.loading_wrapped
        || (call_sites.any
            && file
                .ast
                .jsx_within(owner.body)
                .any(|candidate| jsx_element_is_loading(file, candidate, entities, symbol_names)))
}

fn jsx_element_is_loading(
    file: &solid_facts::FileFacts,
    element: &solid_ast_facts::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    primitive_name(
        file.path.as_str(),
        element.name.span,
        Some(file.source_text(element.name.span).unwrap_or_default()),
        entities,
        symbol_names,
    )
    .as_deref()
        == Some("Loading")
}

fn jsx_target_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    element: &solid_ast_facts::JsxElementFact,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_ast_facts::FunctionFact,
)> {
    lookup.function_called_at(file.path.as_str(), element.name.span)
}

fn computation_is_async(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
) -> bool {
    if lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|function| {
                function.can_return_async
                    && u64::from(argument.start) <= function.expression.start_byte
                    && function.expression.end_byte <= u64::from(argument.end)
            })
        })
    {
        return true;
    }
    file.ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start)
        .is_some_and(|function| function.r#async)
}

fn inside_lowercase_named_function(file: &solid_facts::FileFacts, span: Span) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    if file.ast.arguments_containing(span).any(|(call, _)| {
        call.static_callee(&file.source).is_some_and(|callee| {
            matches!(
                callee.rsplit('.').next(),
                Some(
                    "createMemo"
                        | "createEffect"
                        | "createRenderEffect"
                        | "createTrackedEffect"
                        | "onSettled"
                        | "untrack"
                        | "action"
                )
            )
        })
    }) {
        return false;
    }
    file.ast.functions_body_containing(span).any(|function| {
        function_binding_name(file, function)
            .and_then(|name| {
                file.source_text(name.span)
                    .unwrap_or_default()
                    .chars()
                    .next()
            })
            .is_some_and(char::is_lowercase)
    })
}

fn inside_unclassified_callback(file: &solid_facts::FileFacts, span: Span) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    file.ast
        .functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
        .is_some_and(|function| function_binding_name(file, function).is_none())
}

fn function_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_ast_facts::FunctionFact,
) -> Option<&'a solid_ast_facts::NamedSpan> {
    function.name.as_ref().or_else(|| {
        file.ast
            .bindings_initializer_containing(function.span)
            .find(|binding| {
                binding.initializer_function
                    && binding.initializer.is_some_and(|initializer| {
                        file.ast
                            .functions_within(initializer)
                            .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                            .is_some_and(|candidate| candidate.span == function.span)
                    })
            })
            .and_then(|binding| binding.names.first())
    })
}

fn go_binding_pattern_accepts_call(
    source: &str,
    binding: &solid_ast_facts::BindingFact,
    call: &solid_ast_facts::CallFact,
) -> bool {
    let Some(name) = binding.array_slots.first().and_then(Option::as_ref) else {
        return false;
    };
    let Ok(name_start) = usize::try_from(name.span.start) else {
        return false;
    };
    let Ok(name_end) = usize::try_from(name.span.end) else {
        return false;
    };
    let Ok(start) = usize::try_from(call.callee.end) else {
        return false;
    };
    let Ok(callee_start) = usize::try_from(call.callee.start) else {
        return false;
    };
    let Ok(end) = usize::try_from(call.span.end) else {
        return false;
    };
    let bytes = source.as_bytes();
    let Some(before_name) = bytes.get(..name_start) else {
        return false;
    };
    let before_name = before_name.trim_ascii_end();
    if before_name.last() != Some(&b'[') {
        return false;
    }
    let declaration_prefix = before_name[..before_name.len() - 1].trim_ascii_end();
    if !declaration_prefix.ends_with(b"const") {
        return false;
    }
    let Some(binding_tail) = bytes.get(name_end..callee_start) else {
        return false;
    };
    let Some(close) = binding_tail.iter().rposition(|byte| *byte == b']') else {
        return false;
    };
    if binding_tail[close + 1..].trim_ascii() != b"=" {
        return false;
    }
    let Some(mut suffix) = bytes.get(start..end) else {
        return false;
    };
    suffix = suffix.trim_ascii_start();
    if suffix.first() == Some(&b'<') {
        let Some(close) = suffix.iter().position(|byte| *byte == b'>') else {
            return false;
        };
        suffix = suffix[close + 1..].trim_ascii_start();
    }
    suffix.first() == Some(&b'(')
}

fn go_returned_arrow_pattern_accepts(source: &str, span: Span) -> bool {
    let Ok(start) = usize::try_from(span.start) else {
        return false;
    };
    let Ok(end) = usize::try_from(span.end) else {
        return false;
    };
    let Some(mut value) = source.as_bytes().get(start..end) else {
        return false;
    };
    value = value.trim_ascii_start();
    if value.starts_with(b"async") {
        value = value[5..].trim_ascii_start();
    }
    if value.first() == Some(&b'(') {
        let Some(close) = value.iter().position(|byte| *byte == b')') else {
            return false;
        };
        return value[close + 1..].trim_ascii_start().starts_with(b"=>");
    }
    let identifier_end = value
        .iter()
        .position(|byte| {
            !matches!(
                byte,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'$'
            )
        })
        .unwrap_or(value.len());
    identifier_end != 0
        && value[identifier_end..]
            .trim_ascii_start()
            .starts_with(b"=>")
}

fn inside_effect_apply(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        index == 1
            && matches!(
                primitive_name(
                    file.path.as_str(),
                    call.callee,
                    call.static_callee(&file.source),
                    entities,
                    symbol_names,
                )
                .as_deref(),
                Some("createEffect" | "createRenderEffect")
            )
    })
}

/// The typed descriptor at a callee, kept only when it names a Solid accessor.
fn typed_accessor_descriptor_at<'a>(
    lookup: &SemanticLookup<'a>,
    path: &str,
    callee: Span,
) -> Option<&'a typefacts::TypeDescriptor> {
    lookup
        .smallest_contained_descriptor(path, callee)
        .filter(|descriptor| go_solid_accessor_descriptor(descriptor))
}

fn go_solid_accessor_descriptor(descriptor: &typefacts::TypeDescriptor) -> bool {
    descriptor.origin_module == "solid-js".into()
}

fn source_function_exported(
    indexes: &ProjectIndexes<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_ast_facts::FunctionFact,
) -> bool {
    indexes
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.functions.iter().any(|candidate| {
                candidate.exported
                    && candidate.body.start_byte == u64::from(function.body.start)
                    && candidate.body.end_byte == u64::from(function.body.end)
            })
        })
        || file.ast.exports_containing(function.span).any(|export| {
            !file.ast.functions_within(export.span).any(|candidate| {
                candidate.span != function.span && candidate.span.contains(function.span)
            })
        })
}

fn enclosing_render_function(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.functions_body_containing(span).any(|function| {
        function_binding_name(file, function)
            .or(function.name.as_ref())
            .and_then(|name| {
                file.source_text(name.span)
                    .unwrap_or_default()
                    .chars()
                    .next()
            })
            .is_some_and(char::is_uppercase)
    })
}

fn function_is_solid_callback(
    file: &solid_facts::FileFacts,
    function: &solid_ast_facts::FunctionFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let primitives = lookup.primitives(file);
    if file.ast.jsx_containing(function.span).any(|element| {
        !file
            .ast
            .functions_within(element.span)
            .any(|outer| outer.span != function.span && outer.span.contains(function.span))
            && jsx_primitive_name(file, element, entities, symbol_names).is_some_and(|primitive| {
                matches!(
                    primitive.as_str(),
                    "For" | "Repeat" | "Show" | "Match" | "Switch"
                )
            })
    }) {
        return true;
    }
    let Some(symbol) = function_symbol(file, function, entities) else {
        return false;
    };
    let binding_name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))
        .map(|name| file.source_text(name.span).unwrap_or_default());
    file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        primitives.calls[call_index]
            .as_deref()
            .is_some_and(|primitive| {
                matches!(
                    primitive,
                    "createMemo"
                        | "createEffect"
                        | "createRenderEffect"
                        | "createTrackedEffect"
                        | "createSignal"
                        | "createStore"
                        | "createProjection"
                        | "createOptimistic"
                        | "createOptimisticStore"
                        | "dynamic"
                )
            })
            && call.arguments.iter().any(|argument| {
                argument_references_callback_symbol(file, argument, symbol, entities, symbol_names)
            })
    }) || file
        .ast
        .jsx_elements
        .iter()
        .enumerate()
        .any(|(element_index, element)| {
            primitives.jsx[element_index]
                .as_deref()
                .is_some_and(|primitive| {
                    matches!(primitive, "For" | "Repeat" | "Show" | "Match" | "Switch")
                })
                && file.ast.identifiers_within(element.span).any(|identifier| {
                    identifier.role == solid_ast_facts::IdentifierRole::Reference
                        && !file.ast.jsx_containing(identifier.span).any(|nested| {
                            nested.span != element.span && element.span.contains(nested.span)
                        })
                        && (entities.get(&location(file.path.shared(), identifier.span))
                            == Some(symbol)
                            || binding_name == file.source_text(identifier.span))
                })
        })
}

fn counts_as_strict_read_root(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> bool {
    execution == ExecutionRole::EffectApply || enclosing_render_function(file, span)
}

fn enclosing_function_label(file: &solid_facts::FileFacts, span: Span) -> String {
    let Some(function) = file
        .ast
        .functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
    else {
        return String::new();
    };
    if let Some(name) = &function.name {
        return file.source_text(name.span).unwrap_or_default().to_owned();
    }
    function_binding_name(file, function).map_or_else(String::new, |name| {
        file.source_text(name.span).unwrap_or_default().to_owned()
    })
}

fn analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> String {
    let enclosing = enclosing_function_label(file, span);
    if let Some(rendering) = file
        .ast
        .functions_body_containing(span)
        .filter_map(|function| function.name.as_ref())
        .filter(|name| {
            file.source_text(name.span)
                .unwrap_or_default()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
        })
        .min_by_key(|name| name.span.end - name.span.start)
    {
        return file
            .source_text(rendering.span)
            .unwrap_or_default()
            .to_owned();
    }
    let callback = file
        .ast
        .arguments_containing(span)
        .map(|(call, index)| (call, index, call.arguments[index].span))
        .min_by_key(|(_, _, argument)| argument.end - argument.start);
    if let Some((call, argument, _)) = callback
        && let Some(primitive) = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
        )
    {
        let phase = match (primitive.as_str(), argument) {
            ("createEffect" | "createRenderEffect", 0) => Some("compute"),
            ("createEffect" | "createRenderEffect", 1) => Some("apply callback"),
            (
                "createMemo"
                | "createSignal"
                | "createStore"
                | "createProjection"
                | "createOptimistic"
                | "createOptimisticStore"
                | "dynamic",
                0,
            ) => Some("compute"),
            _ => None,
        };
        if let Some(phase) = phase {
            return format!("{primitive} {phase}");
        }
    }
    enclosing
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

fn contract_callback_execution(execution: ExecutionRole) -> &'static str {
    match execution {
        ExecutionRole::TrackedJsx => "tracked",
        ExecutionRole::DeferredCallback => "deferred",
        ExecutionRole::EffectApply
        | ExecutionRole::EventCallback
        | ExecutionRole::DirectiveApply
        | ExecutionRole::UntrackedRendering => "inline",
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
    functions_for_path(functions, by_path, path)
        .find(|(_, function)| function.body.contains(span))
        .map(|(index, _)| index)
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

struct LocalAccessContext<'a> {
    facts: &'a ProjectFacts,
    lookup: &'a SemanticLookup<'a>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    reachable_calls: &'a HashMap<Location, usize>,
    accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    accessor_origins: &'a HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    setters: &'a HashMap<SymbolId, (SymbolId, Location, bool)>,
    actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    source_primitives: &'a HashMap<SymbolId, SymbolId>,
    async_sources: &'a HashSet<SymbolId>,
    source_declarations: &'a HashMap<SymbolId, Declaration>,
    contract_reads: &'a HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
}

struct LocalAccessReuse<'a> {
    aggregate_reusable: bool,
    typescript_unchanged: bool,
    source_discovery_delta: Option<&'a SourceDiscoveryTypeScriptDelta>,
    changed_source_symbols: &'a HashSet<SymbolId>,
    retained_source_paths: &'a HashSet<String>,
    global_async_context_unchanged: bool,
}

impl LocalAccessContext<'_> {
    fn build(
        &self,
        cache: Option<&mut CachedLocalAccesses>,
        reuse: LocalAccessReuse<'_>,
    ) -> LocalAccessBuild {
        let LocalAccessReuse {
            aggregate_reusable,
            typescript_unchanged,
            source_discovery_delta,
            changed_source_symbols,
            retained_source_paths,
            global_async_context_unchanged,
        } = reuse;
        if aggregate_reusable
            && let Some(cached) = cache.as_deref().and_then(|cache| cache.aggregate.as_ref())
        {
            return LocalAccessBuild {
                result: cached.clone(),
                reused: true,
                reused_files: u64::try_from(self.facts.files.len()).unwrap_or(u64::MAX),
                recomputed_files: 0,
            };
        }

        let mut result = LocalAccessResult::default();
        let mut reused_files = 0;
        let mut recomputed_files = 0;
        if let Some(cache) = cache {
            let exact_typescript_delta = typescript_unchanged || source_discovery_delta.is_some();
            let mut candidate_dependencies = changed_source_symbols.clone();
            if let Some(delta) = source_discovery_delta {
                candidate_dependencies.extend(delta.semantic_symbol_ids.iter().cloned());
            }
            for (symbol, previous) in &cache.prop_sources {
                if self.prop_sources.get(symbol) != Some(previous) {
                    candidate_dependencies.insert(symbol.clone());
                }
            }
            for symbol in self.prop_sources.keys() {
                if !cache.prop_sources.contains_key(symbol) {
                    candidate_dependencies.insert(symbol.clone());
                }
            }
            let changed_dependencies = candidate_dependencies
                .into_iter()
                .filter(|symbol| {
                    cache
                        .dependency_states
                        .get(symbol)
                        .is_some_and(|previous| *previous != self.symbol_state(symbol))
                })
                .collect::<HashSet<_>>();
            let current_paths = self
                .facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache
                .files
                .retain(|path, _| current_paths.contains(path.as_str()));
            for file in &self.facts.files {
                if let Some(cached) = cache.files.get(file.path.as_str())
                    && exact_typescript_delta
                    && self.cached_matches(
                        file,
                        cached,
                        retained_source_paths.contains(file.path.as_str()),
                        &changed_dependencies,
                        global_async_context_unchanged,
                    )
                {
                    reused_files += 1;
                    continue;
                }
                let contribution = self.discover(file);
                cache.files.insert(
                    file.path.clone(),
                    CachedLocalAccessFile {
                        source_hash: file.source_hash.clone(),
                        compiler: file.compiler.clone(),
                        dependencies: self.dependencies(file),
                        call_multiplicities: self.call_multiplicities(file),
                        contribution,
                    },
                );
                recomputed_files += 1;
            }
            let current_dependencies = cache
                .files
                .values()
                .flat_map(|file| file.dependencies.iter().cloned())
                .collect::<HashSet<_>>();
            cache
                .dependency_states
                .retain(|symbol, _| current_dependencies.contains(symbol));
            for symbol in current_dependencies {
                if !cache.dependency_states.contains_key(symbol.as_str())
                    || changed_dependencies.contains(symbol.as_str())
                {
                    cache
                        .dependency_states
                        .insert(symbol.clone(), self.symbol_state(&symbol));
                }
            }
            cache
                .prop_sources
                .retain(|symbol, _| self.prop_sources.contains_key(symbol));
            for (symbol, source) in self.prop_sources {
                if cache.prop_sources.get(symbol) != Some(source) {
                    cache.prop_sources.insert(symbol.clone(), source.clone());
                }
            }
            let local_access_files = &cache.files;
            for partial in parallel_file_chunk_results(&self.facts.files, |files| {
                let mut partial = LocalAccessResult::default();
                for file in files {
                    if let Some(cached) = local_access_files.get(file.path.as_str()) {
                        append_local_access_result(&mut partial, &cached.contribution);
                    }
                }
                partial
            }) {
                append_local_access_result_owned(&mut result, partial);
            }
            cache.aggregate = Some(result.clone());
        } else {
            for file in &self.facts.files {
                let contribution = self.discover(file);
                append_local_access_result(&mut result, &contribution);
                recomputed_files += 1;
            }
        }
        LocalAccessBuild {
            result,
            reused: false,
            reused_files,
            recomputed_files,
        }
    }

    fn dependencies(&self, file: &solid_facts::FileFacts) -> HashSet<SymbolId> {
        file.ast
            .calls
            .iter()
            .map(|call| call.callee)
            .chain(file.ast.members.iter().map(|member| member.object))
            .chain(file.ast.spreads.iter().map(|spread| spread.argument))
            .chain(
                file.ast
                    .jsx_elements
                    .iter()
                    .map(|element| element.name.span),
            )
            .filter_map(|span| {
                self.entities
                    .get(&location(file.path.shared(), span))
                    .cloned()
            })
            .collect()
    }

    fn symbol_state(&self, symbol: &str) -> LocalAccessSymbolState {
        LocalAccessSymbolState {
            accessor: self.accessors.get(symbol).cloned(),
            accessor_origin: self.accessor_origins.get(symbol).cloned(),
            setter: self.setters.get(symbol).cloned(),
            action: self.actions.get(symbol).cloned(),
            source_primitive: self.source_primitives.get(symbol).cloned(),
            async_source: self.async_sources.contains(symbol),
            contract_reads: self.contract_reads.get(symbol).cloned(),
            source_kind: self.source_kinds.get(symbol).copied(),
            prop_source: self.prop_sources.get(symbol).cloned(),
            source_declaration: self.source_declarations.get(symbol).cloned(),
            symbol_name: self.symbol_names.get(symbol).cloned(),
        }
    }

    fn call_multiplicities(&self, file: &solid_facts::FileFacts) -> Vec<(Location, Option<usize>)> {
        file.ast
            .calls
            .iter()
            .map(|call| {
                let callee = location(file.path.shared(), call.callee);
                let multiplicity = self.reachable_calls.get(&callee).copied();
                (callee, multiplicity)
            })
            .collect()
    }

    fn cached_matches(
        &self,
        file: &solid_facts::FileFacts,
        cached: &CachedLocalAccessFile,
        retained_source_path: bool,
        changed_dependencies: &HashSet<SymbolId>,
        global_async_context_unchanged: bool,
    ) -> bool {
        retained_source_path
            && cached.source_hash == file.source_hash
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
            && (global_async_context_unchanged || cached.contribution.async_reads.is_empty())
            && cached.dependencies.is_disjoint(changed_dependencies)
            && cached
                .call_multiplicities
                .iter()
                .all(|(callee, previous)| self.reachable_calls.get(callee).copied() == *previous)
    }

    fn discover(&self, file: &solid_facts::FileFacts) -> LocalAccessResult {
        let mut result = LocalAccessResult::default();
        let mut seen = HashSet::new();
        let allowed = allowed_callback_spans(file, self.lookup);
        for call in &file.ast.calls {
            let callee = location(file.path.shared(), call.callee);
            let Some(symbol) = self.lookup.callee_symbol(file, call.callee) else {
                continue;
            };
            let inside_function = file.ast.any_function_body_containing(call.span);
            if inside_function && self.setters.contains_key(symbol) {
                result.write_action_obligations.insert((
                    "write",
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                ));
            }
            if inside_function && self.actions.contains_key(symbol) {
                result.write_action_obligations.insert((
                    "action",
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                ));
            }
            let execution = semantic_execution_role(
                file,
                call.callee,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            let typed_effect_accessor = execution == ExecutionRole::EffectApply
                && call.arguments.is_empty()
                && typed_accessor_descriptor_at(self.lookup, file.path.as_str(), call.callee)
                    .is_some();
            let Some(multiplicity) = self.reachable_calls.get(&callee).copied().or_else(|| {
                (typed_effect_accessor
                    || (self.accessors.contains_key(symbol)
                        && (execution == ExecutionRole::EffectApply
                            || control_flow_execution_role(
                                file,
                                call.callee,
                                self.entities,
                                self.symbol_names,
                            )
                            .is_some()
                            || named_callback_execution_role(
                                file,
                                call.callee,
                                self.entities,
                                self.symbol_names,
                                self.lookup,
                            )
                            .is_some()
                            || enclosing_render_function(file, call.callee))))
                .then_some(1)
            }) else {
                continue;
            };
            let key = (callee.path.clone(), callee.start_byte, callee.end_byte);
            if let Some((name, declaration)) = self.accessors.get(symbol)
                && (!inside_lowercase_named_function(file, call.callee)
                    || named_callback_execution_role(
                        file,
                        call.callee,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                    .is_some())
                && seen.insert(key.clone())
            {
                let origin = self.accessor_origins.get(symbol);
                let display_name = call.static_callee(&file.source).unwrap_or(name);
                result.reads.push(Arc::new(ReactiveRead {
                    kind: "accessor".into(),
                    accessor: origin
                        .map_or_else(|| display_name.to_string(), |origin| origin.0.to_string())
                        .into(),
                    location: location(file.path.shared(), call.span),
                    declaration: origin
                        .map_or_else(|| declaration.clone(), |origin| origin.2.clone()),
                    execution,
                    context: read_analysis_context(file, call.span, execution).into(),
                    via: origin.map_or_else(String::new, |_| name.to_string()).into(),
                    origin: origin.map(|origin| origin.2.clone()),
                    origin_context: origin
                        .map_or_else(String::new, |origin| origin.1.to_string())
                        .into(),
                }));
                if !matches!(
                    self.source_primitives.get(symbol).map(SymbolId::as_str),
                    Some("createOptimistic" | "createOptimisticStore")
                ) && counts_as_strict_read_root(file, call.span, execution)
                {
                    result.strict_read_obligations += 1;
                }
                if self.async_sources.contains(symbol) {
                    let async_execution = async_execution_role(file, call.callee, execution);
                    result.async_reads.push(Arc::new(AsyncRead {
                        accessor: format!("{name}()").into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution: async_execution,
                        leaf_owner: containing_leaf_owner(
                            file,
                            call.callee,
                            self.entities,
                            self.symbol_names,
                        )
                        .map(Into::into),
                        under_loading: read_is_under_loading(
                            self.lookup,
                            file,
                            call.callee,
                            self.symbol_names,
                        ),
                    }));
                }
            }
            if !self.accessors.contains_key(symbol)
                && execution == ExecutionRole::EffectApply
                && call.arguments.is_empty()
                && let Some(descriptor) =
                    typed_accessor_descriptor_at(self.lookup, file.path.as_str(), call.callee)
                && seen.insert(key.clone())
            {
                let display = usize::try_from(call.callee.start)
                    .ok()
                    .zip(usize::try_from(call.callee.end).ok())
                    .and_then(|(start, end)| file.source.get(start..end))
                    .unwrap_or("accessor")
                    .to_string();
                let declaration = descriptor.alias_declarations.first().map_or_else(
                    || callee.clone(),
                    |declaration| declaration.location.clone(),
                );
                result.reads.push(Arc::new(ReactiveRead {
                    kind: "accessor".into(),
                    accessor: display.into(),
                    location: location(file.path.shared(), call.span),
                    declaration,
                    execution,
                    context: read_analysis_context(file, call.span, execution).into(),
                    via: Arc::from(""),
                    origin: None,
                    origin_context: Arc::from(""),
                }));
                result.strict_read_obligations += 1;
            }
            if let Some(contracted) = self.contract_reads.get(symbol)
                && !inside_lowercase_named_function(file, call.callee)
            {
                for (index, (name, via, declaration, kind)) in contracted.iter().enumerate() {
                    let contract_key = (
                        callee.path.clone(),
                        callee.start_byte,
                        callee
                            .end_byte
                            .saturating_add(u64::try_from(index).unwrap_or(u64::MAX)),
                    );
                    if seen.insert(contract_key) {
                        result.reads.push(Arc::new(ReactiveRead {
                            kind: kind.clone().into(),
                            accessor: name.clone().into(),
                            location: location(file.path.shared(), call.span),
                            declaration: declaration.clone(),
                            execution,
                            context: read_analysis_context(file, call.span, execution).into(),
                            via: via.clone().into(),
                            origin: Some(declaration.clone()),
                            origin_context: via.clone().into(),
                        }));
                        if counts_as_strict_read_root(file, call.span, execution) {
                            result.strict_read_obligations += 1;
                        }
                    }
                }
            }
            if let Some((name, declaration, allowed_by_option)) = self.setters.get(symbol) {
                for _ in 0..multiplicity {
                    result.writes.push(Arc::new(ReactiveWrite {
                        setter: name.to_string().into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution,
                        allowed_by_option: *allowed_by_option,
                        context: analysis_context(
                            file,
                            call.span,
                            self.entities,
                            self.symbol_names,
                        )
                        .into(),
                    }));
                }
            }
            if let Some((name, declaration)) = self.actions.get(symbol) {
                for _ in 0..multiplicity {
                    result.action_invocations.push(Arc::new(ActionInvocation {
                        action: name.to_string().into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution,
                        context: analysis_context(
                            file,
                            call.span,
                            self.entities,
                            self.symbol_names,
                        )
                        .into(),
                    }));
                }
            }
        }
        for member in &file.ast.members {
            if file
                .ast
                .members
                .iter()
                .any(|candidate| candidate.object == member.span)
            {
                continue;
            }
            let object = location(file.path.shared(), member.object);
            let Some(symbol) = self.entities.get(&object) else {
                continue;
            };
            let execution = semantic_execution_role(
                file,
                member.span,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if (inside_lowercase_named_function(file, member.span)
                || inside_unclassified_callback(file, member.span))
                && named_callback_execution_role(
                    file,
                    member.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .is_none()
                && execution != ExecutionRole::EffectApply
            {
                continue;
            }
            let source = if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                self.accessors.get(symbol)
            } else {
                self.prop_sources.get(symbol)
            };
            let Some((name, declaration)) = source else {
                continue;
            };
            let key = (object.path.clone(), object.start_byte, object.end_byte);
            if !seen.insert(key) {
                continue;
            }
            let accessor = usize::try_from(member.span.start)
                .ok()
                .zip(usize::try_from(member.span.end).ok())
                .and_then(|(start, end)| file.source.get(start..end))
                .and_then(|path| {
                    path.find('.')
                        .map(|index| format!("{name}{}", &path[index..]))
                })
                .unwrap_or_else(|| {
                    format!(
                        "{name}.{}",
                        file.source_text(member.property).unwrap_or_default()
                    )
                });
            result.reads.push(Arc::new(ReactiveRead {
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: accessor.into(),
                location: location(file.path.shared(), member.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, member.span, execution).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
            }));
            if !matches!(
                self.source_primitives.get(symbol).map(SymbolId::as_str),
                Some("createOptimistic" | "createOptimisticStore")
            ) && counts_as_strict_read_root(file, member.span, execution)
            {
                result.strict_read_obligations += 1;
            }
            if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
                && self.async_sources.contains(symbol)
            {
                let async_execution = async_execution_role(file, member.span, execution);
                result.async_reads.push(Arc::new(AsyncRead {
                    accessor: format!(
                        "{name}.{}",
                        file.source_text(member.property).unwrap_or_default()
                    )
                    .into(),
                    location: location(file.path.shared(), member.span),
                    declaration: declaration.clone(),
                    execution: async_execution,
                    leaf_owner: containing_leaf_owner(
                        file,
                        member.span,
                        self.entities,
                        self.symbol_names,
                    )
                    .map(Into::into),
                    under_loading: read_is_under_loading(
                        self.lookup,
                        file,
                        member.span,
                        self.symbol_names,
                    ),
                }));
            }
        }
        for spread in &file.ast.spreads {
            let argument = location(file.path.shared(), spread.argument);
            let Some(symbol) = self.entities.get(&argument) else {
                continue;
            };
            let execution = semantic_execution_role(
                file,
                spread.span,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if (inside_lowercase_named_function(file, spread.span)
                || inside_unclassified_callback(file, spread.span))
                && named_callback_execution_role(
                    file,
                    spread.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .is_none()
                && execution != ExecutionRole::EffectApply
            {
                continue;
            }
            let source = if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                self.accessors.get(symbol)
            } else {
                self.prop_sources.get(symbol)
            };
            let Some((name, declaration)) = source else {
                continue;
            };
            result.reads.push(Arc::new(ReactiveRead {
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: format!("{name} spread").into(),
                location: location(file.path.shared(), spread.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, spread.span, execution).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
            }));
            if !matches!(
                self.source_primitives.get(symbol).map(SymbolId::as_str),
                Some("createOptimistic" | "createOptimisticStore")
            ) && counts_as_strict_read_root(file, spread.span, execution)
            {
                result.strict_read_obligations += 1;
            }
        }
        for element in &file.ast.jsx_elements {
            let name_location = location(file.path.shared(), element.name.span);
            let Some(symbol) = self.entities.get(&name_location) else {
                continue;
            };
            if !self.async_sources.contains(symbol)
                || self.source_primitives.get(symbol).map(SymbolId::as_str) != Some("dynamic")
            {
                continue;
            }
            let execution = ExecutionRole::TrackedJsx;
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!(
                    "<{}>",
                    file.source_text(element.name.span).unwrap_or_default()
                )
                .into(),
                location: location(file.path.shared(), element.span),
                declaration: self.source_declarations.get(symbol).map_or_else(
                    || name_location.clone(),
                    |declaration| declaration.location.clone(),
                ),
                execution,
                leaf_owner: containing_leaf_owner(
                    file,
                    element.name.span,
                    self.entities,
                    self.symbol_names,
                )
                .map(Into::into),
                under_loading: read_is_under_loading(
                    self.lookup,
                    file,
                    element.name.span,
                    self.symbol_names,
                ),
            }));
        }
        result
    }
}

fn append_local_access_result(target: &mut LocalAccessResult, source: &LocalAccessResult) {
    target.reads.extend(source.reads.iter().cloned());
    target.writes.extend(source.writes.iter().cloned());
    target
        .action_invocations
        .extend(source.action_invocations.iter().cloned());
    target
        .async_reads
        .extend(source.async_reads.iter().cloned());
    target.strict_read_obligations += source.strict_read_obligations;
    target
        .write_action_obligations
        .extend(source.write_action_obligations.iter().cloned());
}

fn append_local_access_result_owned(target: &mut LocalAccessResult, source: LocalAccessResult) {
    target.reads.extend(source.reads);
    target.writes.extend(source.writes);
    target.action_invocations.extend(source.action_invocations);
    target.async_reads.extend(source.async_reads);
    target.strict_read_obligations += source.strict_read_obligations;
    target
        .write_action_obligations
        .extend(source.write_action_obligations);
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PrimitiveName {
    Action,
    Children,
    CreateEffect,
    CreateMemo,
    CreateOptimistic,
    CreateOptimisticStore,
    CreateOwner,
    CreateProjection,
    CreateReaction,
    CreateRenderEffect,
    CreateRoot,
    CreateSignal,
    CreateStore,
    CreateTrackedEffect,
    Dynamic,
    Flush,
    For,
    Loading,
    MapArray,
    Match,
    OnCleanup,
    OnSettled,
    Repeat,
    Show,
    Switch,
    Untrack,
    Other(String),
}

impl PrimitiveName {
    fn new(name: &str) -> Self {
        match name {
            "action" => Self::Action,
            "children" => Self::Children,
            "createEffect" => Self::CreateEffect,
            "createMemo" => Self::CreateMemo,
            "createOptimistic" => Self::CreateOptimistic,
            "createOptimisticStore" => Self::CreateOptimisticStore,
            "createOwner" => Self::CreateOwner,
            "createProjection" => Self::CreateProjection,
            "createReaction" => Self::CreateReaction,
            "createRenderEffect" => Self::CreateRenderEffect,
            "createRoot" => Self::CreateRoot,
            "createSignal" => Self::CreateSignal,
            "createStore" => Self::CreateStore,
            "createTrackedEffect" => Self::CreateTrackedEffect,
            "dynamic" => Self::Dynamic,
            "flush" => Self::Flush,
            "For" => Self::For,
            "Loading" => Self::Loading,
            "mapArray" => Self::MapArray,
            "Match" => Self::Match,
            "onCleanup" => Self::OnCleanup,
            "onSettled" => Self::OnSettled,
            "Repeat" => Self::Repeat,
            "Show" => Self::Show,
            "Switch" => Self::Switch,
            "untrack" => Self::Untrack,
            _ => Self::Other(name.to_owned()),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Action => "action",
            Self::Children => "children",
            Self::CreateEffect => "createEffect",
            Self::CreateMemo => "createMemo",
            Self::CreateOptimistic => "createOptimistic",
            Self::CreateOptimisticStore => "createOptimisticStore",
            Self::CreateOwner => "createOwner",
            Self::CreateProjection => "createProjection",
            Self::CreateReaction => "createReaction",
            Self::CreateRenderEffect => "createRenderEffect",
            Self::CreateRoot => "createRoot",
            Self::CreateSignal => "createSignal",
            Self::CreateStore => "createStore",
            Self::CreateTrackedEffect => "createTrackedEffect",
            Self::Dynamic => "dynamic",
            Self::Flush => "flush",
            Self::For => "For",
            Self::Loading => "Loading",
            Self::MapArray => "mapArray",
            Self::Match => "Match",
            Self::OnCleanup => "onCleanup",
            Self::OnSettled => "onSettled",
            Self::Repeat => "Repeat",
            Self::Show => "Show",
            Self::Switch => "Switch",
            Self::Untrack => "untrack",
            Self::Other(name) => name,
        }
    }
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
) -> Option<PrimitiveName> {
    let location = location(path, span);
    if let Some(symbol) = entities.get(&location) {
        symbol_names
            .get(symbol)
            .map(|name| PrimitiveName::new(name))
            .or_else(|| {
                let property = static_callee?.rsplit('.').next()?;
                symbol_names
                    .get(format!("{symbol}::{property}").as_str())
                    .map(|name| PrimitiveName::new(name))
            })
    } else {
        None
    }
}

fn jsx_primitive_name(
    file: &solid_facts::FileFacts,
    element: &solid_ast_facts::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> Option<PrimitiveName> {
    primitive_name(
        file.path.as_str(),
        element.name.span,
        Some(file.source_text(element.name.span).unwrap_or_default()),
        entities,
        symbol_names,
    )
    .or_else(|| {
        file.ast
            .imports
            .iter()
            .filter(|import| import.module == "solid-js")
            .flat_map(|import| &import.bindings)
            .find_map(|binding| {
                (binding.kind != solid_ast_facts::ImportKind::Namespace
                    && file.source_text(binding.local.span) == file.source_text(element.name.span))
                .then_some(binding.imported.as_deref())
                .flatten()
            })
            .map(PrimitiveName::new)
    })
}

fn location(path: impl Into<Arc<str>>, span: Span) -> Location {
    Location {
        path: path.into(),
        start_byte: u64::from(span.start),
        end_byte: u64::from(span.end),
    }
}

#[cfg(test)]
mod tests {
    use solid_facts::TypeScriptTable;
    use typefacts::{EntityFact, FileFact, SourceDigest, SymbolFact};

    use super::interproc::InterproceduralResultView;
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
    fn known_primitive_names_use_integer_variants() {
        assert!(matches!(
            PrimitiveName::new("createEffect"),
            PrimitiveName::CreateEffect
        ));
        assert!(matches!(
            PrimitiveName::new("projectSpecificHelper"),
            PrimitiveName::Other(_)
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

    fn summary_node(path: &str, span: Span, body: Span) -> SummaryNode {
        SummaryNode {
            path: path.into(),
            span,
            body,
            name: None,
            symbol: None,
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
            generation: solid_facts_core::Generation::new(generation).unwrap(),
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
            symbol_names: symbol_names(table, &aliases, &interner),
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
        let fresh = build(&first).unwrap();
        let mut incremental = IncrementalBuilder::default();

        let (initial, initial_timings) = incremental.build(&first).unwrap();
        let (reused, reused_timings) = incremental.build(&first).unwrap();
        let mut next_facts = empty_project(2);
        next_facts.typescript_changes = Some(solid_facts::TypeScriptChanges {
            unchanged: true,
            ..solid_facts::TypeScriptChanges::default()
        });
        let (next, next_timings) = incremental.build(&next_facts).unwrap();

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
        let fresh = build(&first).unwrap();
        let mut incremental = IncrementalBuilder::default();

        let (initial, _) = incremental.build(&first).unwrap();
        incremental.retain_for_idle(CacheRetention::Balanced);
        let (same_generation, same_timings) = incremental.build(&first).unwrap();
        incremental.retain_for_idle(CacheRetention::Compact);

        let mut next_facts = empty_project(2);
        next_facts.typescript_changes = Some(solid_facts::TypeScriptChanges {
            unchanged: true,
            ..solid_facts::TypeScriptChanges::default()
        });
        let (next, next_timings) = incremental.build(&next_facts).unwrap();

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

        let (initial, initial_timings) = incremental.build_shared(&facts).unwrap();
        let (reused, reused_timings) = incremental.build_shared(&facts).unwrap();

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
            type_descriptor: None,
            resolved_call: None,
            callability: None,
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
            patch_typescript_indexes(&mut patched, &current, &symbols_by_id, &changes).is_some()
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
            patch_typescript_indexes(&mut patched, &current, &symbols_by_id, &changes).is_some()
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
            patch_typescript_indexes(&mut patched, &current, &symbols_by_id, &changes).is_some()
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
            patch_typescript_indexes(&mut patched, &current, &symbols_by_id, &changes).is_some()
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
            patch_typescript_indexes(&mut patched, &current, &symbols_by_id, &changes).is_some()
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
            patch_typescript_indexes(&mut patched, &current, &symbols_by_id, &changes).is_none()
        );
        assert_eq!(patched.aliases, typescript_index_cache(&old).aliases);
    }

    #[test]
    fn summary_containment_preserves_first_node_order() {
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
            Some(0)
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
        let returned_bindings = HashMap::new();
        let missing = InterproceduralResultDependencyState::Missing;

        let missing_by_symbol = HashMap::new();
        let missing_view = InterproceduralResultView {
            nodes: &nodes,
            indexes: &indexes,
            by_symbol: &missing_by_symbol,
            summaries: &summaries,
            invoked_parameters: &invoked_parameters,
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
