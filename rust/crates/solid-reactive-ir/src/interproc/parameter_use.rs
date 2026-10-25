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
///
/// `direct[i]` is the `i`th of `owners`' rows so far: its graph-recorded
/// invocations, which a use already describing is not repeated.
pub(super) fn unmodeled_parameter_uses(
    file: &solid_facts::FileFacts,
    owners: &[usize],
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    lookup: &SemanticLookup<'_>,
    direct: &mut [Vec<DirectInvocation>],
    excluded: &mut impl FnMut(&solid_facts::FileFacts, Span) -> Exclusion,
) {
    for (position, owner) in owners.iter().copied().enumerate() {
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
                if direct[position].iter().any(|existing| {
                    existing.invocation.parameter == index
                        && existing.invocation.path == path
                        && existing.invocation.unmodeled == unmodeled
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
                    // Decided once every file's rows are in
                    // ([`DirectInvocation::exclusion`]).
                    dispatch_read_excluded: false,
                    unmodeled,
                };
                direct[position].push(DirectInvocation {
                    exclusion: excluded(file, site),
                    invocation: entry,
                });
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

/// How a member invocation's `dispatch_read_excluded` is decided at its
/// site: never (a default parameter initializer runs at the call), always
/// (its execution role, or an `await` continuation, already decides it), or
/// by whether a callback consumer proves the site tracked or fresh, which
/// another file's consumer can change at any edit
/// (`ReadConsumerSummaries::proves_tracked_or_fresh`), so it is asked again
/// in every build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Exclusion {
    Never,
    Always,
    Consumer,
}

/// A member invocation of a function's own rows (its graph contribution and
/// its unmodeled uses), with how its exclusion is decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DirectInvocation {
    pub(super) invocation: ParameterMemberInvocation,
    pub(super) exclusion: Exclusion,
}

/// [`Exclusion`] at `site` of `file`, asking its execution role once per
/// site: the contexts `semantic_execution_role` reads (`allowed`) are the
/// file's callback spans, computed once, when a site first asks.
pub(super) struct SiteExclusions<'a, 'l> {
    file: &'a solid_facts::FileFacts,
    allowed: Option<Vec<Span>>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolId>,
    lookup: &'a SemanticLookup<'l>,
    decided: HashMap<Span, Exclusion>,
}

impl<'a, 'l> SiteExclusions<'a, 'l> {
    pub(super) fn new(
        file: &'a solid_facts::FileFacts,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolId>,
        lookup: &'a SemanticLookup<'l>,
    ) -> Self {
        Self {
            file,
            allowed: None,
            entities,
            symbol_names,
            lookup,
            decided: HashMap::default(),
        }
    }

    pub(super) fn at(&mut self, site: Span) -> Exclusion {
        if let Some(decided) = self.decided.get(&site) {
            return *decided;
        }
        let file = self.file;
        let lookup = self.lookup;
        let default = file
            .ast
            .functions
            .iter()
            .any(|function| function.span.contains(site) && !function.body.contains(site));
        let decided = if default {
            Exclusion::Never
        } else {
            let allowed = self
                .allowed
                .get_or_insert_with(|| allowed_callback_spans(file, lookup));
            let role = semantic_execution_role(
                file,
                site,
                allowed,
                self.entities,
                self.symbol_names,
                lookup,
            );
            if crate::execution_role::excluded_role_holds(file, site, role, lookup)
                || crate::execution_role::await_continuation_decides_nothing(file, site, lookup)
            {
                Exclusion::Always
            } else {
                Exclusion::Consumer
            }
        };
        self.decided.insert(site, decided);
        decided
    }
}

/// ADR 0320: a call of a helper (not a rendering function) that hands its
/// owner's own parameter, as the whole argument at `index`, to an exact
/// project callee: the owner invokes whatever member the callee invokes on
/// its parameter `index`. `target` is the callee (its file and span), found
/// among the summary nodes when the edge is solved. A forwarded invocation's
/// site is the forwarding call's callee, so that call's context decides
/// whether its dispatch can be reported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ForwardEdge {
    /// The owner's position among its file's summary nodes.
    pub(super) owner: usize,
    pub(super) parameter: usize,
    pub(super) target: (String, Span),
    pub(super) index: usize,
    pub(super) in_owner_body: bool,
    pub(super) site: Span,
    pub(super) exclusion: Exclusion,
}

/// The [`ForwardEdge`]s of `file`'s calls, in call and argument order;
/// `owners` are the summary nodes of `file`'s path.
pub(super) fn forward_edges(
    file: &solid_facts::FileFacts,
    owners: &[usize],
    nodes: &[SummaryNode],
    nodes_by_path: &HashMap<String, Vec<usize>>,
    lookup: &SemanticLookup<'_>,
    excluded: &mut impl FnMut(&solid_facts::FileFacts, Span) -> Exclusion,
) -> Vec<ForwardEdge> {
    let mut edges = Vec::new();
    for call in &file.ast.calls {
        let Some(owner) = containing_summary_function_indexed(
            nodes,
            nodes_by_path,
            file.path.as_str(),
            call.span,
        ) else {
            continue;
        };
        // A call inside a rendering function raises its own obligation at
        // the inner site (ADR 0202); only a helper's call, which raises
        // none, forwards it to its callers.
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
                crate::execution_role::project_callee_at_parameter(file, call, index, lookup)
            else {
                continue;
            };
            if !exact {
                continue;
            }
            let in_owner_body = containing_ast_function(&file.ast, call.span)
                .is_some_and(|function| function.body == nodes[owner].body);
            let Some(owner) = owners.iter().position(|candidate| *candidate == owner) else {
                continue;
            };
            edges.push(ForwardEdge {
                owner,
                parameter,
                target: (callee_file.path.to_string(), callee.span),
                index,
                in_owner_body,
                site: call.callee,
                exclusion: excluded(file, call.callee),
            });
        }
    }
    edges
}

/// A [`ForwardEdge`] as the forwarding loop reads it: its exclusion decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Forwarding {
    pub(super) parameter: usize,
    pub(super) index: usize,
    pub(super) in_owner_body: bool,
    pub(super) site: Span,
    pub(super) excluded: bool,
}

/// One forwarding step: the invocations an owner gains through `edge` from
/// its callee's current ones (`target`), appended to `added` unless `own`
/// or `added` holds them already. An invocation whose dispatch is not
/// reported where it runs is not reported through its callers either.
pub(super) fn forward_through(
    edge: &Forwarding,
    target: &[ParameterMemberInvocation],
    own: &[ParameterMemberInvocation],
    added: &mut Vec<ParameterMemberInvocation>,
) {
    for invocation in target {
        if invocation.parameter != edge.index
            || (invocation.path.is_empty() && !invocation.unmodeled)
            || invocation.dispatch_read_excluded
        {
            continue;
        }
        let entry = ParameterMemberInvocation {
            parameter: edge.parameter,
            path: invocation.path.clone(),
            in_owner_body: edge.in_owner_body && invocation.in_owner_body,
            site: edge.site,
            primitive_builtin: invocation.primitive_builtin,
            dispatch_read_excluded: edge.excluded,
            unmodeled: invocation.unmodeled,
        };
        if !own.contains(&entry) && !added.contains(&entry) {
            added.push(entry);
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
