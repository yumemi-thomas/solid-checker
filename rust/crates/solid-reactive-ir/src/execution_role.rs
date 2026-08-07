//! Execution-role classification.
//!
//! Given a source span, classify the reactive execution context it runs in
//! (tracked JSX, deferred callback, effect apply, event handler, …). This is
//! the compiler-fact classifier plus the semantic (AST-driven) classifier and
//! the two role-keyed read helpers that consume its result.

use std::collections::{HashMap, HashSet};

use solid_dialect::{Dialect, Execution, Primitive};
use solid_facts::core::Span;

use super::{
    EntitySymbols, ExecutionRole, PrimitiveName, SemanticLookup, SymbolId,
    callback_execution_at_call, containing_ast_function, enclosing_function_label,
    function_binding_name, jsx_primitive_name, known_primitive, location, primitive_name,
    returned_callback_invocation_sites, returned_primitive_invocation,
};

/// The effect primitives: the ones 2.0 spells `(compute, apply)`.
fn is_effect(primitive: Primitive) -> bool {
    matches!(
        primitive,
        Primitive::CreateEffect | Primitive::CreateRenderEffect
    )
}

/// The argument an effect runs *after* its compute, if this dialect has one.
///
/// 2.0's `createEffect(compute, apply)` has it at index 1. 1.x's second
/// argument is a seed value threaded to the next run as `prev`, so 1.x has
/// none — and this used to be the literal `1` for both. A read in a 1.x seed
/// would be classified `EffectApply`, reporting it as running in a phase that
/// version does not have.
fn effect_apply_argument(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument_count: usize,
) -> Option<usize> {
    if !is_effect(primitive) {
        return None;
    }
    (0..argument_count).find(|index| {
        dialect.callback_execution_at(primitive, *index, argument_count)
            == Some(Execution::Deferred)
    })
}

fn callback_runs_outside_tracking(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument: usize,
    argument_count: usize,
) -> bool {
    match dialect.callback_execution_at(primitive, argument, argument_count) {
        None => false,
        // A tracked callback creates its own observer unless the primitive's
        // exact runtime contract explicitly overrides that classification.
        Some(Execution::Tracked) => {
            !dialect.callback_tracks_reads_at(primitive, argument, argument_count)
        }
        // Deferred callbacks execute after/outside the current tracking pass.
        Some(Execution::Deferred) => true,
        // Inline means the callback inherits the caller's Listener. Only
        // primitives such as untrack/createRoot/runWithOwner that explicitly
        // clear Listener belong to the outside-tracking set.
        Some(Execution::Inline) => dialect.runs_callback_deferred(primitive),
    }
}

/// The argument positions holding a callback that runs outside the
/// surrounding tracking scope — an effect's apply argument, or a deferred
/// executor's whole callback.
///
/// A strict subset of [`Dialect::callback_positions`], which also answers for
/// `createMemo`, `createSignal` and the rest of the tracked index-0 set. The
/// two questions are independent: position says *where* a callback sits,
/// [`Dialect::runs_callback_deferred`] says *how it executes*.
fn deferred_callback_positions(
    dialect: &dyn Dialect,
    primitive: Primitive,
    argument_count: usize,
) -> Vec<usize> {
    (0..argument_count)
        .filter(|index| callback_runs_outside_tracking(dialect, primitive, *index, argument_count))
        .filter(|index| !dialect.reports_untracked_reads_at(primitive, *index, argument_count))
        .collect()
}

pub(super) fn execution_role(
    facts: &solid_facts::compiler::ExecutionMap,
    span: Span,
    allowed: &[Span],
) -> ExecutionRole {
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    if facts
        .tracked_regions
        .iter()
        .any(|region| region.span.contains(span))
    {
        return ExecutionRole::TrackedJsx;
    }
    for callback in &facts.callback_roles {
        if callback.span.contains(span) {
            return match callback.role {
                solid_facts::compiler::CallbackRoleKind::EventHandler => {
                    ExecutionRole::EventCallback
                }
                solid_facts::compiler::CallbackRoleKind::Deferred => {
                    ExecutionRole::DeferredCallback
                }
                solid_facts::compiler::CallbackRoleKind::DirectiveApply => {
                    ExecutionRole::DirectiveApply
                }
                solid_facts::compiler::CallbackRoleKind::Render => {
                    ExecutionRole::UntrackedRendering
                }
            };
        }
    }
    ExecutionRole::UntrackedRendering
}

pub(super) fn semantic_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    semantic_execution_role_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        &mut HashSet::new(),
    )
}

/// `classifying` is the stack of spans whose role is currently being derived
/// from their own invocation sites. A returned adapter invoked inside its own
/// factory callback — `const a = on(() => a(), fn)`, or two adapters invoked
/// in each other's callbacks — makes that derivation cyclic; a site already on
/// the stack supplies no independent execution context and is skipped instead
/// of re-entered.
fn semantic_execution_role_within(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> ExecutionRole {
    if let Some(role) = context_provider_value_role(file, span, lookup) {
        return role;
    }
    if assigned_member_function_contains(file, span, entities) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = named_callback_execution_role(file, span, entities, symbol_names, lookup) {
        return role;
    }
    if let Some(role) = returned_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    if let Some(role) = returned_factory_callback_execution_role(file, span, lookup, classifying) {
        return role;
    }
    let dialect = lookup.dialect;
    if file.ast.arguments_containing(span).any(|(call, index)| {
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
        .and_then(|primitive| effect_apply_argument(dialect, primitive, call.arguments.len()))
            == Some(index)
            && direct_callback_contains(file, call.arguments[index].span, span)
    }) {
        return ExecutionRole::EffectApply;
    }
    if file.ast.arguments_containing(span).any(|(call, index)| {
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
            callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                && dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
                && direct_callback_contains(file, call.arguments[index].span, span)
        })
    }) {
        return ExecutionRole::UntrackedCallback;
    }
    if allowed.iter().any(|region| region.contains(span)) {
        return ExecutionRole::DeferredCallback;
    }
    if let Some(role) = control_flow_execution_role(file, span, entities, symbol_names, dialect) {
        return role;
    }
    if file
        .compiler
        .tracked_regions
        .iter()
        .any(|region| region.span.contains(span))
    {
        return ExecutionRole::TrackedJsx;
    }
    if file.ast.arguments_containing(span).any(|(call, index)| {
        matches!(
            call.arguments[index].value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && primitive_name(
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
            callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                && dialect.callback_tracks_reads_at(primitive, index, call.arguments.len())
        })
    }) {
        return ExecutionRole::TrackedJsx;
    }
    execution_role(&file.compiler, span, allowed)
}

/// Compose an inline callback of a higher-order factory with the proven use of
/// the returned function. For example, `on` reads its dependency inline when
/// the returned adapter runs: `createEffect(on(...))` therefore tracks that
/// read, while a direct top-level adapter call does not.
fn returned_factory_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(factory_call, index)| {
            if !direct_callback_contains(file, factory_call.arguments[index].span, span) {
                return None;
            }
            let call_index = file
                .ast
                .calls
                .iter()
                .position(|candidate| candidate.span == factory_call.span)?;
            let primitive = known_primitive(&lookup.primitives(file).calls[call_index])?;
            if !lookup
                .dialect
                .callback_requires_return_invocation(primitive, index)
                || lookup.dialect.callback_execution_at(
                    primitive,
                    index,
                    factory_call.arguments.len(),
                ) != Some(Execution::Inline)
            {
                return None;
            }

            let mut roles = Vec::new();
            for site in returned_callback_invocation_sites(file, factory_call, lookup) {
                let role = match site.inherited_execution {
                    Some(Execution::Tracked) => Some(ExecutionRole::TrackedJsx),
                    Some(Execution::Deferred) => Some(ExecutionRole::DeferredCallback),
                    Some(Execution::Inline) | None => {
                        let key = (site.path.clone(), site.span);
                        if classifying.contains(&key) {
                            // A cyclic invocation site — the adapter calling
                            // itself through its own callback — has no context
                            // of its own to contribute.
                            None
                        } else {
                            lookup
                                .files()
                                .iter()
                                .find(|candidate| candidate.path.as_str() == site.path)
                                .map(|use_file| {
                                    classifying.insert(key.clone());
                                    let role = semantic_execution_role_within(
                                        use_file,
                                        site.span,
                                        &[],
                                        lookup.entities(),
                                        lookup.symbol_names(),
                                        lookup,
                                        classifying,
                                    );
                                    classifying.remove(&key);
                                    role
                                })
                        }
                    }
                };
                roles.extend(role);
            }
            roles.sort_by_key(|role| *role as u8);
            roles.dedup();
            match roles.as_slice() {
                [] => None,
                [role] => Some(*role),
                // The same returned adapter is used in incompatible execution
                // contexts. A single diagnostic site cannot truthfully claim one
                // dominates, so preserve uncertainty instead of manufacturing a
                // false positive in either direction.
                _ => Some(ExecutionRole::DeferredCallback),
            }
        })
}

fn returned_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    classifying: &mut HashSet<(String, Span)>,
) -> Option<ExecutionRole> {
    file.ast
        .arguments_containing(span)
        .find_map(|(call, index)| {
            if !direct_callback_contains(file, call.arguments[index].span, span) {
                return None;
            }
            let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
            match lookup.dialect.returned_callback_execution_at(
                primitive,
                result_slot,
                index,
                call.arguments.len(),
            )? {
                Execution::Tracked => Some(ExecutionRole::TrackedJsx),
                Execution::Deferred => Some(ExecutionRole::DeferredCallback),
                // The returned function restores/inherits its caller's execution
                // context. Classify the proven invocation span, outside the
                // callback body itself, so a top-level transition remains
                // untracked while one started from an effect remains tracked.
                Execution::Inline => {
                    let key = (file.path.to_string(), call.span);
                    if !classifying.insert(key.clone()) {
                        return None;
                    }
                    let role = semantic_execution_role_within(
                        file,
                        call.span,
                        &[],
                        lookup.entities(),
                        lookup.symbol_names(),
                        lookup,
                        classifying,
                    );
                    classifying.remove(&key);
                    Some(role)
                }
            }
        })
}

/// Whether `span` is evaluated as the `value` getter of a resolved Solid 1.x
/// `createContext().Provider`.
///
/// Both halves are semantic proof: the JSX member object resolves to the
/// binding initialized by the exact Solid `createContext` primitive, and the
/// final member is `Provider`. An arbitrary component named `SomeProvider` or
/// object with a same-spelled property satisfies neither condition.
fn context_provider_value_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    if lookup.dialect.version() != solid_dialect::Version::V1 {
        return None;
    }
    let element = file
        .ast
        .jsx_elements
        .iter()
        .filter(|element| {
            element.attributes.iter().any(|attribute| {
                file.source_text(attribute.local_name) == Some("value")
                    && attribute
                        .expression
                        .is_some_and(|expression| expression.contains(span))
            })
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    let (Some(object), Some(property)) = (element.member_object, element.member_property) else {
        return None;
    };
    if file.source_text(property) != Some("Provider") {
        return None;
    }
    if !lookup.is_context_reference(file.path.as_str(), object) {
        return None;
    }
    // Solid 1.x's createProvider implementation eagerly reads props.value
    // inside untrack. A function expression is the one exception: reading
    // the getter only creates/stores the function; its body is not run.
    let stores_function = file.ast.functions_body_containing(span).any(|function| {
        element.attributes.iter().any(|attribute| {
            attribute
                .expression
                .is_some_and(|expression| expression.contains(function.span))
        })
    });
    Some(if stores_function {
        ExecutionRole::DeferredCallback
    } else {
        ExecutionRole::UntrackedRendering
    })
}

pub(super) fn assigned_member_function_contains(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        file.ast.assignments.iter().any(|assignment| {
            if assignment.value != solid_facts::ast::AssignmentValueKind::Function
                || !assignment.value_span.contains(function.span)
            {
                return false;
            }
            let Some(member) = file
                .ast
                .members
                .iter()
                .find(|member| member.span == assignment.target)
            else {
                return false;
            };
            let Some(object_symbol) = entities.at(file.path.as_str(), member.object) else {
                return false;
            };
            let Some(owner) = containing_ast_function(&file.ast, assignment.target) else {
                return false;
            };
            let caller_owned = owner.parameters.iter().any(|parameter| {
                parameter
                    .names
                    .iter()
                    .any(|name| entities.at(file.path.as_str(), name.span) == Some(object_symbol))
            });
            let returned = file.ast.returns.iter().any(|returned| {
                returned.value == solid_facts::ast::ReturnValueKind::Identifier
                    && containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|candidate| candidate.span == owner.span)
                    && entities.at(file.path.as_str(), returned.span) == Some(object_symbol)
            });
            caller_owned || returned
        })
    })
}

pub(super) fn control_flow_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> Option<ExecutionRole> {
    let element = file
        .ast
        .jsx_containing(span)
        .filter(|element| {
            jsx_primitive_name(file, element, entities, symbol_names, dialect)
                .as_ref()
                .and_then(PrimitiveName::primitive)
                .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        })
        .min_by_key(|element| element.span.end - element.span.start)?;
    let callback = file
        .ast
        .functions_body_containing(span)
        .filter(|function| element.span.contains(function.span))
        .max_by_key(|function| function.body.end - function.body.start)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|nested| callback.body.contains(nested.span))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

pub(super) fn named_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let dialect = lookup.dialect;
    let primitives = lookup.primitives(file);
    let callback = file.ast.functions_body_containing(span).find(|function| {
        let Some(symbol) = function_symbol(file, function, entities) else {
            return false;
        };
        let binding_name = function
            .name
            .as_ref()
            .or_else(|| function_binding_name(file, function))
            .map(|name| file.source_text(name.span).unwrap_or_default());
        file.ast.calls.iter().enumerate().any(|(call_index, call)| {
            let Some(primitive) = known_primitive(&primitives.calls[call_index]) else {
                return false;
            };
            (0..call.arguments.len()).any(|argument_index| {
                if callback_execution_at_call(file, call, primitive, argument_index, lookup)
                    .is_none()
                {
                    return false;
                }
                // An effect's tracked compute is not classified here: none of
                // the arms below answers for it (the tracked arm excludes
                // effects), so admitting it would fall through to the
                // rendering tail and misreport `createEffect(namedCompute)`
                // as untracked. Answering None instead defers those reads to
                // compiler facts. Only the apply argument names a role this
                // function can truthfully return for an effect.
                if is_effect(primitive)
                    && effect_apply_argument(dialect, primitive, call.arguments.len())
                        != Some(argument_index)
                {
                    return false;
                }
                call.arguments.get(argument_index).is_some_and(|argument| {
                    argument_references_callback_symbol(
                        file,
                        argument,
                        symbol,
                        entities,
                        symbol_names,
                    ) || argument
                        .identifier_properties
                        .iter()
                        .any(|property| binding_name == file.source_text(property.span))
                })
            })
        }) || file
            .ast
            .jsx_elements
            .iter()
            .enumerate()
            .any(|(element_index, element)| {
                known_primitive(&primitives.jsx[element_index])
                    .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
                    && file.ast.identifiers_within(element.span).any(|identifier| {
                        identifier.role == solid_facts::ast::IdentifierRole::Reference
                            && !file.ast.jsx_containing(identifier.span).any(|nested| {
                                nested.span != element.span && element.span.contains(nested.span)
                            })
                            && (entities
                                .get(&location(file.path.shared(), identifier.span))
                                .is_some_and(|candidate| candidate.as_str() == symbol.as_str())
                                || binding_name == file.source_text(identifier.span))
                    })
            })
    })?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        known_primitive(&primitives.calls[call_index]).is_some_and(|primitive| {
            call.arguments.iter().enumerate().any(|(index, argument)| {
                callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    && dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
                    && function_symbol(file, callback, entities).is_some_and(|symbol| {
                        argument_references_callback_symbol(
                            file,
                            argument,
                            symbol,
                            entities,
                            symbol_names,
                        )
                    })
            })
        })
    }) {
        return Some(ExecutionRole::UntrackedCallback);
    }
    if file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        known_primitive(&primitives.calls[call_index])
            .and_then(|primitive| effect_apply_argument(dialect, primitive, call.arguments.len()))
            .and_then(|index| call.arguments.get(index))
            .is_some_and(|argument| {
                function_symbol(file, callback, entities).is_some_and(|symbol| {
                    argument_references_callback_symbol(
                        file,
                        argument,
                        symbol,
                        entities,
                        symbol_names,
                    )
                }) || function_binding_name(file, callback)
                    .or(callback.name.as_ref())
                    .is_some_and(|name| {
                        argument.identifier_properties.iter().any(|property| {
                            file.source_text(property.span) == file.source_text(name.span)
                        })
                    })
            })
    }) {
        return Some(ExecutionRole::EffectApply);
    }
    if file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        known_primitive(&primitives.calls[call_index]).is_some_and(|primitive| {
            !is_effect(primitive)
                && call.arguments.iter().enumerate().any(|(index, argument)| {
                    callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                        && dialect.callback_tracks_reads_at(primitive, index, call.arguments.len())
                        && function_symbol(file, callback, entities).is_some_and(|symbol| {
                            entities.get(&location(file.path.shared(), argument.span))
                                == Some(symbol)
                        })
                })
        })
    }) {
        return Some(ExecutionRole::TrackedJsx);
    }
    if file.ast.calls.iter().enumerate().any(|(call_index, call)| {
        known_primitive(&primitives.calls[call_index]).is_some_and(|primitive| {
            call.arguments.iter().enumerate().any(|(index, argument)| {
                callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    && callback_runs_outside_tracking(
                        dialect,
                        primitive,
                        index,
                        call.arguments.len(),
                    )
                    && function_symbol(file, callback, entities).is_some_and(|symbol| {
                        entities.get(&location(file.path.shared(), argument.span)) == Some(symbol)
                    })
            })
        })
    }) {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|element| callback.body.contains(element.span))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

pub(super) fn function_symbol<'a>(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &'a EntitySymbols,
) -> Option<&'a SymbolId> {
    let name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))?;
    entities.get(&location(file.path.shared(), name.span))
}

pub(super) fn argument_references_callback_symbol(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    symbol: &str,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    entities
        .get(&location(file.path.shared(), argument.span))
        .map(SymbolId::as_str)
        == Some(symbol)
        || argument.identifier_properties.iter().any(|property| {
            entities
                .get(&location(file.path.shared(), property.span))
                .map(SymbolId::as_str)
                == Some(symbol)
                || symbol_names.get(symbol).map(SymbolId::as_str) == file.source_text(property.span)
        })
}

fn direct_callback_contains(file: &solid_facts::FileFacts, argument: Span, span: Span) -> bool {
    if !argument.contains(span) {
        return false;
    }
    let callback = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start);
    let owner = containing_ast_function(&file.ast, span);
    match (callback, owner) {
        (Some(callback), Some(owner)) => callback.span == owner.span,
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn read_analysis_context(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> String {
    if execution == ExecutionRole::EffectApply {
        "createEffect apply callback".into()
    } else {
        let context = enclosing_function_label(file, span);
        format_read_context(&context, file.ast.any_conditional_test_containing(span))
    }
}

fn format_read_context(context: &str, in_conditional_test: bool) -> String {
    if in_conditional_test {
        if context.is_empty() {
            "while evaluating a condition".into()
        } else {
            format!("{context} while evaluating a condition")
        }
    } else {
        context.into()
    }
}

pub(super) fn async_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    execution: ExecutionRole,
) -> ExecutionRole {
    if execution == ExecutionRole::DeferredCallback && file.ast.any_jsx_containing(span) {
        ExecutionRole::TrackedJsx
    } else {
        execution
    }
}

pub(super) fn allowed_callback_spans(
    file: &solid_facts::FileFacts,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    let dialect = lookup.dialect;
    let primitives = lookup.primitives(file);
    let mut spans = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let indices =
            known_primitive(&primitives.calls[call_index]).map_or_else(Vec::new, |primitive| {
                deferred_callback_positions(dialect, primitive, call.arguments.len())
                    .into_iter()
                    .filter(|index| {
                        callback_execution_at_call(file, call, primitive, *index, lookup).is_some()
                    })
                    .collect()
            });
        for index in indices {
            if let Some(argument) = call.arguments.get(index) {
                spans.push(argument.span);
            }
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::format_read_context;

    #[test]
    fn conditional_context_does_not_repeat_the_function_name_as_a_return_kind() {
        assert_eq!(
            format_read_context("ConditionalReturn", true),
            "ConditionalReturn while evaluating a condition"
        );
        assert_eq!(
            format_read_context("", true),
            "while evaluating a condition"
        );
    }
}
