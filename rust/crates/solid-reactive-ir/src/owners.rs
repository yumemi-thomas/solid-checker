//! Owner analysis: which computations need a reactive owner, where
//! owners are provided, and the requirement/fix emission around them.

use crate::*;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use crate::cleanup::function_returns_cleanup;
use crate::execution_role::{argument_references_callback_symbol, execution_role, function_symbol};
use crate::identity::SymbolId;
use crate::indexes::{CrossFileProofDigest, EntitySymbols, ProjectIndexes, SemanticLookup};
use solid_dialect::{Dialect, Primitive};
use solid_facts::ProjectFacts;
use solid_facts::core::{SourceHash, SourcePath, Span};

pub(crate) fn component_props_parameter_fix(
    facts: &ProjectFacts,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    parameter: &solid_facts::ast::BindingFact,
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

pub(crate) fn containing_ast_function(
    ast: &solid_facts::ast::AstFacts,
    span: Span,
) -> Option<&solid_facts::ast::FunctionFact> {
    ast.functions_body_containing(span)
        .min_by_key(|function| function.body.end - function.body.start)
}

pub(crate) const OWNER_CONTEXT_OWNED: u8 = 1;
pub(crate) const OWNER_CONTEXT_UNOWNED: u8 = 2;
pub(crate) const OWNER_CONTEXT_LEAF: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerEdgeKind {
    Preserve,
    Owned,
    Unowned,
    Conditional,
    Leaf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReturnedCallbackInvocationSite {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) inherited_execution: Option<solid_dialect::Execution>,
    pub(crate) inherited_owner: OwnerEdgeKind,
}

impl ReturnedCallbackInvocationSite {
    /// Total order over every field, for sort-then-dedup. A key that omitted
    /// a field could leave equal sites non-adjacent, and `dedup` only removes
    /// adjacent duplicates.
    ///
    /// Borrows the path. `sort_by_key` calls its key function on every
    /// comparison, so cloning the path here allocated a `String` per comparison
    /// -- `O(n log n)` allocations to order a list whose elements already own
    /// their paths.
    pub(crate) fn order_key(&self) -> (&str, Span, u8, Option<u8>) {
        (
            self.path.as_str(),
            self.span,
            self.inherited_owner as u8,
            self.inherited_execution.map(|execution| execution as u8),
        )
    }
}

#[derive(Clone)]
pub(crate) struct OwnerCallbackEdge {
    pub(crate) argument: usize,
    pub(crate) kind: OwnerEdgeKind,
    pub(crate) source_path: String,
    pub(crate) source: Span,
}

#[derive(Clone)]
pub(crate) struct OwnerNode {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) body: Span,
    pub(crate) name: Option<String>,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) exported: bool,
}

impl FunctionBoundary for OwnerNode {
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Span {
        self.body
    }
}

pub(crate) struct OwnerFileIndex {
    pub(crate) call_primitives: Vec<Option<PrimitiveName>>,
    pub(crate) providing_regions: Vec<Span>,
}

#[derive(Clone)]
pub(crate) enum OwnerTarget {
    Symbol(SymbolId),
    LocalSpan(Span),
}

#[derive(Clone)]
pub(crate) struct SymbolicOwnerEdge {
    pub(crate) source: Option<OwnerSource>,
    pub(crate) target: OwnerTarget,
    pub(crate) kind: OwnerEdgeKind,
}

#[derive(Clone)]
pub(crate) struct OwnerSource {
    pub(crate) path: String,
    pub(crate) span: Span,
}

#[derive(Clone)]
pub(crate) struct OwnerRequirementCandidate {
    pub(crate) operation: &'static str,
    pub(crate) operation_span: Span,
    pub(crate) owner: Option<Span>,
    pub(crate) report_mask: u8,
    pub(crate) allow_uncertain: bool,
    pub(crate) settled_target: Option<OwnerTarget>,
}

#[derive(Clone, Copy)]
pub(crate) struct OwnerRequirementStatus {
    pub(crate) uncertain: bool,
    pub(crate) conditional_owner: bool,
    pub(crate) report: bool,
}

pub(crate) struct CachedOwnerFile {
    pub(crate) source_hash: SourceHash,
    /// See [`SemanticLookup::returned_callback_proof_digest`]. Owner edges for
    /// a returned adapter's callbacks exist only where the project invokes that
    /// adapter, which is a fact about every other file.
    pub(crate) cross_file_proofs: Option<CrossFileProofDigest>,
    pub(crate) compiler: Arc<solid_facts::compiler::ExecutionMap>,
    pub(crate) nodes: Vec<OwnerNode>,
    pub(crate) edges: Vec<SymbolicOwnerEdge>,
    pub(crate) requirements: Vec<OwnerRequirementCandidate>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct OwnerIncrementalTimings {
    pub(crate) fragment_build: Duration,
    pub(crate) graph_assembly: Duration,
    pub(crate) propagation: Duration,
    pub(crate) requirement_emission: Duration,
}

pub(crate) fn find_missing_owners(
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
                        lookup.dialect,
                    )
                })
                .collect::<Vec<_>>();
            let providing_regions = file
                .ast
                .calls
                .iter()
                .zip(&call_primitives)
                .filter_map(|(call, primitive)| {
                    let argument =
                        owner_providing_argument(file, call, known_primitive(primitive), lookup)?;
                    call.arguments.get(argument).and_then(|argument| {
                        matches!(
                            argument.value,
                            solid_facts::ast::ArgumentValueKind::Identifier
                                | solid_facts::ast::ArgumentValueKind::Function
                                | solid_facts::ast::ArgumentValueKind::AsyncFunction
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
            // The binding-aware name, so an arrow bound to `const Foo = ...`
            // carries the same identity as `function Foo()`: `symbol` is how
            // call edges reach this node, `name` is how component casing and
            // export status seed its context, and the two passes must derive
            // both from the same lookup or fresh and incremental builds
            // disagree on arrow-bound functions.
            let name = function_binding_name(file, function);
            let symbol = name.and_then(|name| {
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
                name: name.map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
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
            for edge in owner_callback_edges(
                file,
                call,
                &owner_file_indexes[file_index].call_primitives[call_index],
                lookup,
            ) {
                let Some(argument) = call.arguments.get(edge.argument) else {
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
                let invocation_owner = containing_function_indexed(
                    &nodes,
                    &nodes_by_path,
                    &edge.source_path,
                    edge.source,
                );
                if let Some(owner) = invocation_owner {
                    edges.push((owner, target_index, edge.kind));
                } else {
                    contexts[target_index] |= owner_edge_context(edge.kind, OWNER_CONTEXT_UNOWNED);
                }
            }
        }
        for callback in &file.compiler.callback_roles {
            if !matches!(
                callback.role,
                solid_facts::compiler::CallbackRoleKind::EventHandler
                    | solid_facts::compiler::CallbackRoleKind::DirectiveApply
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
            let primitive =
                known_primitive(&owner_file_indexes[file_index].call_primitives[call_index]);
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
                // `createRenderEffect` is deliberately included alongside
                // `createEffect`: both register a computation on the owner,
                // and 2.0's render effect outside any owner leaks the same
                // way. The engine matched only `createEffect` and
                // `createTrackedEffect` before the dialect extraction; that
                // omission was the gap, not the rule.
                Some(
                    Primitive::CreateEffect
                    | Primitive::CreateRenderEffect
                    | Primitive::CreateTrackedEffect,
                ) if !root_owned => Some(("effect", context & OWNER_CONTEXT_UNOWNED != 0)),
                Some(Primitive::OnCleanup) if !root_owned => Some((
                    "cleanup",
                    context & (OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF) != 0,
                )),
                Some(Primitive::OnSettled)
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
                let conditional_owner = context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                    == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
                let uncertain = conditional_owner
                    || containing_function_indexed(
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
                    OwnerRequirementStatus {
                        uncertain,
                        conditional_owner,
                        report,
                    },
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
                lookup.dialect,
            );
            if !boundary
                .as_deref()
                .is_some_and(|tag| lookup.dialect.is_async_boundary(tag))
            {
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
            let conditional_owner = context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                "boundary",
                file.path.as_str(),
                Span::new(element.span.start, element.name.span.end),
                OwnerRequirementStatus {
                    uncertain: conditional_owner,
                    conditional_owner,
                    report: context & OWNER_CONTEXT_UNOWNED != 0,
                },
            );
        }
    }
    requirements
}

pub(crate) fn discover_owner_file(
    file: &solid_facts::FileFacts,
    indexes: &ProjectIndexes<'_>,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> CachedOwnerFile {
    let dialect = lookup.dialect;
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
                dialect,
            )
        })
        .collect::<Vec<_>>();
    let providing_regions = file
        .ast
        .calls
        .iter()
        .zip(&call_primitives)
        .filter_map(|(call, primitive)| {
            let argument =
                owner_providing_argument(file, call, known_primitive(primitive), lookup)?;
            call.arguments.get(argument).and_then(|argument| {
                matches!(
                    argument.value,
                    solid_facts::ast::ArgumentValueKind::Identifier
                        | solid_facts::ast::ArgumentValueKind::Function
                        | solid_facts::ast::ArgumentValueKind::AsyncFunction
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
            // The same binding-aware name the fresh pass uses; see
            // `find_missing_owners`. Deriving `symbol` and `name` from one
            // lookup keeps arrow-bound functions identical across passes.
            let name = function_binding_name(file, function);
            let symbol = name.and_then(|name| {
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
                name: name.map(|name| file.source_text(name.span).unwrap_or_default().to_owned()),
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
                source: owner.map(|span| OwnerSource {
                    path: file.path.to_string(),
                    span,
                }),
                target: OwnerTarget::Symbol(symbol.clone()),
                kind: OwnerEdgeKind::Preserve,
            });
        }
        for edge in owner_callback_edges(file, call, &call_primitives[call_index], lookup) {
            if let Some(target) = call
                .arguments
                .get(edge.argument)
                .and_then(|argument| callback_target(argument.span))
            {
                let source = lookup
                    .files()
                    .iter()
                    .find(|candidate| candidate.path.as_str() == edge.source_path)
                    .and_then(|source_file| {
                        containing_ast_function(&source_file.ast, edge.source).map(|function| {
                            OwnerSource {
                                path: edge.source_path.clone(),
                                span: function.span,
                            }
                        })
                    });
                edges.push(SymbolicOwnerEdge {
                    source,
                    target,
                    kind: edge.kind,
                });
            }
        }
        if inside_owner_providing_region(&providing_regions, call.span) {
            continue;
        }
        let operation = match known_primitive(&call_primitives[call_index]) {
            // See the batch owner pass: `createRenderEffect` belongs with the
            // other effect constructors, and its earlier absence there was
            // the two passes' drift, not a narrower contract.
            Some(
                Primitive::CreateEffect
                | Primitive::CreateRenderEffect
                | Primitive::CreateTrackedEffect,
            ) => Some(("effect", OWNER_CONTEXT_UNOWNED, None, call.callee)),
            Some(Primitive::OnCleanup) => Some((
                "cleanup",
                OWNER_CONTEXT_UNOWNED | OWNER_CONTEXT_LEAF,
                None,
                call.callee,
            )),
            Some(Primitive::OnSettled) => Some((
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
            solid_facts::compiler::CallbackRoleKind::EventHandler
                | solid_facts::compiler::CallbackRoleKind::DirectiveApply
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
            dialect,
        );
        if boundary
            .as_deref()
            .is_some_and(|tag| dialect.is_async_boundary(tag))
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
        cross_file_proofs: lookup.returned_callback_proof_digest(),
        compiler: file.compiler.clone(),
        nodes,
        edges,
        requirements,
    }
}

pub(crate) fn resolve_owner_target(
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

pub(crate) fn find_missing_owners_incremental(
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
            && cached.cross_file_proofs == lookup.returned_callback_proof_digest()
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
            discover_owner_file(file, indexes, entities, symbol_names, lookup),
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
            if let Some(source) = edge.source.as_ref().and_then(|source| {
                nodes_by_span
                    .get(&source.path)
                    .and_then(|nodes| nodes.get(&source.span))
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
            let conditional_owner = context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED)
                == (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED);
            let uncertain = conditional_owner
                || (candidate.allow_uncertain
                    && owner_index.is_some_and(|index| {
                        nodes[index].exported
                            && contexts[index] & OWNER_CONTEXT_UNOWNED != 0
                            && !nodes[index]
                                .name
                                .as_deref()
                                .and_then(|name| name.chars().next())
                                .is_some_and(char::is_uppercase)
                    }));
            push_owner_requirement(
                &mut requirements,
                &mut seen,
                candidate.operation,
                file.path.as_str(),
                candidate.operation_span,
                OwnerRequirementStatus {
                    uncertain,
                    conditional_owner,
                    report: context & candidate.report_mask != 0,
                },
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

pub(crate) fn inside_owner_providing_region(providing_regions: &[Span], span: Span) -> bool {
    providing_regions
        .iter()
        .any(|argument| argument.contains(span))
}

pub(crate) fn owner_callback_index(
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

pub(crate) const fn owner_edge_context(kind: OwnerEdgeKind, source: u8) -> u8 {
    match kind {
        OwnerEdgeKind::Preserve => source,
        OwnerEdgeKind::Owned => OWNER_CONTEXT_OWNED,
        OwnerEdgeKind::Unowned => OWNER_CONTEXT_UNOWNED,
        OwnerEdgeKind::Conditional => OWNER_CONTEXT_OWNED | OWNER_CONTEXT_UNOWNED,
        OwnerEdgeKind::Leaf => OWNER_CONTEXT_LEAF,
    }
}

pub(crate) fn owner_context_at(
    nodes: &[OwnerNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    contexts: &[u8],
    path: &str,
    span: Span,
) -> u8 {
    containing_function_indexed(nodes, nodes_by_path, path, span)
        .map_or(OWNER_CONTEXT_UNOWNED, |index| contexts[index])
}

pub(crate) fn push_owner_requirement(
    requirements: &mut Vec<OwnerRequirement>,
    seen: &mut HashSet<(String, u64, u64, String)>,
    operation: &str,
    path: &str,
    span: Span,
    status: OwnerRequirementStatus,
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
            uncertain: status.uncertain,
            conditional_owner: status.conditional_owner,
            report: status.report,
        });
    }
}
/// Which callback arguments of a call get an owner edge, and of what kind.
///
/// Shared by both owner passes on purpose: each used to keep its own literal
/// list, and lists kept separately drift.
pub(crate) fn owner_callback_edges(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: &Option<PrimitiveName>,
    lookup: &SemanticLookup<'_>,
) -> Vec<OwnerCallbackEdge> {
    let Some(primitive) = known_primitive(primitive) else {
        // A call that names no primitive can still be an invocation of a
        // function some primitive returned -- but only where the dialect models
        // such a function. Solid 2.0 models none, so the binding-chain walk
        // below would resolve a primitive and then be told `None` every time.
        if !lookup.models_returned_callbacks() {
            return Vec::new();
        }
        let Some((returned, result_slot)) = returned_primitive_invocation(file, call, lookup)
        else {
            return Vec::new();
        };
        return (0..call.arguments.len())
            .filter_map(|argument| {
                lookup
                    .dialect
                    .returned_callback_owner_at(
                        returned,
                        result_slot,
                        argument,
                        call.arguments.len(),
                    )
                    .map(|owner| OwnerCallbackEdge {
                        argument,
                        kind: callback_owner_edge_kind(owner),
                        source_path: file.path.to_string(),
                        source: call.span,
                    })
            })
            .collect();
    };
    let mut edges = Vec::new();
    for argument in 0..call.arguments.len() {
        let Some(owner) = callback_owner_at_call(file, call, primitive, argument, lookup) else {
            continue;
        };
        let kind = callback_owner_edge_kind(owner);
        if lookup
            .dialect
            .callback_requires_return_invocation(primitive, argument)
        {
            edges.extend(
                returned_callback_invocation_sites(file, call, lookup)
                    .into_iter()
                    .map(|site| OwnerCallbackEdge {
                        argument,
                        kind: compose_owner_edge(site.inherited_owner, kind),
                        source_path: site.path,
                        source: site.span,
                    }),
            );
        } else {
            edges.push(OwnerCallbackEdge {
                argument,
                kind,
                source_path: file.path.to_string(),
                source: call.span,
            });
        }
    }
    edges
}

pub(crate) const fn callback_owner_edge_kind(owner: solid_dialect::CallbackOwner) -> OwnerEdgeKind {
    match owner {
        solid_dialect::CallbackOwner::Creates => OwnerEdgeKind::Owned,
        solid_dialect::CallbackOwner::Conditional => OwnerEdgeKind::Conditional,
        solid_dialect::CallbackOwner::Inherits => OwnerEdgeKind::Preserve,
        solid_dialect::CallbackOwner::None => OwnerEdgeKind::Unowned,
        solid_dialect::CallbackOwner::Leaf => OwnerEdgeKind::Leaf,
    }
}

/// Compose the owner supplied by a returned function's invocation with the
/// owner behavior of the factory callback itself. An explicit owner result on
/// the inner callback dominates; an inheriting callback keeps the invocation
/// site's context.
pub(crate) const fn compose_owner_edge(
    invocation: OwnerEdgeKind,
    callback: OwnerEdgeKind,
) -> OwnerEdgeKind {
    match callback {
        OwnerEdgeKind::Preserve => invocation,
        _ => callback,
    }
}

/// The argument whose callback runs under an owner this call creates (or, for
/// `runWithOwner`, supplies). The two owner passes share this answer; each
/// used to keep its own literal list, and the lists disagreed with every
/// primitive outside seven names.
pub(crate) fn owner_providing_argument(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
    lookup: &SemanticLookup<'_>,
) -> Option<usize> {
    let primitive = primitive?;
    (0..call.arguments.len()).find(|index| {
        callback_owner_at_call(file, call, primitive, *index, lookup)
            == Some(solid_dialect::CallbackOwner::Creates)
    })
}

pub(crate) fn containing_leaf_owner(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            let owner = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                entities,
                symbol_names,
                lookup.dialect,
            )?;
            owner
                .primitive()
                .is_some_and(|primitive| {
                    callback_owner_at_call(file, call, primitive, index, lookup)
                        == Some(solid_dialect::CallbackOwner::Leaf)
                })
                .then(|| owner.to_string())
        })
}

pub(crate) fn read_is_under_loading(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    let entities = lookup.entities();
    if file.ast.jsx_containing(span).any(|element| {
        jsx_element_is_loading(file, element, entities, symbol_names, lookup.dialect)
    }) {
        return true;
    }
    if file.ast.jsx_containing(span).any(|element| {
        jsx_target_function(lookup, file, element).is_some_and(|(target_file, target)| {
            target_file.ast.jsx_within(target.body).any(|candidate| {
                jsx_element_is_loading(
                    target_file,
                    candidate,
                    entities,
                    symbol_names,
                    lookup.dialect,
                )
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
            && file.ast.jsx_within(owner.body).any(|candidate| {
                jsx_element_is_loading(file, candidate, entities, symbol_names, lookup.dialect)
            }))
}

pub(crate) fn jsx_element_is_loading(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    primitive_name(
        file.path.as_str(),
        element.name.span,
        Some(file.source_text(element.name.span).unwrap_or_default()),
        entities,
        symbol_names,
        dialect,
    )
    .as_deref()
    .is_some_and(|tag| dialect.is_async_boundary(tag))
}

pub(crate) fn jsx_target_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
)> {
    lookup.function_called_at(file.path.as_str(), element.name.span)
}

pub(crate) fn computation_is_async(
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

pub(crate) fn inside_lowercase_named_function(
    file: &solid_facts::FileFacts,
    span: Span,
    dialect: &dyn Dialect,
) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    // An argument to a primitive that takes a callback is not "some helper
    // function's body", whatever the helper is called. Asked of the dialect
    // rather than listed: the seven names this had were 2.0's, three of which
    // 1.x does not have, and it was missing every 1.x callback-taker outside
    // the four it shares.
    if file.ast.arguments_containing(span).any(|(call, index)| {
        call.static_callee(&file.source).is_some_and(|callee| {
            callee
                .rsplit('.')
                .next()
                .and_then(|name| dialect.primitive(name))
                .is_some_and(|primitive| {
                    dialect
                        .callback_execution_at(primitive, index, call.arguments.len())
                        .is_some()
                })
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

pub(crate) fn inside_unclassified_callback(file: &solid_facts::FileFacts, span: Span) -> bool {
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

/// Whether `span` is inside a syntactically direct function argument that a
/// known primitive provably never invokes.
///
/// Two positive proofs qualify, and nothing else: the dialect declares the
/// position a stored value (Solid 1.x `createSignal(() => value)`), or the
/// position's modelled callback belongs to a returned lazy adapter that this
/// call never consumes (`mapArray` whose result is discarded). A primitive
/// with no callback model at all — 2.0 `children`, `onCleanup` — proves
/// nothing: those callbacks do run, so their reads must not be treated as
/// dormant. Requiring the argument itself to be a function keeps an IIFE
/// passed as a value from being mistaken for dormant code.
pub(crate) fn inside_known_value_function_argument(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // Both proofs are dialect answers that Solid 2.0 leaves at their negative
    // default, so under 2.0 this question is provably always "no" and the
    // containment query below need not run at all.
    if !lookup.models_stored_function_arguments() && !lookup.models_returned_callbacks() {
        return false;
    }
    // Only the calls whose arguments actually contain `span` can answer. The
    // former whole-file scan asked every call in the file, once per call being
    // classified, which is quadratic in a file's call count.
    file.ast
        .arguments_containing(span)
        .any(|(call, argument_index)| {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                return false;
            };
            let argument = &call.arguments[argument_index];
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && (lookup
                .dialect
                .stores_function_argument_as_value(primitive, argument_index)
                || (lookup
                    .dialect
                    .callback_execution_at(primitive, argument_index, call.arguments.len())
                    .is_some()
                    && callback_execution_at_call(file, call, primitive, argument_index, lookup)
                        .is_none()))
        })
}

/// The callback execution fact for one concrete primitive call, after proving
/// any lazy returned adapter is actually consumed.
///
/// Solid 1.x `mapArray` and `indexArray` merely allocate and return a function;
/// their list and mapper arguments run only when that returned function runs.
/// The dialect owns that runtime distinction, while this helper proves the
/// use from canonical TypeScript symbols and the enclosing AST call shape.
pub(crate) fn callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    if lookup
        .dialect
        .callback_requires_return_invocation(primitive, argument)
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    lookup
        .dialect
        .callback_execution_at(primitive, argument, call.arguments.len())
}

/// The callback-owner fact for one concrete primitive call, after proving
/// that callbacks implemented by a returned lazy adapter can run.
///
/// Execution, reachability, cleanup, and both owner-graph passes must ask the
/// same call-site question. Otherwise a discarded adapter can disappear from
/// read analysis while still manufacturing an impossible owner diagnostic.
pub(crate) fn callback_owner_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    if lookup
        .dialect
        .callback_requires_return_invocation(primitive, argument)
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    if primitive == Primitive::RunWithOwner && argument == 1 {
        return run_with_owner_callback_owner(file, call, lookup);
    }
    lookup
        .dialect
        .callback_owner_at(primitive, argument, call.arguments.len())
}

/// `runWithOwner` is the one ownership primitive whose owner is data. Both
/// dialects accept `Owner | null`; 2.0 makes the null spelling the documented
/// way to detach a root. Preserve a definite answer where the call site proves
/// one and keep a conditional edge everywhere else so certification cannot
/// silently assume a nullable owner exists.
pub(crate) fn run_with_owner_callback_owner(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    let owner = call.arguments.first()?;
    let source = file.source_text(owner.span)?.trim();
    if owner.value == solid_facts::ast::ArgumentValueKind::Undefined
        || source == "null"
        || source == "undefined"
        || source.starts_with("void ")
    {
        return Some(solid_dialect::CallbackOwner::None);
    }

    // 2.0's createOwner() is non-null by construction. Resolve the nested
    // call semantically so a local function with the same spelling proves
    // nothing.
    if lookup.primitive_at_call(file, owner.span) == Some(Primitive::CreateOwner) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }

    if let Some(descriptor) = lookup
        .entity_at(file.path.as_str(), owner.span)
        .and_then(|entity| entity.type_descriptor.as_deref())
    {
        // TypeScript may preserve a user alias's name rather than render its
        // nullable expansion, so absence of the word `null` is not proof.
        // The Solid export itself (including a flow-narrowed Owner | null)
        // renders as Owner; everything else stays conditional.
        if descriptor.text.trim() == "Owner" {
            return Some(solid_dialect::CallbackOwner::Creates);
        }
    }

    Some(solid_dialect::CallbackOwner::Conditional)
}

/// Resolve the primitive whose returned function is the callee of `call`.
///
/// Canonical TypeScript symbols prove identity, AST binding facts prove the
/// initializer/alias chain, and the dialect's resolved primitive index proves
/// that the initializer is the actual Solid export rather than a same-spelled
/// local function.
pub(crate) fn returned_primitive_invocation(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Primitive, Option<usize>)> {
    let mut reference_path = file.path.as_str();
    let mut reference_span = call.callee;
    let mut seen = HashSet::new();
    loop {
        // Callee result values do not always receive an entity fact of their
        // own (tuple slots are the motivating case). Resolve them from the
        // canonical reference set of each demanded binding declaration, then
        // follow local or cross-file aliases one binding at a time.
        let (binding_file, binding, symbol) =
            lookup.binding_at_reference(reference_path, reference_span)?;
        if !seen.insert(symbol.clone()) {
            return None;
        }
        if let Some(initializer) = binding.call_initializer {
            let call_index = lookup.call_index(binding_file, initializer)?;
            let result_slot = match binding.shape {
                solid_facts::ast::BindingShape::Array => {
                    Some(binding.array_slots.iter().position(|slot| {
                        slot.as_ref().and_then(|name| {
                            lookup.entities().at(binding_file.path.as_str(), name.span)
                        }) == Some(&symbol)
                    })?)
                }
                _ => None,
            };
            return known_primitive(&lookup.primitives(binding_file).calls[call_index])
                .map(|primitive| (primitive, result_slot));
        }
        let initializer = binding.initializer_identifier.as_ref()?;
        reference_path = binding_file.path.as_str();
        reference_span = initializer.span;
    }
}

/// The execution contract of one callback argument accepted by a proven
/// returned function (including a specific destructured tuple slot).
pub(crate) fn returned_callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    // Solid 2.0 answers `None` for every returned-callback contract, so the
    // binding-chain resolution below cannot produce an answer there. Skip it
    // rather than walk it once per call and discard the result.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
    lookup.dialect.returned_callback_execution_at(
        primitive,
        result_slot,
        argument,
        call.arguments.len(),
    )
}

pub(crate) fn returned_callback_result_is_invoked(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !returned_callback_invocation_sites(file, factory_call, lookup).is_empty()
}

/// Proven project invocations of the function returned by `factory_call`.
///
/// TypeScript symbols establish that calls, JSX tags, and `.preload()` uses
/// refer to the exact factory result. AST facts establish how the value is
/// consumed. For values passed to another known callback-taking primitive,
/// the invocation inherits that callback's owner contract; direct calls and
/// JSX uses inherit their containing function's owner.
pub(crate) fn returned_callback_invocation_sites(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ReturnedCallbackInvocationSite> {
    let direct_factory_value = |argument: Span| {
        file.ast
            .calls_within(argument)
            .max_by_key(|nested| nested.span.end - nested.span.start)
            .is_some_and(|nested| nested.span == factory_call.span)
    };
    let mut sites = Vec::new();
    for outer in file.ast.calls.iter().filter(|outer| {
        outer.span != factory_call.span
            && outer.callee.contains(factory_call.span)
            && file
                .ast
                .calls_within(outer.callee)
                .filter(|nested| nested.span != outer.span)
                .max_by_key(|nested| nested.span.end - nested.span.start)
                .is_some_and(|nested| nested.span == factory_call.span)
            // `factory(...).member()` invokes the member, not the returned
            // function, so it proves nothing about the factory's callbacks.
            // `preload` is the one member the runtime routes to the loader,
            // matching the binding-based preload proof below.
            && lookup
                .member_property_at(file, outer.callee)
                .is_none_or(|property| file.source_text(property) == Some("preload"))
    }) {
        sites.push(ReturnedCallbackInvocationSite {
            path: file.path.to_string(),
            span: outer.span,
            inherited_execution: None,
            inherited_owner: OwnerEdgeKind::Preserve,
        });
    }

    for (outer, index) in file.ast.arguments_containing(factory_call.span) {
        let primitive = lookup.primitive_at_call(file, outer.span);
        if direct_factory_value(outer.arguments[index].span)
            && primitive.is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_execution_at(primitive, index, outer.arguments.len())
                    .is_some()
            })
        {
            let inherited_owner = primitive
                .and_then(|primitive| callback_owner_at_call(file, outer, primitive, index, lookup))
                .map_or(OwnerEdgeKind::Preserve, callback_owner_edge_kind);
            sites.push(ReturnedCallbackInvocationSite {
                path: file.path.to_string(),
                span: outer.span,
                inherited_execution: primitive.and_then(|primitive| {
                    lookup
                        .dialect
                        .callback_execution_at(primitive, index, outer.arguments.len())
                }),
                inherited_owner,
            });
        }
    }

    let mut factory_bindings = file
        .ast
        .bindings
        .iter()
        .filter(|binding| binding.call_initializer == Some(factory_call.span))
        .flat_map(|binding| &binding.names)
        .filter_map(|name| lookup.entities().at(file.path.as_str(), name.span).cloned())
        .collect::<HashSet<_>>();
    if factory_bindings.is_empty() {
        sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
        sites.dedup();
        return sites;
    }

    // Follow identifier aliases in every project file. The compiler's symbol
    // reference table bridges imports/re-exports; local AST bindings bridge a
    // fresh `const Alias = Imported` symbol introduced after that boundary.
    loop {
        let references = returned_binding_references(lookup, &factory_bindings);
        let aliases = lookup
            .files()
            .iter()
            .flat_map(|candidate| {
                candidate
                    .ast
                    .bindings
                    .iter()
                    .filter_map(|binding| {
                        let initializer = binding.initializer_identifier.as_ref()?;
                        returned_binding_reference(
                            candidate,
                            initializer.span,
                            lookup,
                            &factory_bindings,
                            &references,
                        )
                        .then_some(binding)
                    })
                    .flat_map(move |binding| {
                        binding.names.iter().filter_map(|name| {
                            lookup
                                .entities()
                                .at(candidate.path.as_str(), name.span)
                                .cloned()
                        })
                    })
            })
            .collect::<Vec<_>>();
        let before = factory_bindings.len();
        factory_bindings.extend(aliases);
        if factory_bindings.len() == before {
            break;
        }
    }

    let references = returned_binding_references(lookup, &factory_bindings);
    for use_file in lookup.files() {
        for outer in &use_file.ast.calls {
            if returned_binding_reference(
                use_file,
                outer.callee,
                lookup,
                &factory_bindings,
                &references,
            ) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: outer.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
            let primitive = lookup.primitive_at_call(use_file, outer.span);
            for (index, argument) in outer.arguments.iter().enumerate() {
                if returned_binding_reference(
                    use_file,
                    argument.span,
                    lookup,
                    &factory_bindings,
                    &references,
                ) && primitive.is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_execution_at(primitive, index, outer.arguments.len())
                        .is_some()
                }) {
                    let inherited_owner = primitive
                        .and_then(|primitive| {
                            callback_owner_at_call(use_file, outer, primitive, index, lookup)
                        })
                        .map_or(OwnerEdgeKind::Preserve, callback_owner_edge_kind);
                    sites.push(ReturnedCallbackInvocationSite {
                        path: use_file.path.to_string(),
                        span: outer.span,
                        inherited_execution: primitive.and_then(|primitive| {
                            lookup.dialect.callback_execution_at(
                                primitive,
                                index,
                                outer.arguments.len(),
                            )
                        }),
                        inherited_owner,
                    });
                }
            }

            // `lazyResult.preload()` resolves the property as a distinct
            // method; prove the receiver from the member-expression AST.
            if use_file.ast.members.iter().any(|member| {
                outer.callee.contains(member.span)
                    && use_file.source_text(member.property) == Some("preload")
                    && returned_binding_reference(
                        use_file,
                        member.object,
                        lookup,
                        &factory_bindings,
                        &references,
                    )
            }) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: outer.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
        }

        for element in &use_file.ast.jsx_elements {
            if returned_binding_reference(
                use_file,
                element.name.span,
                lookup,
                &factory_bindings,
                &references,
            ) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: element.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
        }
    }

    sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
    sites.dedup();
    sites
}

pub(crate) fn returned_binding_references(
    lookup: &SemanticLookup<'_>,
    bindings: &HashSet<SymbolId>,
) -> HashSet<(String, u64, u64)> {
    bindings
        .iter()
        .flat_map(|symbol| lookup.symbol_references(symbol.as_str()))
        .map(|reference| {
            (
                reference.path.to_string(),
                reference.start_byte,
                reference.end_byte,
            )
        })
        .collect()
}

pub(crate) fn returned_binding_reference(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    bindings: &HashSet<SymbolId>,
    references: &HashSet<(String, u64, u64)>,
) -> bool {
    lookup
        .entities()
        .at(file.path.as_str(), span)
        .is_some_and(|symbol| bindings.contains(symbol))
        || references.contains(&(
            file.path.to_string(),
            u64::from(span.start),
            u64::from(span.end),
        ))
}

pub(crate) fn function_binding_name<'a>(
    file: &'a solid_facts::FileFacts,
    function: &'a solid_facts::ast::FunctionFact,
) -> Option<&'a solid_facts::ast::NamedSpan> {
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

pub(crate) fn go_binding_pattern_accepts_call(
    source: &str,
    binding: &solid_facts::ast::BindingFact,
    call: &solid_facts::ast::CallFact,
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

pub(crate) fn go_returned_arrow_pattern_accepts(source: &str, span: Span) -> bool {
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

pub(crate) fn inside_effect_apply(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    file.ast.arguments_containing(span).any(|(call, index)| {
        primitive_name(
            file.path.as_str(),
            call.callee,
            call.static_callee(&file.source),
            entities,
            symbol_names,
            dialect,
        )
        .as_ref()
        .and_then(PrimitiveName::primitive)
        .is_some_and(|primitive| {
            matches!(
                primitive,
                Primitive::CreateEffect | Primitive::CreateRenderEffect
            ) && dialect.callback_execution_at(primitive, index, call.arguments.len())
                == Some(solid_dialect::Execution::Deferred)
        })
    })
}

/// The typed descriptor at a callee, kept only when it names a Solid accessor.
pub(crate) fn typed_accessor_descriptor_at<'a>(
    lookup: &SemanticLookup<'a>,
    path: &str,
    callee: Span,
) -> Option<&'a typefacts::TypeDescriptor> {
    lookup
        .smallest_contained_descriptor(path, callee)
        .filter(|descriptor| go_solid_accessor_descriptor(descriptor, lookup.dialect))
}

/// Whether a typed descriptor's declaring module is one this dialect owns.
///
/// The module is the dialect's answer, not a literal: 1.x spreads its exports
/// across `solid-js/store`, `solid-js/web` and `solid-js/universal`, and 2.0
/// declares its DOM surface in `@solidjs/web`. Comparing against the package
/// root alone silently dropped every accessor whose type came from a subpath.
pub(crate) fn go_solid_accessor_descriptor(
    descriptor: &typefacts::TypeDescriptor,
    dialect: &dyn Dialect,
) -> bool {
    dialect.owns_module(descriptor.origin_module.as_ref())
}

pub(crate) fn source_function_exported(
    indexes: &ProjectIndexes<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
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

pub(crate) fn enclosing_render_function(file: &solid_facts::FileFacts, span: Span) -> bool {
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

pub(crate) fn function_is_solid_callback(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
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
            && jsx_primitive_name(file, element, entities, symbol_names, lookup.dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| {
                    lookup.dialect.renders_children_through_callback(primitive)
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
        known_primitive(&primitives.calls[call_index]).is_some_and(|primitive| {
            call.arguments.iter().enumerate().any(|(index, argument)| {
                lookup
                    .dialect
                    .callback_tracks_reads_at(primitive, index, call.arguments.len())
                    && argument_references_callback_symbol(
                        file,
                        argument,
                        symbol,
                        entities,
                        symbol_names,
                    )
            })
        })
    }) || file
        .ast
        .jsx_elements
        .iter()
        .enumerate()
        .any(|(element_index, element)| {
            known_primitive(&primitives.jsx[element_index]).is_some_and(|primitive| {
                lookup.dialect.renders_children_through_callback(primitive)
            }) && file.ast.identifiers_within(element.span).any(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && !file.ast.jsx_containing(identifier.span).any(|nested| {
                        nested.span != element.span && element.span.contains(nested.span)
                    })
                    && (entities.get(&location(file.path.shared(), identifier.span))
                        == Some(symbol)
                        || binding_name == file.source_text(identifier.span))
            })
        })
}

pub(crate) fn counts_as_strict_read_root(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> bool {
    matches!(
        execution,
        ExecutionRole::EffectApply | ExecutionRole::UntrackedCallback
    ) || enclosing_render_function(file, span)
}

pub(crate) fn enclosing_function_label(file: &solid_facts::FileFacts, span: Span) -> String {
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

pub(crate) fn analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
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
            dialect,
        )
    {
        // Which phase of a primitive an argument is, asked of the dialect
        // rather than matched here. The pair this had hardcoded is 2.0's:
        // `createEffect(compute, apply)`. 1.x's second argument is a seed
        // value threaded to the next run as `prev`, so a read in it would be
        // described as living in an "apply callback" that 1.x does not have.
        let phase = primitive.primitive().and_then(|resolved| {
            dialect
                .callback_execution_at(resolved, argument, call.arguments.len())
                .and_then(|execution| match execution {
                    // "compute" only where reads actually subscribe: an
                    // `onSettled` callback is contract-tracked (the graph
                    // schedules it) but imperative to its reads, and calling
                    // it a compute would describe the wrong phase.
                    solid_dialect::Execution::Tracked
                        if dialect.callback_tracks_reads_at(
                            resolved,
                            argument,
                            call.arguments.len(),
                        ) =>
                    {
                        Some("compute")
                    }
                    // Only an effect's deferred argument is an apply phase; a
                    // deferred executor's callback keeps its enclosing label.
                    solid_dialect::Execution::Deferred
                        if matches!(
                            resolved,
                            Primitive::CreateEffect | Primitive::CreateRenderEffect
                        ) =>
                    {
                        Some("apply callback")
                    }
                    _ => None,
                })
        });
        if let Some(phase) = phase {
            return format!("{primitive} {phase}");
        }
    }
    enclosing
}
