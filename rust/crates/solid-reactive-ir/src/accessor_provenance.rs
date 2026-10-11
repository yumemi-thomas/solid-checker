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
    Function { path: String, span: Span },
}

enum Value {
    Callable(Callable),
    Object {
        path: String,
        span: Span,
        context: bool,
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
    declarations: HashMap<SymbolId, (SymbolId, Location)>,
    uses: HashMap<Location, ClosedUses>,
    providers: HashMap<Location, Vec<Option<(String, Span)>>>,
    source_origins: HashSet<SymbolId>,
    blocked_contexts: HashSet<Location>,
}

#[derive(Default)]
struct ClosedUses {
    object_escaped: bool,
    callable_escaped: bool,
    context_escaped: bool,
    context_value_escaped: bool,
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
            declarations: HashMap::new(),
            uses: HashMap::new(),
            providers: HashMap::new(),
            source_origins: HashSet::new(),
            blocked_contexts: HashSet::new(),
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
        // Census each exact reference once. A receiver proves closure only
        // when its selected member is immediately called; a Get that escapes
        // an object/member cannot certify a later invocation.
        for file in &facts.files {
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
            let properties = file
                .ast
                .object_properties
                .iter()
                .filter(|p| p.data)
                .map(|p| file.ast.peel_ts_sugar_span(p.value))
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
                if let Some(key) = resolver.binding_key(file, target) {
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
            for id in file
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                let Some(key) = resolver.binding_key(file, id.span) else {
                    continue;
                };
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
                let uses = resolver.uses.entry(key).or_default();
                uses.object_escaped |= written
                    || !(receivers.contains(&id.span) || provider_values.contains(&id.span));
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
                        || provider_values.contains(&id.span));
            }
            for export in file.ast.exports.iter() {
                for binding in file.ast.exported_bindings(export) {
                    let uses = resolver
                        .uses
                        .entry(location(file.path.shared(), binding.pattern))
                        .or_default();
                    uses.object_escaped = true;
                    uses.context_value_escaped = true;
                }
            }
        }
        for file in &facts.files {
            for binding in &file.ast.bindings {
                if !binding.immutable {
                    continue;
                }
                for name in &binding.names {
                    if let Some(symbol) = lookup.entities().at(file.path.as_str(), name.span)
                        && !source_has_runtime_witness(
                            symbol,
                            accessors,
                            lookup.entities(),
                            names,
                            lookup,
                        )
                        && !crate::indexes::binding_written(file, name.span)
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
            for binding in &file.ast.bindings {
                let Some(context) = binding
                    .initializer
                    .and_then(|span| resolver.context_origin(file, span, 24))
                else {
                    continue;
                };
                let key = location(file.path.shared(), binding.pattern);
                if !binding.immutable
                    || binding.shape != ast::BindingShape::Identifier
                    || binding.names.len() != 1
                    || resolver.uses.get(&key).is_some_and(|uses| {
                        uses.context_value_escaped
                            || (object_contexts.contains(&context) && uses.object_escaped)
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
            .find(|b| b.names.iter().any(|n| n.span == declaration))?;
        Some(location(file.path.shared(), binding.pattern))
    }

    pub fn has_source_origin(&self, symbol: &SymbolId) -> bool {
        self.source_origins.contains(symbol)
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
            let properties = closed_properties(object_file, object)?;
            let property = properties.into_iter().find(|p| {
                object_file
                    .source_text(p.key)
                    .map(|s| s.trim_matches(['\'', '"']))
                    == Some(name)
            })?;
            let value = property.shorthand_binding.unwrap_or(property.value);
            let value = self.value(object_file, value, depth - 1, transported)?;
            // A normal function receives this object as `this` and could
            // replace its members. Only lexical-this arrows are transported
            // until receiver effects have their own exact census.
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
            return matches!(value, Value::Callable(_)).then_some(value);
        }
        if let Some(call) = file.ast.calls.iter().find(|c| c.span == span) {
            if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                *transported = true;
                let (provider_file, value) = self.provider(file, call)?;
                let mut value = self.value(provider_file, value, depth - 1, transported)?;
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
            let value = self.value(target_file, returned, depth - 1, transported)?;
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
        if binding.shape != ast::BindingShape::Identifier || binding.names.len() != 1 {
            return None;
        }
        let value = self.value(bound_file, binding.initializer?, depth - 1, transported)?;
        if !self.binding_is_closed(bound_file, binding, matches!(value, Value::Object { .. })) {
            return None;
        }
        Some(value)
    }

    /// One exact default-less dialect context, one syntactic provider, no
    /// hidden/spread value, and no escape of the context itself. Several
    /// providers are ambiguous even if their values happen to look alike.
    fn provider<'a>(
        &'a self,
        file: &FileFacts,
        call: &ast::CallFact,
    ) -> Option<(&'a FileFacts, Span)> {
        if call.arguments.len() != 1 || call.arguments[0].spread {
            return None;
        }
        let (context_file, binding, _) = self.lookup.binding_at_reference(
            file.path.as_str(),
            file.ast.peel_ts_sugar_span(call.arguments[0].span),
        )?;
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
        if let Some(call) = file.ast.call_at(span) {
            if call.construct {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                return (call.arguments.len() == 1 && !call.arguments[0].spread)
                    .then(|| {
                        self.binding_key(file, file.ast.peel_ts_sugar_span(call.arguments[0].span))
                    })
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
