//! Return escapes: whether a function value escapes through a return, and which functions a call returns.

use super::*;

/// Whether `nested` is returned as a callable value from `owner`.
///
/// This covers direct returned arrows/identifiers and identity-preserving
/// standard helpers such as `Object.assign(wrapper, { clear })`. A nested
/// function that is merely called while computing the return value is not an
/// escape: its callback execution stays inline with the exported call.
pub(super) fn function_escapes_through_return(
    file: &solid_facts::FileFacts,
    nested: &SummaryNode,
    owner: &SummaryNode,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if nested.span == owner.span {
        return false;
    }
    function_value_escapes_through_return(
        file,
        nested.span,
        nested.symbol.as_ref(),
        nested.name.as_deref(),
        owner,
        entities,
        lookup,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn function_value_escapes_through_return(
    file: &solid_facts::FileFacts,
    nested_span: Span,
    nested_symbol: Option<&SymbolId>,
    nested_name: Option<&str>,
    owner: &SummaryNode,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(owner_function) = file.ast.function_at(owner.span) else {
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
                return returned.argument.is_some_and(|argument| {
                    returned_function_spans(file, argument, entities).contains(&nested_span)
                });
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

pub(super) fn local_function_returns_argument(
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

pub(super) fn returned_function_targets(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    functions: &FunctionLookup,
    entities: &EntitySymbols,
) -> HashMap<SymbolId, usize> {
    let mut ast_functions = HashMap::default();
    for function in &file.ast.functions {
        ast_functions.entry(function.span).or_insert(function);
    }
    let mut returns_by_owner = HashMap::<Span, Vec<&solid_facts::ast::ReturnFact>>::default();
    for returned in &file.ast.returns {
        if let Some(owner) = containing_ast_function(&file.ast, returned.span) {
            returns_by_owner
                .entry(owner.span)
                .or_default()
                .push(returned);
        }
    }
    let mut seen = HashSet::default();
    let mut targets = HashMap::default();
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
pub(super) fn returned_function_spans(
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
        if let Some(conditional) = file.ast.conditional_at(value) {
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

    let mut visited = HashSet::default();
    let mut result = Vec::new();
    collect(file, value, entities, &mut visited, &mut result);
    result.sort_unstable();
    result.dedup();
    result
}

pub(super) fn proven_array_method<'a>(
    file: &'a solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
) -> Option<&'a str> {
    let member = file.ast.member_at(call.callee)?;
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

pub(super) fn stored_constructor_argument_escapes(
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

pub(super) fn same_runtime_value(
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
