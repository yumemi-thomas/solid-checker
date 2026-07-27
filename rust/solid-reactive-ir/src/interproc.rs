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

use solid_facts::ProjectFacts;
use solid_facts_core::Span;
use typefacts::Location;

use super::{
    CachedInterproceduralGraph, CachedInterproceduralResultFile, CachedInterproceduralResults,
    CachedReactiveSource, CachedTypedAccessors, ContractAnalysis, ContractCallback, ContractExport,
    ContractGraph, ContractReturn, ContractSemantics, EntitySymbols, ExecutionRole,
    FunctionBoundary, InterproceduralGraphContribution, InterproceduralGraphTarget,
    InterproceduralResultDependency, InterproceduralResultDependencyState, ProjectIndexes,
    ReactiveRead, ReactiveSourceKind, SemanticLookup, TypedAccessorContribution,
    allowed_callback_spans, containing_ast_function, containing_summary_function_indexed,
    contract_callback_execution, contract_export_summaries, contract_export_summaries_incremental,
    enclosing_function_label, enclosing_render_function, execution_role, function_binding_name,
    function_indices_by_path, functions_for_path, go_returned_arrow_pattern_accepts,
    go_solid_accessor_descriptor, inside_effect_apply, location, location_order,
    parallel_slice_results, primitive_name, propagate_returned_summary_deltas,
    propagate_summary_deltas, push_contract_callback, push_unique_summary_read,
    same_compiler_semantics, semantic_execution_role, source_function_exported,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SummaryRead {
    pub(super) symbol: String,
    pub(super) display: String,
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
    seen: HashSet<(String, Location, Location)>,
}

impl SummaryReads {
    fn key(read: &SummaryRead) -> (String, Location, Location) {
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
    pub(super) symbol: Option<String>,
    pub(super) parameters: Vec<String>,
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
    pub(super) reads: Vec<ReactiveRead>,
    pub(super) exports: Arc<BTreeMap<String, ContractExport>>,
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
    symbol_names: &HashMap<String, String>,
) -> Vec<TypedAccessorContribution> {
    let path_entities = project_indexes.entities_for_path(file.path.as_str());
    let mut contributions = Vec::new();
    for call in &file.ast.calls {
        let callee_location = location(file.path.as_str(), call.callee);
        let descriptor = path_entities
            .iter()
            .find(|entity| {
                entity.location.start_byte == callee_location.start_byte
                    && entity.location.end_byte == callee_location.end_byte
            })
            .and_then(|entity| entity.type_descriptor.as_ref());
        let Some(descriptor) =
            descriptor.filter(|descriptor| go_solid_accessor_descriptor(descriptor))
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
        if inside_effect_apply(file, call.callee, entities, symbol_names)
            || enclosing_render_function(file, call.callee)
        {
            continue;
        }
        let call_location = location(file.path.as_str(), call.span);
        let display = usize::try_from(call.callee.start)
            .ok()
            .zip(usize::try_from(call.callee.end).ok())
            .and_then(|(start, end)| file.source.get(start..end))
            .unwrap_or("accessor")
            .to_string();
        let declaration = descriptor.alias_declarations.first().map_or_else(
            || callee_location.clone(),
            |declaration| declaration.location.clone(),
        );
        contributions.push(TypedAccessorContribution {
            owner: nodes[owner].span,
            read: SummaryRead {
                symbol: format!(
                    "typed:{}\0{}\0{}",
                    call_location.path, call_location.start_byte, call_location.end_byte
                ),
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
            let source_function = typescript_file.and_then(|typescript_file| {
                typescript_file.functions.iter().find(|candidate| {
                    candidate.body.start_byte == u64::from(function.body.start)
                        && candidate.body.end_byte.saturating_add(1) == u64::from(function.body.end)
                })
            });
            // Preserve the checker's finite function universe and ordering:
            // declarations first, then block-bodied arrows,
            // each in source order.
            if typescript_file.is_some() && source_function.is_none() {
                continue;
            }
            let is_arrow = source_function.map_or(
                function.kind == solid_ast_facts::FunctionKind::Arrow,
                |function| function.arrow,
            );
            if is_arrow != arrow {
                continue;
            }
            let binding_name = function_binding_name(file, function);
            let symbol = binding_name.as_ref().and_then(|name| {
                entities
                    .get(&location(file.path.as_str(), name.span))
                    .cloned()
            });
            let parameters = function
                .parameters
                .iter()
                .filter(|parameter| parameter.shape == solid_ast_facts::BindingShape::Identifier)
                .filter_map(|parameter| parameter.names.first())
                .filter_map(|name| {
                    entities
                        .get(&location(file.path.as_str(), name.span))
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
    contract_reads: &HashMap<String, Vec<(String, String, Location, String)>>,
    contract_callbacks: &HashMap<String, Vec<ContractCallback>>,
) -> InterproceduralGraphContribution {
    let mut contribution = InterproceduralGraphContribution::default();
    for call in &file.ast.calls {
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            call.span,
        ) else {
            continue;
        };
        let owner_span = nodes[owner].span;
        let callee = location(file.path.as_str(), call.callee);
        let Some(symbol) = entities.get(&callee) else {
            continue;
        };
        if call.direct_callee {
            contribution
                .factory_calls
                .push((owner_span, symbol.clone()));
        }
        if !call.type_arguments
            && let Some(contracted) = contract_reads.get(symbol)
        {
            for (display, _, declaration, kind) in contracted {
                contribution.direct_reads.push((
                    owner_span,
                    SummaryRead {
                        symbol: symbol.clone(),
                        display: display.clone(),
                        kind: Some(kind.clone()),
                        declaration: declaration.clone(),
                        origin: location(file.path.as_str(), call.span),
                        origin_context: nodes[owner].name.clone().unwrap_or_default(),
                    },
                ));
            }
        }
        if !contract_reads.contains_key(symbol) && call.direct_callee && !call.type_arguments {
            contribution.edges.push((
                owner_span,
                InterproceduralGraphTarget::Symbol(symbol.clone()),
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
                let argument_location = location(file.path.as_str(), argument.span);
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
    }
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = file.ast.call_at(initializer) else {
            continue;
        };
        let Some(target_symbol) = entities.get(&location(file.path.as_str(), call.callee)) else {
            continue;
        };
        for name in &binding.names {
            if let Some(binding_symbol) = entities.get(&location(file.path.as_str(), name.span)) {
                contribution
                    .returned_bindings
                    .push((binding_symbol.clone(), target_symbol.clone()));
            }
        }
    }
    contribution
}

struct InterproceduralGraphAssembly<'a> {
    nodes: &'a [SummaryNode],
    nodes_by_path: &'a HashMap<String, Vec<usize>>,
    by_symbol: &'a HashMap<String, usize>,
    summaries: &'a mut [SummaryReads],
    callback_summaries: &'a mut [Vec<ContractCallback>],
    edges: &'a mut [Vec<usize>],
    invoked_parameters: &'a mut [Vec<usize>],
    returned_bindings: &'a mut Vec<(String, String)>,
    factory_calls: &'a mut Vec<(usize, String)>,
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
    pub(super) by_symbol: &'a HashMap<String, usize>,
    pub(super) summaries: &'a [SummaryReads],
    pub(super) invoked_parameters: &'a [Vec<usize>],
    pub(super) returned_bindings: &'a HashMap<String, Vec<SummaryRead>>,
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
    pub(super) contract_callbacks: &'a HashMap<String, Vec<ContractCallback>>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<String, String>,
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
        let callee = location(file.path.as_str(), call.callee);
        let Some(symbol) = entities.get(&callee) else {
            continue;
        };
        dependencies.insert(InterproceduralResultDependency::Symbol(symbol.clone()));
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
                    entities.get(&location(file.path.as_str(), argument.span))
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
                let argument_symbol = entities.get(&location(file.path.as_str(), argument.span));
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
                        format!("{}#callback-{}", read.symbol, callback.parameter),
                    )) {
                        result.push(ReactiveRead {
                            kind: "accessor".into(),
                            accessor: read.display.clone(),
                            location: location(file.path.as_str(), call.span),
                            declaration: read.declaration.clone(),
                            execution: callback_execution,
                            context: context
                                .get_or_insert_with(|| enclosing_function_label(file, call.span))
                                .clone(),
                            via: label.clone(),
                            origin: Some(read.origin.clone()),
                            origin_context: read.origin_context.clone(),
                        });
                    }
                }
            }
        }
        for read in effective {
            let accessor = read.display.clone();
            if seen.insert((callee.path.clone(), callee.start_byte, read.symbol.clone())) {
                result.push(ReactiveRead {
                    kind: if read.display.contains('.') {
                        "store-path".into()
                    } else {
                        "accessor".into()
                    },
                    accessor,
                    location: location(file.path.as_str(), call.span),
                    declaration: read.declaration,
                    execution,
                    context: context
                        .get_or_insert_with(|| enclosing_function_label(file, call.span))
                        .clone(),
                    via: label.clone(),
                    origin: Some(read.origin),
                    origin_context: read.origin_context,
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
    source_phases: &HashMap<String, u8>,
) -> CachedReactiveSource {
    CachedReactiveSource {
        symbol: symbol.to_owned(),
        display: display.to_owned(),
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
    accessors: &HashMap<String, (String, Location)>,
    contracted_accessor_symbols: &HashSet<String>,
    summary_source_symbols: &HashSet<String>,
    source_phases: &HashMap<String, u8>,
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
    context: &InterproceduralContext<'_>,
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
        if inside_effect_apply(file, reference_span, entities, symbol_names) {
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
                kind: None,
                declaration: source.declaration.clone(),
                origin: location(file.path.as_str(), call.span),
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
                let contract_location = Location {
                    path: format!("bundled://solid-js.json#{primitive}").into(),
                    start_byte: 0,
                    end_byte: 0,
                };
                read.display.clone_from(&returned.label);
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
                            display: format!(
                                "{}.{}",
                                source.display,
                                file.source_text(member.property).unwrap_or_default()
                            ),
                            kind: None,
                            declaration: source.declaration.clone(),
                            origin: location(file.path.as_str(), member.span),
                            origin_context: nodes[owner].name.clone().unwrap_or_default(),
                        },
                        unique: false,
                    }),
            );
        }
    }
    contributions
}

pub(super) struct InterproceduralContext<'a> {
    pub(super) facts: &'a ProjectFacts,
    pub(super) project_indexes: &'a ProjectIndexes<'a>,
    pub(super) accessors: &'a HashMap<String, (String, Location)>,
    pub(super) contracted_accessor_symbols: &'a HashSet<String>,
    pub(super) returned_source_symbols: &'a HashSet<String>,
    pub(super) summary_source_symbols: &'a HashSet<String>,
    pub(super) source_phases: &'a HashMap<String, u8>,
    pub(super) source_kinds: &'a HashMap<String, ReactiveSourceKind>,
    pub(super) contract_reads: &'a HashMap<String, Vec<(String, String, Location, String)>>,
    pub(super) contract_callbacks: &'a HashMap<String, Vec<ContractCallback>>,
    pub(super) contract_returns: &'a HashMap<String, (ContractReturn, Location)>,
    pub(super) bundled_returns: &'a HashMap<String, ContractReturn>,
    pub(super) source_primitives: &'a HashMap<String, String>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) references_by_source: &'a HashMap<String, Vec<Location>>,
    pub(super) symbol_names: &'a HashMap<String, String>,
    pub(super) changed_semantic_symbols: Option<&'a HashSet<String>>,
    pub(super) retained_source_paths: &'a HashSet<String>,
    pub(super) lookup: &'a SemanticLookup<'a>,
}

impl InterproceduralContext<'_> {
    pub(super) fn build(
        &self,
        typed_accessor_cache: Option<&mut HashMap<String, CachedTypedAccessors>>,
        interprocedural_graph_cache: Option<&mut HashMap<String, CachedInterproceduralGraph>>,
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
    typed_accessors: Option<&'a mut HashMap<String, CachedTypedAccessors>>,
    graph: Option<&'a mut HashMap<String, CachedInterproceduralGraph>>,
    results: Option<&'a mut CachedInterproceduralResults>,
}

fn interprocedural_reads(
    context: &InterproceduralContext<'_>,
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
        lookup: _,
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
        for file in &facts.files {
            if retained_source_paths.contains(file.path.as_str())
                && let Some(cached) = cache.get(file.path.as_str())
                && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                    || same_compiler_semantics(&cached.compiler, &file.compiler))
            {
                nodes.extend(cached.nodes.iter().cloned());
                graph_node_reused_paths.insert(file.path.as_str());
                continue;
            }
            let file_nodes = discover_summary_nodes(file, project_indexes, entities);
            nodes.extend(file_nodes.iter().cloned());
            cache.insert(
                file.path.to_string(),
                CachedInterproceduralGraph {
                    nodes: file_nodes,
                    contribution: InterproceduralGraphContribution::default(),
                    compiler: file.compiler.clone(),
                },
            );
        }
    } else {
        for file in &facts.files {
            nodes.extend(discover_summary_nodes(file, project_indexes, entities));
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
            edges: &mut edges,
            invoked_parameters: &mut invoked_parameters,
            returned_bindings: &mut returned_binding_candidates,
            factory_calls: &mut factory_call_candidates,
        };
        match interprocedural_graph_cache {
            None => {
                for file in &facts.files {
                    graph_recomputed_files += 1;
                    let contribution = discover_interprocedural_graph(
                        file,
                        &nodes,
                        &nodes_by_path,
                        entities,
                        contract_reads,
                        contract_callbacks,
                    );
                    graph.merge(file.path.as_str(), &contribution);
                }
            }
            Some(cache) => {
                for file in &facts.files {
                    if graph_node_reused_paths.contains(file.path.as_str())
                        && let Some(cached) = cache.get(file.path.as_str())
                    {
                        graph_reused_files += 1;
                        graph.merge(file.path.as_str(), &cached.contribution);
                        continue;
                    }
                    graph_recomputed_files += 1;
                    let contribution = discover_interprocedural_graph(
                        file,
                        &nodes,
                        &nodes_by_path,
                        entities,
                        contract_reads,
                        contract_callbacks,
                    );
                    graph.merge(file.path.as_str(), &contribution);
                    cache.insert(
                        file.path.to_string(),
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
                    );
                    cache.insert(
                        file.path.to_string(),
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
                returned.value == solid_ast_facts::ReturnValueKind::Function
                    && returned.argument.is_some_and(|argument| {
                        go_returned_arrow_pattern_accepts(file.source.as_str(), argument)
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
                solid_ast_facts::ReturnValueKind::Identifier => {
                    let returned_location = location(file.path.as_str(), returned_value.span);
                    if let Some(symbol) = entities.get(&returned_location)
                        && returned_source_symbols.contains(symbol)
                        && let Some((display, declaration)) = accessors.get(symbol)
                    {
                        returned[index].push_unique(SummaryRead {
                            symbol: symbol.clone(),
                            display: display.clone(),
                            kind: None,
                            declaration: declaration.clone(),
                            origin: returned_location,
                            origin_context: node.name.clone().unwrap_or_default(),
                        });
                    }
                }
                solid_ast_facts::ReturnValueKind::Call => {
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
                    let callee_location = location(file.path.as_str(), call.callee);
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
                                )
                                .and_then(|primitive| {
                                    bundled_returns.get(primitive.as_str()).cloned().map(
                                        |returned| {
                                            (
                                                returned,
                                                Location {
                                                    path: format!(
                                                        "bundled://solid-js.json#{primitive}"
                                                    )
                                                    .into(),
                                                    start_byte: 0,
                                                    end_byte: 0,
                                                },
                                            )
                                        },
                                    )
                                })
                            });
                            if let Some((returned_contract, declaration)) = contracted {
                                returned[index].push_unique(SummaryRead {
                                    symbol: symbol.clone(),
                                    display: returned_contract.label,
                                    kind: Some(returned_contract.kind),
                                    declaration,
                                    origin: location(file.path.as_str(), call.span),
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
            if value.value == solid_ast_facts::ReturnValueKind::Function
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
    let mut returned_bindings = HashMap::<String, Vec<SummaryRead>>::new();
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
            .filter(|(dependency, retained)| !result_view.dependency_matches(retained, dependency))
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
                    add_interprocedural_dependency_user(&mut cache.dependency_users, dependency);
                }
            } else {
                for dependency in &dependencies {
                    add_interprocedural_dependency_user(&mut cache.dependency_users, dependency);
                }
            }
            cache.files.insert(
                file.path.to_string(),
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
        reads: result,
        exports,
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
