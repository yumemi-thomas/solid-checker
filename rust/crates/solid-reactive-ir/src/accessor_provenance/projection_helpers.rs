//! Mutation-target projection and the small binding-shape helpers (simple bindings, slots, returned elements, closed properties).

use super::*;

/// Whether `span` is exactly one identifier, as opposed to an expression an
/// identifier merely begins.
/// ADR 0300: a wildcard target whose complete local identity comes from
/// other fresh allocation sites cannot replace this literal's descriptor.
/// Unknown inputs never count as distinct. Cyclic local assignments are
/// checked as a graph: every non-cyclic input must be a distinct allocation
/// (or an uninitialized local's undefined); no source edge is skipped.
pub(super) fn mutation_target_is_distinct(
    file: &FileFacts,
    span: Span,
    object: &Location,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    visited: &mut HashSet<(String, Span)>,
) -> bool {
    mutation_target_projected(file, span, &[], object, lookup, depth, visited)
}

/// A complete identity join. Projections are copied by destructuring, or
/// read through a holder whose exact uses cannot replace the projected slot.
#[allow(clippy::too_many_arguments)]
pub(super) fn mutation_target_projected(
    file: &FileFacts,
    span: Span,
    projection: &[String],
    object: &Location,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    visited: &mut HashSet<(String, Span)>,
) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    // Evaluating a function expression allocates a new callable object. A
    // write to its own fields cannot rewrite the package's getter literal.
    if projection.is_empty()
        && file.ast.functions.iter().any(|function| {
            function.span == span
                && matches!(
                    function.kind,
                    ast::FunctionKind::Expression | ast::FunctionKind::Arrow
                )
        })
    {
        return location(file.path.shared(), span) != *object;
    }
    if file
        .ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == span)
    {
        if projection.is_empty() {
            return location(file.path.shared(), span) != *object;
        }
        return file
            .ast
            .object_get_shapes
            .iter()
            .any(|s| s.span == span && s.closed)
            && Some(direct_data_properties(file, span)).is_some_and(|properties| {
                properties
                    .iter()
                    .find(|p| {
                        file.source_text(p.key).map(|k| k.trim_matches(['\'', '"']))
                            == Some(projection[0].as_str())
                    })
                    .is_some_and(|p| {
                        mutation_target_projected(
                            file,
                            p.shorthand_binding.unwrap_or(p.value),
                            &projection[1..],
                            object,
                            lookup,
                            depth - 1,
                            visited,
                        )
                    })
            });
    }
    if file.ast.array_literals.contains(&span) {
        if projection.is_empty() {
            return location(file.path.shared(), span) != *object;
        }
        return projection[0]
            .parse::<usize>()
            .ok()
            .filter(|index| index.to_string() == projection[0])
            .and_then(|index| {
                ast::literal_array_element(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    span,
                    index,
                )
            })
            .is_some_and(|element| {
                mutation_target_projected(
                    file,
                    element,
                    &projection[1..],
                    object,
                    lookup,
                    depth - 1,
                    visited,
                )
            });
    }
    if let Some(logical) = file.ast.logical_at(span) {
        return mutation_target_projected(
            file,
            logical.left,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        ) && mutation_target_projected(
            file,
            logical.right,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if let Some(choice) = file.ast.conditional_at(span) {
        return mutation_target_projected(
            file,
            choice.consequent,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        ) && mutation_target_projected(
            file,
            choice.alternate,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if let Some(member) = file.ast.member_at(span) {
        let key = if file.ast.computed_members.contains(&span) {
            file.ast
                .literal_computed_members
                .iter()
                .find(|m| m.span == span)
                .map(|m| m.key.to_string())
        } else {
            file.source_text(member.property).map(str::to_owned)
        };
        let Some(key) = key else {
            return false;
        };
        let mut keys = vec![key];
        keys.extend_from_slice(projection);
        return mutation_target_projected(
            file,
            member.object,
            &keys,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if file.source_text(span) == Some("this") {
        let Some(function) = file
            .ast
            .functions_body_containing(span)
            .filter(|f| f.kind != ast::FunctionKind::Arrow)
            .min_by_key(|f| f.span.end - f.span.start)
        else {
            return false;
        };
        return mutation_parameter_inputs(
            file,
            function,
            None,
            projection,
            object,
            lookup,
            depth - 1,
            visited,
        );
    }
    if let Some(call) = file.ast.call_at(span) {
        // Object.assign returns the exact target. No other call is guessed.
        return mutation_builtin(file, call, lookup) == Some("assign")
            && call.arguments.first().is_some_and(|target| {
                !target.spread
                    && mutation_target_projected(
                        file,
                        target.span,
                        projection,
                        object,
                        lookup,
                        depth - 1,
                        visited,
                    )
            });
    }
    if !exact_identifier(file, span) {
        return false;
    }
    let declaration = file.ast.reference_declaration(span).unwrap_or(span);
    if let Some((function, position, binding)) = file.ast.functions.iter().find_map(|f| {
        f.parameters
            .iter()
            .enumerate()
            .find(|(_, p)| p.names.iter().any(|n| n.span == declaration))
            .map(|(i, p)| (f, i, p))
    }) {
        let Some(mut keys) = mutation_binding_projection(binding, declaration) else {
            return false;
        };
        keys.extend_from_slice(projection);
        return !crate::indexes::binding_written(file, declaration)
            && mutation_parameter_inputs(
                file,
                function,
                Some(position),
                &keys,
                object,
                lookup,
                depth - 1,
                visited,
            );
    }
    let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|b| b.names.iter().any(|n| n.span == declaration))
    else {
        return false;
    };
    let Some(mut keys) = mutation_binding_projection(binding, declaration) else {
        return false;
    };
    keys.extend_from_slice(projection);
    if file.ast.identifiers.iter().any(|id| {
        id.role == ast::IdentifierRole::Reference
            && matches!(file.source_text(id.span), Some("eval" | "arguments"))
    }) || file.ast.exports.iter().any(|export| {
        file.ast
            .exported_bindings(export)
            .any(|exported| exported.pattern == binding.pattern)
    }) {
        return false;
    }
    if !projection.is_empty()
        && !mutation_holder_closed(file, declaration, projection[0].as_str(), false)
    {
        return false;
    }
    let key = (file.path.to_string(), declaration);
    if !visited.insert(key.clone()) {
        return true;
    }
    let refers = |target: Span| {
        let target = file.ast.peel_ts_sugar_span(target);
        target == declaration || file.ast.reference_declaration(target) == Some(declaration)
    };
    let initial = binding.initializer.map_or(binding.runtime_let, |initial| {
        mutation_target_projected(file, initial, &keys, object, lookup, depth - 1, visited)
    });
    let writes = file
        .ast
        .write_targets
        .iter()
        .chain(&file.ast.iteration_targets)
        .chain(&file.ast.deleted_targets)
        .filter(|target| refers(**target))
        .all(|target| {
            file.ast
                .assignments
                .iter()
                .find(|a| a.target == *target && !a.reads_target)
                .is_some_and(|assignment| {
                    keys == projection
                        && mutation_target_projected(
                            file,
                            assignment.value_span,
                            projection,
                            object,
                            lookup,
                            depth - 1,
                            visited,
                        )
                })
        });
    let assignments = file
        .ast
        .assignments
        .iter()
        .filter(|a| refers(a.target))
        .all(|a| {
            !a.reads_target
                && keys == projection
                && mutation_target_projected(
                    file,
                    a.value_span,
                    projection,
                    object,
                    lookup,
                    depth - 1,
                    visited,
                )
        });
    visited.remove(&key);
    initial && writes && assignments
}

pub(super) fn mutation_binding_projection(
    binding: &ast::BindingFact,
    declaration: Span,
) -> Option<Vec<String>> {
    match binding.shape {
        ast::BindingShape::Identifier
            if binding.names.len() == 1 && binding.pattern == declaration =>
        {
            Some(vec![])
        }
        ast::BindingShape::Object
            if binding.object_slots.len() == binding.names.len()
                && binding.object_slots.iter().all(|s| s.plain_identifier_key) =>
        {
            binding
                .object_slots
                .iter()
                .find(|s| s.local.span == declaration)
                .map(|s| vec![s.property.to_string()])
        }
        ast::BindingShape::Array
            if binding.array_slots.iter().flatten().count() == binding.names.len() =>
        {
            binding
                .array_slots
                .iter()
                .position(|s| s.as_ref().is_some_and(|s| s.span == declaration))
                .map(|i| vec![i.to_string()])
        }
        _ => None,
    }
}

/// A holder projection needs more than its initializer: all exact references
/// remain static member receivers, and no visible write can replace the slot.
pub(super) fn mutation_holder_closed(
    file: &FileFacts,
    declaration: Span,
    key: &str,
    method_identity: bool,
) -> bool {
    let targets = file
        .ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .collect::<HashSet<_>>();
    let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|b| b.names.iter().any(|n| n.span == declaration))
    else {
        return false;
    };
    if let Some(literal) = binding.initializer.map(|s| file.ast.peel_ts_sugar_span(s))
        && file.ast.object_get_shapes.iter().any(|s| s.span == literal)
    {
        let Some(this_uses) =
            ast::this_expression_spans(std::path::Path::new(file.path.as_str()), &file.source)
        else {
            return false;
        };
        if !this_uses
            .into_iter()
            .filter(|s| literal.contains(*s))
            .all(|s| {
                let owner = file
                    .ast
                    .functions_body_containing(s)
                    .filter(|f| f.kind != ast::FunctionKind::Arrow)
                    .min_by_key(|f| f.span.end - f.span.start);
                // A nested ordinary function has a different this channel. Its
                // own mutation still faces the global exact-target gate.
                if owner.is_some_and(|f| {
                    !file
                        .ast
                        .object_properties
                        .iter()
                        .any(|p| literal.contains(p.span) && p.value == f.span)
                }) {
                    return true;
                }
                file.ast
                    .members
                    .iter()
                    .find(|m| file.ast.peel_ts_sugar_span(m.object) == s)
                    .is_some_and(|m| {
                        (method_identity && targets.contains(&m.span)
                            || !file.ast.computed_members.contains(&m.span))
                            && !(targets.contains(&m.span)
                                && file.source_text(m.property) == Some(key))
                            && !file.ast.calls.iter().any(|c| c.callee == m.span)
                            && (!method_identity
                                || file.source_text(m.property) != Some(key)
                                || targets.contains(&m.span))
                    })
            })
        {
            return false;
        }
    }
    file.ast
        .reference_declarations
        .iter()
        .filter(|(_, d)| *d == declaration)
        .all(|(reference, _)| {
            file.ast
                .members
                .iter()
                .find(|m| file.ast.peel_ts_sugar_span(m.object) == *reference)
                .is_some_and(|m| {
                    !file.ast.computed_members.contains(&m.span)
                        && !(targets.contains(&m.span) && file.source_text(m.property) == Some(key))
                })
        })
        && !file.ast.calls.iter().any(|c| {
            c.arguments.iter().any(|a| {
                file.ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(a.span))
                    == Some(declaration)
            })
        })
}

/// Join every invocation of a private, current source-local helper. An alias,
/// export, unknown call, spread or default is an unknown input, never an empty
/// set of receivers. call/apply require the actual library signature, and
/// methods require one unchanged literal holder with a closed reference census.
#[allow(clippy::too_many_arguments)]
pub(super) fn mutation_parameter_inputs(
    file: &FileFacts,
    function: &ast::FunctionFact,
    position: Option<usize>,
    projection: &[String],
    object: &Location,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    visited: &mut HashSet<(String, Span)>,
) -> bool {
    if depth == 0
        || function.rest_parameter
        || !lookup.function_value_is_current(file, function)
        || function.parameters.iter().any(|p| p.initializer.is_some())
        || file.ast.identifiers.iter().any(|id| {
            id.role == ast::IdentifierRole::Reference
                && matches!(file.source_text(id.span), Some("eval" | "arguments"))
        })
    {
        return false;
    }
    let name = crate::owners::function_binding_name(file, function);
    let method = file
        .ast
        .object_properties
        .iter()
        .find(|p| !p.computed && !p.data && file.ast.peel_ts_sugar_span(p.value) == function.span)
        .and_then(|property| {
            let holder = file
                .ast
                .object_get_shapes
                .iter()
                .filter(|s| s.span.contains(property.span))
                .min_by_key(|s| s.span.end - s.span.start)?;
            let binding = file.ast.bindings.iter().find(|b| {
                b.immutable
                    && b.shape == ast::BindingShape::Identifier
                    && b.initializer == Some(holder.span)
            })?;
            let name = binding.names.first()?;
            let key = file.source_text(property.key)?;
            mutation_holder_closed(file, name.span, key, true).then_some((name.span, key))
        });
    let Some(declaration) = method
        .map(|(d, _)| d)
        .or_else(|| name.as_ref().map(|n| n.span))
    else {
        return false;
    };
    if file.ast.exports.iter().any(|export| {
        export.span.contains(function.span)
            || file
                .ast
                .exported_bindings(export)
                .any(|b| b.names.iter().any(|n| n.span == declaration))
            || export
                .specifiers
                .iter()
                .chain(&export.declarations)
                .any(|s| {
                    s.local.span == declaration
                        || file.ast.reference_declaration(s.local.span) == Some(declaration)
                })
    }) {
        return false;
    }
    let mut seen = false;
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(_, d)| *d == declaration)
    {
        let direct = file
            .ast
            .calls
            .iter()
            .find(|c| file.ast.peel_ts_sugar_span(c.callee) == *reference);
        let member = file.ast.members.iter().find(|m| {
            file.ast.peel_ts_sugar_span(m.object) == *reference
                && !file.ast.computed_members.contains(&m.span)
        });
        let call = direct.or_else(|| {
            member.and_then(|m| {
                file.ast
                    .calls
                    .iter()
                    .find(|c| file.ast.peel_ts_sugar_span(c.callee) == m.span)
            })
        });
        let Some(call) = call else {
            // A method holder may also have ordinary data member reads. They
            // are checked when that projected receiver is proved separately.
            if method.is_some()
                && member
                    .is_some_and(|m| file.source_text(m.property) != method.map(|(_, key)| key))
            {
                continue;
            }
            return false;
        };
        if call.construct || call.arguments.iter().any(|a| a.spread) {
            return false;
        }
        let mut offset = 0;
        let mut apply = false;
        let receiver = if let Some(member) = member {
            let property = file.source_text(member.property);
            if let Some((_, key)) = method {
                if property != Some(key) {
                    continue;
                }
                Some(member.object)
            } else {
                let Some(signature) =
                    lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)
                else {
                    return false;
                };
                if !matches!(
                    signature.qualified_name.as_ref(),
                    "Function.call"
                        | "Function.apply"
                        | "CallableFunction.call"
                        | "CallableFunction.apply"
                ) || std::path::Path::new(signature.source_file.as_ref())
                    .file_name()
                    .and_then(|p| p.to_str())
                    != Some("lib.es5.d.ts")
                    || !matches!(property, Some("call" | "apply"))
                {
                    return false;
                }
                offset = 1;
                apply = property == Some("apply");
                call.arguments.first().map(|a| a.span)
            }
        } else {
            None
        };
        seen = true;
        let input = match position {
            None => receiver,
            Some(index) if apply => call.arguments.get(1).and_then(|a| {
                ast::literal_array_element(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    file.ast.peel_ts_sugar_span(a.span),
                    index,
                )
            }),
            Some(index) => call.arguments.get(index + offset).map(|a| a.span),
        };
        if !input.is_some_and(|input| {
            mutation_target_projected(file, input, projection, object, lookup, depth - 1, visited)
        }) {
            return false;
        }
    }
    seen
}

/// Whether `span` is exactly one identifier, as opposed to an expression an
/// identifier merely begins.
pub(super) fn exact_identifier(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .identifiers
        .binary_search_by_key(&span, |id| id.span)
        .is_ok()
        || file.ast.reference_declaration(span).is_some()
}

pub(super) fn simple_object_binding(binding: &ast::BindingFact) -> bool {
    binding.immutable
        && binding.shape == ast::BindingShape::Object
        && !binding.names.is_empty()
        && binding.object_slots.len() == binding.names.len()
        && binding
            .object_slots
            .iter()
            .all(|slot| slot.plain_identifier_key)
        && binding.names.iter().all(|name| {
            binding
                .object_slots
                .iter()
                .any(|slot| slot.local.span == name.span)
        })
}

/// ADR 0282: a `const` array pattern of plain identifier slots. A default
/// (`[a = f]`), a nested pattern (`[[a]]`, `[{ a }]`) or a rest element binds a
/// name that has no identifier slot, so the counts differ and the pattern is
/// refused. A hole (`[, a]`) skips a position and keeps every other exact.
pub(super) fn simple_array_binding(binding: &ast::BindingFact) -> bool {
    binding.immutable
        && binding.shape == ast::BindingShape::Array
        && !binding.names.is_empty()
        && binding.array_slots.iter().flatten().count() == binding.names.len()
        && binding.names.iter().all(|name| {
            binding
                .array_slots
                .iter()
                .flatten()
                .any(|slot| slot.span == name.span)
        })
}

/// ADR 0282: a slot names its closure only as an inline arrow, or as an
/// identifier for the exact unwritten `const` arrow binding the factory itself
/// declares. Arrows only, as in `member_value`: a `function` value's `this` and
/// a declaration's hoisted, writable binding are outside the summary. An
/// accessor slot (`running`) is source discovery's, never a second claim here;
/// a parameter, an import, a module binding, a call result and every other
/// expression refuse.
pub(super) fn slot_closure(
    file: &FileFacts,
    factory: Span,
    element: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    let (arrow, declaration) = slot_arrow(file, factory, element)?;
    // The same write pair the identifier route checks: `binding_written`
    // (redeclarations) ran in `slot_arrow`; assignment targets join through
    // the binding's symbol, which only the lookup holds.
    if let Some(declaration) = declaration {
        let symbol = lookup.entities().at(file.path.as_str(), declaration)?;
        if crate::value_identity::binding_has_write(file, lookup.entities(), symbol) {
            return None;
        }
    }
    Some(arrow)
}

/// The syntactic half of [`slot_closure`]: the arrow a slot names, and the
/// declaration of the binding that named it, if one did.
pub(super) fn slot_arrow(
    file: &FileFacts,
    factory: Span,
    element: Span,
) -> Option<(Span, Option<Span>)> {
    let element = file.ast.peel_ts_sugar_span(element);
    let (arrow, declaration) = if file.ast.functions.iter().any(|f| f.span == element) {
        (element, None)
    } else {
        if !exact_identifier(file, element) {
            return None;
        }
        let declaration = file.ast.reference_declaration(element)?;
        let binding = file
            .ast
            .bindings
            .iter()
            .find(|b| b.names.iter().any(|n| n.span == declaration))?;
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
            || containing_ast_function(&file.ast, declaration).map(|f| f.span) != Some(factory)
            || crate::indexes::binding_written(file, declaration)
        {
            return None;
        }
        (
            file.ast.peel_ts_sugar_span(binding.initializer?),
            Some(declaration),
        )
    };
    let function = file.ast.function_at(arrow)?;
    (function.kind == ast::FunctionKind::Arrow && !function.r#async && !function.generator)
        .then_some((arrow, declaration))
}

/// ADR 0282: the exact slots of a returned array literal. A hole or a spread
/// moves every later position, so either refuses the whole literal.
pub(super) fn returned_tuple_elements(file: &FileFacts, span: Span) -> Option<Vec<Span>> {
    let span = file.ast.peel_ts_sugar_span(span);
    if !file.ast.array_literals.contains(&span) {
        return None;
    }
    let returned = file
        .ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|f| f.expression_return.as_ref()),
        )
        .find(|r| {
            r.argument
                .is_some_and(|s| file.ast.peel_ts_sugar_span(s) == span)
        })?;
    let structure = returned.structure.as_ref()?;
    if !structure.complete_literal || structure.elements.is_empty() {
        return None;
    }
    structure
        .elements
        .iter()
        .map(|s| s.map(|s| file.ast.peel_ts_sugar_span(s)))
        .collect()
}

/// The signature names the built-in operation; the admitted fresh literal
/// and closed receiver census establish the runtime array identity. A typed
/// custom receiver cannot become an array solely from this signature.
pub(super) fn array_iteration_receiver(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Span> {
    if call.construct {
        return None;
    }
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let property = match declaration.qualified_name.as_ref() {
        "Array.map" | "ReadonlyArray.map" => "map",
        "Array.forEach" | "ReadonlyArray.forEach" => "forEach",
        _ => return None,
    };
    if declaration.location.path != declaration.source_file
        || std::path::Path::new(declaration.source_file.as_ref())
            .file_name()?
            .to_str()?
            != "lib.es5.d.ts"
    {
        return None;
    }
    let member = file.ast.members.iter().find(|m| {
        m.span == file.ast.peel_ts_sugar_span(call.callee)
            && !file.ast.computed_members.contains(&m.span)
            && file.source_text(m.property) == Some(property)
    })?;
    Some(file.ast.peel_ts_sugar_span(member.object))
}

pub(crate) fn single_return(file: &FileFacts, function: &ast::FunctionFact) -> Option<Span> {
    if let Some(returned) = &function.expression_return {
        return returned.argument;
    }
    let mut returns = file.ast.returns.iter().filter(|r| {
        containing_ast_function(&file.ast, r.span).is_some_and(|f| f.span == function.span)
    });
    let returned = returns.next()?;
    if returns.next().is_some() || returned.conditional || !returned.control_tests.is_empty() {
        return None;
    }
    returned.argument
}

pub(super) fn returned_array_elements(file: &FileFacts, span: Span) -> Option<Vec<Span>> {
    let span = file.ast.peel_ts_sugar_span(span);
    if !file.ast.array_literals.contains(&span) {
        return None;
    }
    let returned = file
        .ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|f| f.expression_return.as_ref()),
        )
        .find(|r| {
            r.argument
                .is_some_and(|s| file.ast.peel_ts_sugar_span(s) == span)
        })?;
    let structure = returned.structure.as_ref()?;
    if !structure.complete_literal || structure.elements.is_empty() {
        return None;
    }
    structure
        .elements
        .iter()
        .map(|s| {
            let s = file.ast.peel_ts_sugar_span((*s)?);
            closed_properties(file, s).map(|_| s)
        })
        .collect()
}

/// ADR 0289: the own data properties written directly in the object literal
/// at `span`, whatever else the literal holds. Derivation only.
pub(super) fn direct_data_properties(
    file: &FileFacts,
    span: Span,
) -> Vec<&ast::ObjectPropertyFact> {
    file.ast
        .object_properties
        .iter()
        .filter(|p| {
            p.data
                && !p.computed
                && span.contains(p.span)
                && !file.ast.object_properties.iter().any(|parent| {
                    parent.span != p.span
                        && span.contains(parent.span)
                        && parent.span.contains(p.span)
                })
        })
        .collect()
}

/// Literal syntax supplies member identity only when its entire own property
/// set is closed. Nested properties/callback objects are never siblings.
pub(super) fn module_scoped_source(file: &FileFacts) -> bool {
    if [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| file.path.as_str().ends_with(suffix))
    {
        return false;
    }
    !file.ast.imports.is_empty()
        || file.ast.exports.iter().any(|export| {
            !file
                .ast
                .module_blocks
                .iter()
                .any(|block| block.contains(export.span))
        })
}

pub(crate) fn closed_properties(
    file: &FileFacts,
    span: Span,
) -> Option<Vec<&ast::ObjectPropertyFact>> {
    let shape = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)?;
    let properties = file
        .ast
        .object_properties
        .iter()
        .filter(|p| {
            span.contains(p.span)
                && !file.ast.object_properties.iter().any(|parent| {
                    parent.span != p.span
                        && span.contains(parent.span)
                        && parent.span.contains(p.span)
                })
        })
        .collect::<Vec<_>>();
    let mut names = HashSet::default();
    for property in &properties {
        let name = file.source_text(property.key)?.trim_matches(['\'', '"']);
        let inert_method = property.method
            && file
                .ast
                .function_at(property.value)
                .is_some_and(|function| crate::object_members::receiver_inert(file, function));
        if property.computed
            || !(property.data || inert_method)
            || name == "__proto__"
            || !names.insert(name)
        {
            return None;
        }
    }
    (properties.len() == shape.properties.len() && !properties.is_empty()).then_some(properties)
}
