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
use typefacts::Location;

use super::runtime_semantics::{
    RuntimeArgumentBehavior, argument_behavior, potentially_callable,
    proven_array_method_argument_behavior, resolved_parameter, retains_argument_value,
};
use super::{
    ContractAnalysis, ContractCallback, ContractExport, ContractGenerationObligation,
    ContractGraph, ContractReturn, ContractSemantics, EntitySymbols, ExecutionRole,
    FunctionBoundary, ProjectIndexes, ReactiveRead, ReactiveSourceKind, SemanticLookup, SymbolId,
    allowed_callback_spans, assigned_member_function_contains, containing_summary_function_indexed,
    contract_callback_execution, contract_export_summaries, contract_export_summaries_incremental,
    execution_role, function_indices_by_path, functions_for_path, location, location_order,
    primitive_name, propagate_returned_summary_deltas, propagate_summary_deltas,
    push_contract_callback, push_unique_summary_read, semantic_execution_role,
};
use crate::cache::{
    CachedInterproceduralGraph, CachedInterproceduralResultFile, CachedInterproceduralResults,
    CachedReactiveSource, CachedTypedAccessors, InterproceduralGraphContribution,
    InterproceduralGraphTarget, InterproceduralResultDependency,
    InterproceduralResultDependencyState, TypedAccessorContribution, same_compiler_semantics,
};
use crate::owners::{
    containing_ast_function, enclosing_function_label, enclosing_render_function,
    function_binding_name, go_returned_arrow_pattern_accepts, go_solid_accessor_descriptor,
    inside_effect_apply, source_function_exported,
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

#[derive(Clone, Default)]
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
    dialect: &dyn solid_dialect::Dialect,
) -> Vec<TypedAccessorContribution> {
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
        let Some(descriptor) =
            descriptor.filter(|descriptor| go_solid_accessor_descriptor(descriptor, dialect))
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
            || enclosing_render_function(file, call.callee)
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
        let declaration = descriptor.alias_declarations.first().map_or_else(
            || callee_location.clone(),
            |declaration| declaration.location.clone(),
        );
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
            if typescript_file.is_some() && source_function.is_none() {
                continue;
            }
            let is_arrow = source_function.map_or(
                function.kind == solid_facts::ast::FunctionKind::Arrow,
                |function| function.arrow,
            );
            if is_arrow != arrow {
                continue;
            }
            let symbol = binding_name.as_ref().and_then(|name| {
                entities
                    .get(&location(file.path.shared(), name.span))
                    .cloned()
            });
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

fn discover_interprocedural_graph(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    entities: &EntitySymbols,
    contract_reads: &HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    contract_callbacks: &HashMap<SymbolId, Vec<ContractCallback>>,
    lookup: &SemanticLookup<'_>,
) -> InterproceduralGraphContribution {
    let mut contribution = InterproceduralGraphContribution::default();
    let primitives = lookup.primitives(file);
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
        let Some(symbol) = lookup.callee_symbol(file, call.callee) else {
            continue;
        };
        if call.direct_callee {
            contribution
                .factory_calls
                .push((owner_span, SymbolId::from(symbol)));
        }
        if !call.type_arguments
            && let Some(contracted) = contract_reads.get(symbol)
        {
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
        if !contract_reads.contains_key(symbol) && call.direct_callee && !call.type_arguments {
            contribution.edges.push((
                owner_span,
                InterproceduralGraphTarget::Symbol(SymbolId::from(symbol)),
            ));
        }
        if call.direct_callee
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
            contribution
                .invoked_parameters
                .push((owner_span, parameter));
            contribution.callbacks.push((
                nodes[callback_owner].span,
                ContractCallback {
                    parameter,
                    execution: contract_callback_execution(execution_role(
                        &file.compiler,
                        call.callee,
                        &[],
                    ))
                    .into(),
                },
            ));
        }
        if let Some(callbacks) = contract_callbacks.get(symbol) {
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
            if let Some(target) = nodes
                .iter()
                .position(|node| node.symbol.as_deref() == Some(symbol))
                .or_else(|| returned_function_target(file, nodes, nodes_by_path, entities, symbol))
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
                ));
                continue;
            }
            if let Some(execution) = primitive_callback_execution(
                super::known_primitive(&primitives.calls[call_index]),
                argument_index,
                lookup.dialect,
            ) {
                contribution.callbacks.push((
                    nodes[callback_owner].span,
                    ContractCallback {
                        parameter,
                        execution: execution.into(),
                    },
                ));
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
                            },
                        ));
                    }
                    RuntimeArgumentBehavior::ValueOnly => {}
                }
                continue;
            }
            if contract_callbacks.contains_key(symbol) {
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
                ContractGenerationObligation {
                    function: nodes[callback_owner]
                        .name
                        .clone()
                        .unwrap_or_else(|| "<anonymous>".into()),
                    parameter,
                    location: location(file.path.shared(), argument.span),
                    message: format!(
                        "parameter {parameter} escapes through call to {}; its execution semantics are unknown",
                        file.source_text(call.callee).unwrap_or("<unknown>")
                    ),
                },
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
    function
        .expression_return
        .iter()
        .chain(file.ast.returns.iter().filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        }))
        .filter(|returned| returned.value == solid_facts::ast::ReturnValueKind::Function)
        .find_map(|returned| {
            functions_for_path(nodes, nodes_by_path, file.path.as_str())
                .find(|(_, node)| node.span == returned.span)
                .map(|(index, _)| index)
        })
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
    dialect: &dyn solid_dialect::Dialect,
) -> Option<&'static str> {
    use Primitive as P;
    let primitive = primitive?;
    if matches!(primitive, P::CreateEffect | P::CreateRenderEffect) {
        return dialect
            .callback_executions(primitive)
            .iter()
            .find(|(index, _)| *index == parameter)
            .map(|(_, execution)| match execution {
                solid_dialect::Execution::Tracked => "tracked",
                solid_dialect::Execution::Deferred => "deferred",
                solid_dialect::Execution::Inline => "inline",
            });
    }
    match (primitive, parameter) {
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

struct InterproceduralGraphAssembly<'a> {
    nodes: &'a [SummaryNode],
    nodes_by_path: &'a HashMap<String, Vec<usize>>,
    by_symbol: &'a HashMap<SymbolId, usize>,
    summaries: &'a mut [SummaryReads],
    callback_summaries: &'a mut [Vec<ContractCallback>],
    callback_forwardings: &'a mut Vec<(usize, usize, usize, usize)>,
    contract_generation_obligations: &'a mut [Vec<ContractGenerationObligation>],
    edges: &'a mut [Vec<usize>],
    invoked_parameters: &'a mut [Vec<usize>],
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
        for (owner, callback) in &contribution.callbacks {
            if let Some(owner) = node_index(*owner) {
                push_contract_callback(&mut self.callback_summaries[owner], callback.clone());
            }
        }
        for (owner, target, target_parameter, owner_parameter) in &contribution.callback_forwardings
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
                ));
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
                        } if name == &self.nodes[*index].name
                            && summary.as_slice() == &self.summaries[*index][..]
                            && previous_parameters == &self.invoked_parameters[*index]
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
) -> (Vec<ReactiveRead>, HashSet<InterproceduralResultDependency>) {
    let InterproceduralResultReadContext {
        result:
            InterproceduralResultView {
                nodes,
                by_symbol,
                summaries,
                invoked_parameters,
                returned_bindings,
                ..
            },
        contract_callbacks,
        entities,
        symbol_names,
        lookup,
    } = context;
    let mut result = Vec::new();
    let mut dependencies = HashSet::new();
    let mut seen = HashSet::new();
    let allowed = allowed_callback_spans(file, lookup);
    for call in &file.ast.calls {
        if !enclosing_render_function(file, call.span) {
            continue;
        }
        let callee = location(file.path.shared(), call.callee);
        let Some(symbol) = lookup.callee_symbol(file, call.callee) else {
            continue;
        };
        dependencies.insert(InterproceduralResultDependency::Symbol(SymbolId::from(
            symbol,
        )));
        let (label, mut effective, target) = if let Some(target) = by_symbol.get(symbol).copied() {
            (
                nodes[target]
                    .name
                    .clone()
                    .or_else(|| call.static_callee(&file.source).map(str::to_owned))
                    .unwrap_or_else(|| "helper".into()),
                summaries[target].to_vec(),
                Some(target),
            )
        } else if let Some(summary) = returned_bindings.get(symbol) {
            (
                call.static_callee(&file.source)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "returned helper".into()),
                summary.clone(),
                None,
            )
        } else if contract_callbacks.contains_key(symbol) {
            (
                call.static_callee(&file.source)
                    .map(str::to_owned)
                    .unwrap_or_else(|| "contract callback".into()),
                Vec::new(),
                None,
            )
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
                });
            }
        }
    }
    (result, dependencies)
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
                ) == ExecutionRole::UntrackedRendering
                    && !enclosing_render_function(file, call.span)
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
    let mut contract_generation_obligations =
        vec![Vec::<ContractGenerationObligation>::new(); nodes.len()];
    let mut edges = vec![Vec::<usize>::new(); nodes.len()];
    let mut invoked_parameters = vec![Vec::<usize>::new(); nodes.len()];
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
            contract_generation_obligations: &mut contract_generation_obligations,
            edges: &mut edges,
            invoked_parameters: &mut invoked_parameters,
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
                        contract_reads,
                        contract_callbacks,
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
                            contract_reads,
                            contract_callbacks,
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
        for &(owner, target, target_parameter, owner_parameter) in &callback_forwardings {
            for callback in callback_summaries[target]
                .iter()
                .filter(|callback| callback.parameter == target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                let forwarded = ContractCallback {
                    parameter: owner_parameter,
                    execution: callback.execution,
                };
                if !callback_summaries[owner].contains(&forwarded) {
                    callback_summaries[owner].push(forwarded);
                    changed = true;
                }
            }
            for obligation in contract_generation_obligations[target]
                .iter()
                .filter(|obligation| obligation.parameter == target_parameter)
                .cloned()
                .collect::<Vec<_>>()
            {
                let forwarded = ContractGenerationObligation {
                    function: nodes[owner]
                        .name
                        .clone()
                        .unwrap_or_else(|| "<anonymous>".into()),
                    parameter: owner_parameter,
                    location: obligation.location,
                    message: format!(
                        "parameter {owner_parameter} reaches unresolved behavior through {}; {}",
                        nodes[target].name.as_deref().unwrap_or("<anonymous>"),
                        obligation.message
                    ),
                };
                let key = (forwarded.parameter, forwarded.location.clone());
                if contract_generation_obligation_keys[owner].insert(key) {
                    contract_generation_obligations[owner].push(forwarded);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let contract_generation_obligations = contract_generation_obligations
        .into_iter()
        .enumerate()
        .filter(|(index, _)| nodes[*index].exported)
        .flat_map(|(_, obligations)| obligations)
        .collect::<Vec<_>>();
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
                    lookup.dialect,
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
                        lookup.dialect,
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
                returned.value == solid_facts::ast::ReturnValueKind::Function
                    && returned.argument.is_some_and(|argument| {
                        go_returned_arrow_pattern_accepts(file.source.as_ref(), argument)
                    })
                    && containing_summary_function_indexed(
                        &nodes,
                        &nodes_by_path,
                        file.path.as_str(),
                        returned.span,
                    ) == Some(index)
            })
            .filter_map(|returned| returned.argument)
            .collect::<Vec<_>>();
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
                    if let Some(symbol) = entities.get(&returned_location)
                        && returned_source_symbols.contains(symbol)
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
                            !call.type_arguments && returned_value.argument == Some(call.span)
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
                if returned_source_symbols.contains(&read.symbol) {
                    returned[index].push(read);
                } else {
                    direct.push(read);
                }
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
    let mut result_reused_files = 0;
    let mut result_recomputed_files = 0;
    let result_view = InterproceduralResultView {
        nodes: &nodes,
        indexes: &indexes,
        by_symbol: &by_symbol,
        summaries: &summaries,
        invoked_parameters: &invoked_parameters,
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
            for (file, (reads, dependencies)) in facts.files.iter().zip(per_file) {
                result_recomputed_files += 1;
                result.extend(reads.iter().cloned());
                for dependency in &dependencies {
                    add_interprocedural_dependency_user(&mut cache.dependency_users, dependency);
                }
                cache.files.insert(
                    file.path.clone(),
                    CachedInterproceduralResultFile {
                        dependencies,
                        reads,
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
                    continue;
                }
                result_recomputed_files += 1;
                let (reads, dependencies) =
                    interprocedural_result_reads_for_file(file, &result_read_context);
                result.extend(reads.iter().cloned());
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
            result.extend(interprocedural_result_reads_for_file(file, &result_read_context).0);
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
    let contract_analysis = ContractAnalysis {
        summaries: &summaries,
        returned: &returned,
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
        SummaryRead, SummaryReads, SymbolId, add_interprocedural_dependency_user,
        cached_reactive_source, primitive_callback_execution, reactive_source_order,
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
            primitive_callback_execution(Some(Primitive::CreateEffect), 0, &solid2),
            Some("tracked")
        );
        // 2.0's second effect argument is the deferred apply callback.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 1, &solid2),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 2, &solid2),
            None
        );

        let solid1x = solid_dialect::Solid1x;
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 0, &solid1x),
            Some("tracked")
        );
        // 1.x's second argument is a seed value, not a callback.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateEffect), 1, &solid1x),
            None
        );
    }

    #[test]
    fn non_effect_callback_executions_use_the_module_classification() {
        let dialect = solid_dialect::Solid2;
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateMemo), 0, &dialect),
            Some("tracked")
        );
        // The module deliberately labels `untrack`/`flush` "deferred" (see
        // the function's doc comment) even though the dialect vocabulary
        // calls them inline.
        assert_eq!(
            primitive_callback_execution(Some(Primitive::Untrack), 0, &dialect),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::Flush), 0, &dialect),
            Some("deferred")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::CreateRoot), 0, &dialect),
            Some("inline")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::RunWithOwner), 1, &dialect),
            Some("inline")
        );
        assert_eq!(
            primitive_callback_execution(Some(Primitive::RunWithOwner), 0, &dialect),
            None
        );
        assert_eq!(primitive_callback_execution(None, 0, &dialect), None);
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
