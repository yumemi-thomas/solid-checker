//! Resolver construction (S7 design §3). Each stage is a file's rows over
//! any [`World`]:
//! - [`binding_rows`]: each named binding's symbol, pattern and declaration;
//! - [`parameter_rows`]: the exact argument each admitted parameter receives
//!   (the parameter key overwrites its symbol's binding location);
//! - [`census_rows`]: the receiver census (each binding key's escapes), the
//!   context providers, and the handoff edges;
//! - [`origin_rows`]: the source origins and the contexts the file's
//!   receivers block, over a world with no blocked context.
//!
//! The fresh build ([`Resolver::fresh`]) runs them over the lookup world and
//! folds them in file order, as one pass did before. The memo build
//! ([`super::resolver_memos`]) folds the same rows from per-file memos.

use super::*;

impl<'a, 'facts: 'a> Resolver<'a, 'facts> {
    pub fn new(
        facts: &'a ProjectFacts,
        lookup: &'a SemanticLookup<'facts>,
        accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
        source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
        names: &'a HashMap<SymbolId, SymbolId>,
        functions: Arc<HashSet<(String, Span)>>,
    ) -> Resolver<'a, 'facts> {
        let empty = Self::empty(facts, lookup, accessors, source_kinds, names, functions);
        let mut timer = ResolverTimer::start();
        let resolver = super::resolver_memos::build(empty, &mut timer);
        timer.emit();
        resolver
    }

    /// A resolver whose tables are empty, for the stages to fill.
    pub(super) fn empty(
        facts: &'a ProjectFacts,
        lookup: &'a SemanticLookup<'facts>,
        accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
        source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
        names: &'a HashMap<SymbolId, SymbolId>,
        functions: Arc<HashSet<(String, Span)>>,
    ) -> Resolver<'a, 'facts> {
        Resolver {
            facts,
            lookup,
            accessors,
            source_kinds,
            names,
            functions,
            binding_locations: Arc::default(),
            parameter_inputs: HashMap::default(),
            declarations: Arc::default(),
            uses: Arc::default(),
            providers: HashMap::default(),
            source_origins: HashSet::default(),
            runtime_source_origins: HashSet::default(),
            blocked_contexts: HashSet::default(),
            unkeyed_provider: false,
            member_stability: lookup.member_stability(),
            receiver_channels_opaque: lookup.program_has_eval()
                || lookup.hazard_present(crate::base::Hazard::CodeEvaluatingCall),
        }
    }

    /// A resolver with the same tables (the functions it was built for aside),
    /// for a build that compares two constructions.
    pub(super) fn sibling(&self) -> Resolver<'a, 'facts> {
        Self::empty(
            self.facts,
            self.lookup,
            self.accessors,
            self.source_kinds,
            self.names,
            Arc::default(),
        )
    }

    /// Every stage over the lookup world, folded in file order: the
    /// construction a build without a usable memo store runs, and the one
    /// a checked build compares the memo build with.
    pub(super) fn fresh(mut self, timer: &mut ResolverTimer) -> Self {
        let facts = self.facts;
        let bindings = facts
            .files
            .iter()
            .map(|file| binding_rows(&self, file))
            .collect::<Vec<_>>();
        {
            let binding_locations = Arc::make_mut(&mut self.binding_locations);
            let declarations = Arc::make_mut(&mut self.declarations);
            for (file, rows) in facts.files.iter().zip(&bindings) {
                apply_binding_rows(file, rows, binding_locations, declarations);
            }
        }
        timer.lap("bindings");
        let parameters = facts
            .files
            .iter()
            .map(|file| parameter_rows(&self, file))
            .collect::<Vec<_>>();
        for (file, rows) in facts.files.iter().zip(&parameters) {
            for (pattern, argument) in &rows.inputs {
                self.parameter_inputs.insert(
                    location(file.path.shared(), *pattern),
                    (file.path.to_string(), *argument),
                );
            }
            let binding_locations = Arc::make_mut(&mut self.binding_locations);
            for (symbol, pattern) in &rows.symbols {
                binding_locations.insert(symbol.clone(), location(file.path.shared(), *pattern));
            }
        }
        timer.lap("parameterInputs");
        let forwarded_arguments = self
            .parameter_inputs
            .values()
            .map(|(path, span)| location(path.as_str(), *span))
            .collect::<HashSet<_>>();
        let census = facts
            .files
            .iter()
            .map(|file| {
                census_rows(&self, file, &|span| {
                    forwarded_arguments.contains(&location(file.path.shared(), span))
                })
            })
            .collect::<Vec<_>>();
        let mut edges = Vec::new();
        {
            let uses = Arc::make_mut(&mut self.uses);
            for rows in &census {
                for (key, found) in &rows.uses {
                    crate::base::fixpoint::Lattice::join(
                        uses.entry(key.clone()).or_default(),
                        found,
                    );
                }
                for (context, provider) in &rows.providers {
                    self.providers
                        .entry(context.clone())
                        .or_default()
                        .push(provider.clone());
                }
                self.unkeyed_provider |= rows.unkeyed;
            }
        }
        // Only an argument that is exactly a binding hands that binding
        // over; `f(model.read())` passes the read's result, not `model`.
        edges.extend(
            self.parameter_inputs
                .iter()
                .filter_map(|(parameter, (path, span))| {
                    let file = self.lookup.file_by_path(path)?;
                    if !exact_identifier(file, *span) {
                        return None;
                    }
                    let argument = self.binding_key(file, *span)?;
                    Some((argument, parameter.clone(), false))
                }),
        );
        edges.extend(census.into_iter().flat_map(|rows| rows.edges));
        timer.lap("census");
        let (definitions, arguments) =
            handoff_definitions(edges, |key| self.uses.get(key).copied());
        let mut solver = crate::base::fixpoint::Monotone::default();
        solver.replace(definitions, &ClosedUses::handed_back);
        let joined = arguments
            .into_iter()
            .map(|argument| {
                let uses = solver.value(&argument).copied().unwrap_or_default();
                (argument, uses)
            })
            .collect::<Vec<_>>();
        Arc::make_mut(&mut self.uses).extend(joined);
        timer.lap("handoffs");
        // Join every exact context-derived receiver, including wrapper-return
        // bindings and aliases, before resolving any transported call. One
        // escaped/mutated receiver can replace the shared provider's members.
        // The origins and this census see no blocked context.
        let object_contexts = self
            .providers
            .keys()
            .filter(|context| object_context(&self, context))
            .cloned()
            .collect::<HashSet<_>>();
        let origins = facts
            .files
            .iter()
            .map(|file| {
                origin_rows(
                    &self,
                    file,
                    &|span| forwarded_arguments.contains(&location(file.path.shared(), span)),
                    &mut |context| object_contexts.contains(context),
                )
            })
            .collect::<Vec<_>>();
        for rows in origins {
            self.runtime_source_origins.extend(rows.runtime);
            self.source_origins.extend(rows.source);
            self.blocked_contexts.extend(rows.blocked);
        }
        timer.lap("origins");
        self
    }
}

/// A file's named bindings, in binding order: each name's symbol with its
/// binding's pattern, and, for a name with text, its declaration (the
/// display text and the name). Folded last-wins in file order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct BindingRows {
    pub(super) locations: Vec<(SymbolId, Span)>,
    pub(super) declarations: Vec<(SymbolId, SymbolId, Span)>,
}

pub(super) fn binding_rows<'w, W: World<'w>>(w: &W, file: &'w FileFacts) -> BindingRows {
    let mut rows = BindingRows::default();
    for binding in &file.ast.bindings {
        for name in &binding.names {
            if let Some(symbol) = w.symbol_at(file, name.span) {
                rows.locations.push((symbol.clone(), binding.pattern));
                if let Some(display) = file.source_text(name.span) {
                    rows.declarations
                        .push((symbol.clone(), SymbolId::from(display), name.span));
                }
            }
        }
    }
    rows
}

pub(super) fn apply_binding_rows(
    file: &FileFacts,
    rows: &BindingRows,
    binding_locations: &mut HashMap<SymbolId, Location>,
    declarations: &mut HashMap<SymbolId, (SymbolId, Location)>,
) {
    for (symbol, pattern) in &rows.locations {
        binding_locations.insert(symbol.clone(), location(file.path.shared(), *pattern));
    }
    for (symbol, display, name) in &rows.declarations {
        declarations.insert(
            symbol.clone(),
            (display.clone(), location(file.path.shared(), *name)),
        );
    }
}

/// The parameters of a file's functions that receive one exact argument, in
/// file order: each parameter's pattern with the argument its one call
/// passes (the caller is the same file), and the parameter's symbol, whose
/// binding location the parameter key overwrites.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ParameterRows {
    pub(super) inputs: Vec<(Span, Span)>,
    pub(super) symbols: Vec<(SymbolId, Span)>,
}

/// One source-local function value, one exact identifier call, and no
/// unaccounted reference. Do not use call-site count alone: a stored or
/// aliased value can have an invocation missing from that count. The
/// reference census is the file's own: a caller must be in the same file.
pub(super) fn parameter_rows<'w, W: World<'w>>(w: &W, file: &'w FileFacts) -> ParameterRows {
    let mut rows = ParameterRows::default();
    let mut local_functions = HashMap::default();
    for function in &file.ast.functions {
        if !function.parameters.is_empty()
            && let Some(name) = crate::owners::function_binding_name(file, function)
        {
            local_functions.insert(name.span, function.span);
        }
    }
    if local_functions.is_empty() {
        return rows;
    }
    let mut references = HashMap::<Span, HashSet<Span>>::default();
    let mut argument_observers = HashSet::default();
    for id in file
        .ast
        .identifiers
        .iter()
        .filter(|id| id.role == ast::IdentifierRole::Reference)
    {
        if matches!(file.source_text(id.span), Some("arguments" | "eval")) {
            for function in file.ast.functions_owning(id.span) {
                argument_observers.insert(function.span);
            }
        }
        let target = file
            .ast
            .reference_declaration(id.span)
            .and_then(|span| local_functions.get(&span).copied());
        if let Some(target) = target {
            references.entry(target).or_default().insert(id.span);
        }
    }
    for function in &file.ast.functions {
        if function.parameters.is_empty()
            || function.generator
            || function.rest_parameter
            || !crate::indexes::function_value_is_current_in(file, function)
            || function.parameters.iter().any(|p| {
                p.initializer.is_some()
                    || p.shape != ast::BindingShape::Identifier
                    || p.names.len() != 1
                    || p.pattern != p.names[0].span
            })
            || argument_observers.contains(&function.span)
        {
            continue;
        }
        let Some(name) = crate::owners::function_binding_name(file, function) else {
            continue;
        };
        // Reuse ADR 0193's open-program boundary, and keep this first
        // parameter channel narrower: even app exports are refused (the
        // TypeScript half is `parameter_typefacts_admit`).
        let exported = file.ast.exports.iter().any(|export| {
            (export.span.contains(function.span)
                && !file.ast.functions.iter().any(|outer| {
                    outer.span != function.span
                        && export.span.contains(outer.span)
                        && outer.span.contains(function.span)
                }))
                || export
                    .specifiers
                    .iter()
                    .chain(&export.declarations)
                    .any(|specifier| {
                        !specifier.type_only
                            && (specifier.local.span == name.span
                                || file.ast.reference_declaration(specifier.local.span)
                                    == Some(name.span))
                    })
        });
        if exported {
            continue;
        }
        let Some(uses) = references.get(&function.span) else {
            continue;
        };
        if uses.len() != 1 {
            continue;
        }
        let site = *uses.iter().next().expect("one reference");
        let sites = w.function_call_sites(file, function.span);
        let [(caller, callee)] = sites.as_slice() else {
            continue;
        };
        let caller = *caller;
        let callee = caller.ast.peel_ts_sugar_span(*callee);
        if caller.path != file.path
            || site != callee
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
            || !w.parameter_typefacts_admit(file, function, name.span, caller, call, callee)
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
            rows.inputs.push((
                parameter.pattern,
                caller.ast.peel_ts_sugar_span(argument.span),
            ));
            if let Some(symbol) = w.symbol_at(file, name.span) {
                rows.symbols.push((symbol.clone(), parameter.pattern));
            }
        }
    }
    rows
}

/// A handoff: the argument binding's key, the receiving binding's key, and
/// whether the receiver destructures the argument.
pub(super) type HandoffEdge = (Location, Location, bool);

/// A file's receiver census: each binding key's escapes (joined within the
/// file, keys in first-use order), its context providers in element order,
/// whether some provider is not attributed to one binding, and the
/// destructuring handoffs it declares.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CensusRows {
    pub(super) uses: Vec<(Location, ClosedUses)>,
    pub(super) providers: Vec<(Location, Option<(String, Span)>)>,
    pub(super) unkeyed: bool,
    pub(super) edges: Vec<HandoffEdge>,
}

/// Census each exact reference once. A receiver proves closure only when
/// its selected member is immediately called; a Get that escapes an
/// object/member cannot certify a later invocation. `forwarded` says whether
/// a span of the file is an argument some parameter receives.
pub(super) fn census_rows<'w, W: World<'w>>(
    w: &W,
    file: &'w FileFacts,
    forwarded: &dyn Fn(Span) -> bool,
) -> CensusRows {
    let mut rows = CensusRows::default();
    let mut keys = HashMap::<Location, usize>::default();
    let mut uses_of = |rows: &mut CensusRows, key: Location| -> usize {
        *keys.entry(key.clone()).or_insert_with(|| {
            rows.uses.push((key, ClosedUses::default()));
            rows.uses.len() - 1
        })
    };
    let array_receivers = file
        .ast
        .calls
        .iter()
        .filter_map(|call| w.array_iteration_receiver(file, call))
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
        .filter(|m| !file.ast.computed_members.contains(&m.span) && callees.contains(&m.span))
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
        .filter(|f| {
            !f.r#async && !f.generator && crate::indexes::function_value_is_current_in(file, f)
        })
        .filter_map(|f| single_return(file, f))
        .map(|s| file.ast.peel_ts_sugar_span(s))
        .filter(|s| w.context_origin(file, *s, 24).is_some())
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
        let target = if let Some(member) = w.dialect().context_provider_member() {
            (element.member_property.and_then(|p| file.source_text(p)) == Some(member))
                .then_some(element.member_object)
                .flatten()
        } else {
            Some(element.name.span)
        };
        let Some(target) = target else { continue };
        if !w.is_context_reference(file, target) {
            continue;
        }
        // A provider whose tag is not one exact identifier (a namespace
        // member, for one) cannot be attributed to one context binding, so
        // no context's provider census is closed.
        let key = exact_identifier(file, target)
            .then(|| w.binding_key(file, target))
            .flatten();
        if key.is_none() {
            rows.unkeyed = true;
        }
        if let Some(key) = key {
            let mut values = element
                .attributes
                .iter()
                .filter(|a| file.source_text(a.local_name) == Some("value"));
            let value = values.next().and_then(|a| a.expression);
            let valid = values.next().is_none() && element.spreads.is_empty();
            rows.providers.push((
                key,
                value.filter(|_| valid).map(|v| (file.path.to_string(), v)),
            ));
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
        if w.role(file, call) == Some(ContextRole::Read)
            && call.arguments.len() == 1
            && !call.arguments[0].spread
        {
            context_reads.insert(file.ast.peel_ts_sugar_span(call.arguments[0].span));
            continue;
        }
        for (position, argument) in call.arguments.iter().enumerate() {
            let span = file.ast.peel_ts_sugar_span(argument.span);
            let context = exact_identifier(file, span)
                && w.binding_at_reference(file, span)
                    .is_some_and(|(context_file, binding, _)| {
                        binding
                            .call_initializer
                            .and_then(|created| context_file.ast.call_at(created))
                            .is_some_and(|created| {
                                w.role(context_file, created) == Some(ContextRole::Create)
                            })
                    });
            if context && w.context_read_only_argument(file, call, position, 8) {
                context_reads.insert(span);
            }
        }
    }
    // In a closed program, an exact export or re-export publishes the
    // binding: every visible importer is itself a censused reference.
    // Binding resolution below still has to identify its exact root.
    // It is therefore censused as an export rather than as an escaping
    // reference. An open program keeps the specifier an escape.
    let program_closed = w.program_closed();
    let export_specifiers = file
        .ast
        .exports
        .iter()
        .filter(|e| program_closed && !e.type_only)
        .flat_map(|e| e.specifiers.iter().filter(|s| !s.type_only))
        .map(|s| s.local.span)
        .collect::<HashSet<_>>();
    for id in file
        .ast
        .identifiers
        .iter()
        .filter(|id| id.role == ast::IdentifierRole::Reference)
    {
        let Some(key) = w.binding_key(file, id.span) else {
            continue;
        };
        if export_specifiers.contains(&id.span) {
            let index = uses_of(&mut rows, key);
            rows.uses[index].1.export();
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
        let forwarded = forwarded(id.span);
        let context_guard =
            guard_operands.contains(&id.span) && w.context_origin(file, id.span, 24).is_some();
        let index = uses_of(&mut rows, key);
        let uses = &mut rows.uses[index].1;
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
        uses.context_escaped |=
            written || !(context_reads.contains(&id.span) || context_tags.contains(&id.span));
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
            let index = uses_of(&mut rows, location(file.path.shared(), binding.pattern));
            rows.uses[index].1.export();
        }
    }
    // A handoff permits no new escape: a destructuring of an exact binding
    // hands that binding to the pattern.
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
            .and_then(|span| w.binding_key(file, span))
        {
            rows.edges.push((
                argument,
                location(file.path.shared(), binding.pattern),
                true,
            ));
        }
    }
    rows
}

/// The parameter handoffs of a file's [`ParameterRows`]: only an argument
/// that is exactly a binding hands that binding over; `f(model.read())`
/// passes the read's result, not `model`.
pub(super) fn parameter_edges<'w, W: World<'w>>(
    w: &W,
    file: &'w FileFacts,
    rows: &ParameterRows,
) -> Vec<HandoffEdge> {
    rows.inputs
        .iter()
        .filter_map(|(pattern, argument)| {
            if !exact_identifier(file, *argument) {
                return None;
            }
            let key = w.binding_key(file, *argument)?;
            Some((key, location(file.path.shared(), *pattern), false))
        })
        .collect()
}

/// A location's total order, for canonical definitions.
pub(super) fn location_order(location: &Location) -> (&str, u64, u64) {
    (&location.path, location.start_byte, location.end_byte)
}

/// The handoff join's definitions: a node per binding on a handoff, its
/// direct census as input and its receivers as predecessors, all in key
/// order; and the argument nodes (those with a receiver), whose joined
/// census replaces their direct one. The least fixed point of
/// `uses(argument) ⊔= handed_back(uses(receiver))` joins each receiver's
/// census back into its argument, chained handoffs included.
#[allow(clippy::type_complexity)]
pub(super) fn handoff_definitions(
    mut edges: Vec<HandoffEdge>,
    direct: impl Fn(&Location) -> Option<ClosedUses>,
) -> (
    Vec<(
        Location,
        crate::base::fixpoint::Definition<Location, ClosedUses, bool>,
    )>,
    Vec<Location>,
) {
    edges.sort_by(|left, right| {
        location_order(&left.0)
            .cmp(&location_order(&right.0))
            .then_with(|| location_order(&left.1).cmp(&location_order(&right.1)))
            .then(left.2.cmp(&right.2))
    });
    edges.dedup();
    let mut nodes = edges
        .iter()
        .flat_map(|(argument, receiver, _)| [argument, receiver])
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| location_order(left).cmp(&location_order(right)));
    nodes.dedup();
    let mut next_edge = 0;
    let definitions = nodes
        .into_iter()
        .map(|node| {
            let mut preds = Vec::new();
            while let Some((argument, receiver, destructured)) = edges.get(next_edge)
                && argument == node
            {
                preds.push((receiver.clone(), *destructured));
                next_edge += 1;
            }
            (
                node.clone(),
                crate::base::fixpoint::Definition {
                    input: direct(node).unwrap_or_default(),
                    preds,
                },
            )
        })
        .collect::<Vec<_>>();
    let arguments = definitions
        .iter()
        .filter(|(_, definition)| !definition.preds.is_empty())
        .map(|(node, _)| node.clone())
        .collect::<Vec<_>>();
    (definitions, arguments)
}

/// Whether the one provider of `context` renders an object value: its
/// receivers must then not escape the object, beside the context value.
pub(super) fn object_context<'w, W: World<'w>>(w: &W, context: &Location) -> bool {
    let Some((path, span)) = w
        .providers_of(context)
        .and_then(|providers| match &*providers {
            [Some(provider)] => Some(provider.clone()),
            _ => None,
        })
    else {
        return false;
    };
    w.file(&path).is_some_and(|file| {
        matches!(
            w.value(file, span, 24, &mut false),
            Some(Value::Object { .. })
        )
    })
}

/// A file's source origins (each immutable binding name's symbol a source
/// call or a runtime witness proves) and the contexts its receivers block.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct OriginRows {
    pub(super) runtime: Vec<SymbolId>,
    pub(super) source: Vec<SymbolId>,
    pub(super) blocked: Vec<Location>,
}

/// The origins and the blocked contexts of a file, asked of a world with no
/// blocked context. `object_context` answers [`object_context`].
pub(super) fn origin_rows<'w, W: World<'w>>(
    w: &W,
    file: &'w FileFacts,
    forwarded: &dyn Fn(Span) -> bool,
    object_context: &mut dyn FnMut(&Location) -> bool,
) -> OriginRows {
    let mut rows = OriginRows::default();
    for binding in &file.ast.bindings {
        if !binding.immutable {
            continue;
        }
        for name in &binding.names {
            let Some(symbol) = w.symbol_at(file, name.span) else {
                continue;
            };
            if w.source_has_runtime_witness(symbol) {
                rows.runtime.push(symbol.clone());
            } else if !crate::indexes::binding_written(file, name.span)
                && w.source_call(file, name.span).as_ref() == Some(symbol)
            {
                rows.source.push(symbol.clone());
            }
        }
    }
    if !w.any_providers() {
        return rows;
    }
    for binding in file
        .ast
        .bindings
        .iter()
        .chain(file.ast.functions.iter().flat_map(|f| f.parameters.iter()))
    {
        let Some(context) = binding
            .initializer
            .or_else(|| binding.names.first().map(|name| name.span))
            .and_then(|span| w.context_origin(file, span, 24))
        else {
            continue;
        };
        let key = location(file.path.shared(), binding.pattern);
        let parameter = w.parameter_input(file, binding.pattern).is_some();
        let destructured = simple_object_binding(binding);
        if (!binding.immutable && !parameter)
            || !(binding.shape == ast::BindingShape::Identifier && binding.names.len() == 1
                || destructured)
            || w.uses_of(&key).is_some_and(|uses| {
                if destructured {
                    uses.destructured_escaped
                } else {
                    uses.context_value_escaped || (object_context(&context) && uses.object_escaped)
                }
            })
            || binding
                .names
                .iter()
                .any(|name| crate::indexes::binding_written(file, name.span))
        {
            rows.blocked.push(context);
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
            !f.r#async && !f.generator && crate::indexes::function_value_is_current_in(file, f)
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
        .filter(|m| !file.ast.computed_members.contains(&m.span) && callees.contains(&m.span))
        .map(|m| file.ast.peel_ts_sugar_span(m.object))
        .collect::<HashSet<_>>();
    // An unbound result can mutate/escape the same shared object too.
    // Exact wrapper returns and initializer handoffs are checked at their
    // eventual bindings; direct invocations remain closed.
    for call in &file.ast.calls {
        if bound_values.contains(&call.span)
            || wrapper_returns.contains(&call.span)
            || callees.contains(&call.span)
            || receivers.contains(&call.span)
            || forwarded(call.span)
        {
            continue;
        }
        if let Some(context) = w.context_origin(file, call.span, 24) {
            rows.blocked.push(context);
        }
    }
    rows
}

/// `SOLID_CHECKER_TIMINGS`: the resolver's own time per build, by stage,
/// emitted as one `reactiveIrResolver` line (the interprocedural graph's
/// timing covers the resolver, transport, refresh and merge together).
pub(super) struct ResolverTimer {
    started: Option<std::time::Instant>,
    lap: Option<std::time::Instant>,
    stages: Vec<(&'static str, u64)>,
    counts: Vec<(&'static str, u64)>,
}

impl ResolverTimer {
    fn start() -> Self {
        let started = std::env::var_os("SOLID_CHECKER_TIMINGS").map(|_| std::time::Instant::now());
        Self {
            started,
            lap: started,
            stages: Vec::new(),
            counts: Vec::new(),
        }
    }

    /// A timer that records nothing.
    pub(super) fn off() -> Self {
        Self {
            started: None,
            lap: None,
            stages: Vec::new(),
            counts: Vec::new(),
        }
    }

    fn nanos(duration: std::time::Duration) -> u64 {
        u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
    }

    /// Ends the stage `stage` (stages repeat: their times add up).
    pub(super) fn lap(&mut self, stage: &'static str) {
        if let Some(lap) = self.lap {
            let now = std::time::Instant::now();
            let ns = Self::nanos(now - lap);
            match self.stages.iter_mut().find(|(name, _)| *name == stage) {
                Some((_, total)) => *total += ns,
                None => self.stages.push((stage, ns)),
            }
            self.lap = Some(now);
        }
    }

    /// Records a count beside the stage times.
    pub(super) fn count(&mut self, name: &'static str, count: usize) {
        if self.started.is_some() {
            self.counts
                .push((name, u64::try_from(count).unwrap_or(u64::MAX)));
        }
    }

    fn emit(self) {
        let Some(started) = self.started else {
            return;
        };
        let mut line = serde_json::Map::new();
        line.insert("totalNs".into(), Self::nanos(started.elapsed()).into());
        for (stage, ns) in self.stages {
            line.insert(format!("{stage}Ns"), ns.into());
        }
        for (name, count) in self.counts {
            line.insert(name.into(), count.into());
        }
        eprintln!("{}", serde_json::json!({ "reactiveIrResolver": line }));
    }
}
