//! Resolver construction and the binding, iteration-parameter and input indexes it is built from.

use super::*;

impl Resolver<'_, '_> {
    pub fn new<'a, 'facts>(
        facts: &'a ProjectFacts,
        lookup: &'a SemanticLookup<'facts>,
        accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
        source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
        names: &'a HashMap<SymbolId, SymbolId>,
        functions: HashSet<(String, Span)>,
    ) -> Resolver<'a, 'facts> {
        let mut resolver = Resolver {
            facts,
            lookup,
            accessors,
            source_kinds,
            names,
            functions,
            binding_locations: HashMap::default(),
            parameter_inputs: HashMap::default(),
            declarations: HashMap::default(),
            uses: HashMap::default(),
            providers: HashMap::default(),
            source_origins: HashSet::default(),
            runtime_source_origins: HashSet::default(),
            blocked_contexts: HashSet::default(),
            unkeyed_provider: false,
            member_stability: lookup.member_stability_of(facts),
            receiver_channels_opaque: lookup.program_has_eval()
                || lookup.hazard_present(crate::base::Hazard::CodeEvaluatingCall),
        };
        for file in &facts.files {
            for binding in &file.ast.bindings {
                for name in &binding.names {
                    if let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) {
                        resolver.binding_locations.insert(
                            symbol.clone(),
                            location(file.path.shared(), binding.pattern),
                        );
                        if let Some(display) = file.source_text(name.span) {
                            resolver.declarations.insert(
                                symbol.clone(),
                                (
                                    SymbolId::from(display),
                                    location(file.path.shared(), name.span),
                                ),
                            );
                        }
                    }
                }
            }
        }
        resolver.index_parameter_inputs();
        let forwarded_arguments = resolver
            .parameter_inputs
            .values()
            .map(|(path, span)| location(path.as_str(), *span))
            .collect::<HashSet<_>>();
        // Census each exact reference once. A receiver proves closure only
        // when its selected member is immediately called; a Get that escapes
        // an object/member cannot certify a later invocation.
        for file in &facts.files {
            let array_receivers = file
                .ast
                .calls
                .iter()
                .filter_map(|call| array_iteration_receiver(file, call, lookup))
                .collect::<HashSet<_>>();
            let callees = file
                .ast
                .calls
                .iter()
                .map(|c| file.ast.peel_ts_sugar_span(c.callee))
                .collect::<HashSet<_>>();
            let receivers = file
                .ast
                .members
                .iter()
                .filter(|m| {
                    !file.ast.computed_members.contains(&m.span) && callees.contains(&m.span)
                })
                .map(|m| file.ast.peel_ts_sugar_span(m.object))
                .collect::<HashSet<_>>();
            let returned = file
                .ast
                .returns
                .iter()
                .filter_map(|r| r.argument)
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .collect::<HashSet<_>>();
            // An exact context wrapper may forward its immutable intermediate
            // binding once. This is a handoff to the eventual receiver census,
            // not permission for an arbitrary object return or other escape.
            let context_returns = file
                .ast
                .functions
                .iter()
                .filter(|f| !f.r#async && !f.generator && lookup.function_value_is_current(file, f))
                .filter_map(|f| single_return(file, f))
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .filter(|s| resolver.context_origin(file, *s, 24).is_some())
                .collect::<HashSet<_>>();
            let properties = file
                .ast
                .object_properties
                .iter()
                .filter(|p| p.data)
                .map(|p| file.ast.peel_ts_sugar_span(p.value))
                .collect::<HashSet<_>>();
            let destructured_initializers = file
                .ast
                .bindings
                .iter()
                .filter(|binding| simple_object_binding(binding))
                .filter_map(|binding| binding.initializer)
                .map(|span| file.ast.peel_ts_sugar_span(span))
                .collect::<HashSet<_>>();
            let guard_operands = file
                .ast
                .conditional_tests
                .iter()
                .copied()
                .chain(file.ast.nullish_test_operands.iter().copied())
                .chain(file.ast.coercive_operands.iter().filter_map(|operand| {
                    (operand.kind == ast::CoerciveOperandKind::LogicalNot).then_some(operand.span)
                }))
                .map(|span| file.ast.peel_ts_sugar_span(span))
                .collect::<HashSet<_>>();
            let mut provider_values = HashSet::default();
            let mut context_tags = HashSet::default();
            for element in &file.ast.jsx_elements {
                let target = if let Some(member) = lookup.dialect.context_provider_member() {
                    (element.member_property.and_then(|p| file.source_text(p)) == Some(member))
                        .then_some(element.member_object)
                        .flatten()
                } else {
                    Some(element.name.span)
                };
                let Some(target) = target else { continue };
                if !lookup.is_context_reference(file.path.as_str(), target) {
                    continue;
                }
                // A provider whose tag is not one exact identifier (a
                // namespace member, for one) cannot be attributed to one
                // context binding, so no context's provider census is closed.
                let key = exact_identifier(file, target)
                    .then(|| resolver.binding_key(file, target))
                    .flatten();
                if key.is_none() {
                    resolver.unkeyed_provider = true;
                }
                if let Some(key) = key {
                    let mut values = element
                        .attributes
                        .iter()
                        .filter(|a| file.source_text(a.local_name) == Some("value"));
                    let value = values.next().and_then(|a| a.expression);
                    let valid = values.next().is_none() && element.spreads.is_empty();
                    resolver
                        .providers
                        .entry(key)
                        .or_default()
                        .push(value.filter(|_| valid).map(|v| (file.path.to_string(), v)));
                    if let Some(value) = value.filter(|_| valid) {
                        provider_values.insert(file.ast.peel_ts_sugar_span(value));
                    }
                    context_tags.insert(target);
                    context_tags.insert(element.name.span);
                    context_tags.extend(element.closing_name);
                }
            }
            let mut context_reads = HashSet::default();
            for call in &file.ast.calls {
                if resolver.role(file, call) == Some(ContextRole::Read)
                    && call.arguments.len() == 1
                    && !call.arguments[0].spread
                {
                    context_reads.insert(file.ast.peel_ts_sugar_span(call.arguments[0].span));
                    continue;
                }
                for (position, argument) in call.arguments.iter().enumerate() {
                    let span = file.ast.peel_ts_sugar_span(argument.span);
                    let context = exact_identifier(file, span)
                        && resolver
                            .lookup
                            .binding_at_reference(file.path.as_str(), span)
                            .is_some_and(|(context_file, binding, _)| {
                                binding
                                    .call_initializer
                                    .and_then(|created| context_file.ast.call_at(created))
                                    .is_some_and(|created| {
                                        resolver.role(context_file, created)
                                            == Some(ContextRole::Create)
                                    })
                            });
                    if context && resolver.context_read_only_argument(file, call, position, 8) {
                        context_reads.insert(span);
                    }
                }
            }
            // In a closed program, an exact export or re-export publishes the
            // binding: every visible importer is itself a censused reference.
            // Binding resolution below still has to identify its exact root.
            // It is therefore
            // censused as an export rather than as an escaping reference.
            // An open program keeps the specifier an escape.
            let export_specifiers = file
                .ast
                .exports
                .iter()
                .filter(|e| lookup.program_closed && !e.type_only)
                .flat_map(|e| e.specifiers.iter().filter(|s| !s.type_only))
                .map(|s| s.local.span)
                .collect::<HashSet<_>>();
            for id in file
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                let Some(key) = resolver.binding_key(file, id.span) else {
                    continue;
                };
                if export_specifiers.contains(&id.span) {
                    resolver.uses.entry(key).or_default().export();
                    continue;
                }
                let written = file
                    .ast
                    .assignments
                    .iter()
                    .any(|a| a.target.contains(id.span))
                    || file.ast.deleted_targets.iter().any(|s| s.contains(id.span))
                    || file
                        .ast
                        .iteration_targets
                        .iter()
                        .any(|s| s.contains(id.span));
                let forwarded =
                    forwarded_arguments.contains(&location(file.path.shared(), id.span));
                let context_guard = guard_operands.contains(&id.span)
                    && resolver.context_origin(file, id.span, 24).is_some();
                let uses = resolver.uses.entry(key).or_default();
                uses.object_escaped |= written
                    || !(receivers.contains(&id.span)
                        || provider_values.contains(&id.span)
                        || context_returns.contains(&id.span)
                        || context_guard
                        || destructured_initializers.contains(&id.span)
                        || forwarded);
                uses.array_escaped |= written || !(array_receivers.contains(&id.span) || forwarded);
                uses.callable_escaped |= written
                    || !(callees.contains(&id.span)
                        || returned.contains(&id.span)
                        || properties.contains(&id.span)
                        || provider_values.contains(&id.span));
                uses.context_escaped |= written
                    || !(context_reads.contains(&id.span) || context_tags.contains(&id.span));
                uses.context_value_escaped |= written
                    || !(receivers.contains(&id.span)
                        || callees.contains(&id.span)
                        || provider_values.contains(&id.span)
                        || context_returns.contains(&id.span)
                        || context_guard
                        || destructured_initializers.contains(&id.span)
                        || forwarded);
                uses.destructured_escaped |= written || !callees.contains(&id.span);
            }
            for export in file.ast.exports.iter() {
                for binding in file.ast.exported_bindings(export) {
                    resolver
                        .uses
                        .entry(location(file.path.shared(), binding.pattern))
                        .or_default()
                        .export();
                }
            }
        }
        // A handoff permits no new escape. Join the exact parameter's use
        // census back into its argument binding, including chained handoffs.
        // Index the handoff edges once; the fixed point queries only edges,
        // never rescans the project or repeats binding resolution.
        let mut handoffs = resolver
            .parameter_inputs
            .iter()
            .filter_map(|(parameter, (path, span))| {
                // Only an argument that is exactly a binding hands that binding
                // over; `f(model.read())` passes the read's result, not `model`.
                let file = lookup.file_by_path(path)?;
                if !exact_identifier(file, *span) {
                    return None;
                }
                let argument = resolver.binding_key(file, *span)?;
                Some((argument, parameter.clone(), false))
            })
            .collect::<Vec<_>>();
        for file in &facts.files {
            for binding in file
                .ast
                .bindings
                .iter()
                .filter(|binding| simple_object_binding(binding))
            {
                if let Some(argument) = binding
                    .initializer
                    .map(|span| file.ast.peel_ts_sugar_span(span))
                    .filter(|span| exact_identifier(file, *span))
                    .and_then(|span| resolver.binding_key(file, span))
                {
                    handoffs.push((
                        argument,
                        location(file.path.shared(), binding.pattern),
                        true,
                    ));
                }
            }
        }
        loop {
            let mut changed = false;
            for (argument, receiver, destructured) in &handoffs {
                let mut input = resolver.uses.get(receiver).copied().unwrap_or_default();
                if *destructured {
                    input = ClosedUses {
                        object_escaped: input.destructured_escaped,
                        context_value_escaped: input.destructured_escaped,
                        ..Default::default()
                    };
                }
                let uses = resolver.uses.entry(argument.clone()).or_default();
                for (target, source) in [
                    (&mut uses.object_escaped, input.object_escaped),
                    (&mut uses.array_escaped, input.array_escaped),
                    (&mut uses.callable_escaped, input.callable_escaped),
                    (&mut uses.context_value_escaped, input.context_value_escaped),
                ] {
                    changed |= source && !*target;
                    *target |= source;
                }
            }
            if !changed {
                break;
            }
        }
        for file in &facts.files {
            for binding in &file.ast.bindings {
                if !binding.immutable {
                    continue;
                }
                for name in &binding.names {
                    let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span) else {
                        continue;
                    };
                    if source_has_runtime_witness(
                        symbol,
                        accessors,
                        lookup.entities(),
                        names,
                        lookup,
                    ) {
                        resolver.runtime_source_origins.insert(symbol.clone());
                    } else if !crate::indexes::binding_written(file, name.span)
                        && resolver.source_call(file, name.span).as_ref() == Some(symbol)
                    {
                        resolver.source_origins.insert(symbol.clone());
                    }
                }
            }
        }
        // Join every exact context-derived receiver, including wrapper-return
        // bindings and aliases, before resolving any transported call. One
        // escaped/mutated receiver can replace the shared provider's members.
        let mut blocked_contexts = HashSet::default();
        let object_contexts = resolver
            .providers
            .iter()
            .filter_map(|(context, providers)| {
                let [Some((path, span))] = providers.as_slice() else {
                    return None;
                };
                let file = resolver.lookup.file_by_path(path)?;
                matches!(
                    resolver.value(file, *span, 24, &mut false),
                    Some(Value::Object { .. })
                )
                .then(|| context.clone())
            })
            .collect::<HashSet<_>>();
        for file in facts
            .files
            .iter()
            .filter(|_| !resolver.providers.is_empty())
        {
            for binding in file
                .ast
                .bindings
                .iter()
                .chain(file.ast.functions.iter().flat_map(|f| f.parameters.iter()))
            {
                let Some(context) = binding
                    .initializer
                    .or_else(|| binding.names.first().map(|name| name.span))
                    .and_then(|span| resolver.context_origin(file, span, 24))
                else {
                    continue;
                };
                let key = location(file.path.shared(), binding.pattern);
                let parameter = resolver.parameter_inputs.contains_key(&key);
                let destructured = simple_object_binding(binding);
                if (!binding.immutable && !parameter)
                    || !(binding.shape == ast::BindingShape::Identifier && binding.names.len() == 1
                        || destructured)
                    || resolver.uses.get(&key).is_some_and(|uses| {
                        if destructured {
                            uses.destructured_escaped
                        } else {
                            uses.context_value_escaped
                                || (object_contexts.contains(&context) && uses.object_escaped)
                        }
                    })
                    || binding
                        .names
                        .iter()
                        .any(|name| crate::indexes::binding_written(file, name.span))
                {
                    blocked_contexts.insert(context);
                }
            }
            let bound_values = file
                .ast
                .bindings
                .iter()
                .filter_map(|b| b.initializer)
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .collect::<HashSet<_>>();
            let wrapper_returns = file
                .ast
                .functions
                .iter()
                .filter(|f| {
                    !f.r#async && !f.generator && resolver.lookup.function_value_is_current(file, f)
                })
                .filter_map(|f| single_return(file, f))
                .map(|s| file.ast.peel_ts_sugar_span(s))
                .collect::<HashSet<_>>();
            let callees = file
                .ast
                .calls
                .iter()
                .map(|c| file.ast.peel_ts_sugar_span(c.callee))
                .collect::<HashSet<_>>();
            let receivers = file
                .ast
                .members
                .iter()
                .filter(|m| {
                    !file.ast.computed_members.contains(&m.span) && callees.contains(&m.span)
                })
                .map(|m| file.ast.peel_ts_sugar_span(m.object))
                .collect::<HashSet<_>>();
            // An unbound result can mutate/escape the same shared object too.
            // Exact wrapper returns and initializer handoffs are checked at
            // their eventual bindings; direct invocations remain closed.
            for call in &file.ast.calls {
                if bound_values.contains(&call.span)
                    || wrapper_returns.contains(&call.span)
                    || callees.contains(&call.span)
                    || receivers.contains(&call.span)
                    || forwarded_arguments.contains(&location(file.path.shared(), call.span))
                {
                    continue;
                }
                if let Some(context) = resolver.context_origin(file, call.span, 24) {
                    blocked_contexts.insert(context);
                }
            }
        }
        resolver.blocked_contexts = blocked_contexts;
        resolver
    }

    pub(super) fn binding_key(&self, file: &FileFacts, span: Span) -> Option<Location> {
        if let Some((declared, binding, _)) =
            self.lookup.binding_at_reference(file.path.as_str(), span)
        {
            return Some(location(declared.path.shared(), binding.pattern));
        }
        if let Some(key) = self
            .lookup
            .entities()
            .at(file.path.as_str(), span)
            .and_then(|s| self.binding_locations.get(s))
        {
            return Some(key.clone());
        }
        let declaration = file.ast.reference_declaration(span)?;
        let binding = file
            .ast
            .bindings
            .iter()
            .chain(file.ast.functions.iter().flat_map(|f| f.parameters.iter()))
            .find(|b| b.names.iter().any(|n| n.span == declaration))?;
        Some(location(file.path.shared(), binding.pattern))
    }

    /// One source-local function value, one exact identifier call, and no
    /// unaccounted reference. Do not use call-site count alone: a stored or
    /// aliased value can have an invocation missing from that count.
    fn index_parameter_inputs(&mut self) {
        let mut local_functions = HashMap::default();
        for file in &self.facts.files {
            for function in &file.ast.functions {
                if !function.parameters.is_empty()
                    && let Some(name) = crate::owners::function_binding_name(file, function)
                {
                    local_functions.insert(
                        location(file.path.shared(), name.span),
                        (file.path.to_string(), function.span),
                    );
                }
            }
        }
        if local_functions.is_empty() {
            return;
        }
        let mut references = HashMap::<(String, Span), HashSet<Location>>::default();
        let mut argument_observers = HashSet::default();
        for file in &self.facts.files {
            for id in file
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                if matches!(file.source_text(id.span), Some("arguments" | "eval")) {
                    for function in file.ast.functions_body_containing(id.span) {
                        argument_observers.insert((file.path.to_string(), function.span));
                    }
                }
                let target = file.ast.reference_declaration(id.span).and_then(|span| {
                    local_functions
                        .get(&location(file.path.shared(), span))
                        .cloned()
                });
                if let Some(target) = target {
                    references
                        .entry(target)
                        .or_default()
                        .insert(location(file.path.shared(), id.span));
                }
            }
        }
        for file in &self.facts.files {
            for function in &file.ast.functions {
                if function.parameters.is_empty()
                    || function.generator
                    || function.rest_parameter
                    || !self.lookup.function_value_is_current(file, function)
                    || function.parameters.iter().any(|p| {
                        p.initializer.is_some()
                            || p.shape != ast::BindingShape::Identifier
                            || p.names.len() != 1
                            || p.pattern != p.names[0].span
                    })
                    || argument_observers.contains(&(file.path.to_string(), function.span))
                {
                    continue;
                }
                let Some(name) = crate::owners::function_binding_name(file, function) else {
                    continue;
                };
                let exported =
                    self.lookup
                        .typescript_file(file.path.as_str())
                        .is_some_and(|typescript| {
                            typescript.functions.iter().any(|f| {
                                f.exported
                                    && f.body.start_byte == u64::from(function.body.start)
                                    && f.body.end_byte == u64::from(function.body.end)
                            })
                        })
                        || file.ast.exports.iter().any(|export| {
                            (export.span.contains(function.span)
                                && !file.ast.functions.iter().any(|outer| {
                                    outer.span != function.span
                                        && export.span.contains(outer.span)
                                        && outer.span.contains(function.span)
                                }))
                                || export.specifiers.iter().chain(&export.declarations).any(
                                    |specifier| {
                                        !specifier.type_only
                                            && (specifier.local.span == name.span
                                                || file
                                                    .ast
                                                    .reference_declaration(specifier.local.span)
                                                    == Some(name.span))
                                    },
                                )
                        });
                // Reuse ADR 0193's open-program boundary, and keep this first
                // parameter channel narrower: even app exports are refused.
                if exported && !self.lookup.program_closed {
                    continue;
                }
                if exported {
                    continue;
                }
                let target = (file.path.to_string(), function.span);
                let Some(uses) = references.get(&target) else {
                    continue;
                };
                if uses.len() != 1 {
                    continue;
                }
                let site = uses.iter().next().expect("one reference");
                let sites = self
                    .lookup
                    .function_call_sites(file.path.as_str(), function.span);
                let [(caller, callee)] = sites.as_slice() else {
                    continue;
                };
                let callee = caller.ast.peel_ts_sugar_span(*callee);
                if *site != location(caller.path.shared(), callee)
                    || caller.path != file.path
                    || caller.ast.reference_declaration(callee) != Some(name.span)
                {
                    continue;
                }
                let Some(call) = caller
                    .ast
                    .calls
                    .iter()
                    .find(|c| caller.ast.peel_ts_sugar_span(c.callee) == callee)
                else {
                    continue;
                };
                if call.construct
                    || call.arguments.iter().any(|argument| argument.spread)
                    || !self
                        .lookup
                        .resolved_callee_call(caller, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Valid
                        })
                {
                    continue;
                }
                // Include the producer reference census too: a reference in
                // excluded/missing source is never evidence of closure.
                if self
                    .lookup
                    .symbol_at(file.path.as_str(), name.span)
                    .is_none_or(|symbol| {
                        symbol.references().any(|reference| {
                            reference.path.as_ref() != caller.path.as_str()
                                || reference.start_byte != u64::from(callee.start)
                                || reference.end_byte != u64::from(callee.end)
                        })
                    })
                {
                    continue;
                }
                for (position, parameter) in function.parameters.iter().enumerate() {
                    let name = &parameter.names[0];
                    if crate::indexes::binding_written(file, name.span) {
                        continue;
                    }
                    let Some(argument) = call.arguments.get(position) else {
                        continue;
                    };
                    let key = location(file.path.shared(), parameter.pattern);
                    self.parameter_inputs.insert(
                        key.clone(),
                        (
                            caller.path.to_string(),
                            caller.ast.peel_ts_sugar_span(argument.span),
                        ),
                    );
                    if let Some(symbol) = self.lookup.entities().at(file.path.as_str(), name.span) {
                        self.binding_locations.insert(symbol.clone(), key);
                    }
                }
            }
        }
    }
}
