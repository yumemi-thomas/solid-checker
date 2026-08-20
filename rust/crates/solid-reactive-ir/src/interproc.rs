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

use solid_dialect::Primitive;
use solid_facts::ProjectFacts;
use solid_facts::core::Span;
use typefacts::{CallKind, Location, ResolvedCallValidity};

use super::runtime_semantics::{
    RuntimeArgumentBehavior, argument_behavior, potentially_callable,
    proven_array_method_argument_behavior, resolved_parameter, retains_argument_value,
};
use super::{
    ContractAnalysis, ContractCallback, ContractExport, ContractGenerationObligation,
    ContractGraph, ContractReturn, ContractSemantics, EntitySymbols, ExecutionRole,
    FunctionBoundary, ProjectIndexes, ReactiveRead, ReactiveSourceKind, SemanticLookup,
    StaticDefect, StaticDefectKind, SymbolId, allowed_callback_spans,
    assigned_member_function_contains, containing_summary_function_indexed,
    contract_callback_execution, contract_export_summaries, contract_export_summaries_incremental,
    function_indices_by_path, functions_for_path, location, location_order, primitive_name,
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
    function_binding_name, inside_effect_apply, solid_accessor_declaration,
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
    fn effect(callbacks: &[ContractCallback]) -> HashSet<(usize, &str)> {
        callbacks
            .iter()
            .map(|callback| (callback.parameter, callback.execution.as_str()))
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
        if inside_effect_apply(file, call.callee, entities, symbol_names, dialect)
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

fn discover_interprocedural_graph(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    contracts: InterproceduralContracts<'_>,
    lookup: &SemanticLookup<'_>,
) -> InterproceduralGraphContribution {
    let mut contribution = InterproceduralGraphContribution::default();
    let primitives = lookup.primitives(file);
    let allowed = allowed_callback_spans(file, lookup);
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
        if !ambiguous_dispatch && !contracts.reads.contains_key(symbol) {
            if call.direct_callee
                && let Some(target) =
                    returned_function_target(file, nodes, nodes_by_path, entities, symbol)
            {
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
            && let Some((callback_owner, parameter)) =
                functions_for_path(nodes, nodes_by_path, file.path.as_str())
                    .filter_map(|(index, node)| {
                        node.parameters
                            .iter()
                            .position(|parameter| parameter == symbol)
                            .map(|parameter| (index, parameter))
                    })
                    .next()
        {
            let invocation_owner = functions_for_path(nodes, nodes_by_path, file.path.as_str())
                .filter(|(_, function)| function.body.contains(call.span))
                .min_by_key(|(_, function)| function.body.end - function.body.start)
                .map_or(owner, |(index, _)| index);
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
                let Some(target) = nodes
                    .iter()
                    .find(|node| node.symbol.as_deref() == Some(scheduler_symbol))
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
                    None,
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
                entities,
                symbol_names,
                lookup,
            );
            let execution = runtime_execution
                .or_else(|| contract_callback_execution(semantic))
                .or_else(|| {
                    function_escapes_through_return(
                        file,
                        &nodes[invocation_owner],
                        &nodes[callback_owner],
                        entities,
                        lookup,
                    )
                    .then_some("deferred")
                })
                .or(call.direct_callee.then_some("inline"));
            if let Some(execution) = execution {
                contribution.callbacks.push((
                    nodes[callback_owner].span,
                    ContractCallback {
                        parameter,
                        execution: execution.into(),
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
                if let Some(argument_symbol) = entities.get(&argument_location) {
                    if callback.execution == "inline" {
                        contribution.edges.push((
                            owner_span,
                            InterproceduralGraphTarget::Symbol(argument_symbol.clone()),
                        ));
                    }
                    if let Some((callback_owner, parameter)) =
                        functions_for_path(nodes, nodes_by_path, file.path.as_str())
                            .filter_map(|(index, node)| {
                                node.parameters
                                    .iter()
                                    .position(|parameter| parameter == argument_symbol)
                                    .map(|parameter| (index, parameter))
                            })
                            .next()
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
            if argument.value != solid_facts::ast::ArgumentValueKind::Identifier {
                continue;
            }
            let Some(argument_symbol) = entities.get(&location(file.path.shared(), argument.span))
            else {
                continue;
            };
            let Some((callback_owner, parameter)) =
                functions_for_path(nodes, nodes_by_path, file.path.as_str())
                    .filter_map(|(index, node)| {
                        node.parameters
                            .iter()
                            .position(|candidate| candidate == argument_symbol)
                            .map(|parameter| (index, parameter))
                    })
                    .next()
            else {
                continue;
            };
            if !ambiguous_dispatch
                && let Some(target) = nodes
                    .iter()
                    .position(|node| node.symbol.as_deref() == Some(symbol))
                    .or_else(|| {
                        returned_function_target(file, nodes, nodes_by_path, entities, symbol)
                    })
            {
                // Local calls are summarized transitively. If the parameter
                // later reaches an unknown external call, that call creates
                // the obligation at the actual escape point.
                contribution.callback_forwardings.push((
                    nodes[callback_owner].span,
                    nodes[target].symbol.clone().map_or(
                        InterproceduralGraphTarget::LocalSpan(nodes[target].span),
                        InterproceduralGraphTarget::Symbol,
                    ),
                    argument_index,
                    parameter,
                    enclosing_callback_execution(file, call.span, &contracts, lookup),
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
                contribution.callbacks.push((
                    nodes[callback_owner].span,
                    ContractCallback {
                        parameter,
                        execution: execution.into(),
                        evidence: None,
                    },
                ));
                continue;
            }
            // `splitProps` only creates property views. Its source and key
            // lists are values even when erased JavaScript types leave their
            // callability unknown.
            if primitive == Some(Primitive::SplitProps) {
                continue;
            }
            let resolved_call = lookup.resolved_callee_call(file, call.callee);
            let runtime_argument_callability =
                lookup.smallest_contained_callability(file.path.as_str(), argument.span);
            let runtime_behavior = resolved_call
                .and_then(|resolved_call| {
                    argument_behavior(resolved_call, runtime_argument_callability, argument_index)
                })
                .or_else(|| {
                    proven_array_method(file, call, entities).and_then(|method| {
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
                    assigned_member_function_contains(file, call.callee, entities)
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
            if functions_for_path(nodes, nodes_by_path, file.path.as_str())
                .any(|(_, node)| node.parameters.iter().any(|parameter| parameter == symbol))
                || file.ast.functions.iter().any(|function| {
                    function.parameters.iter().any(|parameter| {
                        parameter.names.iter().any(|name| {
                            entities
                                .at(file.path.as_str(), name.span)
                                .is_some_and(|candidate| candidate == symbol)
                        })
                    })
                })
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
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = file.ast.call_at(initializer) else {
            continue;
        };
        let Some(target_symbol) = entities.get(&location(file.path.shared(), call.callee)) else {
            continue;
        };
        for name in &binding.names {
            if let Some(binding_symbol) = entities.get(&location(file.path.shared(), name.span)) {
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

fn returned_function_target(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    entities: &EntitySymbols,
    binding_symbol: &str,
) -> Option<usize> {
    let initializer = file.ast.bindings.iter().find_map(|binding| {
        binding
            .names
            .iter()
            .any(|name| {
                entities
                    .at(file.path.as_str(), name.span)
                    .is_some_and(|symbol| symbol == binding_symbol)
            })
            .then_some(binding.call_initializer)
            .flatten()
    })?;
    let factory_call = file.ast.call_at(initializer)?;
    let factory_symbol = entities.at(file.path.as_str(), factory_call.callee)?;
    let (_, factory) = functions_for_path(nodes, nodes_by_path, file.path.as_str())
        .find(|(_, node)| node.symbol.as_deref() == Some(factory_symbol))?;
    let function = file
        .ast
        .functions
        .iter()
        .find(|function| function.span == factory.span)?;
    let returned_values = function
        .expression_return
        .iter()
        .filter_map(|returned| returned.argument)
        .chain(file.ast.returns.iter().filter_map(|returned| {
            (containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span))
            .then_some(returned.argument)
            .flatten()
        }));
    returned_values
        .flat_map(|value| returned_function_spans(file, value, entities))
        .find_map(|span| {
            functions_for_path(nodes, nodes_by_path, file.path.as_str())
                .find(|(_, node)| node.span == span)
                .map(|(index, _)| index)
        })
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

fn enclosing_callback_execution(
    file: &solid_facts::FileFacts,
    nested: Span,
    contracts: &InterproceduralContracts<'_>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    let primitives = lookup.primitives(file);
    let mut enclosing = file
        .ast
        .arguments_containing(nested)
        .filter(|(call, argument)| {
            direct_callback_contains(file, call.arguments[*argument].span, nested)
        })
        .collect::<Vec<_>>();
    enclosing.sort_by_key(|(call, argument)| {
        let span = call.arguments[*argument].span;
        span.end - span.start
    });
    enclosing.into_iter().find_map(|(call, parameter)| {
        let call_index = lookup.call_index(file, call.span)?;
        if let Some(execution) = primitive_callback_execution(
            super::known_primitive(&primitives.calls[call_index]),
            parameter,
            call.arguments.len(),
            lookup.dialect,
        ) {
            return Some(execution.to_owned());
        }
        let symbol = lookup.callee_symbol(file, call.callee)?;
        contracts
            .callbacks
            .get(symbol)?
            .iter()
            .find(|callback| callback.parameter == parameter)
            .map(|callback| callback.execution.clone())
    })
}

/// The execution recorded for a callback forwarded into a primitive, in the
/// package contracts' vocabulary.
///
/// The effect pair derives from the dialect, because its phases are the
/// headline dialect difference: 2.0 has a deferred apply argument, 1.x has a
/// tracked callback and a seed value. The other arms keep this module's own
/// classification -- it deliberately labels `untrack`/`flush` callbacks
/// "deferred" where the vocabulary calls them inline, because a contract
/// consumer treats "deferred" as "not tracked here", which is the meaning
/// these summaries need. Reconciling the two vocabularies is a contract-
/// emission change with its own fixtures.
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
        (
            P::OnSettled | P::Action | P::CreateReaction | P::Untrack | P::Flush | P::OnCleanup,
            0,
        ) => Some("deferred"),
        (P::CreateRoot, 0) | (P::RunWithOwner, 1) => Some("inline"),
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
        if !module.starts_with("./") && !module.starts_with("../") {
            return Some(format!(
                "{module:?} is a bare or path-mapped specifier with no exact project-local target"
            ));
        }
        if self.relative_module_file(file, module).is_none() {
            return Some(format!(
                "relative specifier {module:?} has no single project-local target"
            ));
        }
        // The module target itself is exact, which closes the question. A
        // missing or cyclic export past that point is a TypeScript resolution
        // diagnostic (for example TS2303), not an independent checker
        // obligation, so it is deliberately not walked here.
        None
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
    callback_forwardings: &'a mut Vec<(usize, usize, usize, usize, Option<String>)>,
    dispatches: &'a mut Vec<(usize, Vec<usize>)>,
    contract_generation_obligations: &'a mut [Vec<ContractGenerationObligation>],
    edges: &'a mut [Vec<usize>],
    invoked_parameters: &'a mut [Vec<usize>],
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
        if inside_effect_apply(file, reference_span, entities, symbol_names, lookup.dialect) {
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
    let mut edges = vec![Vec::<usize>::new(); nodes.len()];
    let mut invoked_parameters = vec![Vec::<usize>::new(); nodes.len()];
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
            edges: &mut edges,
            invoked_parameters: &mut invoked_parameters,
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
                        entities,
                        symbol_names,
                        InterproceduralContracts {
                            reads: contract_reads,
                            callbacks: contract_callbacks,
                        },
                        lookup,
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
                            entities,
                            symbol_names,
                            InterproceduralContracts {
                                reads: contract_reads,
                                callbacks: contract_callbacks,
                            },
                            lookup,
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
            for callback in callback_summaries[*target]
                .iter()
                .filter(|callback| callback.parameter == *target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                let forwarded = ContractCallback {
                    parameter: *owner_parameter,
                    execution: if callback.execution == "inline" {
                        ambient_execution.clone().unwrap_or(callback.execution)
                    } else {
                        callback.execution
                    },
                    evidence: None,
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
    for (index, node) in nodes.iter().enumerate() {
        let Some(&file) = project_indexes.files_by_path.get(node.path.as_str()) else {
            continue;
        };
        let returned_closures = file
            .ast
            .returns
            .iter()
            .filter(|returned| {
                containing_summary_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    file.path.as_str(),
                    returned.span,
                ) == Some(index)
            })
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
        for returned_value in file.ast.returns.iter().filter(|returned| {
            containing_summary_function_indexed(
                &nodes,
                &nodes_by_path,
                file.path.as_str(),
                returned.span,
            ) == Some(index)
        }) {
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
        for value in function
            .expression_return
            .iter()
            .chain(file.ast.returns.iter().filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            }))
        {
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
    let mut dispatch_obligations = Vec::new();
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
    // callers outside the analyzed project. Even if every visible call site
    // selects one exact implementation, those unseen callers keep its
    // reactive-read contract open. Record that boundary instead of exporting
    // an empty (and therefore falsely safe) summary.
    // `allowed_callback_spans` walks every call in the file. A file usually
    // holds several exported helpers, so computing it per node made this
    // O(exported nodes x calls) where O(files x calls) is enough.
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
                    member: Some(property.clone()),
                },
                location: location(Arc::from(node.path.as_str()), node.span),
                analysis_context: "exported-parameter-member-dispatch".into(),
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

    use solid_dialect::Primitive;
    use typefacts::Location;

    use super::{
        ContractCallback, SummaryRead, SummaryReads, SymbolId, add_interprocedural_dependency_user,
        cached_reactive_source, equivalent_callbacks, equivalent_summary_reads,
        primitive_callback_execution, reactive_source_order,
        remove_interprocedural_dependency_user, retained_reactive_sources,
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
        // The module deliberately labels `untrack`/`flush` "deferred" (see
        // the function's doc comment) even though the dialect vocabulary
        // calls them inline.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::Untrack), 0, 1, &dialect),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::Flush), 0, 1, &dialect),
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
