//! Interprocedural reactive-read analysis.
//!
//! Builds the cross-function summary graph, propagates reactive reads through
//! calls/returns/factories, resolves typed accessors, and emits the per-call
//! reactive reads plus the export contract summaries. The orchestrator populates
//! an `InterproceduralContext` parameter object and calls `build` to obtain an
//! `InterproceduralResult`; every stage function here is module-private.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use solid_dialect::{Primitive, TrackedCallbackTiming};
use solid_facts::ProjectFacts;
use solid_facts::core::Span;
use typefacts::{CallKind, Location, ResolvedCallValidity};

use super::runtime_semantics::{
    RuntimeArgumentBehavior, argument_behavior, literal_argument_is_not_callable,
    potentially_callable, proven_array_method_argument_behavior, resolved_parameter,
    retains_argument_value,
};
use super::{
    ContractAnalysis, ContractCallback, ContractExport, ContractGenerationObligation,
    ContractGraph, ContractReturn, ContractSemantics, EntitySymbols, ExecutionRole,
    FunctionBoundary, FunctionLookup, ProjectIndexes, ReactiveRead, ReactiveSourceKind,
    SemanticLookup, StaticDefect, StaticDefectKind, SymbolId, allowed_callback_spans,
    assigned_member_function_contains, containing_summary_function_indexed,
    contract_callback_execution, contract_export_summaries, contract_export_summaries_incremental,
    function_indices_by_path, function_lookup_for_path, functions_for_path,
    items_by_containing_function, location, location_order, primitive_name,
    propagate_returned_summary_deltas, propagate_summary_deltas, push_contract_callback,
    push_unique_summary_read, semantic_execution_role,
};
use crate::cache::{
    CachedInterproceduralGraph, CachedInterproceduralResultFile, CachedInterproceduralResults,
    CachedReactiveSource, CachedTypedAccessors, InterproceduralGraphContribution,
    InterproceduralGraphTarget, InterproceduralResultDependency,
    InterproceduralResultDependencyState, TypedAccessorContribution, same_compiler_semantics,
};
use crate::execution_role::direct_callback_contains;
use crate::owners::{
    containing_ast_function, enclosing_function_label, enclosing_render_function,
    function_binding_name, read_escapes_synchronous_extent, solid_accessor_declaration,
    source_function_exported,
};
use crate::pipeline::{parallel_file_results, parallel_slice_results};
use crate::source_discovery::bundled_contract_location;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SummaryRead {
    pub(super) symbol: SymbolId,
    pub(super) display: SymbolId,
    pub(super) kind: Option<String>,
    pub(super) declaration: Location,
    pub(super) origin: Location,
    pub(super) origin_context: String,
}

struct DirectReferenceContribution {
    owner: usize,
    read: SummaryRead,
    unique: bool,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub(super) struct SummaryReads {
    pub(super) ordered: Vec<SummaryRead>,
    seen: HashSet<(SymbolId, Location, Location)>,
}

impl SummaryReads {
    fn key(read: &SummaryRead) -> (SymbolId, Location, Location) {
        (
            read.display.clone(),
            read.origin.clone(),
            read.declaration.clone(),
        )
    }

    pub(super) fn push(&mut self, read: SummaryRead) {
        self.seen.insert(Self::key(&read));
        self.ordered.push(read);
    }

    pub(super) fn push_unique(&mut self, read: SummaryRead) -> bool {
        if !self.seen.insert(Self::key(&read)) {
            return false;
        }
        self.ordered.push(read);
        true
    }

    pub(super) fn insert(&mut self, index: usize, read: SummaryRead) {
        self.seen.insert(Self::key(&read));
        self.ordered.insert(index, read);
    }

    fn take(&mut self) -> Vec<SummaryRead> {
        self.seen.clear();
        std::mem::take(&mut self.ordered)
    }

    pub(super) fn replace(&mut self, reads: Vec<SummaryRead>) {
        self.seen = reads.iter().map(Self::key).collect();
        self.ordered = reads;
    }

    pub(super) fn to_vec(&self) -> Vec<SummaryRead> {
        self.ordered.clone()
    }
}

/// Compare the semantic effect of two implementations without treating their
/// source locations as behavior. Equivalent structural methods often read the
/// same accessor from different lines; those origins remain useful for a
/// diagnostic, but must not make dispatch fail closed.
///
/// The comparison is over the *set* of distinct reads. Reading one accessor
/// twice is the same effect as reading it once, so a differing repeat count
/// must not fail the gate — but a read only one candidate performs must,
/// which a same-length containment test does not catch: `[a, a]` and
/// `[a, b]` are the same length and every member of the first appears in the
/// second. The caller unions every candidate's reads once this returns true,
/// so anything short of set equality would attribute an unproven read.
fn equivalent_summary_reads(left: &SummaryReads, right: &SummaryReads) -> bool {
    fn effect(reads: &SummaryReads) -> HashSet<(&SymbolId, &SymbolId, Option<&str>, &Location)> {
        reads
            .iter()
            .map(|read| {
                (
                    &read.symbol,
                    &read.display,
                    read.kind.as_deref(),
                    &read.declaration,
                )
            })
            .collect()
    }
    effect(left) == effect(right)
}

/// Set equality over callback timing, for the same reason as
/// [`equivalent_summary_reads`]: repeating one parameter's timing is not a
/// different effect, but a parameter only one candidate defers is.
fn equivalent_callbacks(left: &[ContractCallback], right: &[ContractCallback]) -> bool {
    fn effect(callbacks: &[ContractCallback]) -> HashSet<(usize, &str, Option<&str>, String)> {
        callbacks
            .iter()
            .map(|callback| {
                (
                    callback.parameter,
                    callback.execution.as_str(),
                    callback.owner.as_deref(),
                    serde_json::to_string(&callback.arguments)
                        .expect("contract callback arguments are JSON-safe"),
                )
            })
            .collect()
    }
    effect(left) == effect(right)
}

impl std::ops::Deref for SummaryReads {
    type Target = [SummaryRead];

    fn deref(&self) -> &Self::Target {
        &self.ordered
    }
}

#[derive(Clone)]
pub(super) struct SummaryNode {
    pub(super) path: String,
    pub(super) span: Span,
    pub(super) body: Span,
    pub(super) name: Option<String>,
    pub(super) symbol: Option<SymbolId>,
    pub(super) runtime_identity: String,
    pub(super) parameters: Vec<SymbolId>,
    pub(super) exported: bool,
    pub(super) r#async: bool,
}

impl FunctionBoundary for SummaryNode {
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Span {
        self.body
    }
}

#[derive(Clone)]
pub(super) struct InterproceduralResult {
    pub(super) reads: Arc<[ReactiveRead]>,
    pub(super) dispatch_obligations: Arc<[StaticDefect]>,
    pub(super) exports: Arc<BTreeMap<String, ContractExport>>,
    pub(super) contract_generation_obligations: Arc<[ContractGenerationObligation]>,
    pub(super) factory_instances: usize,
    pub(super) timings: InterproceduralTimings,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct InterproceduralTimings {
    pub(super) graph: Duration,
    pub(super) direct_summaries: Duration,
    pub(super) direct_index: Duration,
    pub(super) direct_references: Duration,
    pub(super) typed_accessors: Duration,
    pub(super) propagation: Duration,
    pub(super) returned_direct: Duration,
    pub(super) returned_delta: Duration,
    pub(super) call_summary_delta: Duration,
    pub(super) factory_propagation: Duration,
    pub(super) results_and_exports: Duration,
    pub(super) result_reads: Duration,
    pub(super) export_summaries: Duration,
    pub(super) typed_accessor_reused_files: u64,
    pub(super) typed_accessor_recomputed_files: u64,
    pub(super) graph_reused_files: u64,
    pub(super) graph_recomputed_files: u64,
    pub(super) result_reused_files: u64,
    pub(super) result_recomputed_files: u64,
}

fn discover_typed_accessors(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    project_indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Vec<TypedAccessorContribution> {
    let dialect = lookup.dialect;
    let path_entities = project_indexes.entities_for_path(file.path.as_str());
    let mut contributions = Vec::new();
    for call in &file.ast.calls {
        let callee_location = location(file.path.shared(), call.callee);
        let descriptor = path_entities
            .iter()
            .find(|entity| {
                entity.location.start_byte == callee_location.start_byte
                    && entity.location.end_byte == callee_location.end_byte
            })
            .and_then(|entity| entity.type_descriptor.as_ref());
        let Some(accessor_declaration) =
            descriptor.and_then(|descriptor| solid_accessor_declaration(descriptor, dialect))
        else {
            continue;
        };
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            call.callee,
        ) else {
            continue;
        };
        if read_escapes_synchronous_extent(file, call.callee, entities, symbol_names, dialect)
            || enclosing_render_function(file, call.callee, lookup)
        {
            continue;
        }
        let call_location = location(file.path.shared(), call.span);
        let display = usize::try_from(call.callee.start)
            .ok()
            .zip(usize::try_from(call.callee.end).ok())
            .and_then(|(start, end)| file.source.get(start..end))
            .map(SymbolId::from)
            .unwrap_or_else(|| "accessor".into());
        let declaration = accessor_declaration.location.clone();
        contributions.push(TypedAccessorContribution {
            owner: nodes[owner].span,
            read: SummaryRead {
                symbol: SymbolId::from(format!(
                    "typed:{}\0{}\0{}",
                    call_location.path, call_location.start_byte, call_location.end_byte
                )),
                display,
                kind: Some("accessor".into()),
                declaration,
                origin: call_location,
                origin_context: nodes[owner].name.clone().unwrap_or_default(),
            },
        });
    }
    contributions
}

fn merge_typed_accessors(
    path: &str,
    contributions: &[TypedAccessorContribution],
    indexes: &HashMap<(String, Span), usize>,
    summaries: &mut [SummaryReads],
) {
    for contribution in contributions {
        let Some(owner) = indexes
            .get(&(path.to_string(), contribution.owner))
            .copied()
        else {
            continue;
        };
        if summaries[owner]
            .iter()
            .any(|read| read.origin == contribution.read.origin)
        {
            continue;
        }
        let insertion = summaries[owner]
            .iter()
            .position(|existing| existing.origin.path.starts_with("bundled://"))
            .unwrap_or(summaries[owner].len());
        summaries[owner].insert(insertion, contribution.read.clone());
    }
}

fn discover_summary_nodes(
    file: &solid_facts::FileFacts,
    project_indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
) -> Vec<SummaryNode> {
    let mut nodes = Vec::new();
    let typescript_file = project_indexes.typescript_file(file.path.as_str());
    for arrow in [false, true] {
        for function in &file.ast.functions {
            let binding_name = function_binding_name(file, function);
            let source_function = typescript_file.and_then(|typescript_file| {
                typescript_file.functions.iter().find(|candidate| {
                    candidate.body.start_byte == u64::from(function.body.start)
                        && (candidate.body.end_byte.saturating_add(1)
                            == u64::from(function.body.end)
                            || (function.expression_body
                                && candidate.body.end_byte == u64::from(function.body.end)))
                })
            });
            // Preserve the checker's finite function universe and ordering:
            // declarations first, then arrows, each in source order. Structural
            // function facts cover named/assigned expressions and
            // expression-bodied arrows, so a missing fact is genuinely outside
            // the project function universe.
            if typescript_file.is_some()
                && source_function.is_none()
                && function.method_name.is_none()
            {
                continue;
            }
            let is_arrow = source_function.map_or(
                function.kind == solid_facts::ast::FunctionKind::Arrow,
                |function| function.arrow,
            );
            if is_arrow != arrow {
                continue;
            }
            let symbol = binding_name
                .as_ref()
                .and_then(|name| {
                    entities
                        .get(&location(file.path.shared(), name.span))
                        .cloned()
                })
                .or_else(|| {
                    function.method_name.as_ref().and_then(|name| {
                        project_indexes.method_symbol(file.path.as_str(), name.span)
                    })
                });
            let runtime_identity = binding_name
                .as_ref()
                .and_then(|name| {
                    project_indexes
                        .entities_for_path(file.path.as_str())
                        .iter()
                        .find(|entity| {
                            entity.location.start_byte == u64::from(name.span.start)
                                && entity.location.end_byte == u64::from(name.span.end)
                        })
                })
                .map_or_else(String::new, |entity| entity.runtime_identity.to_string());
            let parameters = function
                .parameters
                .iter()
                .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
                .filter_map(|parameter| parameter.names.first())
                .filter_map(|name| {
                    entities
                        .get(&location(file.path.shared(), name.span))
                        .cloned()
                })
                .collect();
            nodes.push(SummaryNode {
                path: file.path.to_string(),
                span: function.span,
                body: function.body,
                name: binding_name
                    .as_ref()
                    .map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
                symbol,
                runtime_identity,
                parameters,
                exported: source_function.map_or_else(
                    || source_function_exported(project_indexes, file, function),
                    |function| function.exported,
                ),
                r#async: source_function.map_or(function.r#async, |function| function.r#async),
            });
        }
    }
    nodes
}

struct InterproceduralContracts<'a> {
    reads: &'a HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    parameter_reads: &'a HashMap<SymbolId, Vec<(usize, String, String, Location)>>,
    callbacks: &'a HashMap<SymbolId, Vec<ContractCallback>>,
}

fn unknown_callback_execution_message(
    file: &solid_facts::FileFacts,
    callee: Span,
    parameter: usize,
    lookup: &SemanticLookup<'_>,
) -> String {
    let target = file.source_text(callee).unwrap_or("<unknown>");
    let Some(resolved) = lookup.resolved_callee_call(file, callee) else {
        return format!(
            "parameter {parameter} is passed to {target}, but no resolved call fact or package contract proves when it executes"
        );
    };
    let declaration = resolved.declaration.as_ref();
    let callable_type = resolved_parameter(resolved, parameter)
        .and_then(|parameter| parameter.type_descriptor.as_ref())
        .map(|descriptor| descriptor.text.as_ref())
        .filter(|text| !text.is_empty())
        .unwrap_or("callable value");
    let qualified = declaration
        .and_then(|declaration| {
            (!declaration.qualified_name.is_empty()).then_some(declaration.qualified_name.as_ref())
        })
        .or_else(|| {
            declaration.and_then(|declaration| {
                (!declaration.name.is_empty()).then_some(declaration.name.as_ref())
            })
        })
        .unwrap_or(target);
    let package = declaration
        .and_then(|declaration| {
            (!declaration.origin_module.is_empty()).then_some(declaration.origin_module.as_ref())
        })
        .or_else(|| {
            declaration.and_then(|declaration| {
                (!declaration.source_file.is_empty()).then_some(declaration.source_file.as_ref())
            })
        })
        .unwrap_or("the current project");
    format!(
        "parameter {parameter} ({callable_type}) is passed to resolved {qualified} from {package}, but no package contract proves when it executes"
    )
}

fn package_entrypoint(module: &str) -> (String, String) {
    let mut parts = module.split('/');
    let first = parts.next().filter(|part| !part.is_empty());
    let Some(first) = first else {
        return ("current project".into(), ".".into());
    };
    let package = if first.starts_with('@') {
        let Some(scope_package) = parts.next() else {
            return (module.into(), ".".into());
        };
        format!("{first}/{scope_package}")
    } else {
        first.to_owned()
    };
    let entrypoint = parts.collect::<Vec<_>>().join("/");
    if entrypoint.is_empty() {
        (package, ".".into())
    } else {
        (package, format!("./{entrypoint}"))
    }
}

fn unknown_callback_obligation(
    file: &solid_facts::FileFacts,
    node: &SummaryNode,
    callee: Span,
    parameter: usize,
    location: Location,
    lookup: &SemanticLookup<'_>,
) -> ContractGenerationObligation {
    let resolved = lookup.resolved_callee_call(file, callee);
    let declaration = resolved.and_then(|call| call.declaration.as_ref());
    let module = declaration
        .and_then(|declaration| {
            (!declaration.origin_module.is_empty()).then_some(declaration.origin_module.as_ref())
        })
        .unwrap_or_default();
    let (package, entrypoint) = package_entrypoint(module);
    let parameter_type = resolved
        .and_then(|call| resolved_parameter(call, parameter))
        .and_then(|parameter| parameter.type_descriptor.as_ref())
        .map(|descriptor| descriptor.text.to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "callable value".into());
    let function = node.name.clone().unwrap_or_else(|| "<anonymous>".into());
    let contract_stub = serde_json::json!({
        "schemaVersion": 1,
        "package": { "name": package, "version": "<exact-installed-version>" },
        "compilerFactsProtocol": 1,
        "summaries": {
            "callback-stub": {
                "kind": "function",
                "callbacks": [{
                    "parameter": parameter,
                    "execution": "<choose: inline | tracked | deferred>"
                }]
            }
        },
        "entrypoints": {
            entrypoint.clone(): { "exports": { "callback-stub": [function.clone()] } }
        },
        "evidence": {
            "kind": "<set reviewed after auditing runtime behavior>",
            "generator": "solid-checker unknown-callback"
        }
    })
    .to_string();
    ContractGenerationObligation {
        function,
        function_identity: node.runtime_identity.clone(),
        parameter,
        package,
        entrypoint,
        parameter_type,
        required_execution: "choose exactly one audited mode: inline, tracked, or deferred".into(),
        contract_stub,
        location,
        message: format!(
            "{}; generate or add the exact package entrypoint/export contract before this callback can be certified",
            unknown_callback_execution_message(file, callee, parameter, lookup)
        ),
    }
}

fn callback_argument_contracts(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> Vec<Option<ContractReturn>> {
    let mut arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let symbol = entities.get(&location(file.path.shared(), argument.span))?;
            let (display, _) = accessors.get(symbol)?;
            Some(ContractReturn {
                kind: "accessor".into(),
                label: display.to_string(),
                ..ContractReturn::default()
            })
        })
        .collect::<Vec<_>>();
    while arguments.last().is_some_and(Option::is_none) {
        arguments.pop();
    }
    arguments
}

/// Whether a contract callback row carries argument descriptors that source
/// discovery cannot bind at this call site.
///
/// A callback `arguments` row claims "this helper hands your callback a
/// reactive value at parameter N". Source discovery materializes such a claim
/// in exactly one shape: an inline function literal whose span *is* the
/// argument, carrying an `accessor` descriptor. Every other schema-valid shape
/// — a named function passed by reference, a `store-path`/`tuple`/`object`/
/// `argument` descriptor — has no binding it can create, and dropping the
/// claim silently would analyze the callback body as if the parameter were
/// ordinary data. That is the fail-open direction, so the call site keeps an
/// explicit obligation instead.
///
/// A descriptor beyond the literal's declared parameters is *not* unbound only
/// when the literal can be *proven* blind to that argument slot, which takes
/// two facts together:
///
/// - it is an arrow function, so there is no `arguments` object to read the
///   slot through — `function (first) { arguments[1] }` observes argument 1
///   without declaring it;
/// - it declares no rest parameter, which is not one of `parameters` and
///   absorbs every argument from `parameters.len()` onward —
///   `(...args) => args[1]` likewise observes argument 1 without declaring it.
///
/// Only `index => …` — an arrow with a restless parameter list shorter than the
/// descriptor row — is genuinely unable to depend on the described argument.
/// Everything else fails closed.
fn contract_callback_arguments_unbound(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    callback: &ContractCallback,
) -> bool {
    if callback.arguments.iter().all(Option::is_none) {
        return false;
    }
    let Some(function) = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == file.ast.peel_ts_sugar_span(argument.span))
    else {
        return true;
    };
    // A non-arrow literal reaches every argument through `arguments`, and a
    // rest parameter reaches every argument past the declared list, so neither
    // can be shown blind to any descriptor slot.
    let observes_every_argument =
        function.kind != solid_facts::ast::FunctionKind::Arrow || function.rest_parameter;
    callback
        .arguments
        .iter()
        .enumerate()
        .any(|(index, descriptor)| {
            descriptor.as_ref().is_some_and(|descriptor| {
                descriptor.kind != "accessor"
                    && (observes_every_argument
                        || function
                            .parameters
                            .get(index)
                            .is_some_and(|parameter| !parameter.names.is_empty()))
            })
        })
}

/// Parameters whose caller-supplied value this function *retains*.
///
/// The call loop below classifies every *argument* position, and a call into a
/// local function is summarized transitively — on the assumption that the
/// callee's own summary accounts for what it does with that parameter. That
/// assumption fails the moment the callee merely **stores** the value:
///
/// ```js
/// function createComputation(fn, init) {          // solid-js 1.9.14
///   const c = { fn, value: init, /* … */ };       // fn is retained, never called
///   return c;
/// }
/// ```
///
/// `createComputation` invokes nothing, so its callback summary is empty, so
/// every export that forwards a callback into it — `createMemo`,
/// `createEffect`, `children`, `createSelector`, `createDeferred`,
/// `createRenderEffect`, `createComputed` — published *no* callbacks row. An
/// omitted `callbacks` list is a **negative** claim, so those contracts
/// certified "invokes no caller-supplied function" for the seven primitives
/// whose whole purpose is to invoke one, and `contract probe`'s discovery pass
/// contradicted every one of them.
///
/// Retention is stated as a closed list of positions, never as "everything the
/// analysis did not recognize". The difference is the whole precision budget:
/// a published runtime artifact is dense with references that *observe* a
/// parameter — `typeof value === "string"`, `prev && …`, `for (const key in
/// props)`, `value[HREF]`, `node[name] = value`, a reassignment of the
/// parameter itself — and treating those as escapes turns a third of a DOM
/// package's exports into sentinels while proving nothing. The positions that
/// do put the value somewhere this analysis stops following are:
///
/// - an **object-literal property value** (`{ fn }`, `{ fn: p }`) — the
///   literal outlives the call, and whoever receives it may invoke the
///   property. This is exactly `createComputation`;
/// - an **assignment value** (`source = pSource`) — stored into a binding or a
///   container whose later use this summary does not track. Storing into a
///   member chain rooted at one of the *caller's own* parameters is excluded:
///   `node.className = value` writes into a container the caller supplied and
///   the caller's code is analyzed too;
/// - a **computed read of a rest parameter** (`sources[index]`) — a rest
///   parameter absorbs an unbounded argument tail that no `callbacks` row can
///   name, so anything at all happening to one of its elements is unstatable
///   in schema v1 as anything but the sentinel.
///
/// Only *potentially callable* parameters can escape into a callback claim, so
/// a parameter the type facts prove non-callable never opens the sentinel.
fn push_unaccounted_parameter_escapes(
    contribution: &mut InterproceduralGraphContribution,
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
) {
    let owners = parameter_declaration_owners(file, nodes, nodes_by_path, entities);
    if owners.is_empty() {
        return;
    }
    let mut retained = HashSet::new();
    for property in file
        .ast
        .object_properties
        .iter()
        .filter(|property| property.data && !property.computed)
    {
        retained.insert(file.ast.peel_ts_sugar_span(property.value));
        retained.insert(property.value);
    }
    for assignment in &file.ast.assignments {
        if member_root_is_parameter(file, assignment.target, &owners) {
            continue;
        }
        retained.insert(file.ast.peel_ts_sugar_span(assignment.value_span));
        retained.insert(assignment.value_span);
    }
    for member in file.ast.members.iter().filter(|member| {
        file.ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
    }) {
        if owners_rest_parameter(file, member.object, &owners) {
            retained.insert(member.object);
        }
    }
    let mut seen = HashSet::new();
    for identifier in &file.ast.identifiers {
        if identifier.role != solid_facts::ast::IdentifierRole::Reference
            || !retained.contains(&identifier.span)
        {
            continue;
        }
        let Some(declaration) = file.ast.reference_declaration(identifier.span) else {
            continue;
        };
        let Some(&(owner, parameter)) = owners.get(&declaration) else {
            continue;
        };
        if !seen.insert((owner, parameter)) {
            continue;
        }
        if !potentially_callable(
            lookup.smallest_contained_callability(file.path.as_str(), identifier.span),
        ) {
            continue;
        }
        contribution
            .escaped_parameters
            .push((nodes[owner].span, parameter));
    }
}

/// Whether `object` is a reference to a rest parameter of a summarized
/// function.
fn owners_rest_parameter(
    file: &solid_facts::FileFacts,
    object: Span,
    owners: &HashMap<Span, (usize, usize)>,
) -> bool {
    file.ast
        .reference_declaration(object)
        .and_then(|declaration| owners.get(&declaration))
        .is_some_and(|(_, parameter)| *parameter == REST_PARAMETER)
}

/// Whether `target` is a member chain rooted at one of this file's summarized
/// parameters — a container the caller handed in.
fn member_root_is_parameter(
    file: &solid_facts::FileFacts,
    target: Span,
    owners: &HashMap<Span, (usize, usize)>,
) -> bool {
    let mut current = target;
    for _ in 0..32 {
        let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| member.span == current)
        else {
            break;
        };
        current = member.object;
    }
    if current == target {
        return false;
    }
    file.ast
        .reference_declaration(current)
        .is_some_and(|declaration| owners.contains_key(&declaration))
}

/// The slot a rest parameter's escape is recorded under.
///
/// A rest parameter absorbs an unbounded tail of argument positions and has no
/// single index, so it is deliberately absent from
/// [`SummaryNode::parameters`]. An escape through it still has to reach the
/// callbacks domain, and this index — which no real slot can take — is how it
/// travels without ever being mistaken for a stated parameter.
const REST_PARAMETER: usize = usize::MAX;

/// Every summarized parameter's declaration span, and the slot it fills.
///
/// The slot indices have to be the ones the rest of the pipeline uses, so the
/// filter chain here is exactly [`function_nodes`]': identifier-shaped
/// parameters whose binding name carries a compiler entity, in declaration
/// order. A rest parameter fills no single slot and is recorded under
/// [`REST_PARAMETER`].
fn parameter_declaration_owners(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    entities: &EntitySymbols,
) -> HashMap<Span, (usize, usize)> {
    let mut owners = HashMap::new();
    for (index, node) in functions_for_path(nodes, nodes_by_path, file.path.as_str()) {
        let Some(function) = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == node.span)
        else {
            continue;
        };
        let spans = function
            .parameters
            .iter()
            .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
            .filter_map(|parameter| parameter.names.first())
            .filter(|name| {
                entities
                    .get(&location(file.path.shared(), name.span))
                    .is_some()
            })
            .map(|name| name.span);
        for (parameter, span) in spans.enumerate() {
            owners.entry(span).or_insert((index, parameter));
        }
        for name in &function.rest_parameter_names {
            owners.entry(name.span).or_insert((index, REST_PARAMETER));
        }
    }
    owners
}

/// Record an unknown-callback obligation for every caller-supplied callable
/// forwarded into a call whose callee has no resolvable identity at all.
///
/// The obligation is only meaningful for an argument that is a parameter of
/// an enclosing analyzed function: that is the value the *caller* supplies,
/// and therefore the one a package contract has to describe. A local value or
/// an inline literal invoked by an unresolved callee is the callee's business,
/// not the exported surface's.
fn push_unresolved_callee_callback_obligations(
    contribution: &mut InterproceduralGraphContribution,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    nodes: &[SummaryNode],
    function_lookup: &FunctionLookup,
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
) {
    for argument in &call.arguments {
        if argument.value != solid_facts::ast::ArgumentValueKind::Identifier {
            continue;
        }
        if !potentially_callable(
            lookup.smallest_contained_callability(file.path.as_str(), argument.span),
        ) || literal_argument_is_not_callable(argument.runtime_value_kind)
        {
            continue;
        }
        let Some(argument_symbol) = entities.get(&location(file.path.shared(), argument.span))
        else {
            continue;
        };
        let Some(&(callback_owner, parameter)) =
            function_lookup.parameter_owner.get(argument_symbol)
        else {
            continue;
        };
        contribution.contract_generation_obligations.push((
            nodes[callback_owner].span,
            unknown_callback_obligation(
                file,
                &nodes[callback_owner],
                call.callee,
                parameter,
                location(file.path.shared(), argument.span),
                lookup,
            ),
        ));
    }
}

#[derive(Clone, Copy)]
struct InterproceduralGraphSymbols<'a> {
    entities: &'a EntitySymbols,
    names: &'a HashMap<SymbolId, SymbolId>,
}

fn discover_interprocedural_graph(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    symbols: InterproceduralGraphSymbols<'_>,
    contracts: InterproceduralContracts<'_>,
    lookup: &SemanticLookup<'_>,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> InterproceduralGraphContribution {
    let mut contribution = InterproceduralGraphContribution::default();
    let primitives = lookup.primitives(file);
    let allowed = allowed_callback_spans(file, lookup);
    let function_lookup = function_lookup_for_path(nodes, nodes_by_path, file.path.as_str());
    let returned_function_targets =
        returned_function_targets(file, nodes, &function_lookup, symbols.entities);
    let ast_parameter_symbols = file
        .ast
        .functions
        .iter()
        .flat_map(|function| &function.parameters)
        .flat_map(|parameter| &parameter.names)
        .filter_map(|name| symbols.entities.at(file.path.as_str(), name.span).cloned())
        .collect::<HashSet<_>>();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            call.span,
        ) else {
            continue;
        };
        let owner_span = nodes[owner].span;
        // `reader.read(value)` where `reader` is this function's parameter.
        // Which implementation runs is a property of the *call site*, not of
        // this function, so record the obligation and let each site resolve
        // its own argument. Pooling every site into one summary here is what
        // makes an unambiguous site uncertifiable because a sibling site is
        // not.
        if let Some((receiver, property)) = lookup.member_callee_receiver(file, call.callee)
            && let Some(parameter) = nodes[owner]
                .parameters
                .iter()
                .position(|candidate| *candidate == receiver)
        {
            let entry = (owner_span, parameter, property);
            if !contribution.invoked_parameter_members.contains(&entry) {
                contribution.invoked_parameter_members.push(entry);
            }
        }
        let candidate_symbols = lookup.callee_symbols(file, call.callee);
        // Dispatch candidates answer "which analyzed implementation could
        // run"; the callee symbol answers "which declaration is called". A
        // member call whose receiver has no inspectable value -- a DOM
        // method, or a structural parameter with no in-project call site --
        // has no candidates, but it still has an exact identity, and its
        // arguments still have to be classified. Falling back keeps that
        // call in the analysis instead of dropping it silently.
        let Some(symbol) = candidate_symbols
            .first()
            .map(SymbolId::as_str)
            .or_else(|| lookup.callee_symbol(file, call.callee))
        else {
            // No candidates *and* no callee identity: the callee is a member
            // of a value with no inspectable type, which is every parameter
            // of a published JavaScript runtime artifact (`list.map(fn)`
            // where `list` is `any`). Nothing downstream can classify this
            // call's arguments, so dropping it silently lets a caller-supplied
            // callback escape with no recorded behavior -- and an omitted
            // `callbacks` field is a *negative* claim, so silence here
            // certifies "never invoked". Record the obligation instead; the
            // emitted claim becomes `{"status": "unknown"}`.
            push_unresolved_callee_callback_obligations(
                &mut contribution,
                file,
                call,
                nodes,
                &function_lookup,
                lookup,
                symbols.entities,
            );
            continue;
        };
        let ambiguous_dispatch = candidate_symbols.len() > 1;
        if ambiguous_dispatch {
            // Keep the exact candidate identities, but do not let any one of
            // them contribute reads or callback timing until the fixed point
            // can prove that all candidates have equivalent summaries.
            contribution
                .dispatches
                .push((owner_span, candidate_symbols.clone()));
        }
        let project_function = lookup.function_for_symbol(symbol).is_some();
        if call.direct_callee && !ambiguous_dispatch {
            contribution
                .factory_calls
                .push((owner_span, SymbolId::from(symbol)));
        }
        if !ambiguous_dispatch && let Some(contracted) = contracts.reads.get(symbol) {
            for (display, _, declaration, kind) in contracted {
                contribution.direct_reads.push((
                    owner_span,
                    SummaryRead {
                        symbol: SymbolId::from(symbol),
                        display: SymbolId::from(display.as_str()),
                        kind: Some(kind.clone()),
                        declaration: declaration.clone(),
                        origin: location(file.path.shared(), call.span),
                        origin_context: nodes[owner].name.clone().unwrap_or_default(),
                    },
                ));
            }
        }
        if !ambiguous_dispatch && let Some(contracted) = contracts.parameter_reads.get(symbol) {
            for (parameter, _, _, _) in contracted {
                let Some(argument) = call.arguments.get(*parameter) else {
                    continue;
                };
                let Some(argument_symbol) = symbols
                    .entities
                    .get(&location(file.path.shared(), argument.span))
                else {
                    continue;
                };
                if let Some(owner_parameter) = nodes[owner]
                    .parameters
                    .iter()
                    .position(|candidate| candidate == argument_symbol)
                {
                    let entry = (owner_span, owner_parameter, String::new());
                    if !contribution.invoked_parameter_members.contains(&entry) {
                        contribution.invoked_parameter_members.push(entry);
                    }
                }
            }
        }
        if !ambiguous_dispatch && !contracts.reads.contains_key(symbol) {
            let returned_target = call
                .direct_callee
                .then(|| returned_function_targets.get(symbol).copied());
            if let Some(target) = returned_target.flatten() {
                contribution.edges.push((
                    owner_span,
                    InterproceduralGraphTarget::LocalSpan(nodes[target].span),
                ));
            } else if call.direct_callee || project_function {
                contribution.edges.push((
                    owner_span,
                    InterproceduralGraphTarget::Symbol(SymbolId::from(symbol)),
                ));
            }
        }
        if call.direct_callee
            && !ambiguous_dispatch
            && let Some(&(callback_owner, parameter)) = function_lookup.parameter_owner.get(symbol)
        {
            let invocation_owner = owner;
            contribution
                .invoked_parameters
                .push((owner_span, parameter));
            let mut forwarded_to_local_scheduler = false;
            for (scheduler, scheduler_parameter) in file
                .ast
                .arguments_containing(call.callee)
                .filter(|(scheduler, index)| {
                    direct_callback_contains(file, scheduler.arguments[*index].span, call.callee)
                })
            {
                let Some(scheduler_symbol) = lookup.callee_symbol(file, scheduler.callee) else {
                    continue;
                };
                let Some(target) = function_lookup
                    .by_symbol
                    .get(scheduler_symbol)
                    .map(|index| &nodes[*index])
                else {
                    continue;
                };
                contribution.callback_forwardings.push((
                    nodes[callback_owner].span,
                    target.symbol.clone().map_or(
                        InterproceduralGraphTarget::LocalSpan(target.span),
                        InterproceduralGraphTarget::Symbol,
                    ),
                    scheduler_parameter,
                    parameter,
                    ForwardedAmbientExecution::Callee,
                ));
                forwarded_to_local_scheduler = true;
            }
            if forwarded_to_local_scheduler {
                continue;
            }
            // `parameter()` is synchronous relative to its immediate
            // containing function, but that function may itself be a returned
            // adapter or a timer/Promise callback. Classify the complete
            // execution context before falling back to inline; otherwise a
            // debouncer contract incorrectly promises to invoke its callback
            // before the debouncer factory returns.
            let runtime_execution = file
                .ast
                .arguments_containing(call.callee)
                .filter(|(scheduler, index)| {
                    direct_callback_contains(file, scheduler.arguments[*index].span, call.callee)
                })
                .filter_map(|(scheduler, index)| {
                    let resolved = lookup.resolved_callee_call(file, scheduler.callee)?;
                    let callability = lookup.smallest_contained_callability(
                        file.path.as_str(),
                        scheduler.arguments[index].span,
                    );
                    argument_behavior(resolved, callability, index)
                })
                .fold(None, |observed, behavior| match behavior {
                    RuntimeArgumentBehavior::DeferredCallback => Some("deferred"),
                    RuntimeArgumentBehavior::InlineCallback if observed.is_none() => Some("inline"),
                    RuntimeArgumentBehavior::InlineCallback
                    | RuntimeArgumentBehavior::ValueOnly => observed,
                });
            let semantic = semantic_execution_role(
                file,
                call.callee,
                &allowed,
                symbols.entities,
                symbols.names,
                lookup,
            );
            // An "inline" row promises the export invokes the callback before
            // it returns, and that promise is about the *export*. Neither rung
            // below establishes it across a closure boundary: `semantic`
            // classifies the lexical region (a capitalized function's body is
            // `UntrackedRendering`, i.e. "inline"), and `direct_callee`
            // classifies the call shape. So
            // `f(props, cb) { helper(props, () => cb(1)) }` -- where `helper`
            // may invoke the arrow later, once, or never -- and
            // `f(cb) { return { g: () => cb(1) } }` both published
            // `execution: "inline"`, a promise the package does not keep.
            //
            // The boundary is read off the AST rather than off
            // `invocation_owner`, so it does not depend on which function
            // shapes the summary-node universe carries.
            let call_in_owner_body = file
                .ast
                .functions_body_containing(call.span)
                .min_by_key(|function| function.body.end - function.body.start)
                .is_some_and(|innermost| innermost.body == nodes[callback_owner].body);
            // Every primitive callback position between this call and the
            // declaring function's body, composed. This rung outranks the
            // lexical role below it because the lexical role answers a
            // different question: `semantic` reports the *tracking scope* the
            // call is written in, and an execution row states the schedule
            // relative to the export's return. The two disagree in both
            // directions -- `untrack(() => cb())` is written outside tracking
            // and runs during the call, `createEffect(() => untrack(cb))` is
            // written inside a tracked region and runs after it -- and both
            // disagreements were published as claims.
            //
            // The chain is only usable when its outermost wrapping call sits
            // in the declaring function's own body: otherwise the closure
            // holding the chain may itself never run, which is exactly the
            // boundary the rungs below were added for.
            //
            // The outer `Option` is "is there a usable chain"; the inner one is
            // the chain's own answer, and a `None` there is authoritative. A
            // usable chain that composes to the unknown sentinel must not fall
            // through to the rungs below: those answer the lexical question,
            // which is the answer this rung exists to replace, so falling
            // through would publish exactly the claim the chain just refused.
            let chain_execution: Option<Option<&'static str>> =
                enclosing_callback_chain(file, call.callee, &contracts, lookup)
                    .filter(|chain| !chain.wrappers.is_empty())
                    .filter(|chain| {
                        callback_chain_reaches_owner_body(file, chain, &nodes[callback_owner])
                    })
                    .map(|chain| compose_callback_chain(&chain.wrappers));
            let execution = match (runtime_execution, chain_execution) {
                (Some(execution), _) => Some(execution),
                (None, Some(composed)) => composed,
                (None, None) => contract_callback_execution(semantic)
                    .filter(|execution| *execution != "inline" || call_in_owner_body)
                    .or_else(|| {
                        function_escapes_through_return(
                            file,
                            &nodes[invocation_owner],
                            &nodes[callback_owner],
                            symbols.entities,
                            lookup,
                        )
                        .then_some("deferred")
                    })
                    // Last resort, and only for a call written directly in the
                    // body of the function that declares the parameter. When no
                    // rung can classify the enclosing schedule, no row is
                    // written and the unknown-callback obligation opens the
                    // sentinel instead.
                    .or((call.direct_callee && call_in_owner_body).then_some("inline")),
            };
            if let Some(execution) = execution {
                contribution.callbacks.push((
                    nodes[callback_owner].span,
                    ContractCallback {
                        parameter,
                        execution: execution.into(),
                        arguments: callback_argument_contracts(
                            file,
                            call,
                            symbols.entities,
                            accessors,
                        ),
                        owner: None,
                        evidence: None,
                    },
                ));
            } else {
                contribution.contract_generation_obligations.push((
                    nodes[callback_owner].span,
                    unknown_callback_obligation(
                        file,
                        &nodes[callback_owner],
                        call.callee,
                        parameter,
                        location(file.path.shared(), call.callee),
                        lookup,
                    ),
                ));
            }
        }
        if !ambiguous_dispatch && let Some(callbacks) = contracts.callbacks.get(symbol) {
            for callback in callbacks {
                let Some(argument) = call.arguments.get(callback.parameter) else {
                    continue;
                };
                let argument_location = location(file.path.shared(), argument.span);
                if contract_callback_arguments_unbound(file, argument, callback)
                    && let Some((package, export)) = lookup.contract_export_identity(symbol)
                {
                    contribution
                        .contract_consumer_obligations
                        .push(StaticDefect {
                            kind: StaticDefectKind::PackageContractExportMissing {
                                module: package.to_owned(),
                                export: export.to_owned(),
                                reexported: false,
                            },
                            location: argument_location.clone(),
                            analysis_context: "unbound-contract-claims:callback arguments".into(),
                            fixes: vec![],
                            uncertain: false,
                        });
                }
                if let Some(argument_symbol) = symbols.entities.get(&argument_location) {
                    if callback.execution == "inline" {
                        contribution.edges.push((
                            owner_span,
                            InterproceduralGraphTarget::Symbol(argument_symbol.clone()),
                        ));
                    }
                    if let Some(&(callback_owner, parameter)) =
                        function_lookup.parameter_owner.get(argument_symbol)
                    {
                        if callback.execution == "inline" {
                            contribution
                                .invoked_parameters
                                .push((owner_span, parameter));
                        }
                        contribution.callbacks.push((
                            nodes[callback_owner].span,
                            ContractCallback {
                                parameter,
                                execution: callback.execution.clone(),
                                arguments: callback.arguments.clone(),
                                owner: None,
                                evidence: None,
                            },
                        ));
                    }
                } else if callback.execution == "inline"
                    && let Some(target) =
                        functions_for_path(nodes, nodes_by_path, file.path.as_str())
                            .filter(|(_, node)| argument.span.contains(node.span))
                            .min_by_key(|(_, node)| node.span.end - node.span.start)
                            .map(|(_, node)| node.span)
                {
                    contribution
                        .edges
                        .push((owner_span, InterproceduralGraphTarget::LocalSpan(target)));
                }
            }
        }
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            let runtime_argument_callability =
                lookup.smallest_contained_callability(file.path.as_str(), argument.span);
            let unknown_contract_callback = lookup.unknown_contract_callback_export(symbol);
            if let Some((package, export)) = unknown_contract_callback
                && potentially_callable(runtime_argument_callability)
                && !literal_argument_is_not_callable(argument.runtime_value_kind)
                && argument.value != solid_facts::ast::ArgumentValueKind::Identifier
            {
                contribution
                    .contract_consumer_obligations
                    .push(StaticDefect {
                        kind: StaticDefectKind::PackageContractExportMissing {
                            module: package.to_owned(),
                            export: export.to_owned(),
                            reexported: false,
                        },
                        location: location(file.path.shared(), argument.span),
                        analysis_context: "unknown-contract-claims:callbacks".into(),
                        fixes: vec![],
                        uncertain: false,
                    });
                continue;
            }
            if argument.value != solid_facts::ast::ArgumentValueKind::Identifier {
                continue;
            }
            let Some(argument_symbol) = symbols
                .entities
                .get(&location(file.path.shared(), argument.span))
            else {
                continue;
            };
            let callback_owner_and_parameter = function_lookup
                .parameter_owner
                .get(argument_symbol)
                .copied();
            let Some((callback_owner, parameter)) = callback_owner_and_parameter else {
                if let Some((package, export)) = unknown_contract_callback
                    && potentially_callable(runtime_argument_callability)
                {
                    contribution
                        .contract_consumer_obligations
                        .push(StaticDefect {
                            kind: StaticDefectKind::PackageContractExportMissing {
                                module: package.to_owned(),
                                export: export.to_owned(),
                                reexported: false,
                            },
                            location: location(file.path.shared(), argument.span),
                            analysis_context: "unknown-contract-claims:callbacks".into(),
                            fixes: vec![],
                            uncertain: false,
                        });
                }
                continue;
            };
            if !ambiguous_dispatch
                && let Some(target) = function_lookup
                    .by_symbol
                    .get(symbol)
                    .copied()
                    .or_else(|| returned_function_targets.get(symbol).copied())
            {
                // Local calls are summarized transitively. If the parameter
                // later reaches an unknown external call, that call creates
                // the obligation at the actual escape point.
                let ambient = forwarded_callback_ambient_execution(
                    file,
                    call,
                    argument_index,
                    &contracts,
                    lookup,
                );
                // A chain that refuses to compose cannot restate the callee's
                // `inline` rows in export-relative terms, so the sentinel opens
                // here rather than in the propagation loop, which has no file
                // or call to build an obligation from. It opens even when the
                // callee turns out to publish no `inline` row for the slot --
                // a precision cost in a shape that needs an unclassifiable
                // tracked wrapper above a clearing one, and never a wrong
                // claim.
                if ambient == ForwardedAmbientExecution::Unknown {
                    contribution.contract_generation_obligations.push((
                        nodes[callback_owner].span,
                        unknown_callback_obligation(
                            file,
                            &nodes[callback_owner],
                            call.callee,
                            parameter,
                            location(file.path.shared(), call.callee),
                            lookup,
                        ),
                    ));
                }
                contribution.callback_forwardings.push((
                    nodes[callback_owner].span,
                    nodes[target].symbol.clone().map_or(
                        InterproceduralGraphTarget::LocalSpan(nodes[target].span),
                        InterproceduralGraphTarget::Symbol,
                    ),
                    argument_index,
                    parameter,
                    ambient,
                ));
                continue;
            }
            let primitive = super::known_primitive(&primitives.calls[call_index]);
            if let Some(execution) = primitive_callback_execution(
                primitive,
                argument_index,
                call.arguments.len(),
                lookup.dialect,
            ) {
                // The primitive's own slot says how the callback runs relative
                // to *this* call; the row says how it runs relative to the
                // export. `onMount(fn) { createEffect(() => untrack(fn)) }` is
                // the shape where those differ: `untrack`'s slot is inline,
                // and the enclosing `createEffect` has not run it by the time
                // `onMount` returns. Composing the enclosing chain answers the
                // second question; an unclassifiable enclosing position leaves
                // the slot's own answer standing, which is what this branch
                // always published.
                //
                // Two levels of `Option`, and they mean different things. The
                // outer one is "was there a usable chain to compose", and its
                // `None` falls back to the slot's own answer. The inner one is
                // the composition itself, and its `None` is the unknown
                // sentinel -- a usable chain that refuses to answer must open
                // the sentinel rather than fall back, because the slot's answer
                // is relative to the wrapping call and the row is relative to
                // the export.
                let composed: Option<Option<&'static str>> =
                    enclosing_callback_chain(file, call.span, &contracts, lookup)
                        .filter(|chain| {
                            chain.wrappers.is_empty()
                                || callback_chain_reaches_owner_body(
                                    file,
                                    chain,
                                    &nodes[callback_owner],
                                )
                        })
                        .and_then(|chain| {
                            let mut wrappers = vec![callback_wrapper_at(
                                file,
                                call,
                                argument_index,
                                &contracts,
                                lookup,
                            )?];
                            wrappers.extend(chain.wrappers);
                            Some(compose_callback_chain(&wrappers))
                        });
                if let Some(execution) = composed.unwrap_or(Some(execution)) {
                    contribution.callbacks.push((
                        nodes[callback_owner].span,
                        ContractCallback {
                            parameter,
                            execution: execution.into(),
                            arguments: Vec::new(),
                            owner: None,
                            evidence: None,
                        },
                    ));
                } else {
                    contribution.contract_generation_obligations.push((
                        nodes[callback_owner].span,
                        unknown_callback_obligation(
                            file,
                            &nodes[callback_owner],
                            call.callee,
                            parameter,
                            location(file.path.shared(), call.callee),
                            lookup,
                        ),
                    ));
                }
                continue;
            }
            // `splitProps` only creates property views. Its source and key
            // lists are values even when erased JavaScript types leave their
            // callability unknown.
            if primitive == Some(Primitive::SplitProps) {
                continue;
            }
            let resolved_call = lookup.resolved_callee_call(file, call.callee);
            if let Some((package, export)) = unknown_contract_callback
                && potentially_callable(runtime_argument_callability)
            {
                if nodes[callback_owner].exported {
                    contribution.contract_generation_obligations.push((
                        nodes[callback_owner].span,
                        unknown_callback_obligation(
                            file,
                            &nodes[callback_owner],
                            call.callee,
                            parameter,
                            location(file.path.shared(), argument.span),
                            lookup,
                        ),
                    ));
                } else {
                    contribution
                        .contract_consumer_obligations
                        .push(StaticDefect {
                            kind: StaticDefectKind::PackageContractExportMissing {
                                module: package.to_owned(),
                                export: export.to_owned(),
                                reexported: false,
                            },
                            location: location(file.path.shared(), argument.span),
                            analysis_context: "unknown-contract-claims:callbacks".into(),
                            fixes: vec![],
                            uncertain: false,
                        });
                }
                continue;
            }
            let runtime_behavior = resolved_call
                .and_then(|resolved_call| {
                    argument_behavior(resolved_call, runtime_argument_callability, argument_index)
                })
                .or_else(|| {
                    proven_array_method(file, call, symbols.entities).and_then(|method| {
                        proven_array_method_argument_behavior(method, runtime_argument_callability)
                    })
                })
                .or_else(|| {
                    resolved_call
                        .is_some_and(|resolved_call| {
                            stored_constructor_argument_escapes(
                                file,
                                call,
                                resolved_call,
                                argument_index,
                                lookup,
                            )
                        })
                        .then_some(RuntimeArgumentBehavior::DeferredCallback)
                })
                .or_else(|| {
                    // Even when a structurally typed method has no inspectable
                    // implementation, invoking it from a callback that itself
                    // escapes the exported call proves that any invocation of the
                    // forwarded callable cannot happen inline with that export.
                    assigned_member_function_contains(file, call.callee, symbols.entities)
                        .then_some(RuntimeArgumentBehavior::DeferredCallback)
                });
            if let Some(runtime_behavior) = runtime_behavior {
                match runtime_behavior {
                    RuntimeArgumentBehavior::InlineCallback
                    | RuntimeArgumentBehavior::DeferredCallback => {
                        contribution.callbacks.push((
                            nodes[callback_owner].span,
                            ContractCallback {
                                parameter,
                                execution: match runtime_behavior {
                                    RuntimeArgumentBehavior::InlineCallback => "inline",
                                    RuntimeArgumentBehavior::DeferredCallback => "deferred",
                                    RuntimeArgumentBehavior::ValueOnly => unreachable!(),
                                }
                                .into(),
                                arguments: Vec::new(),
                                owner: None,
                                evidence: None,
                            },
                        ));
                    }
                    RuntimeArgumentBehavior::ValueOnly => {}
                }
                continue;
            }
            if contracts.callbacks.contains_key(symbol) {
                continue;
            }
            if !potentially_callable(runtime_argument_callability) {
                continue;
            }
            if function_lookup.parameter_owner.contains_key(symbol)
                || ast_parameter_symbols.contains(symbol)
            {
                // Calling a function-valued parameter with another parameter
                // invokes the callee parameter, not the value argument. Any
                // nested use belongs to the caller-provided callback.
                continue;
            }
            contribution.contract_generation_obligations.push((
                nodes[callback_owner].span,
                unknown_callback_obligation(
                    file,
                    &nodes[callback_owner],
                    call.callee,
                    parameter,
                    location(file.path.shared(), argument.span),
                    lookup,
                ),
            ));
        }
    }
    push_unaccounted_parameter_escapes(
        &mut contribution,
        file,
        nodes,
        nodes_by_path,
        lookup,
        symbols.entities,
    );
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = file.ast.call_at(initializer) else {
            continue;
        };
        let Some(target_symbol) = symbols
            .entities
            .get(&location(file.path.shared(), call.callee))
        else {
            continue;
        };
        for name in &binding.names {
            if let Some(binding_symbol) = symbols
                .entities
                .get(&location(file.path.shared(), name.span))
            {
                contribution
                    .returned_bindings
                    .push((binding_symbol.clone(), target_symbol.clone()));
            }
        }
    }
    contribution
}

/// Whether `nested` is returned as a callable value from `owner`.
///
/// This covers direct returned arrows/identifiers and identity-preserving
/// standard helpers such as `Object.assign(wrapper, { clear })`. A nested
/// function that is merely called while computing the return value is not an
/// escape: its callback execution stays inline with the exported call.
fn function_escapes_through_return(
    file: &solid_facts::FileFacts,
    nested: &SummaryNode,
    owner: &SummaryNode,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if nested.span == owner.span {
        return false;
    }
    if function_value_escapes_through_return(
        file,
        nested.span,
        nested.symbol.as_ref(),
        nested.name.as_deref(),
        owner,
        entities,
        lookup,
    ) {
        return true;
    }
    // A callback can be invoked by a helper nested inside the callable that
    // escapes: `factory(cb) { function returned() { const run = () => cb(); }
    // return identity(returned); }`. Check every intervening function, not
    // only the leaf that contains the invocation.
    file.ast
        .functions
        .iter()
        .filter(|candidate| {
            candidate.span != nested.span
                && candidate.span != owner.span
                && candidate.body.contains(nested.span)
                && owner.body.contains(candidate.span)
        })
        .any(|candidate| {
            let name = function_binding_name(file, candidate);
            let symbol = name.and_then(|name| entities.at(file.path.as_str(), name.span));
            function_value_escapes_through_return(
                file,
                candidate.span,
                symbol,
                name.and_then(|name| file.source_text(name.span)),
                owner,
                entities,
                lookup,
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn function_value_escapes_through_return(
    file: &solid_facts::FileFacts,
    nested_span: Span,
    nested_symbol: Option<&SymbolId>,
    nested_name: Option<&str>,
    owner: &SummaryNode,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(owner_function) = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == owner.span)
    else {
        return false;
    };
    owner_function
        .expression_return
        .iter()
        .chain(file.ast.returns.iter().filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|candidate| candidate.span == owner.span)
        }))
        .any(|returned| {
            // ReturnValueKind describes the semantic value, so a call such as
            // Object.assign(wrapper, ...) is `Function`, not `Call`, when its
            // result retains wrapper's callable type. Inspect syntax first.
            if let Some(returned_call) = returned
                .argument
                .and_then(|argument| file.ast.call_at(argument))
            {
                return returned_call
                    .arguments
                    .iter()
                    .enumerate()
                    .any(|(index, argument)| {
                        let names_nested = nested_symbol.is_some_and(|symbol| {
                            entities.at(file.path.as_str(), argument.span).map_or_else(
                                || file.source_text(argument.span) == nested_name,
                                |argument_symbol| argument_symbol == symbol,
                            )
                        });
                        if !names_nested {
                            return false;
                        }
                        let callability = lookup
                            .smallest_contained_callability(file.path.as_str(), argument.span);
                        lookup
                            .resolved_callee_call(file, returned_call.callee)
                            .and_then(|resolved| argument_behavior(resolved, callability, index))
                            == Some(RuntimeArgumentBehavior::ValueOnly)
                            || local_function_returns_argument(file, returned_call, index, entities)
                    });
            }
            if returned.value == solid_facts::ast::ReturnValueKind::Function {
                return returned
                    .argument
                    .is_some_and(|argument| argument.contains(nested_span));
            }
            if returned.value == solid_facts::ast::ReturnValueKind::Identifier {
                if nested_symbol.is_some_and(|symbol| {
                    entities.at(file.path.as_str(), returned.span).map_or_else(
                        || file.source_text(returned.span) == nested_name,
                        |returned_symbol| returned_symbol == symbol,
                    )
                }) {
                    return true;
                }
                // Follow one local binding hop: `const wrapped =
                // identity(callable); return wrapped`. Bundled JavaScript
                // commonly introduces this shape to decorate the wrapper
                // before returning it.
                let returned_span = returned.argument.unwrap_or(returned.span);
                let returned_symbol = entities.at(file.path.as_str(), returned_span);
                if let Some(initializer) = file.ast.bindings.iter().find_map(|binding| {
                    binding
                        .names
                        .iter()
                        .any(|name| {
                            returned_symbol.is_some_and(|returned_symbol| {
                                entities.at(file.path.as_str(), name.span) == Some(returned_symbol)
                            })
                        })
                        .then_some(binding.call_initializer)
                        .flatten()
                }) && let Some(returned_call) = file.ast.call_at(initializer)
                {
                    return returned_call
                        .arguments
                        .iter()
                        .enumerate()
                        .any(|(index, argument)| {
                            nested_symbol.is_some_and(|nested_symbol| {
                                entities.at(file.path.as_str(), argument.span)
                                    == Some(nested_symbol)
                            }) && local_function_returns_argument(
                                file,
                                returned_call,
                                index,
                                entities,
                            )
                        });
                }
            }
            false
        })
}

fn local_function_returns_argument(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    entities: &EntitySymbols,
) -> bool {
    let Some(callee_symbol) = entities.at(file.path.as_str(), call.callee) else {
        return false;
    };
    let Some(function) = file.ast.functions.iter().find(|function| {
        function_binding_name(file, function)
            .and_then(|name| entities.at(file.path.as_str(), name.span))
            .is_some_and(|symbol| symbol == callee_symbol)
    }) else {
        return false;
    };
    let Some(parameter) = function.parameters.get(argument) else {
        return false;
    };
    let Some(parameter_name) = parameter.names.first() else {
        return false;
    };
    let parameter_symbol = entities.at(file.path.as_str(), parameter_name.span);
    let parameter_text = file.source_text(parameter_name.span);
    let returns = function
        .expression_return
        .iter()
        .chain(file.ast.returns.iter().filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        }))
        .collect::<Vec<_>>();
    if returns
        .iter()
        .filter(|returned| returned.value == solid_facts::ast::ReturnValueKind::Identifier)
        .any(|returned| {
            let span = returned.argument.unwrap_or(returned.span);
            parameter_symbol.is_some_and(|parameter_symbol| {
                entities.at(file.path.as_str(), span).map_or_else(
                    || file.source_text(span) == parameter_text,
                    |returned_symbol| returned_symbol == parameter_symbol,
                )
            })
        })
    {
        return true;
    }

    // An identity-preserving wrapper may capture and invoke the parameter in
    // the callable it returns (`return (...args) => callback(...args)`). Such
    // an argument is not returned literally, but its execution is still
    // deferred until the wrapper is called.
    let returned_functions = returns
        .iter()
        .flat_map(|returned| {
            file.ast.functions.iter().filter(move |candidate| {
                returned.argument.is_some_and(|argument| {
                    (returned.value == solid_facts::ast::ReturnValueKind::Function
                        && argument.contains(candidate.span))
                        || (returned.value == solid_facts::ast::ReturnValueKind::Identifier
                            && function_binding_name(file, candidate)
                                .and_then(|name| entities.at(file.path.as_str(), name.span))
                                .is_some_and(|candidate_symbol| {
                                    entities.at(file.path.as_str(), argument)
                                        == Some(candidate_symbol)
                                }))
                })
            })
        })
        .collect::<Vec<_>>();
    parameter_symbol.is_some_and(|parameter_symbol| {
        returned_functions.iter().any(|returned_function| {
            file.ast.calls.iter().any(|candidate_call| {
                if !returned_function.body.contains(candidate_call.span) {
                    return false;
                }
                if entities.at(file.path.as_str(), candidate_call.callee) == Some(parameter_symbol)
                {
                    return true;
                }
                file.ast.members.iter().any(|member| {
                    member.span == candidate_call.callee
                        && entities.at(file.path.as_str(), member.object) == Some(parameter_symbol)
                })
            })
        })
    })
}

fn returned_function_targets(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    functions: &FunctionLookup,
    entities: &EntitySymbols,
) -> HashMap<SymbolId, usize> {
    let mut ast_functions = HashMap::new();
    for function in &file.ast.functions {
        ast_functions.entry(function.span).or_insert(function);
    }
    let mut returns_by_owner = HashMap::<Span, Vec<&solid_facts::ast::ReturnFact>>::new();
    for returned in &file.ast.returns {
        if let Some(owner) = containing_ast_function(&file.ast, returned.span) {
            returns_by_owner
                .entry(owner.span)
                .or_default()
                .push(returned);
        }
    }
    let mut seen = HashSet::new();
    let mut targets = HashMap::new();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let binding_symbols = binding
            .names
            .iter()
            .filter_map(|name| entities.at(file.path.as_str(), name.span).cloned())
            .filter(|symbol| seen.insert(symbol.clone()))
            .collect::<Vec<_>>();
        if binding_symbols.is_empty() {
            continue;
        }
        let target = (|| {
            let factory_call = file.ast.call_at(initializer)?;
            let factory_symbol = entities.at(file.path.as_str(), factory_call.callee)?;
            let factory = &nodes[*functions.by_symbol.get(factory_symbol)?];
            let function = ast_functions.get(&factory.span)?;
            function
                .expression_return
                .iter()
                .filter_map(|returned| returned.argument)
                .chain(
                    returns_by_owner
                        .get(&function.span)
                        .into_iter()
                        .flatten()
                        .filter_map(|returned| returned.argument),
                )
                .flat_map(|value| returned_function_spans(file, value, entities))
                .find_map(|span| functions.by_span.get(&span).copied())
        })();
        if let Some(target) = target {
            for symbol in binding_symbols {
                targets.insert(symbol, target);
            }
        }
    }
    targets
}

/// Return the exact function values reachable from a return expression.
///
/// This deliberately follows only facts that preserve a value identity:
/// transparent wrappers, conditional branches, identifier aliases, and an
/// exact destructured property of an object literal/spread. It does not walk
/// arbitrary nested functions in an object, because doing so would turn a
/// returned data object into a callable value by containment alone.
fn returned_function_spans(
    file: &solid_facts::FileFacts,
    value: Span,
    entities: &EntitySymbols,
) -> Vec<Span> {
    fn collect(
        file: &solid_facts::FileFacts,
        value: Span,
        entities: &EntitySymbols,
        visited: &mut HashSet<Span>,
        result: &mut Vec<Span>,
    ) {
        let value = file.ast.peel_ts_sugar_span(value);
        if !visited.insert(value) {
            return;
        }
        if file
            .ast
            .functions
            .iter()
            .any(|function| function.span == value)
        {
            result.push(value);
            return;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == value)
        {
            collect(file, conditional.consequent, entities, visited, result);
            collect(file, conditional.alternate, entities, visited, result);
            return;
        }
        let Some(symbol) = entities.at(file.path.as_str(), value) else {
            return;
        };
        let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding.names.iter().any(|name| {
                entities
                    .at(file.path.as_str(), name.span)
                    .is_some_and(|candidate| candidate == symbol)
            })
        }) else {
            return;
        };
        if let Some(alias) = &binding.initializer_identifier {
            collect(file, alias.span, entities, visited, result);
            if !result.is_empty() {
                return;
            }
        }
        if binding.shape != solid_facts::ast::BindingShape::Object {
            if let Some(initializer) = binding.initializer
                && binding.call_initializer.is_none()
            {
                collect(file, initializer, entities, visited, result);
            }
            return;
        }
        let Some(initializer) = binding.initializer else {
            return;
        };
        let Some(slot) = binding.object_slots.iter().find(|slot| {
            entities
                .at(file.path.as_str(), slot.local.span)
                .is_some_and(|candidate| candidate == symbol)
        }) else {
            return;
        };
        collect_object_property(file, initializer, &slot.property, entities, visited, result);
    }

    fn collect_object_property(
        file: &solid_facts::FileFacts,
        object: Span,
        property_name: &str,
        entities: &EntitySymbols,
        visited: &mut HashSet<Span>,
        result: &mut Vec<Span>,
    ) {
        let object = file.ast.peel_ts_sugar_span(object);
        let property = file
            .ast
            .object_properties
            .iter()
            .filter(|property| {
                object.contains(property.span)
                    && !property.computed
                    && file.source_text(property.key) == Some(property_name)
            })
            .max_by_key(|property| property.span.start);
        if let Some(property) = property {
            let method = file.ast.functions.iter().find(|function| {
                property.span.contains(function.span)
                    && function
                        .method_name
                        .as_ref()
                        .is_some_and(|name| name.span == property.key)
            });
            if let Some(method) = method {
                result.push(method.span);
            } else {
                collect(file, property.value, entities, visited, result);
            }
            return;
        }
        for spread in file
            .ast
            .spreads
            .iter()
            .filter(|spread| object.contains(spread.span))
        {
            collect_object_property(
                file,
                spread.argument,
                property_name,
                entities,
                visited,
                result,
            );
        }
    }

    let mut visited = HashSet::new();
    let mut result = Vec::new();
    collect(file, value, entities, &mut visited, &mut result);
    result.sort_unstable();
    result.dedup();
    result
}

fn proven_array_method<'a>(
    file: &'a solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
) -> Option<&'a str> {
    let member = file
        .ast
        .members
        .iter()
        .find(|member| member.span == call.callee)?;
    let method = file.source_text(member.property)?;
    if !matches!(method, "push" | "unshift") {
        return None;
    }

    let receiver_is_assigned_array = file.ast.assignments.iter().any(|assignment| {
        assignment.value == solid_facts::ast::AssignmentValueKind::Array
            && same_runtime_value(file, assignment.target, member.object, entities)
    });
    let receiver_is_array_guarded = file.ast.if_regions.iter().any(|region| {
        region.consequent.contains(call.span)
            && file.ast.calls.iter().any(|guard| {
                region.test.contains(guard.span)
                    && guard.static_callee(&file.source) == Some("Array.isArray")
                    && guard.arguments.first().is_some_and(|argument| {
                        same_runtime_value(file, argument.span, member.object, entities)
                    })
            })
    });
    (receiver_is_assigned_array || receiver_is_array_guarded).then_some(method)
}

fn stored_constructor_argument_escapes(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    resolved_call: &typefacts::ResolvedCall,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if resolved_call.kind != typefacts::CallKind::Construct
        || resolved_call
            .declaration
            .as_ref()
            .is_none_or(|declaration| declaration.standard_library)
    {
        return false;
    }
    let Some(parameter) = resolved_parameter(resolved_call, argument) else {
        return false;
    };
    let Some(declaration) = parameter.declaration.as_ref() else {
        return false;
    };
    if declaration.location.path.as_ref() != file.path.as_str()
        || file
            .ast
            .parameter_properties
            .binary_search_by_key(
                &(
                    declaration.location.start_byte,
                    declaration.location.end_byte,
                ),
                |span| (u64::from(span.start), u64::from(span.end)),
            )
            .is_err()
    {
        return false;
    }
    file.ast
        .arguments_containing(call.span)
        .any(|(outer, outer_argument)| {
            lookup
                .resolved_callee_call(file, outer.callee)
                .is_some_and(|outer_call| retains_argument_value(outer_call, outer_argument))
        })
}

fn same_runtime_value(
    file: &solid_facts::FileFacts,
    left: Span,
    right: Span,
    entities: &EntitySymbols,
) -> bool {
    let left_symbol = entities.at(file.path.as_str(), left);
    let right_symbol = entities.at(file.path.as_str(), right);
    left_symbol
        .zip(right_symbol)
        .is_some_and(|(left, right)| left == right)
}

/// How one callback position schedules the code written inside it, relative to
/// the call that owns that position.
///
/// Four answers rather than the contract's three, because "runs during the
/// call" and "reads inside it subscribe the caller" are separate facts and the
/// composition in [`compose_callback_chain`] needs both. The contract
/// vocabulary collapses them: `untrack` and `batch` are both `inline` there,
/// and only one of them stops an enclosing computation from tracking what runs
/// inside.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallbackWrapper {
    /// Runs during the wrapping call and leaves the caller's tracking scope in
    /// place: 1.x `batch`, `startTransition`, `catchError`'s protected body,
    /// 2.0 `latest`/`isPending`.
    Transparent,
    /// Runs during the wrapping call with the listener cleared: `untrack`,
    /// `createRoot`, `runWithOwner`, 2.0 `flush` and `createRevealOrder`.
    Detaching,
    /// The wrapping call builds its own tracked computation around the code.
    ///
    /// The payload is when that computation runs relative to the wrapping
    /// call's return, and it is not derivable from "tracked": 1.x `createMemo`
    /// and `createRenderEffect` run it during the call while `createEffect`
    /// queues it, and 2.0 disagrees with 1.x on `createEffect`. `None` is the
    /// dialect refusing to say ([`solid_dialect::Dialect::tracked_callback_timing`]),
    /// or a package contract row, whose `execution` word carries no schedule
    /// column at all.
    Tracked(Option<TrackedCallbackTiming>),
    /// The wrapping call schedules the code to run after it returns.
    Deferred,
}

/// The wrapper a callback position is, for contract emission.
///
/// `None` is not "transparent": it is "this analysis cannot say what the
/// wrapping call does with the function it was handed", which is why
/// [`enclosing_callback_chain`] refuses the whole chain on it rather than
/// skipping the link.
fn callback_wrapper_at(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    contracts: &InterproceduralContracts<'_>,
    lookup: &SemanticLookup<'_>,
) -> Option<CallbackWrapper> {
    let count = call.arguments.len();
    let primitive = lookup
        .call_index(file, call.span)
        .and_then(|call_index| super::known_primitive(&lookup.primitives(file).calls[call_index]));
    let execution = primitive_callback_execution(primitive, argument, count, lookup.dialect)
        .map(std::borrow::Cow::Borrowed)
        .or_else(|| {
            let symbol = lookup.callee_symbol(file, call.callee)?;
            contracts
                .callbacks
                .get(symbol)?
                .iter()
                .find(|callback| callback.parameter == argument)
                .map(|callback| std::borrow::Cow::Owned(callback.execution.clone()))
        })?;
    Some(match execution.as_ref() {
        "deferred" => CallbackWrapper::Deferred,
        // A package contract row (`primitive` is `None` here) carries no
        // schedule column, so its tracked wrapper has no established timing and
        // the fold fails closed on it.
        "tracked" => CallbackWrapper::Tracked(primitive.and_then(|primitive| {
            lookup
                .dialect
                .tracked_callback_timing(primitive, argument, count)
        })),
        // Only a primitive answers the clearing question; a package contract
        // row carries no such column, so an external `inline` stays
        // transparent. `reports_untracked_reads_at` is consulted for the
        // inline entry points that clear the listener without being in the
        // synchronous-clearing set (1.x/2.0 `render` and `hydrate`).
        _ if primitive.is_some_and(|primitive| {
            lookup.dialect.runs_callback_synchronously(primitive)
                || lookup
                    .dialect
                    .reports_untracked_reads_at(primitive, argument, count)
        }) =>
        {
            CallbackWrapper::Detaching
        }
        _ => CallbackWrapper::Transparent,
    })
}

/// The chain of callback positions between `nested` and the body of the
/// function that lexically owns the chain, innermost first.
///
/// `outermost` is the span the walk stopped at -- the last wrapping call, or
/// `nested` itself for an empty chain. Callers check that it sits in the
/// declaring function's own body before believing the composition: a closure
/// that is merely *returned* has an empty chain too, and reading an empty chain
/// as "runs inline" is the promise `callback-execution-boundary` exists to
/// prevent.
struct CallbackChain {
    wrappers: Vec<CallbackWrapper>,
    outermost: Span,
}

/// `None` means a callback position exists that this analysis cannot classify,
/// so no composition over the chain is honest. `Some` with empty `wrappers`
/// means the walk found no callback position at all -- a different answer, and
/// the reason this is not an `Option<Vec<_>>`.
///
/// [`direct_callback_contains`] resolves exactly one level, so the walk is
/// iterative -- the same innermost-outward loop
/// `semantic_write_execution_role_within` uses for write regions.
fn enclosing_callback_chain(
    file: &solid_facts::FileFacts,
    nested: Span,
    contracts: &InterproceduralContracts<'_>,
    lookup: &SemanticLookup<'_>,
) -> Option<CallbackChain> {
    let mut wrappers = Vec::new();
    let mut span = nested;
    // Bounded like every other span walk in this module: a malformed or
    // pathologically nested AST must not turn a summary into a hang.
    for _ in 0..32 {
        let mut enclosing = file
            .ast
            .arguments_containing(span)
            .filter(|(call, argument)| {
                direct_callback_contains(file, call.arguments[*argument].span, span)
            })
            .collect::<Vec<_>>();
        if enclosing.is_empty() {
            break;
        }
        // Shortest argument span first: `f(g(() => x))` answers both calls for
        // `x`, and the immediately wrapping one is `g`'s.
        enclosing.sort_by_key(|(call, argument)| {
            let span = call.arguments[*argument].span;
            span.end - span.start
        });
        let (call, argument) = enclosing[0];
        wrappers.push(callback_wrapper_at(
            file, call, argument, contracts, lookup,
        )?);
        span = call.span;
    }
    Some(CallbackChain {
        wrappers,
        outermost: span,
    })
}

/// Whether the chain's outermost wrapping call is written in `owner`'s own
/// body, which is what makes a composition over it a promise about `owner`.
fn callback_chain_reaches_owner_body(
    file: &solid_facts::FileFacts,
    chain: &CallbackChain,
    owner: &SummaryNode,
) -> bool {
    file.ast
        .functions_body_containing(chain.outermost)
        .min_by_key(|function| function.body.end - function.body.start)
        .is_some_and(|innermost| innermost.body == owner.body)
}

/// The contract execution a callback carries when it runs synchronously at the
/// innermost end of `wrappers`.
///
/// Read innermost outward, because the order is the whole answer:
/// `untrack(() => createMemo(fn))` tracks `fn` -- the memo subscribes it and
/// the surrounding `untrack` cannot undo that -- while
/// `createEffect(() => untrack(fn))` does not, and defers instead. That second
/// shape is solid-js's own `onMount`, which the generator used to publish as
/// `tracked` because it read the lexical tracking scope rather than the
/// schedule relative to the export's return.
///
/// `None` is the unknown sentinel: a wrapper in the chain has no established
/// schedule, so no word is honest. It arises for exactly one shape -- a
/// *detached* callback under a tracked wrapper whose
/// [`solid_dialect::Dialect::tracked_callback_timing`] the dialect does not
/// state. Once tracking is cleared, the tracked wrapper's remaining
/// contribution is only its schedule, and reading `Tracked` as "runs later" is
/// wrong for most of 1.x's tracked primitives: `createMemo(() => untrack(cb))`
/// runs `cb` during the call (`dist/solid.js:244-256`), so `deferred` there was
/// a claim the probe measures and fails. Where tracking is *not* cleared the
/// answer stays `tracked` regardless of schedule -- the attribution is the
/// claim, and every tracked computation eventually runs its compute.
fn compose_callback_chain(wrappers: &[CallbackWrapper]) -> Option<&'static str> {
    let mut detached = false;
    let mut execution = "inline";
    for wrapper in wrappers {
        match wrapper {
            CallbackWrapper::Transparent => {}
            CallbackWrapper::Detaching => detached = true,
            // A tracked wrapper subscribes what runs inside it -- unless a
            // clearing wrapper already stands between them, in which case what
            // is left of the wrapper is its schedule.
            CallbackWrapper::Tracked(_) if !detached && execution != "deferred" => {
                execution = "tracked";
            }
            // Detached under a tracked wrapper: the schedule is the whole
            // remaining question, and only the dialect can answer it.
            CallbackWrapper::Tracked(timing) if execution != "deferred" => {
                execution = match timing {
                    Some(TrackedCallbackTiming::DuringCall) => "inline",
                    Some(TrackedCallbackTiming::AfterCall) => "deferred",
                    None => return None,
                };
            }
            CallbackWrapper::Tracked(_) => {}
            // Sticky: no outer wrapper can make a callback that runs later run
            // earlier.
            CallbackWrapper::Deferred => execution = "deferred",
        }
    }
    Some(execution)
}

/// What the wrappers around a forwarding call say about a callback the callee's
/// own summary reports as `inline`.
///
/// Three answers, because "no chain fact" and "the chain refuses to answer" are
/// opposites and collapsing them is how a refusal turns back into a claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ForwardedAmbientExecution {
    /// Nothing wraps the forwarding call that this analysis can read, so the
    /// callee's own answer stands -- exactly as it did before this composition
    /// existed. The ambient adjustment is an override, and there is nothing to
    /// override with.
    Callee,
    /// The wrappers compose to this export-relative execution.
    Composed(String),
    /// A wrapper in the chain has no established schedule, so no
    /// export-relative word is honest. The callee's `inline` rows must not be
    /// republished and the unknown sentinel opens instead.
    Unknown,
}

/// The ambient execution to apply to a callback forwarded into `call` at
/// `argument`, when the callee's own summary says it invokes the callback
/// inline.
///
/// The chain starts at the forwarding call's *own* position, not above it: for
/// `createEffect(() => untrack(fn))` the callee is `untrack`, and "untrack
/// clears the listener" is the fact that stops the enclosing `createEffect`
/// from making `fn` tracked. Without that link the ambient answer was the
/// enclosing computation's lexical role, which is the export's tracking scope
/// rather than the callback's schedule.
fn forwarded_callback_ambient_execution(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    contracts: &InterproceduralContracts<'_>,
    lookup: &SemanticLookup<'_>,
) -> ForwardedAmbientExecution {
    let own = callback_wrapper_at(file, call, argument, contracts, lookup);
    // `enclosing_callback_chain`'s `None` is "a callback position exists above
    // this call that the analysis cannot classify" -- a different fact from
    // "there is no wrapper above it", which is the whole reason
    // [`CallbackChain`] is not an `Option<Vec<_>>`. So it is matched rather
    // than `unwrap_or_default()`ed, which spelled the refusal as an empty
    // chain: the refusal drops the *chain* from the composition and leaves the
    // forwarding call's own position -- the one wrapper that was classified --
    // to answer alone.
    //
    // That is deliberately best-effort rather than fail-closed, and it is this
    // seam's pre-existing behavior, preserved here on purpose: an
    // unclassifiable wrapper above the call can still defer a composition that
    // reads `inline` or `tracked` from `own`. Recorded in
    // docs/precision-backlog.md as the chain-refusal residue; closing it is a
    // separate, measured change with its own fixtures, and it applies equally
    // to the two ladder seams.
    let above = match enclosing_callback_chain(file, call.span, contracts, lookup) {
        Some(chain) => chain.wrappers,
        None => Vec::new(),
    };
    if own.is_none() && above.is_empty() {
        return ForwardedAmbientExecution::Callee;
    }
    let mut wrappers = own.into_iter().collect::<Vec<_>>();
    wrappers.extend(above);
    match compose_callback_chain(&wrappers) {
        Some(execution) => ForwardedAmbientExecution::Composed(execution.to_owned()),
        None => ForwardedAmbientExecution::Unknown,
    }
}

/// The execution recorded for a callback forwarded into a primitive, in the
/// package contracts' vocabulary.
///
/// The effect pair derives from the dialect, because its phases are the
/// headline dialect difference: 2.0 has a deferred apply argument, 1.x has a
/// tracked callback and a seed value.
///
/// `untrack` and 2.0's `flush` sit in the `"inline"` arm beside `createRoot`
/// and `runWithOwner`, which is what the contract vocabulary means by the word:
/// `inline` and `deferred` are the *schedule* axis and describe only callbacks
/// the export does not subscribe, while the clearing fact travels separately
/// through [`solid_dialect::Dialect::runs_callback_synchronously`]
/// (docs/package-contracts.md, "one word over two axes"). This module used to
/// answer `"deferred"` for the pair on the grounds that a consumer reads
/// `"deferred"` as "not tracked here" -- but `"deferred"` is also a promise
/// that the callback does *not* run before the export returns, and every one of
/// these runs it during the call. `contract probe` measures the timing, so the
/// divergence published claims the runtime contradicts. The "not tracked"
/// half is now carried by [`callback_wrapper_at`], which reads the clearing
/// fact from the dialect instead of encoding it in the word.
///
/// This table is also the *reach* of the wrapper-chain fold: a primitive with no
/// row here cannot be classified as a wrapper at all, so a chain containing one
/// is refused ([`enclosing_callback_chain`]) and the row falls back to the
/// lexical answer. `startTransition` and `createResource` are deliberately
/// absent for that reason -- their dialect `Execution` states attribution and
/// not a schedule, and restating it as one is the mistake the fold exists to
/// avoid. `batch`, `createComputed`, `onMount`, `catchError`, `children` and the
/// rest are absent because nobody has established their schedule here yet,
/// which is a precision residue recorded in docs/precision-backlog.md.
fn primitive_callback_execution(
    primitive: Option<Primitive>,
    parameter: usize,
    argument_count: usize,
    dialect: &dyn solid_dialect::Dialect,
) -> Option<&'static str> {
    use Primitive as P;
    let primitive = primitive?;
    if matches!(
        primitive,
        P::CreateEffect | P::CreateRenderEffect | P::CreateResource
    ) {
        return dialect
            .callback_execution_at(primitive, parameter, argument_count)
            .map(|execution| match execution {
                solid_dialect::Execution::Tracked => "tracked",
                solid_dialect::Execution::Deferred => "deferred",
                solid_dialect::Execution::Inline => "inline",
            });
    }
    match (primitive, parameter) {
        // `on` returns an adapter; neither the dependency callback nor the
        // user callback runs during the call that creates that adapter. The
        // dialect labels the dependency callback `Inline` for the checker so
        // its role can be derived from the eventual invocation site, but a
        // package contract must describe the exported wrapper's call itself.
        (P::On, 0 | 1) => Some("deferred"),
        // Solid 1 wraps every function-valued merge source in a memo. JavaScript
        // distributions do not retain the declaration type that proves an
        // ordinary props object is non-callable, so preserve the primitive's
        // conservative callable semantics instead of rejecting the export.
        (P::MergeProps, _) => Some("tracked"),
        (
            P::CreateMemo
            | P::CreateTrackedEffect
            | P::CreateSignal
            | P::CreateStore
            | P::CreateProjection
            | P::CreateOptimistic
            | P::CreateOptimisticStore
            | P::Dynamic,
            0,
        ) => Some("tracked"),
        (P::OnSettled | P::Action | P::CreateReaction | P::OnCleanup, 0) => Some("deferred"),
        (P::CreateRoot | P::Untrack | P::Flush, 0) | (P::RunWithOwner, 1) => Some("inline"),
        _ => None,
    }
}

struct StructuredReturnDiscovery<'a, 'facts> {
    facts: &'a ProjectFacts,
    nodes: &'a [SummaryNode],
    indexes: &'a HashMap<(String, Span), usize>,
    by_symbol: &'a HashMap<SymbolId, usize>,
    summaries: &'a [SummaryReads],
    returned: &'a [SummaryReads],
    structured_returns: &'a [Option<ContractReturn>],
    accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    source_primitives: &'a HashMap<SymbolId, SymbolId>,
    bundled_returns: &'a HashMap<SymbolId, ContractReturn>,
    contract_returns: &'a HashMap<SymbolId, (ContractReturn, Location)>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    lookup: &'a SemanticLookup<'facts>,
}

impl StructuredReturnDiscovery<'_, '_> {
    /// Why a shorthand return value cannot be resolved strongly enough to
    /// prove either a reactive leaf or an exact non-reactive project value.
    /// `None` means the binder/import graph closed the question.
    fn unresolved_shorthand_reason(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
    ) -> Option<String> {
        let property = file
            .ast
            .object_properties
            .iter()
            .find(|property| property.value == span && property.key == property.value)?;
        let Some(declaration) = property.shorthand_binding else {
            return Some(
                "the shorthand resolves only through a global or unavailable binding".into(),
            );
        };
        let Some((module, binding)) = file.ast.imports.iter().find_map(|import| {
            import
                .bindings
                .iter()
                .find(|binding| binding.local.span == declaration)
                .map(|binding| (import.module.as_str(), binding))
        }) else {
            // A local declaration is exact. Whether it is reactive is answered
            // by the local accessor/source indexes, so absence there is a
            // proven non-reactive result rather than uncertainty.
            return None;
        };
        if binding.type_only || matches!(binding.kind, solid_facts::ast::ImportKind::Namespace) {
            return None;
        }
        if !matches!(
            binding.kind,
            solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
        ) {
            return None;
        }
        // A reviewed package contract is the exact runtime owner for this
        // binding. This covers both a direct package import and a relative
        // project barrel whose TypeFacts runtime identity was joined to that
        // package export. Do not require a project declaration in this case:
        // doing so would turn a contracted external function back into
        // SC9012 merely because it crossed a local re-export.
        if let Some(symbol) = self.entities.at(file.path.as_str(), declaration)
            && self.lookup.has_contract_binding(symbol)
        {
            return None;
        }
        // TypeScript already resolved the import, including tsconfig paths,
        // extension priority, and re-export cycles. Follow its exact symbol
        // chain and require a declaration in an analyzed project source file;
        // merely seeing the identity at a project re-export is insufficient,
        // because that export may forward an external package value. Local
        // accessor/source maps decide reactivity from there. External package
        // declarations therefore remain fail-closed even behind a relative
        // project re-export.
        if self.imported_binding_has_project_source_declaration(file, declaration) {
            return None;
        }
        Some(format!(
            "{module:?} resolves to no runtime declaration in the analyzed project"
        ))
    }

    fn parameter_return(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        span: Span,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        let symbol = self.entities.at(file.path.as_str(), span)?;
        if let Some(parameter) = function.parameters.iter().position(|parameter| {
            parameter
                .names
                .iter()
                .any(|name| self.entities.at(file.path.as_str(), name.span) == Some(symbol))
        }) {
            return Some(ContractReturn {
                kind: "argument".into(),
                parameter: Some(parameter),
                ..ContractReturn::default()
            });
        }
        let initializer = self.binding_initializer(file, span)?;
        (initializer != span)
            .then(|| self.parameter_return(file, function, initializer, depth - 1))
            .flatten()
    }

    fn instantiate_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        returned: &ContractReturn,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        match returned.kind.as_str() {
            "argument" => call
                .arguments
                .get(returned.parameter?)
                .and_then(|argument| {
                    self.leaf_with_depth(file, argument.span, fallback_label, depth - 1)
                }),
            "tuple" => Some(ContractReturn {
                elements: returned
                    .elements
                    .iter()
                    .map(|element| {
                        element.as_ref().and_then(|element| {
                            self.instantiate_return(file, call, element, fallback_label, depth - 1)
                        })
                    })
                    .collect(),
                ..returned.clone()
            }),
            "object" => Some(ContractReturn {
                properties: returned
                    .properties
                    .iter()
                    .filter_map(|(name, property)| {
                        self.instantiate_return(file, call, property, name, depth - 1)
                            .map(|property| (name.clone(), property))
                    })
                    .collect(),
                ..returned.clone()
            }),
            _ => Some(returned.clone()),
        }
    }

    fn context_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        let context = call
            .arguments
            .first()
            .and_then(|argument| self.entities.at(file.path.as_str(), argument.span))?;
        self.facts.files.iter().find_map(|provider_file| {
            let jsx_value = provider_file.ast.jsx_elements.iter().find_map(|element| {
                let provider_span =
                    if let Some(provider_member) = self.lookup.dialect.context_provider_member() {
                        (element
                            .member_property
                            .and_then(|property| provider_file.source_text(property))
                            == Some(provider_member))
                        .then_some(element.member_object)
                        .flatten()
                    } else {
                        Some(element.name.span)
                    }?;
                (self.entities.at(provider_file.path.as_str(), provider_span) == Some(context))
                    .then_some(())?;
                let value = element.attributes.iter().find_map(|attribute| {
                    (provider_file.source_text(attribute.local_name) == Some("value"))
                        .then_some(attribute.expression)
                        .flatten()
                })?;
                self.leaf_with_depth(provider_file, value, fallback_label, depth - 1)
            });
            jsx_value.or_else(|| {
                provider_file.ast.calls.iter().find_map(|provider| {
                    let component = provider.arguments.first()?.span;
                    let properties = provider.arguments.get(1)?.span;
                    let provider_context = if let Some(provider_member) =
                        self.lookup.dialect.context_provider_member()
                    {
                        let member = provider_file
                            .ast
                            .members
                            .iter()
                            .find(|member| member.span == component)?;
                        (provider_file.source_text(member.property) == Some(provider_member))
                            .then_some(member.object)?
                    } else {
                        component
                    };
                    (self
                        .entities
                        .at(provider_file.path.as_str(), provider_context)
                        == Some(context))
                    .then_some(())?;
                    let value =
                        provider_file
                            .ast
                            .object_properties
                            .iter()
                            .find_map(|property| {
                                (properties.contains(property.span)
                                    && provider_file.source_text(property.key) == Some("value"))
                                .then_some(property.value)
                            })?;
                    self.leaf_with_depth(provider_file, value, fallback_label, depth - 1)
                })
            })
        })
    }

    fn reactive_proxy_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        fallback_label: &str,
    ) -> Option<ContractReturn> {
        let resolved = self.lookup.resolved_callee_call(file, call.callee)?;
        let declaration = resolved.declaration.as_ref()?;
        if resolved.kind != CallKind::Construct
            || !declaration.standard_library
            || declaration.qualified_name.as_ref() != "ProxyConstructor.construct"
        {
            return None;
        }
        let handler = call.arguments.get(1)?.span;
        let reactive = file.ast.calls.iter().any(|nested| {
            handler.contains(nested.span)
                && super::known_primitive(&primitive_name(
                    file.path.as_str(),
                    nested.callee,
                    nested.static_callee(&file.source),
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                ))
                .is_some_and(|primitive| self.lookup.dialect.creates_reactive_source(primitive))
        });
        reactive.then(|| ContractReturn {
            kind: "store-path".into(),
            label: fallback_label.into(),
            ..ContractReturn::default()
        })
    }

    fn callable_value_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        span: Span,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == span)
        {
            let consequent = self.callable_value_return(
                file,
                call,
                conditional.consequent,
                fallback_label,
                depth - 1,
            );
            let alternate = self.callable_value_return(
                file,
                call,
                conditional.alternate,
                fallback_label,
                depth - 1,
            );
            return match (consequent, alternate) {
                (Some(left), Some(right)) if left.kind == right.kind => {
                    let mut merged = Some(left);
                    merge_structured_return(&mut merged, right);
                    merged
                }
                (Some(returned), None) | (None, Some(returned)) => Some(returned),
                _ => None,
            };
        }
        let function = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == span)?;
        let index = self.indexes.get(&(file.path.to_string(), function.span))?;
        self.structured_returns[*index]
            .as_ref()
            .and_then(|returned| {
                self.instantiate_return(file, call, returned, fallback_label, depth - 1)
            })
    }

    fn call_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        let symbol = self.entities.at(file.path.as_str(), call.callee);
        let primitive = primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            self.entities,
            self.symbol_names,
            self.lookup.dialect,
        );
        if super::known_primitive(&primitive) == Some(Primitive::UseContext)
            && let Some(returned) = self.context_return(file, call, fallback_label, depth - 1)
        {
            return Some(returned);
        }
        if let Some(returned) = symbol
            .and_then(|symbol| self.contract_returns.get(symbol))
            .map(|(returned, _)| returned)
            .or_else(|| {
                primitive
                    .as_ref()
                    .and_then(|primitive| self.bundled_returns.get(primitive.as_str()))
            })
            .or_else(|| {
                call.static_callee(&file.source)
                    .and_then(|callee| self.imported_primitive_return(file, callee))
            })
        {
            return self.instantiate_return(file, call, returned, fallback_label, depth - 1);
        }
        if let Some(primitive) = super::known_primitive(&primitive)
            && self.lookup.dialect.creates_reactive_source(primitive)
        {
            return Some(ContractReturn {
                kind: if self.lookup.dialect.returns_store(primitive) {
                    "store-path".into()
                } else {
                    "accessor".into()
                },
                label: fallback_label.into(),
                ..ContractReturn::default()
            });
        }
        if let Some(returned) = self.reactive_proxy_return(file, call, fallback_label) {
            return Some(returned);
        }
        if file.ast.members.iter().any(|member| {
            member.span == call.callee
                && file
                    .ast
                    .calls
                    .iter()
                    .any(|receiver| receiver.span == member.object)
        }) {
            return None;
        }
        if let Some(initializer) = self.binding_initializer(file, call.callee)
            && let Some(returned) =
                self.callable_value_return(file, call, initializer, fallback_label, depth - 1)
        {
            return Some(returned);
        }
        let index = symbol.and_then(|symbol| self.by_symbol.get(symbol))?;
        if let Some(returned) = self.structured_returns[*index].as_ref() {
            return self.instantiate_return(file, call, returned, fallback_label, depth - 1);
        }
        self.returned[*index]
            .first()
            .map(|read| self.contract_return_from_read(read, fallback_label))
    }

    fn binding_initializer(&self, file: &solid_facts::FileFacts, span: Span) -> Option<Span> {
        if let Some((binding_file, binding, _)) =
            self.lookup.binding_at_reference(file.path.as_str(), span)
            && binding_file.path == file.path
            && let Some(initializer) = binding.initializer
        {
            return Some(initializer);
        }
        if let Some(symbol) = self.entities.at(file.path.as_str(), span)
            && let Some(initializer) = file.ast.bindings.iter().find_map(|binding| {
                binding.names.iter().find_map(|name| {
                    (self.entities.at(file.path.as_str(), name.span) == Some(symbol))
                        .then_some(binding.initializer)
                        .flatten()
                })
            })
        {
            return Some(initializer);
        }
        // TypeScript exposes a shorthand property's own property symbol at
        // `{ value }`, not the referenced value symbol, so neither lookup
        // above names the value. The binder that built these AST facts did
        // resolve that exact reference, so its declaration is the evidence
        // here -- exact, and block-scope aware.
        let declaration = self.shorthand_value_declaration(file, span)?;
        file.ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))
            .and_then(|binding| binding.initializer)
    }

    /// The declaration a shorthand property's value refers to, when `span` is
    /// that shorthand.
    ///
    /// `None` for every other span, and for a shorthand the binder resolved
    /// to no declaration in this file -- a global, or an import namespace
    /// member. Callers treat that as a missing fact, not as proof.
    fn shorthand_value_declaration(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
    ) -> Option<Span> {
        file.ast
            .object_properties
            .iter()
            .find(|property| property.value == span)
            .and_then(|property| property.shorthand_binding)
    }

    fn projection(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        let member = file.ast.members.iter().find(|member| member.span == span)?;
        let base = if let Some(call) = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == member.object)
        {
            self.call_return(file, call, fallback_label, depth - 1)
        } else if file
            .ast
            .members
            .iter()
            .any(|candidate| candidate.span == member.object)
        {
            self.projection(file, member.object, fallback_label, depth - 1)
        } else if let Some(initializer) = self.binding_initializer(file, member.object) {
            self.leaf_with_depth(file, initializer, fallback_label, depth - 1)
        } else {
            self.entities
                .at(file.path.as_str(), member.object)
                .filter(|symbol| self.source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store))
                .map(|_| ContractReturn {
                    kind: "store-path".into(),
                    label: fallback_label.into(),
                    ..ContractReturn::default()
                })
        }?;
        let property = file.source_text(member.property).unwrap_or_default();
        match base.kind.as_str() {
            "object" => base.properties.get(property).cloned(),
            "tuple" => property
                .parse::<usize>()
                .ok()
                .and_then(|index| base.elements.get(index))
                .and_then(Clone::clone),
            "store-path" => Some(ContractReturn {
                kind: "store-path".into(),
                label: fallback_label.into(),
                ..ContractReturn::default()
            }),
            _ => None,
        }
    }

    fn imported_primitive_return<'a>(
        &'a self,
        file: &solid_facts::FileFacts,
        callee: &str,
    ) -> Option<&'a ContractReturn> {
        file.ast.imports.iter().find_map(|import| {
            let primitives = self
                .lookup
                .dialect
                .namespace_import_primitives(import.module.as_str());
            import.bindings.iter().find_map(|binding| {
                let local = file.source_text(binding.local.span)?;
                let imported = match binding.kind {
                    solid_facts::ast::ImportKind::Named if local == callee => binding
                        .imported
                        .as_deref()
                        .or_else(|| file.source_text(binding.local.span)),
                    solid_facts::ast::ImportKind::Namespace => callee
                        .strip_prefix(local)
                        .and_then(|property| property.strip_prefix('.')),
                    _ => None,
                }?;
                primitives
                    .contains(&imported)
                    .then(|| self.bundled_returns.get(imported))
                    .flatten()
            })
        })
    }

    /// The discovered accessor a shorthand property's value names.
    ///
    /// TypeScript projects a shorthand property's *own* symbol at
    /// `{ pathname }`, never the value binding's, so the entity table cannot
    /// answer here. The binder's resolution of that reference can, and it
    /// identifies the declaration exactly: a same-spelled binding in a
    /// sibling block declares a different symbol at a different span and
    /// cannot be mistaken for this one.
    fn named_accessor(&self, file: &solid_facts::FileFacts, span: Span) -> Option<&SymbolId> {
        let declaration = self.shorthand_value_declaration(file, span)?;
        self.accessors
            .iter()
            .find_map(|(symbol, (_, location))| {
                (location.path.as_ref() == file.path.as_str()
                    && u32::try_from(location.start_byte).ok()? == declaration.start
                    && u32::try_from(location.end_byte).ok()? == declaration.end)
                    .then_some(symbol)
            })
            .or_else(|| self.imported_accessor(file, declaration))
    }

    /// The accessor a named import specifier re-exports, when `declaration`
    /// is that specifier's local name span.
    ///
    /// The binder resolves an imported shorthand's value to the import
    /// specifier in this file, where no accessor is declared, and the
    /// TypeScript entity at that span is the alias symbol rather than the
    /// original declaration's. For a plain relative specifier the exporting
    /// file is part of the analyzed project, so the join is exact ESM
    /// resolution against the project's own file set: direct exports,
    /// named/default re-exports, and export-all chains are followed with a
    /// cycle guard, then matched in the accessor map exactly as the
    /// same-file arm matches. Bare or path-mapped specifiers, namespace
    /// imports, ambiguous modules, and unresolved cycles prove nothing and
    /// stay fail-closed.
    fn imported_accessor(
        &self,
        file: &solid_facts::FileFacts,
        declaration: Span,
    ) -> Option<&SymbolId> {
        if let Some(accessor) = self.accessor_with_same_runtime_identity(file, declaration) {
            return Some(accessor);
        }
        let (module, imported) = file.ast.imports.iter().find_map(|import| {
            if import.type_only {
                return None;
            }
            import.bindings.iter().find_map(|binding| {
                (binding.local.span == declaration
                    && matches!(
                        binding.kind,
                        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
                    )
                    && !binding.type_only)
                    .then(|| {
                        (
                            import.module.as_str(),
                            binding.imported.as_deref().unwrap_or("default"),
                        )
                    })
            })
        })?;
        let sibling = self.relative_module_file(file, module)?;
        let mut visiting = HashSet::new();
        let (target_path, target) =
            self.resolve_export_binding(sibling.path.as_str(), imported, &mut visiting)?;
        self.accessors.iter().find_map(|(symbol, (_, location))| {
            (location.path.as_ref() == target_path
                && u32::try_from(location.start_byte).ok()? == target.start
                && u32::try_from(location.end_byte).ok()? == target.end)
                .then_some(symbol)
        })
    }

    fn accessor_with_same_runtime_identity(
        &self,
        file: &solid_facts::FileFacts,
        declaration: Span,
    ) -> Option<&SymbolId> {
        let identity = self
            .lookup
            .entity_at(file.path.as_str(), declaration)?
            .runtime_identity
            .as_ref();
        if identity.is_empty() {
            return None;
        }
        self.accessors.iter().find_map(|(symbol, (_, location))| {
            self.entity_at_location(location)
                .is_some_and(|entity| entity.runtime_identity.as_ref() == identity)
                .then_some(symbol)
        })
    }

    fn imported_binding_has_project_source_declaration(
        &self,
        file: &solid_facts::FileFacts,
        declaration: Span,
    ) -> bool {
        let Some(entity) = self.lookup.entity_at(file.path.as_str(), declaration) else {
            return false;
        };
        if entity.runtime_identity.is_empty() || entity.symbol.is_empty() {
            return false;
        }
        let mut symbol_id = entity.symbol.as_ref();
        let mut seen = HashSet::new();
        while seen.insert(symbol_id) {
            let Some(symbol) = self.facts.typescript.symbol(symbol_id) else {
                return false;
            };
            if symbol.declarations().iter().any(|declaration| {
                !declaration.location.path.ends_with(".d.ts")
                    && self
                        .lookup
                        .file_by_path(declaration.location.path.as_ref())
                        .is_some_and(|target_file| {
                            let is_binding = target_file.ast.identifiers.iter().any(|identifier| {
                                identifier.role == solid_facts::ast::IdentifierRole::Binding
                                    && u64::from(identifier.span.start)
                                        == declaration.location.start_byte
                                    && u64::from(identifier.span.end)
                                        == declaration.location.end_byte
                            });
                            let is_import = target_file.ast.imports.iter().any(|import| {
                                import.bindings.iter().any(|binding| {
                                    u64::from(binding.local.span.start)
                                        == declaration.location.start_byte
                                        && u64::from(binding.local.span.end)
                                            == declaration.location.end_byte
                                })
                            });
                            is_binding && !is_import
                        })
            }) {
                return true;
            }
            let alias = symbol.alias_target();
            if alias.is_empty() {
                return false;
            }
            symbol_id = alias;
        }
        false
    }

    fn entity_at_location(&self, location: &Location) -> Option<&typefacts::EntityFact> {
        self.facts
            .typescript
            .entities_for_path(location.path.as_ref())
            .iter()
            .find(|entity| entity.location == *location)
    }

    /// The project file a plain relative import specifier resolves to, by
    /// textual normalization against the analyzed file set — never the
    /// filesystem, so a file outside the project cannot resolve. `None` for
    /// bare specifiers and any path that walks above the root.
    ///
    /// Extensionless specifiers are ambiguous on their own: `./values` can
    /// name `values.ts`, `values.tsx`, or `values/index.ts`, and which one a
    /// bundler picks depends on resolution settings this pass does not model.
    /// The suffixes are tried in the usual priority order, but when more than
    /// one distinct project file matches the specifier the answer is **not**
    /// the first one enumerated — file order is not evidence — and the
    /// resolution fails closed with `None`. A contract that would have been
    /// generated from the wrong module is a wrong proven claim; no claim is
    /// the correct one. Recorded in `docs/precision-backlog.md`.
    fn relative_module_file(
        &self,
        from: &solid_facts::FileFacts,
        module: &str,
    ) -> Option<&solid_facts::FileFacts> {
        let paths = self.lookup.files().iter().map(|file| file.path.as_str());
        let path = solid_facts::resolve_relative_module_path(from.path.as_str(), module, paths)?;
        self.lookup.file_by_path(path)
    }

    fn resolve_export_binding(
        &self,
        path: &str,
        exported: &str,
        visiting: &mut HashSet<(String, String)>,
    ) -> Option<(String, Span)> {
        let key = (path.to_owned(), exported.to_owned());
        if !visiting.insert(key.clone()) {
            return None;
        }
        let result = (|| {
            let file = self.lookup.file_by_path(path)?;
            let direct = file
                .ast
                .exports
                .iter()
                .filter(|export| !export.type_only && export.module.is_none())
                .flat_map(|export| export.specifiers.iter().chain(&export.declarations))
                .filter(|specifier| !specifier.type_only && specifier.exported.as_str() == exported)
                .map(|specifier| specifier.local.span)
                .collect::<Vec<_>>();
            if direct.len() > 1 {
                return None;
            }
            if let Some(local) = direct.first().copied() {
                return self.resolve_export_local(file, local, visiting);
            }

            let mut result = None;
            for export in file
                .ast
                .exports
                .iter()
                .filter(|export| !export.type_only && export.module.is_some())
            {
                let source_name = match export.kind {
                    solid_facts::ast::ExportKind::Named => export
                        .specifiers
                        .iter()
                        .find(|specifier| {
                            !specifier.type_only && specifier.exported.as_str() == exported
                        })
                        .and_then(|specifier| {
                            file.source_text(specifier.local.span).map(|name| {
                                name.trim_matches(|character| character == '"' || character == '\'')
                            })
                        }),
                    solid_facts::ast::ExportKind::All if exported != "default" => Some(exported),
                    _ => None,
                };
                let Some(source_name) = source_name else {
                    continue;
                };
                let module = export.module.as_deref()?;
                let sibling = self.relative_module_file(file, module)?;
                let candidate =
                    self.resolve_export_binding(sibling.path.as_str(), source_name, visiting);
                let Some(candidate) = candidate else {
                    continue;
                };
                if let Some(existing) = &result
                    && existing != &candidate
                {
                    return None;
                }
                result = Some(candidate);
            }
            result
        })();
        visiting.remove(&key);
        result
    }

    fn resolve_export_local(
        &self,
        file: &solid_facts::FileFacts,
        local: Span,
        visiting: &mut HashSet<(String, String)>,
    ) -> Option<(String, Span)> {
        let symbol = self.lookup.entities().at(file.path.as_str(), local);
        if let Some((import, binding)) = file
            .ast
            .imports
            .iter()
            .flat_map(|import| import.bindings.iter().map(move |binding| (import, binding)))
            .find(|(_, binding)| {
                !binding.type_only
                    && matches!(
                        binding.kind,
                        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
                    )
                    && (binding.local.span == local
                        || symbol.is_some_and(|symbol| {
                            self.lookup
                                .entities()
                                .at(file.path.as_str(), binding.local.span)
                                == Some(symbol)
                        }))
            })
        {
            let sibling = self.relative_module_file(file, import.module.as_str())?;
            let imported = binding.imported.as_deref().unwrap_or("default");
            return self.resolve_export_binding(sibling.path.as_str(), imported, visiting);
        }
        let declaration = symbol.and_then(|symbol| {
            file.ast.bindings.iter().find_map(|binding| {
                binding.names.iter().find_map(|name| {
                    (self.lookup.entities().at(file.path.as_str(), name.span) == Some(symbol))
                        .then_some(name.span)
                })
            })
        });
        Some((file.path.to_string(), declaration.unwrap_or(local)))
    }

    fn leaf(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        fallback_label: &str,
    ) -> Option<ContractReturn> {
        self.leaf_with_depth(file, span, fallback_label, 16)
    }

    fn leaf_with_depth(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ContractReturn> {
        if depth == 0 {
            return None;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == span)
        {
            let consequent =
                self.leaf_with_depth(file, conditional.consequent, fallback_label, depth - 1);
            let alternate =
                self.leaf_with_depth(file, conditional.alternate, fallback_label, depth - 1);
            return match (consequent, alternate) {
                (Some(left), Some(right)) if left.kind == right.kind => {
                    let mut merged = Some(left);
                    merge_structured_return(&mut merged, right);
                    merged
                }
                (Some(returned), None) | (None, Some(returned)) => Some(returned),
                _ => None,
            };
        }
        if let Some(logical) = file
            .ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == span)
        {
            let left = self.leaf_with_depth(file, logical.left, fallback_label, depth - 1);
            let right = self.leaf_with_depth(file, logical.right, fallback_label, depth - 1);
            return match (left, right) {
                (Some(left), Some(right)) if left.kind == right.kind => {
                    let mut merged = Some(left);
                    merge_structured_return(&mut merged, right);
                    merged
                }
                (Some(returned), None) | (None, Some(returned)) => Some(returned),
                _ => None,
            };
        }
        let source = file.source_text(span).unwrap_or_default().trim();
        if source.starts_with('{') && source.ends_with('}') {
            let properties = file
                .ast
                .object_properties
                .iter()
                .filter(|property| {
                    span.contains(property.span)
                        && !file.ast.object_properties.iter().any(|parent| {
                            parent.span != property.span
                                && span.contains(parent.span)
                                && parent.span.contains(property.span)
                        })
                })
                .filter_map(|property| {
                    let name = file.source_text(property.key)?.trim_matches(['\'', '"']);
                    self.leaf_with_depth(file, property.value, name, depth - 1)
                        .map(|returned| (name.to_owned(), returned))
                })
                .collect::<BTreeMap<_, _>>();
            if !properties.is_empty() {
                return Some(ContractReturn {
                    kind: "object".into(),
                    properties,
                    ..ContractReturn::default()
                });
            }
        }
        if file.ast.members.iter().any(|member| member.span == span) {
            return self.projection(file, span, fallback_label, depth - 1);
        }
        if let Some(function) = file
            .ast
            .functions_within(span)
            .filter(|function| function.span == span)
            .min_by_key(|function| function.span.end - function.span.start)
        {
            let summarized = self
                .indexes
                .get(&(file.path.to_string(), function.span))
                .is_some_and(|index| !self.summaries[*index].is_empty());
            let calls_accessor = file.ast.calls.iter().any(|call| {
                function.body.contains(call.span)
                    && self
                        .entities
                        .at(file.path.as_str(), call.callee)
                        .or_else(|| self.named_accessor(file, call.callee))
                        .is_some_and(|symbol| self.accessors.contains_key(symbol))
            });
            if summarized || calls_accessor {
                return Some(ContractReturn {
                    kind: "accessor".into(),
                    label: fallback_label.into(),
                    ..ContractReturn::default()
                });
            }
        }
        if let Some(call) = file
            .ast
            .calls
            .iter()
            .filter(|call| {
                call.span == span || (span.contains(call.span) && call.span.start == span.start)
            })
            .max_by_key(|call| call.span.end - call.span.start)
            && let Some(returned) = self.call_return(file, call, fallback_label, depth - 1)
        {
            return Some(returned);
        }
        if let Some(callee) = file.source_text(span)
            && let Some(returned) = self.imported_primitive_return(file, callee)
        {
            return Some(returned.clone());
        }
        // The entity at a shorthand span is the property's (or an import
        // alias's) symbol, which no source map knows — only when TypeScript's
        // answer misses every map may the binder's shorthand resolution
        // speak, so the entity lookup is filtered to known symbols rather
        // than short-circuiting the join.
        if let Some(symbol) = self
            .entities
            .at(file.path.as_str(), span)
            .filter(|symbol| {
                self.accessors.contains_key(*symbol) || self.by_symbol.contains_key(*symbol)
            })
            .or_else(|| self.named_accessor(file, span))
        {
            if self.accessors.contains_key(symbol) {
                return Some(ContractReturn {
                    kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                        "store-path".into()
                    } else {
                        "accessor".into()
                    },
                    label: fallback_label.into(),
                    ..ContractReturn::default()
                });
            }
            if let Some(index) = self.by_symbol.get(symbol)
                && !self.summaries[*index].is_empty()
            {
                return Some(ContractReturn {
                    kind: "accessor".into(),
                    label: fallback_label.into(),
                    ..ContractReturn::default()
                });
            }
        }
        if let Some(initializer) = self.binding_initializer(file, span)
            && initializer != span
        {
            // A shorthand property (`{ pathname }`) carries the property's own
            // symbol at this span, not the value binding's, so the lookup above
            // cannot see that the value is a discovered source. The binding that
            // owns the initializer we just followed can: match it by initializer
            // span, then ask for that binding's own symbol. Without this the
            // leaf falls through to the initializing call and inherits the
            // primitive's generic label ("memo result") instead of the
            // structural position the consumer actually reads.
            if let Some(symbol) = file
                .ast
                .bindings
                .iter()
                .find(|binding| binding.initializer == Some(initializer))
                .and_then(|binding| binding.names.first())
                .and_then(|name| self.entities.at(file.path.as_str(), name.span))
                && self.accessors.contains_key(symbol)
            {
                return Some(ContractReturn {
                    kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                        "store-path".into()
                    } else {
                        "accessor".into()
                    },
                    label: fallback_label.into(),
                    ..ContractReturn::default()
                });
            }
            if let Some(returned) =
                self.leaf_with_depth(file, initializer, fallback_label, depth - 1)
            {
                return Some(returned);
            }
        }
        if let Some(member) = file.ast.members.iter().find(|member| member.span == span)
            && let Some(symbol) = self.entities.at(file.path.as_str(), member.object)
            && self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
        {
            return Some(ContractReturn {
                kind: "store-path".into(),
                label: fallback_label.into(),
                ..ContractReturn::default()
            });
        }
        None
    }

    fn contract_return_from_read(
        &self,
        read: &SummaryRead,
        fallback_label: &str,
    ) -> ContractReturn {
        let label = self
            .source_primitives
            .get(&read.symbol)
            .and_then(|primitive| self.bundled_returns.get(primitive))
            .map_or_else(
                || fallback_label.to_owned(),
                |returned| returned.label.clone(),
            );
        ContractReturn {
            kind: if self.source_kinds.get(&read.symbol) == Some(&ReactiveSourceKind::Store) {
                "store-path".into()
            } else {
                "accessor".into()
            },
            label,
            ..ContractReturn::default()
        }
    }
}

fn discover_structured_returns(
    discovery: &StructuredReturnDiscovery<'_, '_>,
) -> Vec<Option<ContractReturn>> {
    parallel_slice_results(discovery.nodes, |node| {
        let file = discovery.lookup.file_by_path(node.path.as_str())?;
        let function = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == node.span)?;
        function
            .expression_return
            .iter()
            .chain(file.ast.returns.iter().filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            }))
            .find_map(|returned| {
                if !returned.elements().is_empty() {
                    let elements = returned
                        .elements()
                        .iter()
                        .enumerate()
                        .map(|(index, element)| {
                            element.and_then(|span| {
                                discovery.leaf(file, span, &format!("result[{index}]"))
                            })
                        })
                        .collect::<Vec<_>>();
                    if elements.iter().any(Option::is_some) {
                        return Some(ContractReturn {
                            kind: "tuple".into(),
                            elements,
                            ..ContractReturn::default()
                        });
                    }
                }
                if !returned.properties().is_empty() {
                    let properties = returned
                        .properties()
                        .iter()
                        .filter_map(|property| {
                            discovery
                                .leaf(file, property.value, property.name.as_str())
                                .map(|returned| (property.name.to_string(), returned))
                        })
                        .collect::<BTreeMap<_, _>>();
                    if !properties.is_empty() {
                        return Some(ContractReturn {
                            kind: "object".into(),
                            properties,
                            ..ContractReturn::default()
                        });
                    }
                }
                if let Some(argument) =
                    discovery.parameter_return(file, function, returned.span, 16)
                {
                    return Some(argument);
                }
                discovery.leaf(file, returned.span, "result")
            })
    })
}

fn merge_structured_return(target: &mut Option<ContractReturn>, incoming: ContractReturn) -> bool {
    let Some(current) = target.as_mut() else {
        *target = Some(incoming);
        return true;
    };
    if current.kind != incoming.kind {
        return false;
    }
    match current.kind.as_str() {
        "tuple" => {
            if current.elements.len() < incoming.elements.len() {
                current.elements.resize(incoming.elements.len(), None);
            }
            incoming
                .elements
                .into_iter()
                .enumerate()
                .fold(false, |changed, (index, returned)| {
                    changed
                        | returned.is_some_and(|returned| {
                            merge_structured_return(&mut current.elements[index], returned)
                        })
                })
        }
        "object" => incoming
            .properties
            .into_iter()
            .fold(false, |changed, (property, returned)| {
                if let Some(existing) = current.properties.get_mut(&property) {
                    let mut slot = Some(existing.clone());
                    let property_changed = merge_structured_return(&mut slot, returned);
                    *existing = slot.expect("an occupied property remains occupied");
                    changed | property_changed
                } else {
                    current.properties.insert(property, returned);
                    true
                }
            }),
        _ => false,
    }
}

struct InterproceduralGraphAssembly<'a> {
    nodes: &'a [SummaryNode],
    nodes_by_path: &'a HashMap<String, Vec<usize>>,
    by_symbol: &'a HashMap<SymbolId, usize>,
    summaries: &'a mut [SummaryReads],
    callback_summaries: &'a mut [Vec<ContractCallback>],
    callback_forwardings: &'a mut Vec<(usize, usize, usize, usize, ForwardedAmbientExecution)>,
    dispatches: &'a mut Vec<(usize, Vec<usize>)>,
    contract_generation_obligations: &'a mut [Vec<ContractGenerationObligation>],
    contract_consumer_obligations: &'a mut Vec<StaticDefect>,
    edges: &'a mut [Vec<usize>],
    invoked_parameters: &'a mut [Vec<usize>],
    escaped_parameters: &'a mut [Vec<usize>],
    invoked_parameter_members: &'a mut [Vec<(usize, String)>],
    returned_bindings: &'a mut Vec<(SymbolId, SymbolId)>,
    factory_calls: &'a mut Vec<(usize, SymbolId)>,
}

impl InterproceduralGraphAssembly<'_> {
    fn merge(&mut self, path: &str, contribution: &InterproceduralGraphContribution) {
        let node_index = |span| {
            self.nodes_by_path.get(path).and_then(|indices| {
                indices
                    .iter()
                    .rev()
                    .find(|index| self.nodes[**index].span == span)
                    .copied()
            })
        };
        for (owner, read) in &contribution.direct_reads {
            if let Some(owner) = node_index(*owner) {
                self.summaries[owner].push_unique(read.clone());
            }
        }
        for (owner, target) in &contribution.edges {
            let Some(owner) = node_index(*owner) else {
                continue;
            };
            let target = match target {
                InterproceduralGraphTarget::Symbol(symbol) => self.by_symbol.get(symbol).copied(),
                InterproceduralGraphTarget::LocalSpan(span) => node_index(*span),
            };
            if let Some(target) = target {
                self.edges[owner].push(target);
            }
        }
        for (owner, parameter) in &contribution.invoked_parameters {
            if let Some(owner) = node_index(*owner) {
                self.invoked_parameters[owner].push(*parameter);
            }
        }
        for (owner, parameter) in &contribution.escaped_parameters {
            if let Some(owner) = node_index(*owner)
                && !self.escaped_parameters[owner].contains(parameter)
            {
                self.escaped_parameters[owner].push(*parameter);
            }
        }
        for (owner, parameter, property) in &contribution.invoked_parameter_members {
            if let Some(owner) = node_index(*owner) {
                let entry = (*parameter, property.clone());
                if !self.invoked_parameter_members[owner].contains(&entry) {
                    self.invoked_parameter_members[owner].push(entry);
                }
            }
        }
        for (owner, callback) in &contribution.callbacks {
            if let Some(owner) = node_index(*owner) {
                push_contract_callback(&mut self.callback_summaries[owner], callback.clone());
            }
        }
        for (owner, target, target_parameter, owner_parameter, ambient_execution) in
            &contribution.callback_forwardings
        {
            let target = match target {
                InterproceduralGraphTarget::Symbol(symbol) => self.by_symbol.get(symbol).copied(),
                InterproceduralGraphTarget::LocalSpan(span) => node_index(*span),
            };
            if let (Some(owner), Some(target)) = (node_index(*owner), target) {
                self.callback_forwardings.push((
                    owner,
                    target,
                    *target_parameter,
                    *owner_parameter,
                    ambient_execution.clone(),
                ));
            }
        }
        for (owner, candidates) in &contribution.dispatches {
            let Some(owner) = node_index(*owner) else {
                continue;
            };
            let mut resolved = Vec::with_capacity(candidates.len());
            let mut complete = true;
            for symbol in candidates {
                if let Some(candidate) = self.by_symbol.get(symbol).copied() {
                    resolved.push(candidate);
                } else {
                    // A candidate omitted from the project graph is not
                    // equivalent to the candidates we did resolve. Keep the
                    // dispatch unresolved instead of silently narrowing the
                    // runtime set.
                    complete = false;
                    break;
                }
            }
            resolved.sort_unstable();
            resolved.dedup();
            if complete && !resolved.is_empty() && resolved.len() == candidates.len() {
                self.dispatches.push((owner, resolved));
            }
        }
        for (owner, obligation) in &contribution.contract_generation_obligations {
            if let Some(owner) = node_index(*owner)
                && !self.contract_generation_obligations[owner].contains(obligation)
            {
                self.contract_generation_obligations[owner].push(obligation.clone());
            }
        }
        self.contract_consumer_obligations
            .extend(contribution.contract_consumer_obligations.iter().cloned());
        self.returned_bindings
            .extend(contribution.returned_bindings.iter().cloned());
        for (owner, symbol) in &contribution.factory_calls {
            if let Some(owner) = node_index(*owner) {
                self.factory_calls.push((owner, symbol.clone()));
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct InterproceduralResultView<'a> {
    pub(super) nodes: &'a [SummaryNode],
    pub(super) indexes: &'a HashMap<(String, Span), usize>,
    pub(super) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(super) summaries: &'a [SummaryReads],
    pub(super) invoked_parameters: &'a [Vec<usize>],
    pub(super) invoked_parameter_members: &'a [Vec<(usize, String)>],
    pub(super) returned_bindings: &'a HashMap<SymbolId, Vec<SummaryRead>>,
}

impl InterproceduralResultView<'_> {
    fn dependency_state(
        &self,
        dependency: &InterproceduralResultDependency,
    ) -> InterproceduralResultDependencyState {
        match dependency {
            InterproceduralResultDependency::Symbol(symbol) => {
                if let Some(index) = self.by_symbol.get(symbol) {
                    InterproceduralResultDependencyState::Function {
                        name: self.nodes[*index].name.clone(),
                        summary: self.summaries[*index].to_vec(),
                        invoked_parameters: self.invoked_parameters[*index].clone(),
                        invoked_parameter_members: self.invoked_parameter_members[*index].clone(),
                    }
                } else if let Some(summary) = self.returned_bindings.get(symbol) {
                    InterproceduralResultDependencyState::Returned(summary.clone())
                } else {
                    InterproceduralResultDependencyState::Missing
                }
            }
            InterproceduralResultDependency::InlineFunction(path, span) => self
                .indexes
                .get(&(path.clone(), *span))
                .map_or(InterproceduralResultDependencyState::Missing, |index| {
                    InterproceduralResultDependencyState::Inline(self.summaries[*index].to_vec())
                }),
        }
    }

    pub(super) fn dependency_matches(
        &self,
        retained: &InterproceduralResultDependencyState,
        dependency: &InterproceduralResultDependency,
    ) -> bool {
        match dependency {
            InterproceduralResultDependency::Symbol(symbol) => {
                if let Some(index) = self.by_symbol.get(symbol) {
                    matches!(
                        retained,
                        InterproceduralResultDependencyState::Function {
                            name,
                            summary,
                            invoked_parameters: previous_parameters,
                            invoked_parameter_members: previous_members,
                        } if name == &self.nodes[*index].name
                            && summary.as_slice() == &self.summaries[*index][..]
                            && previous_parameters == &self.invoked_parameters[*index]
                            && previous_members == &self.invoked_parameter_members[*index]
                    )
                } else if let Some(summary) = self.returned_bindings.get(symbol) {
                    matches!(
                        retained,
                        InterproceduralResultDependencyState::Returned(previous)
                            if previous == summary
                    )
                } else {
                    matches!(retained, InterproceduralResultDependencyState::Missing)
                }
            }
            InterproceduralResultDependency::InlineFunction(path, span) => {
                if let Some(index) = self.indexes.get(&(path.clone(), *span)) {
                    matches!(
                        retained,
                        InterproceduralResultDependencyState::Inline(previous)
                            if previous.as_slice() == &self.summaries[*index][..]
                    )
                } else {
                    matches!(retained, InterproceduralResultDependencyState::Missing)
                }
            }
        }
    }
}

fn add_interprocedural_dependency_user(
    users: &mut HashMap<InterproceduralResultDependency, usize>,
    dependency: &InterproceduralResultDependency,
) {
    *users.entry(dependency.clone()).or_default() += 1;
}

fn remove_interprocedural_dependency_user(
    users: &mut HashMap<InterproceduralResultDependency, usize>,
    states: &mut HashMap<InterproceduralResultDependency, InterproceduralResultDependencyState>,
    dependency: &InterproceduralResultDependency,
) {
    let Some(count) = users.get_mut(dependency) else {
        debug_assert!(false, "missing interprocedural dependency reference count");
        return;
    };
    *count -= 1;
    if *count == 0 {
        users.remove(dependency);
        states.remove(dependency);
    }
}

pub(super) struct InterproceduralResultReadContext<'a, 'b> {
    pub(super) result: InterproceduralResultView<'a>,
    pub(super) contract_callbacks: &'a HashMap<SymbolId, Vec<ContractCallback>>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) lookup: &'a SemanticLookup<'b>,
}

fn interprocedural_result_reads_for_file(
    file: &solid_facts::FileFacts,
    context: &InterproceduralResultReadContext<'_, '_>,
) -> (
    Vec<ReactiveRead>,
    Vec<StaticDefect>,
    HashSet<InterproceduralResultDependency>,
) {
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                nodes,
                by_symbol,
                summaries,
                invoked_parameters,
                invoked_parameter_members,
                returned_bindings,
                ..
            },
        contract_callbacks,
        source_kinds,
        accessors,
        entities,
        symbol_names,
        lookup,
    } = context;
    let mut result = Vec::new();
    let mut dispatch_obligations = Vec::new();
    let mut dependencies = HashSet::new();
    let mut seen = HashSet::new();
    let allowed = allowed_callback_spans(file, lookup);
    for call in &file.ast.calls {
        if !enclosing_render_function(file, call.span, lookup) {
            continue;
        }
        let callee = location(file.path.shared(), call.callee);
        let label = call
            .static_callee(&file.source)
            .map(str::to_owned)
            .unwrap_or_else(|| "dynamic call".into());
        let valid_call = lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| resolved.validity == ResolvedCallValidity::Valid);
        if lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| declaration.standard_library)
        {
            // Type Facts tied this exact call target to the compiler's
            // standard/platform library declaration. Its implementation is
            // outside the package and cannot contribute package-owned
            // reactive reads; callback bodies were already connected by the
            // graph's exact argument-behavior proof.
            continue;
        }
        // A member invoked on the innermost function's parameter is not a
        // local runtime-dispatch choice. The graph records it in
        // `invoked_parameter_members`, and each caller either selects an
        // exact implementation or receives one call-site obligation. Also
        // reporting the unresolved member here duplicates that same proof
        // obligation at the helper definition.
        let parameter_member_call = lookup
            .member_callee_receiver(file, call.callee)
            .is_some_and(|(receiver, _)| {
                nodes
                    .iter()
                    .filter(|node| node.path == file.path.as_str() && node.body.contains(call.span))
                    .min_by_key(|node| node.body.end - node.body.start)
                    .is_some_and(|owner| owner.parameters.contains(&receiver))
            });
        if parameter_member_call {
            continue;
        }
        let candidate_symbols = lookup.callee_symbols(file, call.callee);
        if candidate_symbols.is_empty()
            && file
                .ast
                .computed_members
                .binary_search(&file.ast.peel_ts_sugar_span(call.callee))
                .is_ok()
        {
            if valid_call {
                dispatch_obligations.push(StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: label,
                        member: None,
                    },
                    location: callee,
                    analysis_context: "computed-call-target-unresolved".into(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
            continue;
        }
        let ambiguous_candidates = (candidate_symbols.len() > 1).then_some(&candidate_symbols);
        let Some(symbol) = candidate_symbols
            .first()
            .map(SymbolId::as_str)
            .or_else(|| lookup.callee_symbol(file, call.callee))
        else {
            continue;
        };
        dependencies.insert(InterproceduralResultDependency::Symbol(SymbolId::from(
            symbol,
        )));
        let (label, mut effective, target) = if let Some(candidates) = ambiguous_candidates {
            let mut candidate_summaries = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                dependencies.insert(InterproceduralResultDependency::Symbol(candidate.clone()));
                if let Some(index) = by_symbol.get(candidate) {
                    candidate_summaries.push(&summaries[*index]);
                }
            }
            let equivalent = candidate_summaries.len() == candidates.len()
                && candidate_summaries
                    .split_first()
                    .is_some_and(|(first, rest)| {
                        rest.iter()
                            .all(|candidate| equivalent_summary_reads(candidate, first))
                    });
            if !equivalent {
                dispatch_obligations.push(StaticDefect {
                    kind: StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: label,
                        member: None,
                    },
                    location: callee,
                    analysis_context: "runtime-call-targets-diverge".into(),
                    fixes: vec![],
                    uncertain: true,
                });
                continue;
            }
            (
                label,
                candidate_summaries[0].to_vec(),
                by_symbol.get(&candidates[0]).copied(),
            )
        } else if let Some(target) = by_symbol.get(symbol).copied() {
            (
                nodes[target].name.clone().unwrap_or(label),
                summaries[target].to_vec(),
                Some(target),
            )
        } else if let Some(summary) = returned_bindings.get(symbol) {
            (label, summary.clone(), None)
        } else if contract_callbacks.contains_key(symbol) {
            (label, Vec::new(), None)
        } else {
            continue;
        };
        if let Some(target) = target {
            for parameter in &invoked_parameters[target] {
                let Some(argument) = call.arguments.get(*parameter) else {
                    continue;
                };
                let Some(argument_symbol) =
                    entities.get(&location(file.path.shared(), argument.span))
                else {
                    continue;
                };
                dependencies.insert(InterproceduralResultDependency::Symbol(
                    argument_symbol.clone(),
                ));
                let argument_summary = by_symbol
                    .get(argument_symbol)
                    .map(|index| &summaries[*index][..])
                    .or_else(|| returned_bindings.get(argument_symbol).map(Vec::as_slice));
                if let Some(argument_summary) = argument_summary {
                    for read in argument_summary {
                        push_unique_summary_read(&mut effective, read.clone());
                    }
                }
            }
            // The callee invokes a member of one of its parameters. Which
            // implementation that is belongs to this call site, not to the
            // callee: resolve it from the argument actually passed here. An
            // argument that is exactly one object proves what runs; anything
            // else -- unresolved, or a conditional over two objects -- proves
            // nothing and contributes no read.
            for (parameter, property) in &invoked_parameter_members[target] {
                let Some(argument) = call.arguments.get(*parameter) else {
                    continue;
                };
                if property.is_empty() {
                    let argument_location = location(file.path.shared(), argument.span);
                    let argument_symbol = entities.get(&argument_location);
                    if let Some(argument_symbol) = argument_symbol
                        && source_kinds.get(argument_symbol.as_str())
                            == Some(&ReactiveSourceKind::Store)
                    {
                        push_unique_summary_read(
                            &mut effective,
                            SummaryRead {
                                symbol: argument_symbol.clone(),
                                display: SymbolId::from(
                                    file.source_text(argument.span).unwrap_or("store"),
                                ),
                                kind: Some("store-path".into()),
                                declaration: accessors
                                    .get(argument_symbol.as_str())
                                    .map(|(_, location)| location.clone())
                                    .unwrap_or_else(|| argument_location.clone()),
                                origin: location(file.path.shared(), call.span),
                                origin_context: label.clone(),
                            },
                        );
                    } else if !crate::local_access::argument_proves_non_reactive(
                        file,
                        argument,
                        entities,
                        source_kinds,
                        lookup,
                    ) && valid_call
                    {
                        dispatch_obligations.push(StaticDefect {
                            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                                callee: label.clone(),
                                member: None,
                            },
                            location: argument_location,
                            analysis_context: "contract-parameter-member-argument-unresolved"
                                .into(),
                            fixes: vec![],
                            uncertain: true,
                        });
                    }
                    continue;
                }
                let implementations = lookup.member_value_symbols_at(file, argument.span, property);
                let mut member_summaries = Vec::with_capacity(implementations.len());
                for implementation in &implementations {
                    dependencies.insert(InterproceduralResultDependency::Symbol(
                        implementation.clone(),
                    ));
                    if let Some(index) = by_symbol.get(implementation) {
                        member_summaries.push(&summaries[*index]);
                    }
                }
                let equivalent = member_summaries.len() == implementations.len()
                    && member_summaries.split_first().is_some_and(|(first, rest)| {
                        rest.iter()
                            .all(|candidate| equivalent_summary_reads(candidate, first))
                    });
                if !equivalent {
                    if valid_call {
                        dispatch_obligations.push(StaticDefect {
                            kind: StaticDefectKind::ReactiveDispatchUnresolved {
                                callee: label.clone(),
                                member: Some(property.clone()),
                            },
                            location: location(file.path.shared(), argument.span),
                            analysis_context: if implementations.is_empty() {
                                "parameter-member-target-unresolved".into()
                            } else {
                                "parameter-member-targets-diverge".into()
                            },
                            fixes: vec![],
                            uncertain: true,
                        });
                    }
                    continue;
                }
                for read in member_summaries[0].iter() {
                    push_unique_summary_read(&mut effective, read.clone());
                }
            }
        }
        let execution =
            semantic_execution_role(file, call.callee, &allowed, entities, symbol_names, lookup);
        let mut context = None::<String>;
        if let Some(callbacks) = contract_callbacks.get(symbol) {
            for callback in callbacks {
                let Some(argument) = call.arguments.get(callback.parameter) else {
                    continue;
                };
                let argument_symbol = entities.get(&location(file.path.shared(), argument.span));
                let argument_summary = argument_symbol
                    .and_then(|argument_symbol| {
                        dependencies.insert(InterproceduralResultDependency::Symbol(
                            argument_symbol.clone(),
                        ));
                        by_symbol
                            .get(argument_symbol)
                            .map(|index| &summaries[*index][..])
                            .or_else(|| returned_bindings.get(argument_symbol).map(Vec::as_slice))
                    })
                    .or_else(|| {
                        nodes
                            .iter()
                            .enumerate()
                            .filter(|(_, node)| {
                                node.path == file.path.as_str() && argument.span.contains(node.span)
                            })
                            .min_by_key(|(_, node)| node.span.end - node.span.start)
                            .map(|(index, node)| {
                                dependencies.insert(
                                    InterproceduralResultDependency::InlineFunction(
                                        node.path.clone(),
                                        node.span,
                                    ),
                                );
                                &summaries[index][..]
                            })
                    });
                let Some(argument_summary) = argument_summary else {
                    continue;
                };
                let callback_execution = match callback.execution.as_str() {
                    "tracked" => ExecutionRole::TrackedJsx,
                    "deferred" => ExecutionRole::DeferredCallback,
                    _ => execution,
                };
                for read in argument_summary {
                    if seen.insert((
                        callee.path.clone(),
                        callee.start_byte,
                        format!(
                            "{}#callback-{}-{callback_execution:?}",
                            read.symbol, callback.parameter
                        ),
                    )) {
                        result.push(ReactiveRead {
                            kind: "accessor".into(),
                            accessor: read.display.to_string().into(),
                            location: location(file.path.shared(), call.span),
                            declaration: read.declaration.clone(),
                            execution: callback_execution,
                            context: context
                                .get_or_insert_with(|| enclosing_function_label(file, call.span))
                                .clone()
                                .into(),
                            via: label.clone().into(),
                            origin: Some(read.origin.clone()),
                            origin_context: read.origin_context.clone().into(),
                            uncertain: false,
                        });
                    }
                }
            }
        }
        for read in effective {
            let accessor = read.display.to_string();
            if seen.insert((
                callee.path.clone(),
                callee.start_byte,
                read.symbol.to_string(),
            )) {
                result.push(ReactiveRead {
                    kind: read
                        .kind
                        .clone()
                        .unwrap_or_else(|| "accessor".into())
                        .into(),
                    accessor: accessor.into(),
                    location: location(file.path.shared(), call.span),
                    declaration: read.declaration,
                    execution,
                    context: context
                        .get_or_insert_with(|| enclosing_function_label(file, call.span))
                        .clone()
                        .into(),
                    via: label.clone().into(),
                    origin: Some(read.origin),
                    origin_context: read.origin_context.into(),
                    uncertain: false,
                });
            }
        }
    }
    (result, dispatch_obligations, dependencies)
}

fn cached_reactive_source(
    symbol: &str,
    display: &str,
    declaration: &Location,
    source_phases: &HashMap<SymbolId, u8>,
) -> CachedReactiveSource {
    CachedReactiveSource {
        symbol: SymbolId::from(symbol),
        display: SymbolId::from(display),
        declaration: declaration.clone(),
        phase: source_phases.get(symbol).copied().unwrap_or(1),
    }
}

fn reactive_source_order(
    left: &CachedReactiveSource,
    right: &CachedReactiveSource,
) -> std::cmp::Ordering {
    left.phase
        .cmp(&right.phase)
        .then_with(|| location_order(&left.declaration, &right.declaration))
}

fn retained_reactive_sources(
    cache: &mut Option<Arc<Vec<CachedReactiveSource>>>,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    contracted_accessor_symbols: &HashSet<SymbolId>,
    summary_source_symbols: &HashSet<SymbolId>,
    source_phases: &HashMap<SymbolId, u8>,
) -> Arc<Vec<CachedReactiveSource>> {
    let eligible = |symbol: &str| {
        !contracted_accessor_symbols.contains(symbol) && summary_source_symbols.contains(symbol)
    };
    let matches = |source: &CachedReactiveSource| {
        eligible(source.symbol.as_str())
            && accessors
                .get(source.symbol.as_str())
                .is_some_and(|(display, declaration)| {
                    display == &source.display
                        && declaration == &source.declaration
                        && source.phase
                            == source_phases
                                .get(source.symbol.as_str())
                                .copied()
                                .unwrap_or(1)
                })
    };
    let eligible_count = accessors
        .keys()
        .filter(|symbol| eligible(symbol.as_str()))
        .count();
    if let Some(retained) = cache.as_ref()
        && retained.len() == eligible_count
        && retained.iter().all(matches)
    {
        return retained.clone();
    }

    if cache.is_none() {
        let mut sources = accessors
            .iter()
            .filter(|(symbol, _)| eligible(symbol.as_str()))
            .map(|(symbol, (display, declaration))| {
                cached_reactive_source(symbol, display, declaration, source_phases)
            })
            .collect::<Vec<_>>();
        sources.sort_by(reactive_source_order);
        let sources = Arc::new(sources);
        *cache = Some(sources.clone());
        return sources;
    }

    let retained = Arc::make_mut(cache.as_mut().expect("reactive sources initialized"));
    retained.retain(&matches);
    let mut retained_symbols = retained
        .iter()
        .map(|source| source.symbol.clone())
        .collect::<HashSet<_>>();
    for (symbol, (display, declaration)) in accessors {
        if !eligible(symbol.as_str()) || retained_symbols.contains(symbol.as_str()) {
            continue;
        }
        let source = cached_reactive_source(symbol, display, declaration, source_phases);
        let insert_at = retained.partition_point(|current| {
            reactive_source_order(current, &source) != std::cmp::Ordering::Greater
        });
        retained.insert(insert_at, source);
        retained_symbols.insert(symbol.clone());
    }
    cache.as_ref().expect("reactive sources retained").clone()
}

fn direct_reference_contributions(
    source: &CachedReactiveSource,
    context: &InterproceduralContext<'_, '_>,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
) -> Vec<DirectReferenceContribution> {
    let InterproceduralContext {
        references_by_source,
        project_indexes,
        entities,
        symbol_names,
        source_primitives,
        bundled_returns,
        source_kinds,
        lookup,
        ..
    } = context;
    let mut contributions = Vec::new();
    for reference in references_by_source
        .get(source.symbol.as_str())
        .into_iter()
        .flatten()
    {
        let Some(&file) = project_indexes.files_by_path.get(reference.path.as_ref()) else {
            continue;
        };
        let Ok(start) = u32::try_from(reference.start_byte) else {
            continue;
        };
        let Ok(end) = u32::try_from(reference.end_byte) else {
            continue;
        };
        let reference_span = Span::new(start, end);
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            reference_span,
        ) else {
            continue;
        };
        if read_escapes_synchronous_extent(
            file,
            reference_span,
            entities,
            symbol_names,
            lookup.dialect,
        ) {
            continue;
        }
        if let Some(call) = project_indexes
            .ast_files_by_path
            .get(file.path.as_str())
            .and_then(|index| index.direct_call_by_callee(reference_span))
        {
            let mut read = SummaryRead {
                symbol: source.symbol.clone(),
                display: source.display.clone(),
                kind: Some(
                    match source_kinds.get(source.symbol.as_str()) {
                        Some(ReactiveSourceKind::Store) => "store-path",
                        Some(ReactiveSourceKind::Accessor) | None => "accessor",
                    }
                    .into(),
                ),
                declaration: source.declaration.clone(),
                origin: location(file.path.shared(), call.span),
                origin_context: nodes[owner].name.clone().unwrap_or_default(),
            };
            let factory_return =
                source_primitives
                    .get(source.symbol.as_str())
                    .and_then(|primitive| {
                        bundled_returns
                            .get(primitive)
                            .map(|returned| (primitive, returned))
                    });
            if let Some((primitive, returned)) = factory_return {
                let contract_location = bundled_contract_location(lookup.dialect, primitive);
                read.display = SymbolId::from(returned.label.as_str());
                read.kind = Some(returned.kind.clone());
                read.declaration.clone_from(&contract_location);
                if semantic_execution_role(
                    file,
                    call.callee,
                    &[],
                    entities,
                    symbol_names,
                    context.lookup,
                )
                .reports_untracked_read()
                    && !enclosing_render_function(file, call.span, context.lookup)
                {
                    read.origin = contract_location;
                }
                contributions.push(DirectReferenceContribution {
                    owner,
                    read,
                    unique: true,
                });
            } else {
                contributions.push(DirectReferenceContribution {
                    owner,
                    read,
                    unique: false,
                });
            }
            continue;
        }
        if source_kinds.get(source.symbol.as_str()) == Some(&ReactiveSourceKind::Store) {
            contributions.extend(
                file.ast
                    .members
                    .iter()
                    .filter(|member| member.object == reference_span)
                    .map(|member| DirectReferenceContribution {
                        owner,
                        read: SummaryRead {
                            symbol: source.symbol.clone(),
                            display: SymbolId::from(format!(
                                "{}.{}",
                                source.display,
                                file.source_text(member.property).unwrap_or_default()
                            )),
                            kind: Some("store-path".into()),
                            declaration: source.declaration.clone(),
                            origin: location(file.path.shared(), member.span),
                            origin_context: nodes[owner].name.clone().unwrap_or_default(),
                        },
                        unique: false,
                    }),
            );
        }
    }
    contributions
}

pub(super) struct InterproceduralContext<'a, 'facts> {
    pub(super) facts: &'a ProjectFacts,
    pub(super) project_indexes: &'a ProjectIndexes<'facts>,
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) contracted_accessor_symbols: &'a HashSet<SymbolId>,
    pub(super) returned_source_symbols: &'a HashSet<SymbolId>,
    pub(super) summary_source_symbols: &'a HashSet<SymbolId>,
    pub(super) source_phases: &'a HashMap<SymbolId, u8>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) contract_reads: &'a HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    pub(super) contract_parameter_reads:
        &'a HashMap<SymbolId, Vec<(usize, String, String, Location)>>,
    pub(super) contract_callbacks: &'a HashMap<SymbolId, Vec<ContractCallback>>,
    pub(super) contract_returns: &'a HashMap<SymbolId, (ContractReturn, Location)>,
    pub(super) bundled_returns: &'a HashMap<SymbolId, ContractReturn>,
    pub(super) source_primitives: &'a HashMap<SymbolId, SymbolId>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) references_by_source: &'a HashMap<SymbolId, Vec<Location>>,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) changed_semantic_symbols: Option<&'a HashSet<SymbolId>>,
    pub(super) retained_source_paths: &'a HashSet<String>,
    pub(super) lookup: &'a SemanticLookup<'facts>,
}

impl InterproceduralContext<'_, '_> {
    pub(super) fn build(
        &self,
        typed_accessor_cache: Option<
            &mut HashMap<solid_facts::core::SourcePath, CachedTypedAccessors>,
        >,
        interprocedural_graph_cache: Option<
            &mut HashMap<solid_facts::core::SourcePath, CachedInterproceduralGraph>,
        >,
        interprocedural_result_cache: Option<&mut CachedInterproceduralResults>,
    ) -> InterproceduralResult {
        interprocedural_reads(
            self,
            InterproceduralCaches {
                typed_accessors: typed_accessor_cache,
                graph: interprocedural_graph_cache,
                results: interprocedural_result_cache,
            },
        )
    }
}

struct InterproceduralCaches<'a> {
    typed_accessors: Option<&'a mut HashMap<solid_facts::core::SourcePath, CachedTypedAccessors>>,
    graph: Option<&'a mut HashMap<solid_facts::core::SourcePath, CachedInterproceduralGraph>>,
    results: Option<&'a mut CachedInterproceduralResults>,
}

fn interprocedural_reads(
    context: &InterproceduralContext<'_, '_>,
    caches: InterproceduralCaches<'_>,
) -> InterproceduralResult {
    let InterproceduralContext {
        facts,
        project_indexes,
        accessors,
        contracted_accessor_symbols,
        returned_source_symbols,
        summary_source_symbols,
        source_phases,
        source_kinds,
        contract_reads,
        contract_parameter_reads,
        contract_callbacks,
        contract_returns,
        bundled_returns,
        source_primitives,
        entities,
        references_by_source: _,
        symbol_names,
        changed_semantic_symbols,
        retained_source_paths,
        lookup,
    } = context;
    let InterproceduralCaches {
        typed_accessors: typed_accessor_cache,
        graph: mut interprocedural_graph_cache,
        results: mut interprocedural_result_cache,
    } = caches;
    let mut phase_started = Instant::now();
    let mut nodes = Vec::new();
    let mut graph_node_reused_paths = HashSet::new();
    if let Some(cache) = interprocedural_graph_cache.as_deref_mut() {
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
                (retained_source_paths.contains(file.path.as_str())
                    && cache.get(file.path.as_str()).is_some_and(|cached| {
                        Arc::ptr_eq(&cached.compiler, &file.compiler)
                            || same_compiler_semantics(&cached.compiler, &file.compiler)
                    }))
                .then_some(file.path.as_str())
            })
            .collect::<HashSet<_>>();
        let discovered = parallel_file_results(&facts.files, |file| {
            (!reusable_paths.contains(file.path.as_str()))
                .then(|| discover_summary_nodes(file, project_indexes, entities))
        });
        for (file, discovered) in facts.files.iter().zip(discovered) {
            if let Some(file_nodes) = discovered {
                nodes.extend(file_nodes.iter().cloned());
                cache.insert(
                    file.path.clone(),
                    CachedInterproceduralGraph {
                        nodes: file_nodes,
                        contribution: InterproceduralGraphContribution::default(),
                        compiler: file.compiler.clone(),
                    },
                );
            } else {
                let cached = cache
                    .get(file.path.as_str())
                    .expect("reusable graph path has a cached contribution");
                nodes.extend(cached.nodes.iter().cloned());
                graph_node_reused_paths.insert(file.path.as_str());
            }
        }
    } else {
        for file_nodes in parallel_file_results(&facts.files, |file| {
            discover_summary_nodes(file, project_indexes, entities)
        }) {
            nodes.extend(file_nodes);
        }
    }
    let nodes_by_path = function_indices_by_path(&nodes);
    let indexes = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| ((node.path.clone(), node.span), index))
        .collect::<HashMap<_, _>>();
    let by_symbol = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.symbol.clone().map(|symbol| (symbol, index)))
        .collect::<HashMap<_, _>>();
    let mut summaries = vec![SummaryReads::default(); nodes.len()];
    let mut callback_summaries = vec![Vec::<ContractCallback>::new(); nodes.len()];
    let mut callback_forwardings = Vec::new();
    let mut dispatches = Vec::<(usize, Vec<usize>)>::new();
    let mut contract_generation_obligations =
        vec![Vec::<ContractGenerationObligation>::new(); nodes.len()];
    let mut dispatch_obligations = Vec::new();
    let mut edges = vec![Vec::<usize>::new(); nodes.len()];
    let mut invoked_parameters = vec![Vec::<usize>::new(); nodes.len()];
    let mut escaped_parameters = vec![Vec::<usize>::new(); nodes.len()];
    let mut invoked_parameter_members = vec![Vec::<(usize, String)>::new(); nodes.len()];
    let mut returned_binding_candidates = Vec::new();
    let mut factory_call_candidates = Vec::new();
    let mut graph_reused_files = 0;
    let mut graph_recomputed_files = 0;
    {
        let mut graph = InterproceduralGraphAssembly {
            nodes: &nodes,
            nodes_by_path: &nodes_by_path,
            by_symbol: &by_symbol,
            summaries: &mut summaries,
            callback_summaries: &mut callback_summaries,
            callback_forwardings: &mut callback_forwardings,
            dispatches: &mut dispatches,
            contract_generation_obligations: &mut contract_generation_obligations,
            contract_consumer_obligations: &mut dispatch_obligations,
            edges: &mut edges,
            invoked_parameters: &mut invoked_parameters,
            escaped_parameters: &mut escaped_parameters,
            invoked_parameter_members: &mut invoked_parameter_members,
            returned_bindings: &mut returned_binding_candidates,
            factory_calls: &mut factory_call_candidates,
        };
        match interprocedural_graph_cache {
            None => {
                let contributions = parallel_file_results(&facts.files, |file| {
                    discover_interprocedural_graph(
                        file,
                        &nodes,
                        &nodes_by_path,
                        InterproceduralGraphSymbols {
                            entities,
                            names: symbol_names,
                        },
                        InterproceduralContracts {
                            reads: contract_reads,
                            parameter_reads: contract_parameter_reads,
                            callbacks: contract_callbacks,
                        },
                        lookup,
                        accessors,
                    )
                });
                for (file, contribution) in facts.files.iter().zip(contributions) {
                    graph_recomputed_files += 1;
                    graph.merge(file.path.as_str(), &contribution);
                }
            }
            Some(cache) => {
                let contributions = parallel_file_results(&facts.files, |file| {
                    (!graph_node_reused_paths.contains(file.path.as_str())).then(|| {
                        discover_interprocedural_graph(
                            file,
                            &nodes,
                            &nodes_by_path,
                            InterproceduralGraphSymbols {
                                entities,
                                names: symbol_names,
                            },
                            InterproceduralContracts {
                                reads: contract_reads,
                                parameter_reads: contract_parameter_reads,
                                callbacks: contract_callbacks,
                            },
                            lookup,
                            accessors,
                        )
                    })
                });
                for (file, contribution) in facts.files.iter().zip(contributions) {
                    if graph_node_reused_paths.contains(file.path.as_str())
                        && let Some(cached) = cache.get(file.path.as_str())
                    {
                        graph_reused_files += 1;
                        graph.merge(file.path.as_str(), &cached.contribution);
                        continue;
                    }
                    graph_recomputed_files += 1;
                    let contribution =
                        contribution.expect("recomputed graph path has a fresh contribution");
                    graph.merge(file.path.as_str(), &contribution);
                    cache.insert(
                        file.path.clone(),
                        CachedInterproceduralGraph {
                            nodes: nodes_by_path
                                .get(file.path.as_str())
                                .into_iter()
                                .flatten()
                                .map(|index| nodes[*index].clone())
                                .collect(),
                            contribution,
                            compiler: file.compiler.clone(),
                        },
                    );
                }
            }
        }
    }
    let mut contract_generation_obligation_keys = contract_generation_obligations
        .iter()
        .map(|obligations| {
            obligations
                .iter()
                .map(|obligation| (obligation.parameter, obligation.location.clone()))
                .collect::<HashSet<_>>()
        })
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for (owner, target, target_parameter, owner_parameter, ambient_execution) in
            &callback_forwardings
        {
            // Forwarding a callback into a local callee inherits that callee's
            // *whole* answer for the slot, and "the callee never accounted for
            // this parameter" is part of it. Without this hop the forwarding
            // silently upgrades an unknown into the empty (negative) claim.
            let target_escaped = escaped_parameters[*target].contains(target_parameter)
                || (escaped_parameters[*target].contains(&REST_PARAMETER)
                    && *target_parameter >= nodes[*target].parameters.len());
            if target_escaped && !escaped_parameters[*owner].contains(owner_parameter) {
                escaped_parameters[*owner].push(*owner_parameter);
                changed = true;
            }
            for callback in callback_summaries[*target]
                .iter()
                .filter(|callback| callback.parameter == *target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                // Only an `inline` callee row is relative to the callee's own
                // call and therefore needs restating; `tracked` and `deferred`
                // survive any wrapper. `Unknown` refuses to restate it, and the
                // row is dropped rather than republished -- the obligation that
                // came with the forwarding opens the sentinel for the export.
                if callback.execution == "inline"
                    && *ambient_execution == ForwardedAmbientExecution::Unknown
                {
                    continue;
                }
                let forwarded = ContractCallback {
                    parameter: *owner_parameter,
                    execution: match (callback.execution.as_str(), ambient_execution) {
                        ("inline", ForwardedAmbientExecution::Composed(ambient)) => ambient.clone(),
                        _ => callback.execution,
                    },
                    arguments: callback.arguments.clone(),
                    owner: callback.owner.clone(),
                    evidence: callback.evidence.clone(),
                };
                if !callback_summaries[*owner].contains(&forwarded) {
                    callback_summaries[*owner].push(forwarded);
                    changed = true;
                }
            }
            for obligation in contract_generation_obligations[*target]
                .iter()
                .filter(|obligation| obligation.parameter == *target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                let forwarded = ContractGenerationObligation {
                    function: nodes[*owner]
                        .name
                        .clone()
                        .unwrap_or_else(|| "<anonymous>".into()),
                    function_identity: nodes[*owner].runtime_identity.clone(),
                    parameter: *owner_parameter,
                    package: obligation.package.clone(),
                    entrypoint: obligation.entrypoint.clone(),
                    parameter_type: obligation.parameter_type.clone(),
                    required_execution: obligation.required_execution.clone(),
                    contract_stub: obligation.contract_stub.clone(),
                    location: obligation.location,
                    message: format!(
                        "parameter {owner_parameter} reaches unresolved behavior through {}; {}",
                        nodes[*target].name.as_deref().unwrap_or("<anonymous>"),
                        obligation.message
                    ),
                };
                let key = (forwarded.parameter, forwarded.location.clone());
                if contract_generation_obligation_keys[*owner].insert(key) {
                    contract_generation_obligations[*owner].push(forwarded);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let graph = phase_started.elapsed();
    phase_started = Instant::now();
    let owned_reactive_sources;
    let reactive_sources = if let Some(cache) = interprocedural_result_cache.as_deref_mut() {
        owned_reactive_sources = retained_reactive_sources(
            &mut cache.reactive_sources,
            accessors,
            contracted_accessor_symbols,
            summary_source_symbols,
            source_phases,
        );
        owned_reactive_sources.as_slice()
    } else {
        owned_reactive_sources = {
            let mut sources = accessors
                .iter()
                .filter(|(symbol, _)| !contracted_accessor_symbols.contains(*symbol))
                .filter(|(symbol, _)| summary_source_symbols.contains(*symbol))
                .map(|(symbol, (display, declaration))| {
                    cached_reactive_source(symbol, display, declaration, source_phases)
                })
                .collect::<Vec<_>>();
            sources.sort_by(reactive_source_order);
            Arc::new(sources)
        };
        owned_reactive_sources.as_slice()
    };
    let direct_index = phase_started.elapsed();
    let direct_references_started = Instant::now();
    for contributions in parallel_slice_results(reactive_sources, |source| {
        direct_reference_contributions(source, context, &nodes, &nodes_by_path)
    }) {
        for contribution in contributions {
            if contribution.unique {
                summaries[contribution.owner].push_unique(contribution.read);
            } else {
                summaries[contribution.owner].push(contribution.read);
            }
        }
    }
    let direct_references = direct_references_started.elapsed();
    let typed_accessors_started = Instant::now();
    let mut typed_accessor_reused_files = 0;
    let mut typed_accessor_recomputed_files = 0;
    match typed_accessor_cache {
        None => {
            for file in &facts.files {
                let contributions = discover_typed_accessors(
                    file,
                    &nodes,
                    &nodes_by_path,
                    project_indexes,
                    entities,
                    symbol_names,
                    lookup,
                );
                merge_typed_accessors(file.path.as_str(), &contributions, &indexes, &mut summaries);
            }
        }
        Some(cache) => {
            let current_paths = facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache.retain(|path, _| current_paths.contains(path.as_str()));
            for file in &facts.files {
                let contributions = if retained_source_paths.contains(file.path.as_str())
                    && let Some(cached) = cache.get(file.path.as_str())
                {
                    typed_accessor_reused_files += 1;
                    cached.contributions.clone()
                } else {
                    typed_accessor_recomputed_files += 1;
                    let contributions = discover_typed_accessors(
                        file,
                        &nodes,
                        &nodes_by_path,
                        project_indexes,
                        entities,
                        symbol_names,
                        lookup,
                    );
                    cache.insert(
                        file.path.clone(),
                        CachedTypedAccessors {
                            contributions: contributions.clone(),
                        },
                    );
                    contributions
                };
                merge_typed_accessors(file.path.as_str(), &contributions, &indexes, &mut summaries);
            }
        }
    }
    let typed_accessors = typed_accessors_started.elapsed();
    let direct_summaries = phase_started.elapsed();
    phase_started = Instant::now();
    let mut returned = vec![SummaryReads::default(); nodes.len()];
    let mut returned_edges = Vec::<(usize, usize)>::new();
    let returns_by_owner = items_by_containing_function(
        &nodes,
        &nodes_by_path,
        facts.files.iter().flat_map(|file| {
            file.ast
                .returns
                .iter()
                .map(move |returned| (file.path.as_str(), returned))
        }),
        |returned| returned.span,
    );
    let mut returns_by_ast_owner = HashMap::new();
    for file in &facts.files {
        for returned in &file.ast.returns {
            if let Some(function) = containing_ast_function(&file.ast, returned.span) {
                returns_by_ast_owner
                    .entry((file.path.as_str(), function.span))
                    .or_insert_with(Vec::new)
                    .push(returned);
            }
        }
    }
    for (index, node) in nodes.iter().enumerate() {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let returned_closures = returns_by_owner[index]
            .iter()
            .copied()
            .flat_map(|returned| {
                returned
                    .argument
                    .into_iter()
                    .flat_map(|argument| returned_function_spans(file, argument, entities))
            })
            .collect::<Vec<_>>();
        for closure in &returned_closures {
            if let Some(target) = indexes.get(&(node.path.clone(), *closure)) {
                returned_edges.push((index, *target));
            }
        }
        for returned_value in &returns_by_owner[index] {
            match returned_value.value {
                solid_facts::ast::ReturnValueKind::Identifier => {
                    let returned_location = location(file.path.shared(), returned_value.span);
                    if let Some(symbol) = entities.get(&returned_location) {
                        if returned_source_symbols.contains(symbol)
                            && let Some((display, declaration)) = accessors.get(symbol)
                        {
                            returned[index].push_unique(SummaryRead {
                                symbol: symbol.clone(),
                                display: display.clone(),
                                kind: Some(
                                    match source_kinds.get(symbol.as_str()) {
                                        Some(ReactiveSourceKind::Store) => "store-path",
                                        Some(ReactiveSourceKind::Accessor) | None => "accessor",
                                    }
                                    .into(),
                                ),
                                declaration: declaration.clone(),
                                origin: returned_location,
                                origin_context: node.name.clone().unwrap_or_default(),
                            });
                        } else if let Some(target) = by_symbol.get(symbol).copied()
                            && target != index
                        {
                            for read in summaries[target].iter() {
                                returned[index].push_unique(read.clone());
                            }
                        }
                    }
                }
                solid_facts::ast::ReturnValueKind::Call => {
                    let Some(callee) = returned_value.callee else {
                        continue;
                    };
                    let Some(call) = project_indexes
                        .ast_files_by_path
                        .get(file.path.as_str())
                        .into_iter()
                        .flat_map(|index| index.calls_by_callee(callee))
                        .find(|call| {
                            returned_value.argument.is_some_and(|argument| {
                                file.ast.peel_ts_sugar_span(argument) == call.span
                            })
                        })
                    else {
                        continue;
                    };
                    let callee_location = location(file.path.shared(), call.callee);
                    if let Some(symbol) = entities.get(&callee_location) {
                        if let Some(target) = by_symbol.get(symbol).copied() {
                            returned_edges.push((index, target));
                        } else {
                            let contracted = contract_returns.get(symbol).cloned().or_else(|| {
                                primitive_name(
                                    file.path.as_str(),
                                    call.callee,
                                    call.static_callee(&file.source),
                                    entities,
                                    symbol_names,
                                    lookup.dialect,
                                )
                                .and_then(|primitive| {
                                    bundled_returns.get(primitive.as_str()).cloned().map(
                                        |returned| {
                                            (
                                                returned,
                                                bundled_contract_location(
                                                    lookup.dialect,
                                                    &primitive,
                                                ),
                                            )
                                        },
                                    )
                                })
                            });
                            if let Some((returned_contract, declaration)) = contracted {
                                returned[index].push_unique(SummaryRead {
                                    symbol: symbol.clone(),
                                    display: SymbolId::from(returned_contract.label),
                                    kind: Some(returned_contract.kind),
                                    declaration,
                                    origin: location(file.path.shared(), call.span),
                                    origin_context: node.name.clone().unwrap_or_default(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if returned_closures.is_empty() {
            continue;
        }
        let mut direct = Vec::with_capacity(summaries[index].len());
        for read in summaries[index].take() {
            let in_returned_closure = returned_closures.iter().any(|closure| {
                read.origin.path == node.path.clone().into()
                    && u64::from(closure.start) <= read.origin.start_byte
                    && read.origin.end_byte <= u64::from(closure.end)
            });
            if in_returned_closure {
                returned[index].push(read);
            } else {
                direct.push(read);
            }
        }
        summaries[index].replace(direct);
    }
    let returned_direct = phase_started.elapsed();
    let returned_delta_started = Instant::now();
    propagate_returned_summary_deltas(&mut returned, &returned_edges);
    let returned_delta = returned_delta_started.elapsed();
    let call_summary_delta_started = Instant::now();
    let mut reverse_edges = vec![Vec::new(); nodes.len()];
    for (owner, targets) in edges.iter_mut().enumerate() {
        targets.sort_unstable();
        targets.dedup();
        for target in targets.iter().copied() {
            reverse_edges[target].push(owner);
        }
    }
    let mut propagated_lengths = vec![0; summaries.len()];
    propagate_summary_deltas(&mut summaries, &reverse_edges, &mut propagated_lengths);

    // Composite structural dispatch is intentionally a second phase. The
    // first propagation computes each exact implementation's own summary;
    // only an equal set of summaries may then be attached to the caller. If
    // one candidate is missing or differs in reads, callbacks, or async
    // shape, the call remains uncertifiable and contributes no edge.
    let mut dispatch_edges_added = false;
    for (owner, candidates) in &dispatches {
        let Some(first) = candidates.first().copied() else {
            continue;
        };
        let equivalent = candidates.iter().all(|candidate| {
            contract_generation_obligations[*candidate].is_empty()
                && equivalent_summary_reads(&summaries[*candidate], &summaries[first])
                && equivalent_callbacks(&callback_summaries[*candidate], &callback_summaries[first])
                && invoked_parameters[*candidate] == invoked_parameters[first]
                && invoked_parameter_members[*candidate] == invoked_parameter_members[first]
                && nodes[*candidate].r#async == nodes[first].r#async
        });
        if !equivalent {
            continue;
        }
        for target in candidates.iter().copied() {
            if !edges[*owner].contains(&target) {
                edges[*owner].push(target);
                reverse_edges[target].push(*owner);
                let target_reads = summaries[target].to_vec();
                for read in target_reads {
                    if summaries[*owner].push_unique(read) {
                        dispatch_edges_added = true;
                    }
                }
            }
        }
    }
    if dispatch_edges_added {
        propagate_summary_deltas(&mut summaries, &reverse_edges, &mut propagated_lengths);
    }

    let contract_generation_obligations = contract_generation_obligations
        .into_iter()
        .enumerate()
        .filter(|(index, _)| nodes[*index].exported)
        .flat_map(|(_, obligations)| obligations)
        .collect::<Vec<_>>();

    for (index, node) in nodes.iter().enumerate() {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let Some(function) = project_indexes
            .ast_files_by_path
            .get(file.path.as_str())
            .and_then(|index| index.function_by_span(node.span))
        else {
            continue;
        };
        for value in function.expression_return.iter().chain(
            returns_by_ast_owner
                .get(&(node.path.as_str(), function.span))
                .into_iter()
                .flatten()
                .copied(),
        ) {
            if value.value == solid_facts::ast::ReturnValueKind::Function
                && let Some(target) = indexes.get(&(node.path.clone(), value.span))
            {
                for read in summaries[*target].iter() {
                    returned[index].push_unique(read.clone());
                }
            }
        }
    }
    let call_summary_delta = call_summary_delta_started.elapsed();
    let factory_propagation_started = Instant::now();
    let mut returned_bindings = HashMap::<SymbolId, Vec<SummaryRead>>::new();
    if returned.iter().any(|summary| !summary.is_empty()) {
        for (binding_symbol, target_symbol) in &returned_binding_candidates {
            let Some(target) = by_symbol.get(target_symbol).copied() else {
                continue;
            };
            let mut summary = returned[target].to_vec();
            for read in &mut summary {
                if read.origin_context.is_empty() {
                    read.origin_context = nodes[target].name.clone().unwrap_or_default();
                }
            }
            returned_bindings.insert(binding_symbol.clone(), summary);
        }
        let mut factory_reads_added = false;
        if !returned_bindings.is_empty() {
            for (owner, symbol) in &factory_call_candidates {
                if accessors.contains_key(symbol) {
                    continue;
                }
                let Some(factory_reads) = returned_bindings.get(symbol) else {
                    continue;
                };
                for read in factory_reads {
                    let previous_len = summaries[*owner].len();
                    summaries[*owner].push_unique(read.clone());
                    factory_reads_added |= summaries[*owner].len() != previous_len;
                }
            }
        }
        if factory_reads_added {
            propagate_summary_deltas(&mut summaries, &reverse_edges, &mut propagated_lengths);
        }
    }

    let factory_propagation = factory_propagation_started.elapsed();
    let propagation = phase_started.elapsed();
    phase_started = Instant::now();
    let result_capacity = interprocedural_result_cache.as_ref().map_or(0, |cache| {
        cache.files.values().map(|file| file.reads.len()).sum()
    });
    let mut result = Vec::with_capacity(result_capacity);
    let mut result_reused_files = 0;
    let mut result_recomputed_files = 0;
    let result_view = InterproceduralResultView {
        nodes: &nodes,
        indexes: &indexes,
        by_symbol: &by_symbol,
        summaries: &summaries,
        invoked_parameters: &invoked_parameters,
        invoked_parameter_members: &invoked_parameter_members,
        returned_bindings: &returned_bindings,
    };
    let result_read_context = InterproceduralResultReadContext {
        result: result_view,
        contract_callbacks,
        source_kinds,
        accessors,
        entities,
        symbol_names,
        lookup: context.lookup,
    };
    if let Some(cache) = interprocedural_result_cache.as_deref_mut() {
        if cache.files.is_empty()
            && cache.dependency_states.is_empty()
            && cache.dependency_users.is_empty()
        {
            let per_file = parallel_file_results(&facts.files, |file| {
                interprocedural_result_reads_for_file(file, &result_read_context)
            });
            for (file, (reads, obligations, dependencies)) in facts.files.iter().zip(per_file) {
                result_recomputed_files += 1;
                result.extend(reads.iter().cloned());
                dispatch_obligations.extend(obligations.iter().cloned());
                for dependency in &dependencies {
                    add_interprocedural_dependency_user(&mut cache.dependency_users, dependency);
                }
                cache.files.insert(
                    file.path.clone(),
                    CachedInterproceduralResultFile {
                        dependencies,
                        reads,
                        dispatch_obligations: obligations,
                        compiler: file.compiler.clone(),
                    },
                );
            }
        } else {
            let current_paths = facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            let removed_paths = cache
                .files
                .keys()
                .filter(|path| !current_paths.contains(path.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            for path in removed_paths {
                let Some(removed) = cache.files.remove(path.as_str()) else {
                    continue;
                };
                for dependency in &removed.dependencies {
                    remove_interprocedural_dependency_user(
                        &mut cache.dependency_users,
                        &mut cache.dependency_states,
                        dependency,
                    );
                }
            }
            let changed_dependencies = cache
                .dependency_states
                .iter()
                .filter(|(dependency, retained)| {
                    !result_view.dependency_matches(retained, dependency)
                })
                .map(|(dependency, _)| dependency.clone())
                .collect::<HashSet<_>>();
            for file in &facts.files {
                if retained_source_paths.contains(file.path.as_str())
                    && let Some(cached) = cache.files.get(file.path.as_str())
                    && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                        || same_compiler_semantics(&cached.compiler, &file.compiler))
                    && cached.dependencies.is_disjoint(&changed_dependencies)
                {
                    result_reused_files += 1;
                    result.extend(cached.reads.iter().cloned());
                    dispatch_obligations.extend(cached.dispatch_obligations.iter().cloned());
                    continue;
                }
                result_recomputed_files += 1;
                let (reads, obligations, dependencies) =
                    interprocedural_result_reads_for_file(file, &result_read_context);
                result.extend(reads.iter().cloned());
                dispatch_obligations.extend(obligations.iter().cloned());
                if let Some(previous) = cache.files.remove(file.path.as_str()) {
                    for dependency in previous.dependencies.difference(&dependencies) {
                        remove_interprocedural_dependency_user(
                            &mut cache.dependency_users,
                            &mut cache.dependency_states,
                            dependency,
                        );
                    }
                    for dependency in dependencies.difference(&previous.dependencies) {
                        add_interprocedural_dependency_user(
                            &mut cache.dependency_users,
                            dependency,
                        );
                    }
                } else {
                    for dependency in &dependencies {
                        add_interprocedural_dependency_user(
                            &mut cache.dependency_users,
                            dependency,
                        );
                    }
                }
                cache.files.insert(
                    file.path.clone(),
                    CachedInterproceduralResultFile {
                        dependencies,
                        reads,
                        dispatch_obligations: obligations,
                        compiler: file.compiler.clone(),
                    },
                );
            }
            for dependency in cache.dependency_users.keys() {
                if changed_dependencies.contains(dependency)
                    || !cache.dependency_states.contains_key(dependency)
                {
                    cache
                        .dependency_states
                        .insert(dependency.clone(), result_view.dependency_state(dependency));
                }
            }
        }
        if cache.dependency_states.is_empty() {
            for dependency in cache.dependency_users.keys() {
                cache
                    .dependency_states
                    .insert(dependency.clone(), result_view.dependency_state(dependency));
            }
        }
    } else {
        for file in &facts.files {
            result_recomputed_files += 1;
            let (reads, obligations, _) =
                interprocedural_result_reads_for_file(file, &result_read_context);
            result.extend(reads);
            dispatch_obligations.extend(obligations);
        }
    }
    // An exported helper that invokes a member supplied by a parameter has
    // callers outside the analyzed project. Normal project analysis must keep
    // that boundary explicit. Contract emission can discharge this specific
    // obligation because `contract_export_function` serializes the same exact
    // parameter provenance as a `parameter-member` read.
    let mut allowed_by_path: HashMap<&str, Vec<Span>> = HashMap::new();
    for node in nodes.iter().filter(|node| node.exported) {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let Some(function) = project_indexes
            .ast_files_by_path
            .get(file.path.as_str())
            .and_then(|index| index.function_by_span(node.span))
        else {
            continue;
        };
        let allowed = allowed_by_path
            .entry(file.path.as_str())
            .or_insert_with(|| allowed_callback_spans(file, context.lookup));
        let direct_member = file.ast.calls.iter().find_map(|call| {
            if !function.body.contains(call.span)
                || !containing_ast_function(&file.ast, call.span)
                    .is_some_and(|owner| owner.span == function.span)
            {
                return None;
            }
            if semantic_execution_role(
                file,
                call.callee,
                allowed,
                entities,
                symbol_names,
                context.lookup,
            ) == ExecutionRole::TrackedJsx
            {
                return None;
            }
            if context
                .lookup
                .resolved_callee_call(file, call.callee)
                .and_then(|resolved| resolved.declaration.as_ref())
                .is_some_and(|declaration| declaration.standard_library)
            {
                return None;
            }
            let (receiver, property) = context.lookup.member_callee_receiver(file, call.callee)?;
            node.parameters
                .iter()
                .any(|parameter| parameter == &receiver)
                .then_some(property)
        });
        if let Some(property) = direct_member {
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: node
                        .name
                        .clone()
                        .unwrap_or_else(|| "exported helper".into()),
                    member: Some(property),
                },
                location: location(Arc::from(node.path.as_str()), node.span),
                analysis_context: crate::EXPORTED_PARAMETER_MEMBER_DISPATCH.into(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
    let factory_instances = returned_bindings
        .values()
        .filter(|summary| !summary.is_empty())
        .count();
    let result_reads = phase_started.elapsed();
    let export_started = Instant::now();
    let contract_graph = ContractGraph {
        nodes: &nodes,
        nodes_by_path: &nodes_by_path,
        by_symbol: &by_symbol,
        entities,
    };
    let mut structured_returns = vec![None; nodes.len()];
    let has_structured_return_seed = facts.files.iter().any(|file| {
        !file.ast.object_properties.is_empty()
            || file
                .ast
                .functions
                .iter()
                .filter_map(|function| function.expression_return.as_ref())
                .chain(file.ast.returns.iter())
                .any(|returned| {
                    !returned.elements().is_empty() || !returned.properties().is_empty()
                })
    }) || bundled_returns
        .values()
        .chain(contract_returns.values().map(|(returned, _)| returned))
        .any(|returned| matches!(returned.kind.as_str(), "tuple" | "object"));
    if has_structured_return_seed {
        for _ in 0..=nodes.len() {
            let discovered = discover_structured_returns(&StructuredReturnDiscovery {
                facts,
                nodes: &nodes,
                indexes: &indexes,
                by_symbol: &by_symbol,
                summaries: &summaries,
                returned: &returned,
                structured_returns: &structured_returns,
                accessors,
                source_kinds,
                source_primitives,
                bundled_returns,
                contract_returns,
                entities,
                symbol_names,
                lookup,
            });
            let mut changed = false;
            for (target, returned) in structured_returns.iter_mut().zip(discovered) {
                if *target != returned {
                    *target = returned;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    let structured_discovery = StructuredReturnDiscovery {
        facts,
        nodes: &nodes,
        indexes: &indexes,
        by_symbol: &by_symbol,
        summaries: &summaries,
        returned: &returned,
        structured_returns: &structured_returns,
        accessors,
        source_kinds,
        source_primitives,
        bundled_returns,
        contract_returns,
        entities,
        symbol_names,
        lookup,
    };
    for node in nodes.iter().filter(|node| node.exported) {
        let Some(file) = lookup.file_by_path(node.path.as_str()) else {
            continue;
        };
        let Some(function) = file
            .ast
            .functions
            .iter()
            .find(|function| function.span == node.span)
        else {
            continue;
        };
        for property in function
            .expression_return
            .iter()
            .chain(file.ast.returns.iter().filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            }))
            .flat_map(|returned| returned.properties())
        {
            let Some(reason) =
                structured_discovery.unresolved_shorthand_reason(file, property.value)
            else {
                continue;
            };
            dispatch_obligations.push(StaticDefect {
                kind: StaticDefectKind::StructuredReturnUnresolved {
                    function: node
                        .name
                        .clone()
                        .unwrap_or_else(|| "exported function".into()),
                    property: property.name.to_string(),
                    reason,
                },
                location: location(file.path.shared(), property.value),
                analysis_context: "exported-structured-return".into(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }
    let contract_analysis = ContractAnalysis {
        summaries: &summaries,
        returned: &returned,
        structured_returns: &structured_returns,
        callbacks: &callback_summaries,
        escaped_parameters: &escaped_parameters,
        invoked_parameter_members: &invoked_parameter_members,
        semantics: ContractSemantics {
            bundled_returns,
            source_kinds,
            source_primitives,
        },
    };
    let exports = if let Some(cache) = interprocedural_result_cache {
        contract_export_summaries_incremental(
            &mut cache.contract_exports,
            facts,
            &contract_graph,
            &reverse_edges,
            &graph_node_reused_paths,
            *changed_semantic_symbols,
            &contract_analysis,
        )
    } else {
        Arc::new(contract_export_summaries(
            facts,
            &contract_graph,
            &contract_analysis,
        ))
    };
    let export_summaries = export_started.elapsed();
    let results_and_exports = phase_started.elapsed();
    InterproceduralResult {
        reads: result.into(),
        dispatch_obligations: dispatch_obligations.into(),
        exports,
        contract_generation_obligations: contract_generation_obligations.into(),
        factory_instances,
        timings: InterproceduralTimings {
            graph,
            direct_summaries,
            direct_index,
            direct_references,
            typed_accessors,
            propagation,
            returned_direct,
            returned_delta,
            call_summary_delta,
            factory_propagation,
            results_and_exports,
            result_reads,
            export_summaries,
            typed_accessor_reused_files,
            typed_accessor_recomputed_files,
            graph_reused_files,
            graph_recomputed_files,
            result_reused_files,
            result_recomputed_files,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use solid_dialect::{Primitive, TrackedCallbackTiming};
    use typefacts::Location;

    use super::{
        CallbackWrapper, ContractCallback, SummaryRead, SummaryReads, SymbolId,
        add_interprocedural_dependency_user, cached_reactive_source, compose_callback_chain,
        equivalent_callbacks, equivalent_summary_reads, primitive_callback_execution,
        reactive_source_order, remove_interprocedural_dependency_user, retained_reactive_sources,
    };
    use crate::cache::InterproceduralResultDependency;

    fn location(start: u64) -> Location {
        Location {
            path: Arc::from("app.tsx"),
            start_byte: start,
            end_byte: start + 1,
        }
    }

    fn read(symbol: &str, display: &str, origin: u64) -> SummaryRead {
        SummaryRead {
            symbol: SymbolId::from(symbol),
            display: SymbolId::from(display),
            kind: None,
            declaration: location(0),
            origin: location(origin),
            origin_context: "test".to_owned(),
        }
    }

    fn reads(entries: &[SummaryRead]) -> SummaryReads {
        let mut summary = SummaryReads::default();
        for entry in entries {
            summary.push(entry.clone());
        }
        summary
    }

    /// The dispatch gate unions every candidate's reads once it returns true,
    /// so a read only one candidate performs must fail it. Same-length
    /// containment does not catch that: `[count, count]` and `[count, other]`
    /// are the same length and every member of the first is in the second.
    #[test]
    fn dispatch_equivalence_rejects_a_read_only_one_candidate_performs() {
        let repeated = reads(&[read("sym-a", "count", 10), read("sym-a", "count", 11)]);
        let distinct = reads(&[read("sym-a", "count", 12), read("sym-b", "other", 13)]);
        assert!(!equivalent_summary_reads(&repeated, &distinct));
        assert!(!equivalent_summary_reads(&distinct, &repeated));
    }

    /// Reading one accessor twice is the same effect as reading it once, so a
    /// differing repeat count -- or a differing origin line -- must not make
    /// the dispatch fail closed.
    #[test]
    fn dispatch_equivalence_ignores_repeat_count_and_origin() {
        let twice = reads(&[read("sym-a", "count", 10), read("sym-a", "count", 11)]);
        let once = reads(&[read("sym-a", "count", 99)]);
        assert!(equivalent_summary_reads(&twice, &once));
        assert!(equivalent_summary_reads(&once, &twice));
    }

    #[test]
    fn dispatch_equivalence_accepts_the_same_read_set() {
        let left = reads(&[read("sym-a", "count", 10), read("sym-b", "other", 11)]);
        let right = reads(&[read("sym-b", "other", 20), read("sym-a", "count", 21)]);
        assert!(equivalent_summary_reads(&left, &right));
    }

    #[test]
    fn callback_equivalence_rejects_timing_only_one_candidate_declares() {
        let callback = |parameter, execution: &str| ContractCallback {
            parameter,
            execution: execution.to_owned(),
            arguments: Vec::new(),
            owner: None,
            evidence: None,
        };
        let repeated = [callback(0, "deferred"), callback(0, "deferred")];
        let distinct = [callback(0, "deferred"), callback(1, "inline")];
        assert!(!equivalent_callbacks(&repeated, &distinct));
        assert!(!equivalent_callbacks(&distinct, &repeated));
        // The same timing set in a different order stays equivalent.
        let reordered = [callback(1, "inline"), callback(0, "deferred")];
        assert!(equivalent_callbacks(&distinct, &reordered));
    }

    #[test]
    fn callback_equivalence_rejects_different_owner_contexts() {
        let inherited = ContractCallback {
            parameter: 0,
            execution: "deferred".into(),
            arguments: Vec::new(),
            owner: Some("inherited".into()),
            evidence: None,
        };
        let leaf = ContractCallback {
            owner: Some("leaf".into()),
            ..inherited.clone()
        };
        assert!(!equivalent_callbacks(&[inherited], &[leaf]));
    }

    #[test]
    fn summary_reads_dedupe_on_display_origin_and_declaration_only() {
        let mut reads = SummaryReads::default();
        assert!(reads.push_unique(read("sym-a", "count", 10)));
        // A different underlying symbol with the same display, origin, and
        // declaration is the same read for summary purposes.
        assert!(!reads.push_unique(read("sym-b", "count", 10)));
        // Any differing key component makes the read new again.
        assert!(reads.push_unique(read("sym-a", "count", 11)));
        assert!(reads.push_unique(read("sym-a", "other", 10)));
        assert_eq!(reads.len(), 3);
    }

    #[test]
    fn unconditional_push_appends_duplicates_but_still_records_the_key() {
        let mut reads = SummaryReads::default();
        reads.push(read("sym", "count", 10));
        reads.push(read("sym", "count", 10));
        // `push` never dedupes its own input...
        assert_eq!(reads.len(), 2);
        // ...but it seeds the key, so a later `push_unique` is refused.
        assert!(!reads.push_unique(read("sym", "count", 10)));
    }

    #[test]
    fn replacing_the_reads_resets_the_dedupe_state() {
        let mut reads = SummaryReads::default();
        reads.push(read("sym", "count", 10));
        reads.replace(vec![read("sym", "name", 20)]);
        // The old key is forgotten, the replacement's key is live.
        assert!(reads.push_unique(read("sym", "count", 10)));
        assert!(!reads.push_unique(read("sym", "name", 20)));
        assert_eq!(
            reads.to_vec(),
            vec![read("sym", "name", 20), read("sym", "count", 10)]
        );
    }

    #[test]
    fn inserting_preserves_order_and_marks_the_key() {
        let mut reads = SummaryReads::default();
        reads.push(read("sym", "a", 1));
        reads.push(read("sym", "c", 3));
        reads.insert(1, read("sym", "b", 2));
        assert_eq!(
            reads
                .iter()
                .map(|read| read.display.as_ref())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert!(!reads.push_unique(read("sym", "b", 2)));
    }

    #[test]
    fn effect_callback_executions_come_from_the_dialect() {
        let solid2 = solid_dialect::Solid2;
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 0, 2, &solid2),
            Some("tracked")
        );
        // 2.0's second effect argument is the deferred apply callback.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 1, 2, &solid2),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 2, 2, &solid2),
            None
        );

        let solid1x = solid_dialect::Solid1x;
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 0, 2, &solid1x),
            Some("tracked")
        );
        // 1.x's second argument is a seed value, not a callback.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 1, 2, &solid1x),
            None
        );
    }

    #[test]
    fn non_effect_callback_executions_use_the_module_classification() {
        let dialect = solid_dialect::Solid2;
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateMemo), 0, 1, &dialect),
            Some("tracked")
        );
        // `untrack` and `flush` run their callback before returning, so the
        // contract word for both is `inline` -- the same word the reviewed
        // bundled contract for solid-js@2.0.0-rc.0 uses for them. The
        // listener-clearing half is a separate dialect fact, not this word.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::Untrack), 0, 1, &dialect),
            Some("inline")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::Flush), 0, 1, &dialect),
            Some("inline")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::OnCleanup), 0, 1, &dialect),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateRoot), 0, 1, &dialect),
            Some("inline")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::RunWithOwner), 1, 2, &dialect),
            Some("inline")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::RunWithOwner), 0, 2, &dialect),
            None
        );
        assert_eq!(primitive_callback_execution(None, 0, 0, &dialect), None);

        let solid1x = solid_dialect::Solid1x;
        assert_eq!(
            primitive_callback_execution(Some(Primitive::On), 0, 2, &solid1x),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::On), 1, 2, &solid1x),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::MergeProps), 3, 4, &solid1x),
            Some("tracked")
        );
    }

    /// The composition rule, innermost wrapper first. Each row is a real shape
    /// the corpus measurement produced a wrong claim for.
    #[test]
    fn a_callback_chain_composes_detachment_and_schedule_in_order() {
        use CallbackWrapper::{Deferred, Detaching, Tracked, Transparent};
        // The tracked wrapper's schedule, which decides what a *detached*
        // callback under it composes to. 1.x `createEffect` is the deferring
        // one; 1.x `createMemo`/`createRenderEffect`/`mergeProps` and every
        // 2.0 effect are eager; a package-contract row has neither.
        const EAGER: CallbackWrapper = Tracked(Some(TrackedCallbackTiming::DuringCall));
        const LATER: CallbackWrapper = Tracked(Some(TrackedCallbackTiming::AfterCall));
        const UNKNOWN: CallbackWrapper = Tracked(None);

        // `use(fn, el, arg) { return untrack(() => fn(el, arg)) }` --
        // @solidjs/web. Runs before the export returns.
        assert_eq!(compose_callback_chain(&[Detaching]), Some("inline"));
        // `createSubRoot(fn) { return createRoot(d => fn(d)) }` --
        // @solid-primitives/rootless. Same shape, same answer.
        assert_eq!(
            compose_callback_chain(&[Detaching, Transparent]),
            Some("inline")
        );
        // `onMount(fn) { createEffect(() => untrack(fn)) }` -- solid-js. The
        // clearing wrapper stops `tracked`; the effect still schedules.
        assert_eq!(
            compose_callback_chain(&[Detaching, LATER]),
            Some("deferred")
        );
        // `createMemo(() => untrack(fn))`, and its `createRenderEffect` and
        // `mergeProps` twins: the same chain shape with an *eager* tracked
        // wrapper runs `fn` during the call. Measured against solid-js@1.9.14
        // under `--conditions browser`: `ranDuringCall`, so a `deferred` claim
        // here is one the probe fails.
        assert_eq!(compose_callback_chain(&[Detaching, EAGER]), Some("inline"));
        // No established schedule for the tracked wrapper: no word is honest,
        // and the unknown sentinel is the answer rather than either guess.
        assert_eq!(compose_callback_chain(&[Detaching, UNKNOWN]), None);
        // Order is the answer: `untrack(() => createMemo(fn))` still tracks
        // `fn`, because the memo subscribes it and the outer untrack cannot
        // undo that. The wrapper's schedule is irrelevant once attribution
        // decides, so even the unknown one answers `tracked` here.
        assert_eq!(compose_callback_chain(&[EAGER, Detaching]), Some("tracked"));
        assert_eq!(
            compose_callback_chain(&[UNKNOWN, Detaching]),
            Some("tracked")
        );
        // No clearing wrapper: the tracked claim survives untouched.
        assert_eq!(compose_callback_chain(&[LATER]), Some("tracked"));
        assert_eq!(compose_callback_chain(&[UNKNOWN]), Some("tracked"));
        assert_eq!(
            compose_callback_chain(&[Transparent, EAGER]),
            Some("tracked")
        );
        // A transparent wrapper is exactly its call site.
        assert_eq!(compose_callback_chain(&[Transparent]), Some("inline"));
        // Deferral is sticky in both directions, and it outranks the sentinel:
        // an inner wrapper that already runs later cannot be made earlier by
        // anything above it, so the outer schedule is not asked for.
        assert_eq!(compose_callback_chain(&[Deferred]), Some("deferred"));
        assert_eq!(compose_callback_chain(&[Deferred, EAGER]), Some("deferred"));
        assert_eq!(
            compose_callback_chain(&[Deferred, Detaching, UNKNOWN]),
            Some("deferred")
        );
        assert_eq!(compose_callback_chain(&[LATER, Deferred]), Some("deferred"));
        assert_eq!(
            compose_callback_chain(&[Detaching, LATER, Transparent]),
            Some("deferred")
        );
        // Two tracked wrappers above a clearing one: the outer one's schedule
        // is asked for too, so an unknown outer wrapper refuses even when the
        // inner one is established.
        assert_eq!(compose_callback_chain(&[Detaching, EAGER, UNKNOWN]), None);
    }

    #[test]
    fn reactive_sources_order_by_phase_before_location() {
        let phases = HashMap::from([(SymbolId::from("late"), 2_u8)]);
        let early = cached_reactive_source("early", "early", &location(90), &phases);
        let late = cached_reactive_source("late", "late", &location(10), &phases);
        // `late` sits at an earlier byte but a later phase.
        assert_eq!(
            reactive_source_order(&early, &late),
            std::cmp::Ordering::Less
        );
        let sibling = cached_reactive_source("sibling", "sibling", &location(95), &phases);
        assert_eq!(
            reactive_source_order(&early, &sibling),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn retained_reactive_sources_build_sorted_and_reuse_the_cache() {
        let accessors = HashMap::from([
            (SymbolId::from("a"), (SymbolId::from("count"), location(50))),
            (SymbolId::from("b"), (SymbolId::from("name"), location(10))),
            (SymbolId::from("c"), (SymbolId::from("gone"), location(5))),
        ]);
        let contracted = HashSet::new();
        // `c` never appears in the summaries, so it is ineligible.
        let summary_sources = HashSet::from([SymbolId::from("a"), SymbolId::from("b")]);
        let phases = HashMap::from([(SymbolId::from("a"), 2_u8)]);

        let mut cache = None;
        let first = retained_reactive_sources(
            &mut cache,
            &accessors,
            &contracted,
            &summary_sources,
            &phases,
        );
        assert_eq!(
            first
                .iter()
                .map(|source| (source.symbol.as_ref(), source.phase))
                .collect::<Vec<_>>(),
            [("b", 1), ("a", 2)]
        );

        let second = retained_reactive_sources(
            &mut cache,
            &accessors,
            &contracted,
            &summary_sources,
            &phases,
        );
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn retained_reactive_sources_drop_stale_entries_and_insert_new_ones_in_order() {
        let mut accessors = HashMap::from([
            (SymbolId::from("a"), (SymbolId::from("count"), location(50))),
            (SymbolId::from("b"), (SymbolId::from("name"), location(10))),
        ]);
        let contracted = HashSet::new();
        let mut summary_sources = HashSet::from([SymbolId::from("a"), SymbolId::from("b")]);
        let phases = HashMap::new();

        let mut cache = None;
        retained_reactive_sources(
            &mut cache,
            &accessors,
            &contracted,
            &summary_sources,
            &phases,
        );

        // `b` stops being a summary source; `c` appears between the others.
        summary_sources.remove("b");
        summary_sources.insert(SymbolId::from("c"));
        accessors.insert(SymbolId::from("c"), (SymbolId::from("mid"), location(30)));
        let retained = retained_reactive_sources(
            &mut cache,
            &accessors,
            &contracted,
            &summary_sources,
            &phases,
        );
        assert_eq!(
            retained
                .iter()
                .map(|source| source.display.as_ref())
                .collect::<Vec<_>>(),
            ["mid", "count"]
        );
    }

    #[test]
    fn dependency_users_are_reference_counted() {
        let dependency = InterproceduralResultDependency::Symbol(SymbolId::from("f"));
        let mut users = HashMap::new();
        let mut states = HashMap::new();
        states.insert(
            dependency.clone(),
            crate::cache::InterproceduralResultDependencyState::Missing,
        );
        add_interprocedural_dependency_user(&mut users, &dependency);
        add_interprocedural_dependency_user(&mut users, &dependency);

        remove_interprocedural_dependency_user(&mut users, &mut states, &dependency);
        // One user remains: both maps keep the dependency.
        assert!(users.contains_key(&dependency));
        assert!(states.contains_key(&dependency));

        remove_interprocedural_dependency_user(&mut users, &mut states, &dependency);
        assert!(users.is_empty());
        assert!(states.is_empty());
    }
}
