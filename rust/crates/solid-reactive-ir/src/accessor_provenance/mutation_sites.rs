//! Mutation and invocation sites: tuple and iteration writes, string-code slots, gateways, proxy and store setters, reflective prototype writes.

use super::*;

/// Whether a reflective write's target may be spelled through a `prototype`.
/// A spread can supply any target, a prototype included.
pub(super) fn targets_prototype(file: &FileFacts, call: &ast::CallFact) -> bool {
    call.arguments.first().is_some_and(|target| {
        target.spread
            || file
                .source_text(target.span)
                .is_some_and(|text| text.contains("prototype"))
    })
}

/// The engine's dense returned literal owns its indices and length, and the
/// iterator result owns value/done. Iteration still Gets @@iterator, next and
/// (on early close) return through mutable prototype chains. A __proto__ write
/// or setPrototypeOf can change any of those lookups. Unknown keys may name
/// these properties; a known different string cannot.
pub(super) fn iteration_key(key: Option<&str>) -> bool {
    key.is_none_or(|key| matches!(key, "next" | "return" | "__proto__"))
}

pub(super) fn member_key<'a>(file: &'a FileFacts, member: &ast::MemberFact) -> Option<&'a str> {
    if file.ast.computed_members.contains(&member.span) {
        file.ast
            .literal_computed_members
            .iter()
            .find(|key| key.span == member.span)
            .map(|key| key.key.as_str())
    } else {
        file.source_text(member.property)
    }
}

/// A negative proof only: a separate source allocation cannot be one of the
/// intrinsic prototypes, nor the fresh literal consumed directly at its return.
/// Follow exact immutable local aliases. Parameters, calls (getPrototypeOf
/// included), imports, reassigned bindings and cycles remain unknown.
pub(super) fn separate_iteration_target(file: &FileFacts, span: Span, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if file.ast.array_literals.contains(&span)
        || file
            .ast
            .functions
            .iter()
            .any(|function| function.span == span)
        || file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == span)
    {
        return true;
    }
    if let Some(member) = file.ast.member_at(span)
        && member_key(file, member) == Some("prototype")
        && let Some(declaration) = file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
        && file.ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        })
        && !iteration_binding_written(file, declaration)
    {
        return true;
    }
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.immutable
                && binding.shape == ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declaration
                && !iteration_binding_written(file, declaration)
        })
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| separate_iteration_target(file, initializer, depth - 1))
}

pub(super) fn iteration_binding_written(file: &FileFacts, declaration: Span) -> bool {
    file.ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .any(|target| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(*target))
                == Some(declaration)
        })
}

pub(super) fn iteration_map(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    let Some(shape) = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)
    else {
        return true;
    };
    // closed excludes spreads and unnamed computed keys. Do not accidentally
    // treat a symbol descriptor (which has no string key) as an empty map.
    shape
        .properties
        .iter()
        .any(|property| iteration_key(Some(property.key.as_str())))
}

pub(super) fn tuple_mutation_method<'a>(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    // A library signature on an alias is insufficient to certify the runtime
    // callee, but is sufficient to retain a possible mutation as a veto.
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let (method, library) = match declaration.qualified_name.as_ref() {
        "ObjectConstructor.assign" => ("assign", "lib.es2015.core.d.ts"),
        "ObjectConstructor.defineProperty" => ("defineProperty", "lib.es5.d.ts"),
        "ObjectConstructor.defineProperties" => ("defineProperties", "lib.es5.d.ts"),
        "ObjectConstructor.setPrototypeOf" => ("setPrototypeOf", "lib.es2015.core.d.ts"),
        "Reflect.set" => ("set", "lib.es2015.reflect.d.ts"),
        "Reflect.defineProperty" => ("defineProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.deleteProperty" => ("deleteProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.setPrototypeOf" => ("setPrototypeOf", "lib.es2015.reflect.d.ts"),
        "Function.call"
        | "Function.apply"
        | "Function.bind"
        | "CallableFunction.call"
        | "CallableFunction.apply"
        | "CallableFunction.bind"
        | "NewableFunction.call"
        | "NewableFunction.apply"
        | "NewableFunction.bind" => ("invoke", "lib.es5.d.ts"),
        "Reflect.apply" => ("reflectInvoke", "lib.es2015.reflect.d.ts"),
        "FunctionConstructor" | "FunctionConstructor.new" => ("stringCode", "lib.es5.d.ts"),
        "eval" => ("stringCode", "lib.es5.d.ts"),
        _ => return None,
    };
    (std::path::Path::new(declaration.source_file.as_ref())
        .file_name()?
        .to_str()?
        == library)
        .then_some(method)
}

pub(crate) fn tuple_iteration_write(lookup: &SemanticLookup<'_>) -> bool {
    if lookup.visible_string_code() {
        return true;
    }
    lookup.files().iter().any(|file| {
        let targets = file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .map(|span| file.ast.peel_ts_sugar_span(*span))
            .collect::<HashSet<_>>();
        file.ast.members.iter().any(|member| {
            targets.contains(&member.span)
                && iteration_key(member_key(file, member))
                && !separate_iteration_target(file, member.object, 24)
        }) || file.ast.calls.iter().any(|call| {
            let Some(method) = tuple_mutation_method(file, call, lookup) else {
                // An erased reflective signature cannot use its receiver or
                // first argument as the mutator's target. These spellings are
                // possible invocation only, never positive callee authority.
                if lookup
                    .standard_library_declaration(file, call, typefacts::CallKind::Call)
                    .is_none()
                    && file.ast.members.iter().any(|member| {
                        member.span == file.ast.peel_ts_sugar_span(call.callee)
                            && matches!(
                                member_key(file, member),
                                None | Some("call" | "apply" | "bind")
                            )
                    })
                    && !iteration_source_function(file, call.callee, lookup)
                {
                    return true;
                }
                // Missing callee evidence must not turn an opaque mutation
                // helper into a negative census. A supplied receiver that may
                // be an intrinsic retains the veto. Source implementations
                // are covered by the same project-wide write census; exact
                // non-mutating library signatures are outside this catalog.
                return lookup.primitive_at_call(file, call.span).is_none()
                    && !intrinsic_disjoint_store_setter(file, call, lookup)
                    && !intrinsic_disjoint_proxy_creation(file, call, lookup)
                    && !source_bound_invocation(file, call.callee, lookup)
                    && lookup
                        .standard_library_declaration(file, call, typefacts::CallKind::Call)
                        .is_none()
                    // A factory's declaration does not describe the callable
                    // it returns. Its object handoff needs its own census.
                    && (file.ast.call_at(file.ast.peel_ts_sugar_span(call.callee)).is_some()
                        || lookup
                            .function_called_at(
                                file.path.as_str(),
                                file.ast.peel_ts_sugar_span(call.callee),
                            )
                            .is_none())
                    && call.arguments.first().is_some_and(|target| {
                        target.spread
                            || (!matches!(
                                target.runtime_value_kind,
                                ast::RuntimeValueKind::Primitive
                                    | ast::RuntimeValueKind::Nullish
                                    | ast::RuntimeValueKind::Function
                            ) && !separate_iteration_target(file, target.span, 24))
                    });
            };
            if method == "stringCode" {
                return true;
            }
            if matches!(method, "invoke" | "reflectInvoke") {
                if source_bound_invocation(file, call.callee, lookup) {
                    return false;
                }
                // Receiver/argument slots are different for reflective calls.
                // Only a source-resolved implementation discharges the target;
                // its writes are covered by this same whole-program census.
                let target = if method == "reflectInvoke" {
                    call.arguments
                        .first()
                        .filter(|argument| !argument.spread)
                        .map(|argument| argument.span)
                } else {
                    file.ast
                        .member_at(file.ast.peel_ts_sugar_span(call.callee))
                        .map(|member| member.object)
                };
                return !invocation_operator_is_exact(file, call, method, lookup)
                    || target
                        .is_none_or(|target| !iteration_source_function(file, target, lookup));
            }
            if call.arguments.iter().any(|argument| argument.spread) {
                return true;
            }
            let Some(target) = call.arguments.first() else {
                return true;
            };
            if separate_iteration_target(file, target.span, 24) {
                return false;
            }
            match method {
                "setPrototypeOf" => true,
                "assign" => call
                    .arguments
                    .iter()
                    .skip(1)
                    .any(|argument| iteration_map(file, argument.span)),
                "defineProperties" => call
                    .arguments
                    .get(1)
                    .is_none_or(|argument| iteration_map(file, argument.span)),
                _ => iteration_key(call.arguments.get(1).and_then(|argument| {
                    match &argument.literal_value {
                        ast::ArgumentLiteralFact::String(key) => Some(key.as_ref()),
                        ast::ArgumentLiteralFact::Integer(_) => Some("0"),
                        _ => None,
                    }
                })),
            }
        })
    })
}

/// This census withholds authority: it never proves that a gateway ran or
/// mutated a particular property. Erased signatures cannot hide visible code
/// construction, including source slots and returned gateway values.
pub(crate) fn visible_string_code(lookup: &SemanticLookup<'_>) -> bool {
    lookup.files().iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == ast::ModuleHazardKind::Eval)
            || file.ast.calls.iter().any(|call| {
                string_code_reference(file, call.callee, lookup, 24, false)
                    || tuple_mutation_method(file, call, lookup) == Some("stringCode")
                    || (call.construct
                        && lookup
                            .standard_library_declaration(
                                file,
                                call,
                                typefacts::CallKind::Construct,
                            )
                            .is_some_and(|declaration| {
                                declaration
                                    .qualified_name
                                    .starts_with("FunctionConstructor")
                            }))
            })
    })
}

pub(super) fn string_code_reference(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    returned_value: bool,
) -> bool {
    if depth == 0 {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(branch) = file.ast.conditional_at(span) {
        return string_code_reference(file, branch.consequent, lookup, depth - 1, returned_value)
            || string_code_reference(file, branch.alternate, lookup, depth - 1, returned_value);
    }
    if let Some(branch) = file.ast.logical_at(span) {
        return string_code_reference(file, branch.left, lookup, depth - 1, returned_value)
            || string_code_reference(file, branch.right, lookup, depth - 1, returned_value);
    }
    if let Some(member) = file.ast.member_at(span) {
        return string_code_slot(
            file,
            member.object,
            member_key(file, member),
            lookup,
            depth - 1,
            false,
        );
    }
    if let Some(call) = file.ast.call_at(span) {
        if string_code_reference(file, call.callee, lookup, depth - 1, false) {
            return true;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if gateway_binding(file, callee, lookup).is_some_and(|(origin, binding, declaration)| {
            !binding.immutable || iteration_binding_written(origin, declaration)
        }) || file
            .ast
            .reference_declaration(callee)
            .is_some_and(|declaration| iteration_binding_written(file, declaration))
        {
            return true;
        }
        // Source implementation, not an identifier's nominal signature, owns
        // the value returned by a visible gateway factory.
        if let Some((origin, function)) =
            lookup.function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
        {
            return function
                .expression_return
                .as_ref()
                .filter(|returned| {
                    !matches!(
                        returned.runtime_value_kind,
                        ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                    )
                })
                .and_then(|returned| returned.argument)
                .into_iter()
                .chain(
                    origin
                        .ast
                        .returns
                        .iter()
                        .filter(|returned| {
                            containing_ast_function(&origin.ast, returned.span)
                                .is_some_and(|owner| owner.span == function.span)
                        })
                        .filter(|returned| {
                            !matches!(
                                returned.runtime_value_kind,
                                ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                            )
                        })
                        .filter_map(|returned| returned.argument),
                )
                .any(|returned| string_code_reference(origin, returned, lookup, depth - 1, true));
        }
        return string_code_reference(file, call.callee, lookup, depth - 1, returned_value);
    }
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == span)
    {
        // Unsupported expression transports (a sequence, for example) do
        // not prove absence. Exact fresh functions/allocations are not code
        // constructors; their bodies still participate in the call census.
        return !file
            .ast
            .functions
            .iter()
            .any(|function| function.span == span)
            && !file.ast.array_literals.contains(&span)
            && !file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == span);
    }
    let Some((file, binding, declaration)) = gateway_binding(file, span, lookup) else {
        if file
            .ast
            .reference_declaration(span)
            .is_some_and(|declaration| gateway_assignment(file, declaration, lookup, depth))
        {
            return true;
        }
        return matches!(file.source_text(span), Some("Function" | "eval"))
            && file.ast.reference_declaration(span).is_none()
            || (returned_value
                && !file.ast.functions.iter().any(|function| {
                    function
                        .name
                        .as_ref()
                        .is_some_and(|name| Some(name.span) == file.ast.reference_declaration(span))
                        && lookup.function_value_is_current(file, function)
                }));
    };
    if gateway_assignment(file, declaration, lookup, depth) {
        return true;
    }
    let Some(initializer) = binding.initializer else {
        return returned_value;
    };
    if binding.shape == ast::BindingShape::Object {
        return binding
            .object_slots
            .iter()
            .find(|slot| slot.local.span == declaration)
            .is_none_or(|slot| {
                string_code_slot(
                    file,
                    initializer,
                    Some(slot.property.as_str()),
                    lookup,
                    depth - 1,
                    false,
                )
            });
    }
    if binding.shape == ast::BindingShape::Array
        && file
            .ast
            .array_literals
            .contains(&file.ast.peel_ts_sugar_span(initializer))
    {
        // No element identity is guessed. Any possible gateway in a literal
        // withholds every selected callable slot until exact value flow exists.
        return file
            .ast
            .identifiers
            .iter()
            .filter(|id| id.role == ast::IdentifierRole::Reference && initializer.contains(id.span))
            .any(|id| string_code_reference(file, id.span, lookup, depth - 1, true))
            || file
                .ast
                .members
                .iter()
                .filter(|member| initializer.contains(member.span))
                .any(|member| string_code_reference(file, member.span, lookup, depth - 1, true));
    }
    if matches!(
        binding.initializer_value_kind,
        ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
    ) {
        return false;
    }
    string_code_reference(file, initializer, lookup, depth - 1, returned_value)
}

pub(super) fn gateway_assignment(
    file: &FileFacts,
    declaration: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    iteration_binding_written(file, declaration)
        && file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .filter(|target| {
                file.ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(**target))
                    == Some(declaration)
            })
            .any(|target| {
                file.ast
                    .assignments
                    .iter()
                    .find(|assignment| assignment.target == *target && !assignment.reads_target)
                    .is_none_or(|assignment| {
                        assignment.value != ast::AssignmentValueKind::Function
                            || string_code_reference(
                                file,
                                assignment.value_span,
                                lookup,
                                depth - 1,
                                true,
                            )
                    })
            })
}

pub(super) fn gateway_binding<'a>(
    file: &'a FileFacts,
    span: Span,
    lookup: &'a SemanticLookup<'_>,
) -> Option<(&'a FileFacts, &'a ast::BindingFact, Span)> {
    file.ast
        .reference_declaration(span)
        .and_then(|declaration| {
            file.ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
                .map(|binding| (file, binding, declaration))
        })
        .or_else(|| {
            let (origin, binding, symbol) =
                lookup.binding_at_reference(file.path.as_str(), span)?;
            let declaration = binding
                .names
                .iter()
                .find(|name| lookup.entity_symbol(origin, name.span) == Some(symbol.as_str()))?
                .span;
            Some((origin, binding, declaration))
        })
}

pub(super) fn string_code_slot(
    file: &FileFacts,
    object: Span,
    key: Option<&str>,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    returned_receiver: bool,
) -> bool {
    let possible_gateway_key = matches!(key, None | Some("Function" | "eval" | "constructor"));
    // Only exact, current source implementations discharge these potentially
    // executable slots. A Function/Proxy type is no runtime identity proof.
    // A selected property declaration on a structural receiver is not enough.
    // The receiver below must be a closed, non-escaping source allocation.
    if depth == 0 {
        return true;
    }
    let object = file.ast.peel_ts_sugar_span(object);
    if let Some(branch) = file.ast.conditional_at(object) {
        return string_code_slot(
            file,
            branch.consequent,
            key,
            lookup,
            depth - 1,
            returned_receiver,
        ) || string_code_slot(
            file,
            branch.alternate,
            key,
            lookup,
            depth - 1,
            returned_receiver,
        );
    }
    if let Some(branch) = file.ast.logical_at(object) {
        return string_code_slot(file, branch.left, key, lookup, depth - 1, returned_receiver)
            || string_code_slot(
                file,
                branch.right,
                key,
                lookup,
                depth - 1,
                returned_receiver,
            );
    }
    if let Some(call) = file.ast.call_at(object)
        && let Some((origin, function)) =
            lookup.function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
    {
        return function
            .expression_return
            .as_ref()
            .filter(|returned| {
                !matches!(
                    returned.runtime_value_kind,
                    ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                )
            })
            .and_then(|returned| returned.argument)
            .into_iter()
            .chain(
                origin
                    .ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        containing_ast_function(&origin.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    })
                    .filter(|returned| {
                        !matches!(
                            returned.runtime_value_kind,
                            ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                        )
                    })
                    .filter_map(|returned| returned.argument),
            )
            .any(|returned| string_code_slot(origin, returned, key, lookup, depth - 1, true));
    }
    if let Some((file, binding, declaration)) = gateway_binding(file, object, lookup)
        && binding.shape == ast::BindingShape::Identifier
        && binding.immutable
        && !iteration_binding_written(file, declaration)
        && let Some(initializer) = binding.initializer
    {
        let possible =
            string_code_slot(file, initializer, key, lookup, depth - 1, returned_receiver);
        if possible || !possible_gateway_key {
            return possible;
        }
        if file.ast.exports.iter().any(|export| {
            export
                .declarations
                .iter()
                .any(|value| value.local.span == declaration)
                || export.specifiers.iter().any(|value| {
                    file.ast.reference_declaration(value.local.span) == Some(declaration)
                })
        }) {
            return true;
        }
        if !file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, bound)| *bound == declaration)
            .all(|(reference, _)| {
                file.ast.members.iter().any(|member| {
                    file.ast.peel_ts_sugar_span(member.object) == *reference
                        && !file.ast.write_targets.contains(&member.span)
                        && !file.ast.deleted_targets.contains(&member.span)
                }) || file.ast.bindings.iter().any(|binding| {
                    binding.immutable
                        && binding.shape == ast::BindingShape::Object
                        && binding
                            .initializer
                            .map(|span| file.ast.peel_ts_sugar_span(span))
                            == Some(*reference)
                })
            })
        {
            return true;
        }
        return false;
    }
    if let Some(key) = key
        && let Some(shape) = file.ast.object_get_shapes.iter().find(|shape| {
            shape.span == object
                && shape.closed
                && shape
                    .properties
                    .iter()
                    .all(|property| property.getter.is_none())
        })
        && let Some(property) = file.ast.object_properties.iter().find(|property| {
            object.contains(property.span)
                && !property.computed
                && file.source_text(property.key) == Some(key)
                && !file.ast.object_get_shapes.iter().any(|nested| {
                    nested.span != object
                        && object.contains(nested.span)
                        && nested.span.contains(property.span)
                })
        })
        && shape.properties.iter().any(|property| property.key == key)
    {
        // A method body can expose/replace its slot through `this`, without a
        // reference to the object binding. Refuse possible slot replacement
        // before using an own source method as negative gateway evidence.
        if lookup.files().iter().any(|candidate| {
            candidate.ast.write_targets.iter().chain(&candidate.ast.deleted_targets).any(|target| {
                candidate.ast.members.iter().any(|member| member.span == candidate.ast.peel_ts_sugar_span(*target)
                    && member_key(candidate, member).is_none_or(|written| written == key || written == "__proto__"))
            }) || candidate.ast.calls.iter().any(|call| {
                tuple_mutation_method(candidate, call, lookup).is_some_and(|method| match method {
                    "setPrototypeOf" => true,
                    "assign" | "defineProperties" => call.arguments.iter().skip(1).any(|argument| {
                        candidate.ast.object_get_shapes.iter().find(|shape| shape.span == candidate.ast.peel_ts_sugar_span(argument.span) && shape.closed)
                            .is_none_or(|shape| shape.properties.iter().any(|property| property.key == key || property.key == "__proto__"))
                    }),
                    "set" | "defineProperty" | "deleteProperty" => call.arguments.get(1).is_none_or(|argument| {
                        !matches!(&argument.literal_value, ast::ArgumentLiteralFact::String(written) if written.as_str() != key && written.as_str() != "__proto__")
                    }),
                    _ => false,
                })
            })
        }) { return true; }
        let Some(function) = file
            .ast
            .function_at(file.ast.peel_ts_sugar_span(property.value))
        else {
            if matches!(
                property.value_kind,
                ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
            ) {
                return false;
            }
            return string_code_reference(file, property.value, lookup, depth - 1, true);
        };
        return function
            .expression_return
            .as_ref()
            .and_then(|returned| returned.argument)
            .into_iter()
            .chain(
                file.ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    })
                    .filter(|returned| {
                        !matches!(
                            returned.runtime_value_kind,
                            ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                        )
                    })
                    .filter_map(|returned| returned.argument),
            )
            .any(|returned| string_code_reference(file, returned, lookup, depth - 1, true));
    }
    // Unknown member authority remains a possibility, including a function's
    // inherited constructor and erased/aliased realm objects. No name grants
    // positive authority and no unresolved value proves a mutation.
    possible_gateway_key || returned_receiver
}

pub(super) fn iteration_source_function(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if invocation_property_write(lookup) {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if lookup
        .function_called_at(file.path.as_str(), span)
        .is_some()
    {
        return true;
    }
    let declaration = file.ast.reference_declaration(span);
    file.ast.functions.iter().any(|function| {
        (function.span == span
            || function
                .name
                .as_ref()
                .is_some_and(|name| Some(name.span) == declaration))
            && lookup.function_value_is_current(file, function)
    })
}

pub(super) fn invocation_key(key: Option<&str>) -> bool {
    key.is_none_or(|key| matches!(key, "call" | "apply" | "bind" | "__proto__" | "Reflect"))
}

pub(super) fn invocation_operator_is_exact(
    file: &FileFacts,
    call: &ast::CallFact,
    method: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    if method == "invoke" {
        return matches!(member_key(file, member), Some("call" | "apply" | "bind"));
    }
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if member_key(file, member) != Some("apply")
        || file.source_text(receiver) != Some("Reflect")
        || file.ast.reference_declaration(receiver).is_some()
    {
        return false;
    }
    let Some(symbol) = lookup.symbol_at(file.path.as_str(), receiver) else {
        return false;
    };
    let Some(declaration) =
        lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)
    else {
        return false;
    };
    symbol.alias_target().is_empty()
        && !symbol.declarations().is_empty()
        && symbol.declarations().iter().all(|binding| {
            binding.name.as_ref() == "Reflect" && binding.location.path == declaration.source_file
        })
}

/// Knowing f's implementation does not prove that f.call/f.apply/f.bind, or
/// Reflect.apply, still selects the intrinsic invocation operator. Retain
/// visible possible replacement, including writes onto a fresh function.
pub(super) fn invocation_property_write(lookup: &SemanticLookup<'_>) -> bool {
    if lookup.visible_string_code() {
        return true;
    }
    let map = |file: &FileFacts, span: Span| {
        file.ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == file.ast.peel_ts_sugar_span(span) && shape.closed)
            .is_none_or(|shape| {
                shape
                    .properties
                    .iter()
                    .any(|property| invocation_key(Some(property.key.as_str())))
            })
    };
    lookup.files().iter().any(|file| {
        file.ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .any(|target| {
                let target = file.ast.peel_ts_sugar_span(*target);
                (file.source_text(target) == Some("Reflect")
                    && file.ast.reference_declaration(target).is_none())
                    || file.ast.members.iter().any(|member| {
                        member.span == target && invocation_key(member_key(file, member))
                    })
            })
            || file.ast.calls.iter().any(|call| {
                let Some(method) = tuple_mutation_method(file, call, lookup) else {
                    return false;
                };
                if call.arguments.iter().any(|argument| argument.spread) {
                    return true;
                }
                match method {
                    "invoke" | "reflectInvoke" | "stringCode" => false,
                    "setPrototypeOf" => true,
                    "assign" => call
                        .arguments
                        .iter()
                        .skip(1)
                        .any(|argument| map(file, argument.span)),
                    "defineProperties" => call
                        .arguments
                        .get(1)
                        .is_none_or(|argument| map(file, argument.span)),
                    _ => invocation_key(call.arguments.get(1).and_then(|argument| {
                        match &argument.literal_value {
                            ast::ArgumentLiteralFact::String(key) => Some(key.as_ref()),
                            ast::ArgumentLiteralFact::Integer(_) => Some("0"),
                            _ => None,
                        }
                    })),
                }
            })
    })
}

/// ProxyCreate validates two object values and installs internal slots; it
/// invokes neither value and writes no intrinsic. A ProxyConstructor type is
/// not this runtime identity: require the unaliased global's library variable,
/// and refuse visible replacement (including unknown reflective targets).
pub(super) fn intrinsic_disjoint_proxy_creation(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if lookup.visible_string_code()
        || !call.construct
        || call.arguments.iter().any(|argument| argument.spread)
    {
        return false;
    }
    let Some(declaration) =
        lookup.standard_library_declaration(file, call, typefacts::CallKind::Construct)
    else {
        return false;
    };
    if declaration.qualified_name.as_ref() != "ProxyConstructor.construct"
        || std::path::Path::new(declaration.source_file.as_ref())
            .file_name()
            .and_then(|name| name.to_str())
            != Some("lib.es2015.proxy.d.ts")
    {
        return false;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
        || file.ast.reference_declaration(callee).is_some()
    {
        return false;
    }
    let Some(symbol) = lookup.symbol_at(file.path.as_str(), callee) else {
        return false;
    };
    if !symbol.alias_target().is_empty()
        || symbol.declarations().len() != 1
        || !symbol.declarations().iter().all(|binding| {
            binding.name.as_ref() == "Proxy"
                && binding.kind.as_ref() == "variable"
                && binding.location.path == declaration.source_file
        })
    {
        return false;
    }
    !lookup.files().iter().any(|candidate| {
        candidate
            .ast
            .write_targets
            .iter()
            .chain(&candidate.ast.deleted_targets)
            .any(|target| {
                let target = candidate.ast.peel_ts_sugar_span(*target);
                (candidate.source_text(target) == Some("Proxy")
                    && candidate.ast.reference_declaration(target).is_none())
                    || candidate.ast.members.iter().any(|member| {
                        member.span == target
                            && matches!(member_key(candidate, member), None | Some("Proxy"))
                    })
            })
            || candidate.ast.calls.iter().any(|mutation| {
                tuple_mutation_method(candidate, mutation, lookup).is_some_and(|method| {
                    !matches!(method, "invoke" | "reflectInvoke" | "stringCode")
                }) && mutation.arguments.first().is_none_or(|target| {
                    target.spread || !separate_iteration_target(candidate, target.span, 24)
                })
            })
    })
}

pub(super) fn source_bound_invocation(
    file: &FileFacts,
    callee: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(bound) = file.ast.call_at(file.ast.peel_ts_sugar_span(callee)) else {
        return false;
    };
    let Some(declaration) =
        lookup.standard_library_declaration(file, bound, typefacts::CallKind::Call)
    else {
        return false;
    };
    declaration.qualified_name.ends_with(".bind")
        && tuple_mutation_method(file, bound, lookup) == Some("invoke")
        && file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(bound.callee))
            .is_some_and(|member| {
                member_key(file, member) == Some("bind")
                    && iteration_source_function(file, member.object, lookup)
            })
}

/// A store-setter type corroborates the binding, but does not prove runtime
/// identity alone. Require the exact unwritten slot produced by a dialect
/// primitive as well. Casts and declared-only setters cannot discharge a call.
pub(super) fn intrinsic_disjoint_store_setter(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
    {
        return false;
    }
    let Some((origin, binding, symbol)) = lookup.binding_at_reference(file.path.as_str(), callee)
    else {
        return false;
    };
    let Some(slot) = binding.array_slots.get(1).and_then(Option::as_ref) else {
        return false;
    };
    let store_role = lookup
        .entity_at(file.path.as_str(), callee)
        .or_else(|| lookup.entity_at(origin.path.as_str(), slot.span))
        .and_then(|entity| entity.type_descriptor.as_ref())
        .is_some_and(|descriptor| {
            descriptor.alias_declarations.iter().any(|declaration| {
                lookup
                    .dialect
                    .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                    == Some(solid_dialect::TypeRole::StoreSetter)
            })
        });
    simple_array_binding(binding)
        && lookup.entity_symbol(origin, slot.span) == Some(symbol.as_str())
        && !crate::indexes::binding_written(origin, slot.span)
        && binding
            .initializer
            .and_then(|span| origin.ast.call_at(origin.ast.peel_ts_sugar_span(span)))
            .and_then(|created| lookup.primitive_at_call(origin, created.span))
            .is_some_and(|primitive| {
                lookup.dialect.returns_reactive_tuple(primitive)
                    && (store_role || lookup.dialect.returns_store(primitive))
            })
}

/// ADR 0292: whether any analyzed file holds a reflective write
/// (`Object.assign`, `defineProperty`, `Reflect.set`, ...) whose target is
/// spelled through a `prototype`, the one write [`MemberStability`] lets reach
/// a fresh literal nothing else holds (ADR 0282, narrowed by ADR 0288's
/// review). The same visible veto, for a fresh module object.
pub(crate) fn reflective_prototype_write(lookup: &SemanticLookup<'_>) -> bool {
    lookup.files().iter().any(|file| {
        file.ast.calls.iter().any(|call| {
            targets_prototype(file, call) && mutation_builtin(file, call, lookup).is_some()
        })
    })
}

/// Signature and runtime receiver identities are independent, as in ADR 0272.
/// A local ObjectConstructor-typed value or a homonymous helper is not Object.
pub(super) fn mutation_builtin<'a>(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let (global, property, library) = match declaration.qualified_name.as_ref() {
        "ObjectConstructor.defineProperty" => ("Object", "defineProperty", "lib.es5.d.ts"),
        "ObjectConstructor.defineProperties" => ("Object", "defineProperties", "lib.es5.d.ts"),
        "ObjectConstructor.assign" => ("Object", "assign", "lib.es2015.core.d.ts"),
        "Reflect.set" => ("Reflect", "set", "lib.es2015.reflect.d.ts"),
        "Reflect.defineProperty" => ("Reflect", "defineProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.deleteProperty" => ("Reflect", "deleteProperty", "lib.es2015.reflect.d.ts"),
        _ => return None,
    };
    let member = file.ast.members.iter().find(|member| {
        member.span == file.ast.peel_ts_sugar_span(call.callee)
            && !file.ast.computed_members.contains(&member.span)
            && file.source_text(member.property) == Some(property)
    })?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if file.ast.reference_declaration(receiver).is_some() {
        return None;
    }
    let entity = lookup.entity_at(file.path.as_str(), receiver)?;
    if entity.symbol_unresolved() || entity.symbol.is_empty() {
        return None;
    }
    let symbol = lookup.symbol_at(file.path.as_str(), receiver)?;
    if !symbol.alias_target().is_empty()
        || symbol.declarations().iter().any(|binding| {
            lookup
                .file_by_path(binding.location.path.as_ref())
                .is_some()
        })
    {
        return None;
    }
    let mut bindings = symbol.declarations().iter().filter(|binding| {
        binding.name.as_ref() == global
            && (global == "Reflect" || binding.kind.as_ref() == "variable")
    });
    let binding = bindings.next()?;
    // Object's runtime variable is in es5 even when assign's signature is in es2015.core.
    let expected = std::path::Path::new(declaration.source_file.as_ref())
        .parent()?
        .join(if global == "Object" {
            "lib.es5.d.ts"
        } else {
            library
        });
    (bindings.next().is_none()
        && std::path::Path::new(binding.location.path.as_ref()) == expected
        && declaration.location.path == declaration.source_file
        && std::path::Path::new(declaration.source_file.as_ref())
            .file_name()?
            .to_str()?
            == library)
        .then_some(property)
}
