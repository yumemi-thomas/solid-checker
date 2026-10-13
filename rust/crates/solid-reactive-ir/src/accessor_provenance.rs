//! Exact project-owned callable transport. Shapes and declared Accessor types
//! are deliberately not evidence here. This join retains an implementation
//! or source identity; the interprocedural consumer decides execution.

use std::collections::{HashMap, HashSet};

use solid_dialect::ContextRole;
use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use super::{SummaryRead, source_has_runtime_witness};
use crate::{SymbolId, indexes::SemanticLookup, location, owners::containing_ast_function};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Callable {
    Accessor(SymbolId),
    Function {
        path: String,
        span: Span,
    },
    /// Exact synchronous element-member invocations at a built-in map/forEach.
    ArrayMembers(Vec<(String, Span)>),
}

enum Value {
    Callable(Callable),
    Object {
        path: String,
        span: Span,
        context: bool,
    },
    Array {
        path: String,
        elements: Vec<Span>,
    },
    /// ADR 0282: the slots of the fresh array literal that a factory's one
    /// unconditional return builds. Only an exact `const` array pattern
    /// bound to that very call consumes it. Every other route (an alias, a
    /// parameter handoff, a context provider) refuses it, because there the
    /// array is no longer fresh: a holder could replace a slot.
    Tuple {
        path: String,
        factory: Span,
        elements: Vec<Span>,
    },
}

pub(super) struct Resolver<'a, 'facts> {
    pub facts: &'a ProjectFacts,
    pub lookup: &'a SemanticLookup<'facts>,
    pub accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
    pub names: &'a HashMap<SymbolId, SymbolId>,
    functions: HashSet<(String, Span)>,
    binding_locations: HashMap<SymbolId, Location>,
    parameter_inputs: HashMap<Location, (String, Span)>,
    declarations: HashMap<SymbolId, (SymbolId, Location)>,
    uses: HashMap<Location, ClosedUses>,
    providers: HashMap<Location, Vec<Option<(String, Span)>>>,
    source_origins: HashSet<SymbolId>,
    runtime_source_origins: HashSet<SymbolId>,
    blocked_contexts: HashSet<Location>,
    unkeyed_provider: bool,
    member_stability: MemberStability,
}

/// ADR 0273: absence of a visible veto is an assumption, not a closed census.
#[derive(Default)]
struct MemberStability {
    written: HashMap<String, Location>,
    all: Option<Location>,
    /// ADR 0284: the wildcard vetoes written inside a package analyzed as
    /// source, by its root. Dependency internals are outside ADR 0273's
    /// premise, as when only the package's declarations are seen, so a write
    /// that could name any member vetoes only members of objects that package
    /// creates. A write naming its member vetoes it everywhere (review
    /// round 1): the package may write it on an object the project handed it.
    dependencies: HashMap<String, MemberStability>,
}

enum Stability<'a> {
    Assumed,
    Vetoed(&'a Location),
}

impl MemberStability {
    fn state(&self, name: &str) -> Stability<'_> {
        match self.all.as_ref().or_else(|| self.written.get(name)) {
            Some(site) => Stability::Vetoed(site),
            None => Stability::Assumed,
        }
    }

    /// [`Self::state`] for a member of an object created in `owner`: the
    /// project's vetoes, and those of the package analyzed as source that
    /// holds `owner`, if one does.
    fn state_of(&self, facts: &ProjectFacts, owner: &str, name: &str) -> Stability<'_> {
        match self.state(name) {
            Stability::Assumed => facts
                .dependency_source_root(owner)
                .and_then(|root| self.dependencies.get(root))
                .map_or(Stability::Assumed, |package| package.state(name)),
            vetoed => vetoed,
        }
    }

    fn index(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut project = Self::default();
        for file in &facts.files {
            let index = match facts.dependency_source_root(file.path.as_str()) {
                Some(root) => project.dependencies.entry(root.to_owned()).or_default(),
                None => &mut project,
            };
            index.syntax(file);
            for call in &file.ast.calls {
                let Some(method) = mutation_builtin(file, call, lookup) else {
                    continue;
                };
                let site = location(file.path.shared(), call.span);
                // A spread argument shifts every later position (a tuple can
                // supply target, key and value at once), so no key position is
                // known: it could write any member.
                if call.arguments.iter().any(|argument| argument.spread) {
                    index.all.get_or_insert(site);
                    continue;
                }
                match method {
                    // A key the literal fact cannot name (a template, an
                    // identifier, a spread) could be any member, so it vetoes
                    // every transported member: over-vetoing only withholds.
                    "defineProperty" | "set" | "deleteProperty" => {
                        let name = call
                            .arguments
                            .get(1)
                            .filter(|argument| !argument.spread)
                            .and_then(|argument| match &argument.literal_value {
                                ast::ArgumentLiteralFact::String(name) => Some(name.to_string()),
                                ast::ArgumentLiteralFact::Integer(index) => Some(index.to_string()),
                                _ => None,
                            });
                        match name {
                            Some(name) => {
                                index.written.entry(name).or_insert(site);
                            }
                            None => {
                                index.all.get_or_insert(site);
                            }
                        }
                    }
                    "defineProperties" => {
                        // A literal descriptor map names its literal keys; a
                        // spread in it, or a map that is not a literal, could
                        // name any member. A non-literal computed key stays
                        // outside the premise (ADR 0273).
                        let shape = call
                            .arguments
                            .get(1)
                            .filter(|argument| !argument.spread)
                            .and_then(|argument| {
                                file.ast.object_get_shapes.iter().find(|shape| {
                                    shape.span == file.ast.peel_ts_sugar_span(argument.span)
                                })
                            })
                            .filter(|shape| {
                                !file
                                    .ast
                                    .spreads
                                    .iter()
                                    .any(|spread| shape.span.contains(spread.span))
                            });
                        match shape {
                            Some(shape) => {
                                for property in &shape.properties {
                                    index
                                        .written
                                        .entry(property.key.to_string())
                                        .or_insert_with(|| site.clone());
                                }
                            }
                            None => {
                                index.all.get_or_insert(site);
                            }
                        }
                    }
                    "assign" => {
                        for (position, argument) in call.arguments.iter().enumerate() {
                            // A first-slot spread can supply both target and sources.
                            if argument.spread {
                                index.all.get_or_insert_with(|| site.clone());
                                continue;
                            }
                            if position == 0 {
                                continue;
                            }
                            let shape = file.ast.object_get_shapes.iter().find(|shape| {
                                shape.span == file.ast.peel_ts_sugar_span(argument.span)
                                    && shape.closed
                            });
                            if shape.is_none() {
                                index.all.get_or_insert_with(|| site.clone());
                            } else if let Some(shape) = shape {
                                for property in &shape.properties {
                                    index
                                        .written
                                        .entry(property.key.to_string())
                                        .or_insert_with(|| site.clone());
                                }
                            }
                        }
                    }
                    _ => unreachable!("mutation builtin catalog"),
                }
            }
        }
        for package in project.dependencies.values_mut() {
            for (name, site) in package.written.drain() {
                project.written.entry(name).or_insert(site);
            }
        }
        project
    }

    fn syntax(&mut self, file: &FileFacts) {
        // Normalized leaves exclude receiver Gets, destructuring keys/defaults
        // and reads inside computed keys. Updates and loop targets are included.
        let targets = file
            .ast
            .write_targets
            .iter()
            .copied()
            .chain(file.ast.deleted_targets.iter().copied())
            .map(|span| file.ast.peel_ts_sugar_span(span))
            .collect::<HashSet<_>>();
        let literal_keys = file
            .ast
            .literal_computed_members
            .iter()
            .map(|key| (key.span, key.key.as_str()))
            .collect::<HashMap<_, _>>();
        for member in &file.ast.members {
            if !targets.contains(&member.span) {
                continue;
            }
            let name = if file.ast.computed_members.contains(&member.span) {
                literal_keys.get(&member.span).copied()
            } else {
                file.source_text(member.property)
            };
            if let Some(name) = name {
                self.written
                    .entry(name.to_owned())
                    .or_insert_with(|| location(file.path.shared(), member.span));
            }
        }
    }
}

/// Signature and runtime receiver identities are independent, as in ADR 0272.
/// A local ObjectConstructor-typed value or a homonymous helper is not Object.
fn mutation_builtin<'a>(
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
    if entity.symbol_unresolved || entity.symbol.is_empty() {
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

#[derive(Clone, Copy, Default)]
struct ClosedUses {
    object_escaped: bool,
    array_escaped: bool,
    callable_escaped: bool,
    context_escaped: bool,
    context_value_escaped: bool,
    destructured_escaped: bool,
}

impl ClosedUses {
    /// An exported binding's shared object can be replaced by any importer.
    fn export(&mut self) {
        self.object_escaped = true;
        self.array_escaped = true;
        self.context_value_escaped = true;
        self.destructured_escaped = true;
    }
}

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
            binding_locations: HashMap::new(),
            parameter_inputs: HashMap::new(),
            declarations: HashMap::new(),
            uses: HashMap::new(),
            providers: HashMap::new(),
            source_origins: HashSet::new(),
            runtime_source_origins: HashSet::new(),
            blocked_contexts: HashSet::new(),
            unkeyed_provider: false,
            member_stability: MemberStability::index(facts, lookup),
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
            let mut provider_values = HashSet::new();
            let mut context_tags = HashSet::new();
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
            let context_reads = file
                .ast
                .calls
                .iter()
                .filter(|c| resolver.role(file, c) == Some(ContextRole::Read))
                .filter_map(|c| c.arguments.first())
                .map(|a| file.ast.peel_ts_sugar_span(a.span))
                .collect::<HashSet<_>>();
            // In a closed program, a local `export { name }` publishes the
            // binding exactly as an `export const` declaration does: every
            // importer is itself a censused reference. It is therefore
            // censused as an export rather than as an escaping reference.
            // An open program keeps the specifier an escape.
            let export_specifiers = file
                .ast
                .exports
                .iter()
                .filter(|e| lookup.program_closed && e.module.is_none() && !e.type_only)
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
        let mut blocked_contexts = HashSet::new();
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

    fn binding_key(&self, file: &FileFacts, span: Span) -> Option<Location> {
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
        let mut local_functions = HashMap::new();
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
        let mut references = HashMap::<(String, Span), HashSet<Location>>::new();
        let mut argument_observers = HashSet::new();
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

    pub fn has_source_origin(&self, symbol: &SymbolId) -> bool {
        self.source_origins.contains(symbol)
    }
    pub fn has_runtime_source_origin(&self, symbol: &SymbolId) -> bool {
        self.runtime_source_origins.contains(symbol)
    }
    pub fn proven_source_origins(&self) -> impl Iterator<Item = &SymbolId> {
        self.runtime_source_origins
            .iter()
            .chain(self.source_origins.iter())
    }
    pub fn has_extra_source_origins(&self) -> bool {
        !self.source_origins.is_empty()
    }
    pub fn source_call(&self, file: &FileFacts, callee: Span) -> Option<SymbolId> {
        match self.value(file, callee, 24, &mut false)? {
            Value::Callable(Callable::Accessor(symbol)) => Some(symbol),
            _ => None,
        }
    }
    pub fn calls(&self) -> (HashMap<Location, Callable>, HashSet<Location>) {
        let mut calls = HashMap::new();
        let mut unavailable = HashSet::new();
        for call_file in &self.facts.files {
            for call in &call_file.ast.calls {
                if let Some(targets) = self.array_members(call_file, call) {
                    let callee = location(call_file.path.shared(), call.callee);
                    if targets.iter().all(|target| self.functions.contains(target)) {
                        calls.insert(callee, Callable::ArrayMembers(targets));
                    } else if targets.iter().any(|(path, span)| {
                        let file = self
                            .lookup
                            .file_by_path(path)
                            .expect("resolved array member file");
                        let function = file
                            .ast
                            .functions
                            .iter()
                            .find(|f| f.span == *span)
                            .expect("resolved array member");
                        file.ast
                            .calls
                            .iter()
                            .any(|c| function.body.contains(c.span))
                            || file
                                .ast
                                .members
                                .iter()
                                .any(|m| function.body.contains(m.span))
                    }) {
                        unavailable.insert(callee);
                    }
                    continue;
                }
                let mut transported = false;
                let value = self.value(call_file, call.callee, 24, &mut transported);
                if transported {
                    match value {
                        Some(Value::Callable(Callable::Function { path, span }))
                            if !self.functions.contains(&(path.clone(), span)) =>
                        {
                            let file = self
                                .lookup
                                .file_by_path(&path)
                                .expect("resolved function file");
                            let function = file
                                .ast
                                .functions
                                .iter()
                                .find(|f| f.span == span)
                                .expect("resolved function span");
                            // Preserve the missing implementation obligation without
                            // admitting a synthetic node or inferring its summary.
                            if file
                                .ast
                                .calls
                                .iter()
                                .any(|c| function.body.contains(c.span))
                                || file
                                    .ast
                                    .members
                                    .iter()
                                    .any(|m| function.body.contains(m.span))
                            {
                                unavailable.insert(location(call_file.path.shared(), call.callee));
                            }
                        }
                        Some(Value::Callable(target)) => {
                            calls.insert(location(call_file.path.shared(), call.callee), target);
                        }
                        _ => {}
                    }
                }
            }
        }
        (calls, unavailable)
    }

    fn array_members(&self, file: &FileFacts, call: &ast::CallFact) -> Option<Vec<(String, Span)>> {
        let receiver = array_iteration_receiver(file, call, self.lookup)?;
        // A stored or named callback's witnessed use cannot establish its
        // other invocations. Leave those definitions and obligations alone;
        // only direct call-site execution and ordinary helper entry compose.
        if super::runs_in_object_callable(file, call.span)
            || crate::execution_role::named_callback_execution_role(file, call.callee, self.lookup)
                .is_some()
        {
            return None;
        }
        if call.arguments.len() != 1 || call.arguments[0].spread {
            return None;
        }
        let callback_span = file.ast.peel_ts_sugar_span(call.arguments[0].span);
        let callback = file
            .ast
            .functions
            .iter()
            .find(|f| f.span == callback_span)?;
        if callback.kind != ast::FunctionKind::Arrow
            || callback.r#async
            || callback.generator
            || !callback.expression_body
            || callback.parameters.len() != 1
            || callback.rest_parameter
        {
            return None;
        }
        let parameter = &callback.parameters[0];
        let [name] = parameter.names.as_slice() else {
            return None;
        };
        if parameter.initializer.is_some() || parameter.pattern != name.span {
            return None;
        }
        // Exactly one invocation; no deferred nested body, alias or element escape.
        let mut invocations = file
            .ast
            .calls
            .iter()
            .filter(|c| callback.body.contains(c.span));
        let invocation = invocations.next()?;
        if invocations.next().is_some()
            || invocation.construct
            || invocation.arguments.iter().any(|a| a.spread)
            || file.ast.peel_ts_sugar_span(callback.body) != invocation.span
            || !super::body_site_runs_during_call(file, callback, invocation.span)
        {
            return None;
        }
        let member = file.ast.members.iter().find(|m| {
            m.span == file.ast.peel_ts_sugar_span(invocation.callee)
                && !file.ast.computed_members.contains(&m.span)
        })?;
        let object = file.ast.peel_ts_sugar_span(member.object);
        if file.ast.reference_declaration(object) != Some(name.span)
            || file.ast.identifiers.iter().any(|id| {
                id.role == ast::IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span) == Some(name.span)
                    && id.span != object
            })
        {
            return None;
        }
        let name = file.source_text(member.property)?;
        let Value::Array { path, elements } = self.value(file, receiver, 24, &mut false)? else {
            return None;
        };
        // The array and its element objects are created in `path`.
        if ["map", "forEach", name].into_iter().any(|member| {
            matches!(
                self.member_stability.state_of(self.facts, &path, member),
                Stability::Vetoed(_)
            )
        }) {
            return None;
        }
        let array_file = self.lookup.file_by_path(&path)?;
        let mut targets = Vec::new();
        for element in elements {
            let properties = closed_properties(array_file, element)?;
            let property = properties.into_iter().find(|p| {
                array_file
                    .source_text(p.key)
                    .map(|s| s.trim_matches(['\'', '"']))
                    == Some(name)
            })?;
            let Value::Callable(Callable::Function { path, span }) = self.value(
                array_file,
                property.shorthand_binding.unwrap_or(property.value),
                24,
                &mut false,
            )?
            else {
                return None;
            };
            let selected = self
                .lookup
                .file_by_path(&path)?
                .ast
                .functions
                .iter()
                .find(|f| f.span == span)?;
            if selected.kind != ast::FunctionKind::Arrow || selected.r#async || selected.generator {
                return None;
            }
            if !targets.contains(&(path.clone(), span)) {
                targets.push((path, span));
            }
        }
        (!targets.is_empty()).then_some(targets)
    }

    pub fn read(&self, symbol: &SymbolId, file: &FileFacts, span: Span) -> Option<SummaryRead> {
        let (display, declaration) = self
            .accessors
            .get(symbol)
            .or_else(|| self.declarations.get(symbol))
            .cloned()?;
        Some(SummaryRead {
            transported: true,
            contract_read_context: None,
            symbol: symbol.clone(),
            display,
            kind: Some("accessor".into()),
            declaration,
            origin: location(file.path.shared(), span),
            origin_context: crate::owners::enclosing_function_label(file, span),
            owner: None,
            local: false,
            conditional: false,
        })
    }

    fn role(&self, file: &FileFacts, call: &ast::CallFact) -> Option<ContextRole> {
        let primitive = crate::call_primitive_name(
            file,
            call,
            self.lookup.entities(),
            self.names,
            self.lookup.dialect,
        )?
        .primitive()?;
        self.lookup
            .dialect
            .context_role(self.lookup.dialect.name_of(primitive)?)
    }

    fn value(
        &self,
        file: &FileFacts,
        span: Span,
        depth: usize,
        transported: &mut bool,
    ) -> Option<Value> {
        if depth == 0 {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        let declaration = file.ast.reference_declaration(span).unwrap_or(span);
        if let Some((path, argument)) = self
            .parameter_inputs
            .get(&location(file.path.shared(), declaration))
        {
            let argument_file = self.lookup.file_by_path(path)?;
            let mut origin_transport = false;
            let value = self.value(argument_file, *argument, depth - 1, &mut origin_transport)?;
            if !origin_transport {
                return None;
            }
            let uses = self.uses.get(&location(file.path.shared(), declaration));
            let closed = match &value {
                Value::Object { .. } => uses.is_none_or(|uses| !uses.object_escaped),
                Value::Array { .. } => uses.is_none_or(|uses| !uses.array_escaped),
                // ADR 0282: a handed-off tuple is a held array, not a fresh one.
                Value::Callable(_) | Value::Tuple { .. } => false,
            };
            if !closed {
                return None;
            }
            *transported = true;
            return Some(value);
        }
        if let Some(function) = file.ast.functions.iter().find(|f| f.span == span) {
            if function.generator || function.r#async {
                return None;
            }
            return Some(Value::Callable(Callable::Function {
                path: file.path.to_string(),
                span,
            }));
        }
        let declared_function = file
            .ast
            .identifiers
            .iter()
            .any(|id| id.span == span)
            .then(|| {
                self.lookup
                    .callee_symbol(file, span)
                    .and_then(|symbol| self.lookup.function_for_symbol(symbol))
            })
            .flatten()
            .or_else(|| self.lookup.namespace_member_function(file, span));
        if let Some((target_file, function)) = declared_function
            && function.kind == ast::FunctionKind::Declaration
            && self.lookup.function_value_is_current(target_file, function)
        {
            return Some(Value::Callable(Callable::Function {
                path: target_file.path.to_string(),
                span: function.span,
            }));
        }
        if let Some(member) = file.ast.members.iter().find(|m| m.span == span) {
            if file.ast.computed_members.contains(&span) {
                return None;
            }
            let Value::Object {
                path, span: object, ..
            } = self.value(file, member.object, depth - 1, transported)?
            else {
                return None;
            };
            let object_file = self.lookup.file_by_path(&path)?;
            let name = file.source_text(member.property)?;
            return self.member_value(object_file, object, name, depth - 1, transported);
        }
        if let Some(call) = file.ast.calls.iter().find(|c| c.span == span) {
            if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                *transported = true;
                let (provider_file, value) = self.provider(file, call)?;
                let mut value = self.value(provider_file, value, depth - 1, transported)?;
                // ADR 0282: every consumer of the context shares the provider's
                // one array, and any of them can write a slot. (An array
                // pattern of a context value already blocks that context in
                // the receiver census; this keeps the refusal local.)
                if matches!(value, Value::Tuple { .. }) {
                    return None;
                }
                if let Value::Object { context, .. } = &mut value {
                    *context = true;
                }
                return Some(value);
            }
            let Value::Callable(Callable::Function { path, span }) =
                self.value(file, call.callee, depth - 1, transported)?
            else {
                return None;
            };
            let target_file = self.lookup.file_by_path(&path)?;
            let function = target_file.ast.functions.iter().find(|f| f.span == span)?;
            if function.r#async
                || function.generator
                || !self.lookup.function_value_is_current(target_file, function)
            {
                return None;
            }
            let returned = single_return(target_file, function)?;
            *transported = true;
            // ADR 0282: a returned array literal whose slots are not all closed
            // objects (ADR 0273's map/forEach receivers keep their own path) is
            // a tuple. It is described here, at the call, and nowhere else, so
            // only a literal this one return builds is ever a tuple: `const t =
            // [..]; return t` is not one, nor is a literal the factory merely
            // holds.
            let literal = target_file.ast.peel_ts_sugar_span(returned);
            if target_file.ast.array_literals.contains(&literal)
                && returned_array_elements(target_file, literal).is_none()
            {
                return Some(Value::Tuple {
                    path: target_file.path.to_string(),
                    factory: function.span,
                    elements: returned_tuple_elements(target_file, literal)?,
                });
            }
            let value = self.value(target_file, returned, depth - 1, transported)?;
            // ADR 0282: `return make()` forwards another factory's tuple. It is
            // still fresh, but v1 admits only the literal's own factory.
            if matches!(value, Value::Tuple { .. }) {
                return None;
            }
            if matches!(value, Value::Array { .. })
                && !target_file
                    .ast
                    .array_literals
                    .contains(&target_file.ast.peel_ts_sugar_span(returned))
            {
                return None;
            }
            if let Value::Object {
                path,
                span,
                context: false,
            } = &value
                && (path != target_file.path.as_str()
                    || *span != target_file.ast.peel_ts_sugar_span(returned))
            {
                return None;
            }
            return Some(value);
        }
        if closed_properties(file, span).is_some() {
            return Some(Value::Object {
                path: file.path.to_string(),
                span,
                context: false,
            });
        }
        if file.ast.array_literals.contains(&span) {
            // ReturnStructure owns the exact slots, including holes/spreads.
            // Do not infer them from contained object spans.
            let elements = returned_array_elements(file, span)?;
            return Some(Value::Array {
                path: file.path.to_string(),
                elements,
            });
        }
        // A demanded entity at a composite span carries its leading token's
        // symbol, not the expression's value: `ctx.a() === "x"` names `ctx`.
        // Only an exact identifier denotes its binding.
        if !exact_identifier(file, span) {
            return None;
        }
        let (bound_file, binding, symbol) = self
            .lookup
            .binding_at_reference(file.path.as_str(), span)
            .or_else(|| {
                // Shorthand values use the binder's declaration, where no
                // reference fact need have been demanded.
                let declaration = file.ast.reference_declaration(span).unwrap_or(span);
                let binding = file
                    .ast
                    .bindings
                    .iter()
                    .find(|b| b.names.iter().any(|n| n.span == declaration))?;
                let symbol = self
                    .lookup
                    .entities()
                    .at(file.path.as_str(), declaration)?
                    .clone();
                Some((file, binding, symbol))
            })?;
        if !binding.immutable
            || crate::value_identity::binding_has_write(bound_file, self.lookup.entities(), &symbol)
            || binding
                .names
                .iter()
                .any(|name| crate::indexes::binding_written(bound_file, name.span))
        {
            return None;
        }
        if binding.shape == ast::BindingShape::Object {
            if !simple_object_binding(binding)
                || self
                    .uses
                    .get(&location(bound_file.path.shared(), binding.pattern))
                    .is_some_and(|uses| uses.destructured_escaped)
            {
                return None;
            }
            let slot = binding
                .object_slots
                .iter()
                .find(|slot| slot.local.span == declaration)?;
            let Value::Object {
                path, span: object, ..
            } = self.value(bound_file, binding.initializer?, depth - 1, transported)?
            else {
                return None;
            };
            if !*transported {
                return None;
            }
            return self.member_value(
                self.lookup.file_by_path(&path)?,
                object,
                slot.property.as_str(),
                depth - 1,
                transported,
            );
        }
        if self.source_kinds.get(&symbol) != Some(&crate::ReactiveSourceKind::Store)
            && (source_has_runtime_witness(
                &symbol,
                self.accessors,
                self.lookup.entities(),
                self.names,
                self.lookup,
            ) || (binding.shape == ast::BindingShape::Identifier
                && binding
                    .call_initializer
                    .is_some_and(|span| self.created_accessor(bound_file, span, 8))))
        {
            if !self.accessors.contains_key(&symbol) {
                *transported = true;
            }
            return Some(Value::Callable(Callable::Accessor(symbol)));
        }
        if binding.shape == ast::BindingShape::Array {
            return self.tuple_slot(file, bound_file, binding, declaration, depth, transported);
        }
        if binding.shape != ast::BindingShape::Identifier || binding.names.len() != 1 {
            return None;
        }
        let value = self.value(bound_file, binding.initializer?, depth - 1, transported)?;
        // ADR 0282: `const t = make()` holds the tuple. `t[1] = other` or a
        // call that receives `t` can replace a slot, so a held tuple is no
        // tuple; `t[1]()` keeps its computed-callee obligation.
        if matches!(value, Value::Tuple { .. }) {
            return None;
        }
        if matches!(value, Value::Array { .. }) {
            let key = location(bound_file.path.shared(), binding.pattern);
            return self
                .uses
                .get(&key)
                .is_none_or(|uses| !uses.array_escaped)
                .then_some(value);
        }
        if !self.binding_is_closed(bound_file, binding, matches!(value, Value::Object { .. })) {
            return None;
        }
        Some(value)
    }

    fn member_value(
        &self,
        file: &FileFacts,
        object: Span,
        name: &str,
        depth: usize,
        transported: &mut bool,
    ) -> Option<Value> {
        if let Stability::Vetoed(_site) =
            self.member_stability
                .state_of(self.facts, file.path.as_str(), name)
        {
            return None;
        }
        let property = closed_properties(file, object)?
            .into_iter()
            .find(|property| {
                file.source_text(property.key)
                    .map(|s| s.trim_matches(['\'', '"']))
                    == Some(name)
            })?;
        let value = self.value(
            file,
            property.shorthand_binding.unwrap_or(property.value),
            depth,
            transported,
        )?;
        if let Value::Callable(Callable::Function { path, span }) = &value {
            let selected = self
                .lookup
                .file_by_path(path)?
                .ast
                .functions
                .iter()
                .find(|f| f.span == *span)?;
            if selected.kind != ast::FunctionKind::Arrow {
                return None;
            }
        }
        matches!(value, Value::Callable(_)).then_some(value)
    }

    /// ADR 0282: `const [running, start] = make(); start()` calls the closure
    /// in slot 1 of the array literal `make`'s one unconditional return
    /// builds. The array is fresh and this pattern consumes it in the same
    /// statement, so no other code holds it and no write can replace a slot
    /// between the return and the destructuring. The caller has already
    /// refused a `let`, a written name and a declaration this lookup cannot
    /// pair with one binding.
    fn tuple_slot(
        &self,
        file: &FileFacts,
        bound_file: &FileFacts,
        binding: &ast::BindingFact,
        declaration: Span,
        depth: usize,
        transported: &mut bool,
    ) -> Option<Value> {
        // `declaration` is a span of `file`; a binding another file holds
        // cannot be paired with it by position.
        if bound_file.path != file.path
            || !simple_array_binding(binding)
            // Destructuring runs `Array.prototype[Symbol.iterator]`. Like ADR
            // 0273's members, its stability is a premise; a write whose key
            // no literal names is the one veto visible here.
            || self.member_stability.all.is_some()
        {
            return None;
        }
        let index = binding
            .array_slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.span == declaration))?;
        // The initializer is the call itself, so the array is the call's
        // fresh result: `const [, start] = t` destructures a held tuple.
        let initializer = bound_file.ast.peel_ts_sugar_span(binding.initializer?);
        bound_file.ast.call_at(initializer)?;
        let Value::Tuple {
            path,
            factory,
            elements,
        } = self.value(bound_file, initializer, depth - 1, transported)?
        else {
            return None;
        };
        // ADR 0284: a wildcard write inside a package analyzed as source
        // vetoes the tuples that package creates.
        if !*transported
            || self
                .facts
                .dependency_source_root(&path)
                .and_then(|root| self.member_stability.dependencies.get(root))
                .is_some_and(|package| package.all.is_some())
        {
            return None;
        }
        // A slot past the literal's end destructures `undefined`.
        let element = *elements.get(index)?;
        let span = slot_closure(
            self.lookup.file_by_path(&path)?,
            factory,
            element,
            self.lookup,
        )?;
        Some(Value::Callable(Callable::Function { path, span }))
    }

    /// One exact default-less dialect context, one syntactic provider, no
    /// hidden/spread value, and no escape of the context itself. Several
    /// providers are ambiguous even if their values happen to look alike.
    fn provider<'a>(
        &'a self,
        file: &FileFacts,
        call: &ast::CallFact,
    ) -> Option<(&'a FileFacts, Span)> {
        // useContext is a channel any code beneath the provider can use. In an
        // open program, unseen importers can reach the value through an
        // exported context, wrapper or returned closure and replace a member
        // where ADR 0273's name index cannot see the write, so no context's
        // provider census is closed.
        if !self.lookup.program_closed
            || self.unkeyed_provider
            || call.arguments.len() != 1
            || call.arguments[0].spread
        {
            return None;
        }
        let argument = file.ast.peel_ts_sugar_span(call.arguments[0].span);
        if !exact_identifier(file, argument) {
            return None;
        }
        let (context_file, binding, _) = self
            .lookup
            .binding_at_reference(file.path.as_str(), argument)?;
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
            || crate::indexes::binding_written(context_file, binding.names[0].span)
        {
            return None;
        }
        let created = context_file.ast.call_at(binding.call_initializer?)?;
        if self.role(context_file, created) != Some(ContextRole::Create)
            || !created.arguments.is_empty()
        {
            return None;
        }
        let key = location(context_file.path.shared(), binding.pattern);
        if self.blocked_contexts.contains(&key)
            || self.uses.get(&key).is_some_and(|uses| uses.context_escaped)
        {
            return None;
        }
        let providers = self.providers.get(&key)?;
        if providers.len() != 1 {
            return None;
        }
        let (path, value) = providers[0].as_ref()?;
        let provider_file = self.lookup.file_by_path(path)?;
        if let Some((bound_file, binding, _)) = self.lookup.binding_at_reference(
            provider_file.path.as_str(),
            provider_file.ast.peel_ts_sugar_span(*value),
        ) {
            let key = location(bound_file.path.shared(), binding.pattern);
            if self
                .uses
                .get(&key)
                .is_some_and(|uses| uses.context_value_escaped)
            {
                return None;
            }
        }
        Some((provider_file, *value))
    }

    /// Derivation only: this deliberately ignores receiver closure/provider
    /// acceptance so a bad receiver cannot disappear from the shared census.
    fn context_origin(&self, file: &FileFacts, span: Span, depth: usize) -> Option<Location> {
        if depth == 0 {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        let declaration = file.ast.reference_declaration(span).unwrap_or(span);
        if let Some((path, argument)) = self
            .parameter_inputs
            .get(&location(file.path.shared(), declaration))
        {
            return self.context_origin(self.lookup.file_by_path(path)?, *argument, depth - 1);
        }
        if let Some(call) = file.ast.call_at(span) {
            if call.construct {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                let argument = call
                    .arguments
                    .first()
                    .map(|a| file.ast.peel_ts_sugar_span(a.span))?;
                return (call.arguments.len() == 1
                    && !call.arguments[0].spread
                    && exact_identifier(file, argument))
                .then(|| self.binding_key(file, argument))
                .flatten();
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let target = self
                .lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| self.lookup.namespace_member_function(file, callee));
            let (target_file, function) = target?;
            if function.r#async
                || function.generator
                || !self.lookup.function_value_is_current(target_file, function)
            {
                return None;
            }
            return self.context_origin(
                target_file,
                single_return(target_file, function)?,
                depth - 1,
            );
        }
        if !exact_identifier(file, span) {
            return None;
        }
        let (bound_file, binding, _) =
            self.lookup.binding_at_reference(file.path.as_str(), span)?;
        self.context_origin(bound_file, binding.initializer?, depth - 1)
    }

    fn created_accessor(&self, file: &FileFacts, span: Span, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(call) = file.ast.call_at(file.ast.peel_ts_sugar_span(span)) else {
            return false;
        };
        if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
            return false;
        }
        if let Some(primitive) = crate::call_primitive_name(
            file,
            call,
            self.lookup.entities(),
            self.names,
            self.lookup.dialect,
        )
        .and_then(|p| p.primitive())
        {
            return self.lookup.dialect.creates_reactive_source(primitive)
                && !self.lookup.dialect.returns_reactive_tuple(primitive)
                && !self.lookup.dialect.returns_store(primitive);
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let target = if file.ast.identifiers.iter().any(|id| id.span == callee) {
            self.lookup.function_called_at(file.path.as_str(), callee)
        } else {
            self.lookup.namespace_member_function(file, callee)
        };
        let Some((target_file, function)) = target else {
            return false;
        };
        !function.r#async
            && !function.generator
            && self.lookup.function_value_is_current(target_file, function)
            && single_return(target_file, function)
                .is_some_and(|returned| self.created_accessor(target_file, returned, depth - 1))
    }

    fn binding_is_closed(
        &self,
        file: &FileFacts,
        binding: &ast::BindingFact,
        object: bool,
    ) -> bool {
        let key = location(file.path.shared(), binding.pattern);
        self.uses.get(&key).is_none_or(|uses| {
            if object {
                !uses.object_escaped
            } else {
                !uses.callable_escaped
            }
        })
    }
}

/// Whether `span` is exactly one identifier, as opposed to an expression an
/// identifier merely begins.
fn exact_identifier(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .identifiers
        .binary_search_by_key(&span, |id| id.span)
        .is_ok()
        || file.ast.reference_declaration(span).is_some()
}

fn simple_object_binding(binding: &ast::BindingFact) -> bool {
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
fn simple_array_binding(binding: &ast::BindingFact) -> bool {
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
fn slot_closure(
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
fn slot_arrow(file: &FileFacts, factory: Span, element: Span) -> Option<(Span, Option<Span>)> {
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
    let function = file.ast.functions.iter().find(|f| f.span == arrow)?;
    (function.kind == ast::FunctionKind::Arrow && !function.r#async && !function.generator)
        .then_some((arrow, declaration))
}

/// ADR 0282: the exact slots of a returned array literal. A hole or a spread
/// moves every later position, so either refuses the whole literal.
fn returned_tuple_elements(file: &FileFacts, span: Span) -> Option<Vec<Span>> {
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
fn array_iteration_receiver(
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

fn single_return(file: &FileFacts, function: &ast::FunctionFact) -> Option<Span> {
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

fn returned_array_elements(file: &FileFacts, span: Span) -> Option<Vec<Span>> {
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

/// Literal syntax supplies member identity only when its entire own property
/// set is closed. Nested properties/callback objects are never siblings.
fn closed_properties(file: &FileFacts, span: Span) -> Option<Vec<&ast::ObjectPropertyFact>> {
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
    let mut names = HashSet::new();
    for property in &properties {
        let name = file.source_text(property.key)?.trim_matches(['\'', '"']);
        if property.computed || !property.data || name == "__proto__" || !names.insert(name) {
            return None;
        }
    }
    (properties.len() == shape.properties.len() && !properties.is_empty()).then_some(properties)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::{
        compiler::ExecutionMap,
        core::{Generation, SourceHash, SourcePath},
    };
    use std::sync::Arc;

    fn file(source: &str) -> FileFacts {
        let ast = ast::extract("provenance.ts", source).unwrap();
        FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("provenance.ts").unwrap(),
            source_hash: SourceHash::of(source),
            source: Arc::from(source),
            ast: Arc::new(ast),
            compiler: Arc::new(ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        }
    }

    #[test]
    fn visible_member_writes_use_normalized_targets_and_cooked_keys() {
        for source in [
            "x.rows = f;",
            "x.rows += 1;",
            "x.rows ||= f;",
            "++x.rows;",
            "x.rows--;",
            "[x.rows] = values;",
            "({ key: x.rows } = value);",
            "for (x.rows of values) {}",
            "for (x.rows in values) {}",
            "delete x.rows;",
            "x['rows'] = f;",
            "delete x['r\\u006fws'];",
            "x['rows' as const] = f;",
            "x['rows' satisfies string] = f;",
            "delete x['rows'!];",
            "x[(<string>'rows')] = f;",
            "x[`r\\u006fws`] = f;",
            "(x.rows as any) = f;",
        ] {
            let file = file(source);
            let mut index = MemberStability::default();
            index.syntax(&file);
            assert!(
                matches!(index.state("rows"), Stability::Vetoed(_)),
                "{source}"
            );
            assert!(matches!(index.state("other"), Stability::Assumed));
        }
        for source in [
            "x.rows.other = f;",
            "x[x.rows] = f;",
            "const rows = x.rows;",
            "({ rows: x.other = x.rows } = value);",
            "delete x[key];",
            "x[key as string] = f;",
            "x[`r${key}ws`] = f;",
        ] {
            let file = file(source);
            let mut index = MemberStability::default();
            index.syntax(&file);
            assert!(
                matches!(index.state("rows"), Stability::Assumed),
                "{source}"
            );
        }
    }

    #[test]
    fn returned_members_require_the_entire_literal_to_be_closed() {
        for (source, admitted) in [
            ("const result = { rows, selected: () => rows() };", true),
            (
                "const result = { rows, selected: () => ({rows: other}) };",
                true,
            ),
            ("const result = { rows, ...other };", false),
            ("const result = { [key]: rows };", false),
            ("const result = { rows, rows: other };", false),
            ("const result = { get rows() { return read(); } };", false),
            ("const result = { rows() { return read(); } };", false),
            ("const result = { __proto__: other, rows };", false),
        ] {
            let file = file(source);
            let span = file.ast.bindings[0].initializer.unwrap();
            assert_eq!(
                closed_properties(&file, span).is_some(),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn returned_array_census_requires_every_exact_slot_to_be_a_closed_object() {
        for (source, admitted) in [
            (
                "function make() { return [{ query: read }, { query: other }]; }",
                true,
            ),
            ("const make = () => ([{ query: read }] as const);", true),
            (
                "function make() { return [{ query: read }, ...other]; }",
                false,
            ),
            ("function make() { return [{ query: read }, ,]; }", false),
            ("function make() { return [{ query: read }, 0]; }", false),
            ("function make() { return [[{ query: read }]]; }", false),
            (
                "function make() { return [{ query: read, ...extra }]; }",
                false,
            ),
            ("function make() { return [{ [key]: read }]; }", false),
            (
                "function make() { return [{ get query() { return read; } }]; }",
                false,
            ),
            (
                "function make() { return [{ query() { return read(); } }]; }",
                false,
            ),
            ("function make() { return []; }", false),
        ] {
            let file = file(source);
            let returned = single_return(&file, &file.ast.functions[0]).unwrap();
            assert_eq!(
                returned_array_elements(&file, returned).is_some(),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn returned_tuple_census_requires_every_slot_literal() {
        for (source, slots) in [
            (
                "function make() { const start = () => 1; return [read, start]; }",
                Some(2),
            ),
            (
                "function make() { return [read, () => 1] as const; }",
                Some(2),
            ),
            ("const make = () => [read, () => 1] as const;", Some(2)),
            ("function make() { return [read, , start]; }", None),
            ("function make() { return [read, ...rest]; }", None),
            ("function make() { return [...rest, start]; }", None),
            ("function make() { return []; }", None),
        ] {
            let file = file(source);
            let returned = single_return(&file, &file.ast.functions[0]).unwrap();
            assert_eq!(
                returned_tuple_elements(&file, returned).map(|elements| elements.len()),
                slots,
                "{source}"
            );
        }
    }

    #[test]
    fn simple_array_binding_refuses_defaults_rest_nested_and_mutable() {
        for (source, admitted) in [
            ("const [running, start] = make();", true),
            ("const [, start] = make();", true),
            ("const [running, start] = make() as const;", true),
            ("let [running, start] = make();", false),
            ("var [running, start] = make();", false),
            ("const [running, start = other] = make();", false),
            ("const [running, ...rest] = make();", false),
            ("const [running, [start]] = make();", false),
            ("const [running, { start }] = make();", false),
        ] {
            let file = file(source);
            assert_eq!(
                simple_array_binding(&file.ast.bindings[0]),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn tuple_slot_is_the_exact_arrow_the_factory_declares() {
        let source = "const outer = () => 0;
function make(parameter: () => number) {
  const start = () => 1;
  const typed = (() => 2) as () => number;
  let replaceable = () => 3;
  const expression = function () { return 4; };
  function declared() { return 5; }
  const wrapped = wrap(() => 6);
  return [start, typed, () => 9, (() => 10) as () => number, replaceable,
    expression, declared, wrapped, outer, parameter, async () => 11];
}";
        let file = file(source);
        let factory = file
            .ast
            .functions
            .iter()
            .find(|f| f.kind == ast::FunctionKind::Declaration && f.parameters.len() == 1)
            .unwrap();
        let returned = single_return(&file, factory).unwrap();
        let elements = returned_tuple_elements(&file, returned).unwrap();
        let admitted = elements
            .iter()
            .map(|element| slot_arrow(&file, factory.span, *element).is_some())
            .collect::<Vec<_>>();
        assert_eq!(
            admitted,
            [
                true, true, true, true, false, false, false, false, false, false, false
            ]
        );
        // A slot names the arrow itself, never the binding or the wrapper.
        let (arrow, declaration) = slot_arrow(&file, factory.span, elements[1]).unwrap();
        assert_eq!(file.source_text(arrow), Some("() => 2"));
        assert_eq!(declaration.and_then(|d| file.source_text(d)), Some("typed"));
        let (arrow, declaration) = slot_arrow(&file, factory.span, elements[3]).unwrap();
        assert_eq!(file.source_text(arrow), Some("() => 10"));
        assert_eq!(declaration, None);
    }

    #[test]
    fn factory_return_census_excludes_alternatives_and_nested_returns() {
        for (source, admitted) in [
            ("function make() { return { rows }; }", true),
            (
                "function make() { const read = () => { return 1; }; return { rows }; }",
                true,
            ),
            (
                "function make() { if (flag) return { rows }; return other; }",
                false,
            ),
            ("function make() { if (flag) return { rows }; }", false),
            ("function make() { return; }", false),
        ] {
            let file = file(source);
            let function = file
                .ast
                .functions
                .iter()
                .find(|f| f.kind == ast::FunctionKind::Declaration)
                .unwrap();
            assert_eq!(
                single_return(&file, function).is_some(),
                admitted,
                "{source}"
            );
        }
    }

    #[test]
    fn object_callable_definitions_never_supply_their_invocation_role() {
        for (source, stored) in [
            ("const decor = { header: () => read() };", true),
            ("const notes = () => read(); const decor = { notes };", true),
            (
                "const notes = () => read(); const decor = { notes: (notes as () => number) };",
                true,
            ),
            ("consume({ header: () => inline(() => read()) });", true),
            (
                "const notes = () => read(); function other(notes) { return { notes }; }",
                false,
            ),
            (
                "function Consumer() { const value = read(); return value; }",
                false,
            ),
        ] {
            let file = file(source);
            let read = file
                .ast
                .calls
                .iter()
                .find(|call| call.static_callee(&file.source) == Some("read"))
                .unwrap();
            assert_eq!(
                super::super::runs_in_object_callable(&file, read.span),
                stored,
                "{source}"
            );
        }
    }
}
