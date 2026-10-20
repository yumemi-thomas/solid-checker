//! Parameter classification: how a function body uses each of its parameters (member invocations, forwarding, unmodeled uses).

use super::*;

/// ADR 0320: record every use of a non-rendering function's parameter that
/// no member invocation describes. A member read whose value may be called
/// (an extracted method, a destructured member, a member handed to another
/// call) is a possible invocation of that member, resolved at each call
/// site like a call of it. Any other use that may hand the object to code
/// the analysis does not follow -- an alias, a spread, a wrapper, an unknown
/// callee, a closure's forwarding call, `arguments` -- is `unmodeled`: no
/// call site can resolve what it runs. A whole-parameter argument of an exact
/// project callee is the forwarding rule's; tests (`typeof`, comparisons,
/// truthiness) hand nothing on.
pub(super) fn unmodeled_parameter_uses(
    facts: &ProjectFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    lookup: &SemanticLookup<'_>,
    invoked_parameter_members: &mut [Vec<ParameterMemberInvocation>],
    excluded: &mut impl FnMut(&solid_facts::FileFacts, Span) -> bool,
) {
    for file in &facts.files {
        for owner in nodes_by_path
            .get(file.path.as_str())
            .into_iter()
            .flatten()
            .copied()
        {
            let Some(function) = file.ast.function_at(nodes[owner].span) else {
                continue;
            };
            if crate::owners::enclosing_render_function(file, function.body, lookup)
                || lookup.function_component_status(file, function)
                    != crate::indexes::ComponentStatus::No
            {
                continue;
            }
            let arguments_object = file.ast.identifiers.iter().any(|identifier| {
                function.body.contains(identifier.span)
                    && file.source_text(identifier.span) == Some("arguments")
                    && !file.ast.functions.iter().any(|inner| {
                        inner.span != function.span
                            && function.span.contains(inner.span)
                            && inner.kind != solid_facts::ast::FunctionKind::Arrow
                            && inner.span.contains(identifier.span)
                    })
            });
            for (index, parameter) in function.parameters.iter().enumerate() {
                let mut found: Vec<(Vec<String>, bool, Span)> = Vec::new();
                if arguments_object {
                    found.push((Vec::new(), true, function.body));
                }
                if parameter.shape == solid_facts::ast::BindingShape::Identifier {
                    let [name] = parameter.names.as_slice() else {
                        continue;
                    };
                    for (reference, _) in
                        file.ast
                            .reference_declarations
                            .iter()
                            .filter(|(reference, declaration)| {
                                *declaration == name.span && *reference != name.span
                            })
                    {
                        if let Some(entry) = classify_parameter_use(
                            file,
                            nodes,
                            nodes_by_path,
                            owner,
                            *reference,
                            lookup,
                        ) {
                            found.push(entry);
                        }
                    }
                } else {
                    // A destructured parameter: a local that is called, has
                    // a member called or is handed on came out of the
                    // caller's object. A plain `key: local` slot names the
                    // member path; a default, nested pattern or rest does
                    // not, and is unmodeled.
                    found.extend(destructured_parameter_uses(file, parameter));
                }
                for (path, unmodeled, site) in found {
                    if invoked_parameter_members[owner].iter().any(|existing| {
                        existing.parameter == index
                            && existing.path == path
                            && existing.unmodeled == unmodeled
                    }) {
                        continue;
                    }
                    // The use's own context decides, as for a member call
                    // the graph records: a use inside a function proven to run
                    // as an event, a deferred callback or a tracked
                    // computation is not reported through the callers.
                    let nested = containing_ast_function(&file.ast, site)
                        .is_some_and(|inner| inner.span != function.span);
                    let entry = ParameterMemberInvocation {
                        parameter: index,
                        path,
                        in_owner_body: function.body.contains(site) && !nested,
                        site,
                        primitive_builtin: false,
                        dispatch_read_excluded: excluded(file, site),
                        unmodeled,
                    };
                    invoked_parameter_members[owner].push(entry);
                }
            }
        }
    }
}

/// ADR 0320: the member paths a destructured parameter's locals dispatch
/// on, relative to the parameter (`{ o }` with `o.run()` is `["o", "run"]`),
/// or one `unmodeled` entry when a local the pattern binds without a plain
/// `key: local` slot (a default, a nested pattern, a rest, a computed key)
/// is used in any way that may run its members.
pub(super) fn destructured_parameter_uses(
    file: &solid_facts::FileFacts,
    parameter: &solid_facts::ast::BindingFact,
) -> Vec<(Vec<String>, bool, Span)> {
    let mut found = Vec::new();
    for name in &parameter.names {
        let slot = (parameter.shape == solid_facts::ast::BindingShape::Object)
            .then(|| {
                parameter
                    .object_slots
                    .iter()
                    .find(|slot| slot.local.span == name.span)
            })
            .flatten();
        match (slot, local_dispatch_suffixes(file, name.span)) {
            (_, Some(suffixes)) if suffixes.is_empty() => {}
            (Some(slot), Some(suffixes)) => {
                for suffix in suffixes {
                    let mut path = vec![slot.property.to_string()];
                    path.extend(suffix);
                    found.push((path, false, name.span));
                }
            }
            _ => found.push((Vec::new(), true, name.span)),
        }
    }
    found
}

/// How the local declared at `local` may run members of its value: the
/// member paths it dispatches on (empty for the local itself being called or
/// handed to a call; `["run"]` for `local.run()`), or `None` when a use
/// hands it on in a form no path describes (a member read that is itself
/// handed on, a computed member, a chain, a wrapper inside an argument).
pub(super) fn local_dispatch_suffixes(
    file: &solid_facts::FileFacts,
    local: Span,
) -> Option<Vec<Vec<String>>> {
    let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
    let mut suffixes: Vec<Vec<String>> = Vec::new();
    for (reference, _) in file
        .ast
        .reference_declarations
        .iter()
        .filter(|(reference, declaration)| *declaration == local && *reference != local)
    {
        let reference = *reference;
        if let Some(member) = file
            .ast
            .members
            .iter()
            .find(|member| peel(member.object) == reference)
        {
            let called = file
                .ast
                .calls
                .iter()
                .any(|call| peel(call.callee) == member.span);
            let computed = file
                .ast
                .computed_members
                .binary_search(&member.span)
                .is_ok();
            let chained = file
                .ast
                .members
                .iter()
                .any(|outer| peel(outer.object) == member.span);
            if called && !computed {
                let property = file.source_text(member.property)?.to_owned();
                if !suffixes
                    .iter()
                    .any(|suffix| suffix.len() == 1 && suffix[0] == property)
                {
                    suffixes.push(vec![property]);
                }
            } else if chained || computed || value_may_be_invoked(file, member.span) {
                return None;
            }
            continue;
        }
        // Called itself: the slot's member is invoked. Handed to a call or
        // aliased into one: whatever runs on it is out of sight.
        if file
            .ast
            .calls
            .iter()
            .any(|call| peel(call.callee) == reference)
        {
            if !suffixes.iter().any(Vec::is_empty) {
                suffixes.push(Vec::new());
            }
            continue;
        }
        if value_may_be_invoked(file, reference) {
            return None;
        }
        // Wrapped into a literal, spread or operand inside a call argument.
        if file.ast.arguments_containing(reference).next().is_some() {
            return None;
        }
    }
    Some(suffixes)
}

/// Whether the value at `span` (a local or a member read) may be called: it
/// is a callee, an argument of a call, or a binding initializer whose name is
/// one of those.
pub(super) fn value_may_be_invoked(file: &solid_facts::FileFacts, span: Span) -> bool {
    let callee_or_argument = |span: Span| {
        file.ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == span)
            || file.ast.arguments_containing(span).any(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[index].span) == span
            })
    };
    if callee_or_argument(span) {
        return true;
    }
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == span)
        })
        .is_some_and(|binding| {
            binding.names.iter().any(|name| {
                file.ast
                    .reference_declarations
                    .iter()
                    .filter(|(reference, declaration)| {
                        *declaration == name.span && *reference != name.span
                    })
                    .any(|(reference, _)| callee_or_argument(*reference))
            })
        })
}

/// One reference to a parameter: `None` when it is a member call the graph
/// records, a forwarding the forwarding rule follows, a test, a data read or
/// a write of a member; otherwise the member path it may invoke (or empty,
/// with `unmodeled`), and the site.
pub(super) fn classify_parameter_use(
    file: &solid_facts::FileFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    owner: usize,
    reference: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(Vec<String>, bool, Span)> {
    let peel = |span: Span| file.ast.peel_ts_sugar_span(span);
    let direct_owner =
        containing_summary_function_indexed(nodes, nodes_by_path, file.path.as_str(), reference)
            == Some(owner);
    // The object of a member access.
    if let Some(member) = file
        .ast
        .members
        .iter()
        .find(|member| peel(member.object) == reference)
    {
        if file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
            && file
                .ast
                .literal_computed_members
                .iter()
                .all(|literal| literal.span != member.span)
        {
            // A member nothing names: unmodeled only when what it reads may
            // be called; a write, or a data read, runs nothing on it.
            return value_may_be_invoked(file, member.span).then(|| (Vec::new(), true, reference));
        }
        let key = file
            .ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == member.span)
            .map(|literal| literal.key.to_string())
            .or_else(|| file.source_text(member.property).map(str::to_owned))?;
        if file
            .ast
            .assignments
            .iter()
            .any(|assignment| peel(assignment.target) == member.span)
        {
            return None;
        }
        let called = file
            .ast
            .calls
            .iter()
            .any(|call| peel(call.callee) == member.span);
        if called {
            // Recorded by the graph when the call is this function's own.
            return (!direct_owner).then(|| (vec![key], false, member.span));
        }
        // A chain `p.a.b()` is the graph's; a value read that may be called
        // is a possible invocation of the member.
        if file
            .ast
            .members
            .iter()
            .any(|outer| peel(outer.object) == member.span)
        {
            return (!direct_owner).then(|| (Vec::new(), true, reference));
        }
        return (value_may_be_invoked(file, member.span)
            && !lookup.proven_primitive(file, member.span, 6))
        .then(|| (vec![key], false, member.span));
    }
    // A whole argument.
    if let Some((call, index)) = file
        .ast
        .arguments_containing(reference)
        .find(|(call, index)| peel(call.arguments[*index].span) == reference)
    {
        // The forwarding rule follows exactly the calls whose summary owner
        // is this function.
        let forwarding_owner = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            call.span,
        ) == Some(owner);
        let forwarded = forwarding_owner
            && !crate::owners::enclosing_render_function(file, call.span, lookup)
            && !call
                .arguments
                .iter()
                .take(index + 1)
                .any(|argument| argument.spread)
            && crate::execution_role::project_callee_at_parameter(file, call, index, lookup)
                .is_some_and(|(callee_file, callee, exact)| {
                    exact
                        && callee.parameters.get(index).is_some_and(|parameter| {
                            parameter.shape == solid_facts::ast::BindingShape::Identifier
                        })
                        && !callee_file.path.as_str().is_empty()
                });
        // An audited standard-library call that only reads or copies the
        // value invokes nothing on it. (For any other callee `ValueOnly`
        // says only that the argument itself is not called; its members
        // may be.)
        let value_only = lookup
            .resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved
                    .declaration
                    .as_ref()
                    .is_some_and(|declaration| declaration.standard_library)
            })
            .is_some_and(|resolved| {
                let callability = lookup
                    .entity_at(file.path.as_str(), call.arguments[index].span)
                    .and_then(|entity| entity.callability);
                crate::runtime_semantics::argument_behavior(resolved, callability, index)
                    == Some(crate::runtime_semantics::RuntimeArgumentBehavior::ValueOnly)
            });
        // A dialect primitive's handling of its arguments is the dialect's
        // (its callback semantics, its reads of a props object).
        let primitive = lookup.primitive_at_call(file, call.span).is_some();
        return (!forwarded
            && !value_only
            && !primitive
            && !lookup.proven_primitive(file, call.arguments[index].span, 6))
        .then(|| (Vec::new(), true, reference));
    }
    let binding = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| peel(initializer) == reference)
    });
    // An alias: unmodeled when it reaches a call (as callee, argument or a
    // member call's receiver) or is handed on as below; a test or an
    // arithmetic use of it runs nothing.
    if let Some(binding) = binding
        && binding.shape != solid_facts::ast::BindingShape::Object
    {
        let reaches_call = binding.names.iter().any(|name| {
            file.ast
                .reference_declarations
                .iter()
                .filter(|(use_span, declaration)| {
                    *declaration == name.span && *use_span != name.span
                })
                .any(|(use_span, _)| {
                    value_may_be_invoked(file, *use_span)
                        || file
                            .ast
                            .members
                            .iter()
                            .any(|member| peel(member.object) == *use_span)
                        || file.ast.arguments_containing(*use_span).next().is_some()
                })
        });
        return reaches_call.then(|| (Vec::new(), true, reference));
    }
    // A destructuring of it: each member taken that may be called.
    if let Some(binding) = binding
        && binding.shape == solid_facts::ast::BindingShape::Object
    {
        // A local without a plain `key: local` slot (a default, a nested
        // pattern, a rest element, a computed key) may hold any member: any
        // use of it that may run members is unmodeled.
        if binding.names.iter().any(|name| {
            !binding
                .object_slots
                .iter()
                .any(|slot| slot.local.span == name.span)
                && local_dispatch_suffixes(file, name.span)
                    .is_none_or(|suffixes| !suffixes.is_empty())
        }) {
            return Some((Vec::new(), true, reference));
        }
        let mut paths = Vec::new();
        for slot in &binding.object_slots {
            let Some(suffixes) = local_dispatch_suffixes(file, slot.local.span) else {
                return Some((Vec::new(), true, reference));
            };
            for suffix in suffixes {
                let mut path = vec![slot.property.to_string()];
                path.extend(suffix);
                paths.push(path);
            }
        }
        return match paths.as_slice() {
            [] => None,
            [path] => Some((path.clone(), false, reference)),
            _ => Some((Vec::new(), true, reference)),
        };
    }
    // Positions that hand the object to something else -- a conditional's or
    // a non-test logical operand's value, a spread, a property value or
    // array element of a literal -- matter when the alias or wrapper they
    // make is an argument of a call: whatever that callee invokes on it is
    // out of every call site's sight. A wrapper returned or stored is the
    // caller's value or the store's, outside this rule (ADR 0320's open
    // list).
    let handed_on = file.ast.conditional_expressions.iter().any(|conditional| {
        peel(conditional.consequent) == reference || peel(conditional.alternate) == reference
    }) || file.ast.logical_expressions.iter().any(|logical| {
        peel(logical.right) == reference
            || (peel(logical.left) == reference
                && logical.operator != solid_facts::ast::LogicalOperatorKind::And)
    }) || file
        .ast
        .spreads
        .iter()
        .any(|spread| peel(spread.argument) == reference)
        || file
            .ast
            .object_properties
            .iter()
            .any(|property| peel(property.value) == reference)
        || file.ast.array_literals.iter().any(|array| {
            array.contains(reference)
                && *array != reference
                && !file
                    .ast
                    .array_literals
                    .iter()
                    .chain(file.ast.object_get_shapes.iter().map(|shape| &shape.span))
                    .chain(file.ast.calls.iter().map(|call| &call.span))
                    .chain(file.ast.functions.iter().map(|function| &function.span))
                    .any(|inner| {
                        array.contains(*inner) && *inner != *array && inner.contains(reference)
                    })
        });
    let in_argument = file
        .ast
        .arguments_containing(reference)
        .any(|(call, index)| {
            let argument = call.arguments[index].span;
            // The wrapper reaches the argument without passing through a
            // function that is not itself the argument.
            !file.ast.functions.iter().any(|function| {
                argument.contains(function.span)
                    && function.span != peel(argument)
                    && function.span.contains(reference)
            })
        });
    (handed_on && in_argument).then(|| (Vec::new(), true, reference))
}

/// ADR 0320: see the call in the interprocedural build. A forwarded
/// invocation's site is the forwarding call's callee, so the refreshed
/// context of that call decides whether its dispatch can be reported.
pub(super) fn forward_parameter_member_invocations(
    facts: &ProjectFacts,
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    lookup: &SemanticLookup<'_>,
    invoked_parameter_members: &mut [Vec<ParameterMemberInvocation>],
    mut excluded: impl FnMut(&solid_facts::FileFacts, Span) -> bool,
) {
    // Bounded: each round can only lengthen forwarding chains by one.
    for _ in 0..8 {
        let mut added = Vec::new();
        for file in &facts.files {
            for call in &file.ast.calls {
                let Some(owner) = containing_summary_function_indexed(
                    nodes,
                    nodes_by_path,
                    file.path.as_str(),
                    call.span,
                ) else {
                    continue;
                };
                // A call inside a rendering function raises its own
                // obligation at the inner site (ADR 0202); only a helper's
                // call, which raises none, forwards it to its callers.
                if crate::owners::enclosing_render_function(file, call.span, lookup) {
                    continue;
                }
                let Some(owner_function) = file.ast.function_at(nodes[owner].span) else {
                    continue;
                };
                for (index, argument) in call.arguments.iter().enumerate() {
                    if call
                        .arguments
                        .iter()
                        .take(index + 1)
                        .any(|argument| argument.spread)
                    {
                        break;
                    }
                    let value = file.ast.peel_ts_sugar_span(argument.span);
                    let Some(declaration) = file.ast.reference_declaration(value) else {
                        continue;
                    };
                    let Some(parameter) = owner_function.parameters.iter().position(|parameter| {
                        parameter.shape == solid_facts::ast::BindingShape::Identifier
                            && parameter.names.iter().any(|name| name.span == declaration)
                    }) else {
                        continue;
                    };
                    let Some((callee_file, callee, exact)) =
                        crate::execution_role::project_callee_at_parameter(
                            file, call, index, lookup,
                        )
                    else {
                        continue;
                    };
                    if !exact {
                        continue;
                    }
                    let Some(target) = nodes_by_path
                        .get(callee_file.path.as_str())
                        .into_iter()
                        .flatten()
                        .copied()
                        .find(|candidate| nodes[*candidate].span == callee.span)
                    else {
                        continue;
                    };
                    let in_owner_body = containing_ast_function(&file.ast, call.span)
                        .is_some_and(|function| function.body == nodes[owner].body);
                    let forwarding_excluded = excluded(file, call.callee);
                    for invocation in &invoked_parameter_members[target] {
                        // An invocation whose dispatch is not reported where
                        // it runs is not reported through its callers either.
                        if invocation.parameter != index
                            || (invocation.path.is_empty() && !invocation.unmodeled)
                            || invocation.dispatch_read_excluded
                        {
                            continue;
                        }
                        let entry = ParameterMemberInvocation {
                            parameter,
                            path: invocation.path.clone(),
                            in_owner_body: in_owner_body && invocation.in_owner_body,
                            site: call.callee,
                            primitive_builtin: invocation.primitive_builtin,
                            dispatch_read_excluded: forwarding_excluded,
                            unmodeled: invocation.unmodeled,
                        };
                        if !invoked_parameter_members[owner].contains(&entry)
                            && !added.contains(&(owner, entry.clone()))
                        {
                            added.push((owner, entry));
                        }
                    }
                }
            }
        }
        if added.is_empty() {
            return;
        }
        for (owner, entry) in added {
            invoked_parameter_members[owner].push(entry);
        }
    }
}

pub(super) fn equivalent_member_invocations(
    left: &[ParameterMemberInvocation],
    right: &[ParameterMemberInvocation],
) -> bool {
    // Source positions refresh cache dependencies, but are not behavioral
    // differences between two implementations. Repeated equal uses were
    // already coalesced before the source-site field was introduced.
    let same = |a: &ParameterMemberInvocation, b: &ParameterMemberInvocation| {
        a.parameter == b.parameter
            && a.path == b.path
            && a.in_owner_body == b.in_owner_body
            && a.primitive_builtin == b.primitive_builtin
            && a.dispatch_read_excluded == b.dispatch_read_excluded
    };
    left.iter().all(|a| right.iter().any(|b| same(a, b)))
        && right.iter().all(|b| left.iter().any(|a| same(a, b)))
}

/// ADR 0320: whether an argument evaluates to a primitive other than
/// `undefined`, so every member reached through it is a built-in
/// prototype's and no project implementation can run. Proven from the
/// argument's own expression (`SemanticLookup::proven_primitive`), never
/// from its declared type: an assertion, a type predicate or `any` changes
/// the type and not the value.
pub(super) fn argument_is_proven_primitive(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !argument.spread
        && !argument.runtime_type_escape
        && lookup.proven_primitive(file, argument.span, 6)
}

/// Whether the summarized function's `index`th parameter has a default
/// initializer. An unreadable declaration answers yes: the caller then keeps
/// its obligation.
pub(super) fn parameter_has_default(
    node: &SummaryNode,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(file) = lookup.file_by_path(node.path.as_str()) else {
        return true;
    };
    let Some(function) = file.ast.function_at(node.span) else {
        return true;
    };
    function
        .parameters
        .get(index)
        .is_none_or(|parameter| parameter.initializer.is_some())
}

/// Whether Type Facts resolved `callee`, validly and to exactly one
/// declaration, to a standard-library method of a primitive wrapper. Methods of
/// object types (`Date.getTime`, `Array.map`) are excluded: a subclass or a
/// structurally compatible object can override them.
pub(super) fn primitive_builtin_member_call(
    file: &solid_facts::FileFacts,
    callee: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    lookup
        .resolved_callee_call(file, callee)
        .filter(|resolved| {
            resolved.validity == ResolvedCallValidity::Valid
                && resolved.kind == CallKind::Call
                && resolved.targets.is_none()
        })
        .and_then(|resolved| resolved.declaration.as_ref())
        .is_some_and(|declaration| {
            declaration.standard_library
                && declaration
                    .qualified_name
                    .rsplit_once('.')
                    .is_some_and(|(owner, member)| {
                        !member.is_empty()
                            && matches!(owner, "String" | "Number" | "Boolean" | "BigInt")
                    })
        })
}
