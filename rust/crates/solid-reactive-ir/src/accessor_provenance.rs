//! Exact project-owned callable transport. Shapes and declared Accessor types
//! are deliberately not evidence here. This join retains an implementation
//! or source identity; the interprocedural consumer decides execution.

use std::collections::{HashMap, HashSet};

use solid_dialect::ContextRole;
use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use super::{SummaryRead, source_has_runtime_witness};
use crate::{SymbolId, indexes::SemanticLookup, location, owners::containing_ast_function};

#[derive(Clone, Debug)]
pub(super) enum Callable {
    Accessor(SymbolId),
    Function { path: String, span: Span },
}

enum Value {
    Callable(Callable),
    Object { path: String, span: Span },
}

pub(super) struct Resolver<'a, 'facts> {
    pub facts: &'a ProjectFacts,
    pub lookup: &'a SemanticLookup<'facts>,
    pub accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
    pub names: &'a HashMap<SymbolId, SymbolId>,
}

impl Resolver<'_, '_> {
    pub fn has_source_origin(&self, symbol: &SymbolId) -> bool {
        self.facts.files.iter().any(|file| {
            file.ast.bindings.iter().any(|binding| {
                binding.immutable
                    && binding.names.iter().any(|name| {
                        self.lookup.entities().at(file.path.as_str(), name.span) == Some(symbol)
                            && !crate::indexes::binding_written(file, name.span)
                    })
                    && self.source_call(file, binding.names[0].span).as_ref() == Some(symbol)
            })
        })
    }
    pub fn source_call(&self, file: &FileFacts, callee: Span) -> Option<SymbolId> {
        match self.value(file, callee, 24, &mut false)? {
            Value::Callable(Callable::Accessor(symbol)) => Some(symbol),
            _ => None,
        }
    }
    pub fn calls(&self) -> HashMap<Location, Callable> {
        let mut calls = HashMap::new();
        for file in &self.facts.files {
            for call in &file.ast.calls {
                let mut transported = false;
                if let Some(Value::Callable(target)) =
                    self.value(file, call.callee, 24, &mut transported)
                    && transported
                {
                    calls.insert(location(file.path.shared(), call.callee), target);
                }
            }
        }
        calls
    }

    pub fn read(&self, symbol: &SymbolId, file: &FileFacts, span: Span) -> Option<SummaryRead> {
        let (display, declaration) = self.accessors.get(symbol).cloned().or_else(|| {
            self.facts.files.iter().find_map(|file| {
                file.ast.bindings.iter().find_map(|binding| {
                    let name = binding.names.iter().find(|name| {
                        self.lookup.entities().at(file.path.as_str(), name.span) == Some(symbol)
                    })?;
                    Some((
                        SymbolId::from(file.source_text(name.span)?),
                        location(file.path.shared(), name.span),
                    ))
                })
            })
        })?;
        Some(SummaryRead {
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
            if function.generator {
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
            let Value::Object { path, span: object } =
                self.value(file, member.object, depth - 1, transported)?
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
            return Some(value);
        }
        if let Some(call) = file.ast.calls.iter().find(|c| c.span == span) {
            if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                *transported = true;
                let (provider_file, value) = self.provider(file, call)?;
                return self.value(provider_file, value, depth - 1, transported);
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
            return self.value(target_file, returned, depth - 1, transported);
        }
        if closed_properties(file, span).is_some() {
            return Some(Value::Object {
                path: file.path.to_string(),
                span,
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
        let (context_file, binding, symbol) = self.lookup.binding_at_reference(
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
        let mut providers = Vec::new();
        for candidate in &self.facts.files {
            for element in &candidate.ast.jsx_elements {
                let provider = if let Some(member) = self.lookup.dialect.context_provider_member() {
                    (element
                        .member_property
                        .and_then(|p| candidate.source_text(p))
                        == Some(member))
                    .then_some(element.member_object)
                    .flatten()
                } else {
                    Some(element.name.span)
                };
                if !provider.is_some_and(|span| {
                    self.same_binding(candidate, span, context_file, binding, &symbol)
                }) {
                    continue;
                }
                let mut values = element
                    .attributes
                    .iter()
                    .filter(|a| candidate.source_text(a.local_name) == Some("value"));
                let value = values.next()?.expression?;
                if values.next().is_some() || !element.spreads.is_empty() {
                    return None;
                }
                providers.push((candidate, value));
            }
            for id in candidate
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                if !self.same_binding(candidate, id.span, context_file, binding, &symbol) {
                    continue;
                }
                let read = candidate.ast.calls.iter().any(|c| {
                    self.role(candidate, c) == Some(ContextRole::Read)
                        && c.arguments
                            .first()
                            .is_some_and(|a| candidate.ast.peel_ts_sugar_span(a.span) == id.span)
                });
                let jsx = candidate.ast.jsx_elements.iter().any(|e| {
                    e.name.span == id.span
                        || e.closing_name == Some(id.span)
                        || e.member_object == Some(id.span)
                });
                if !read && !jsx {
                    return None;
                }
            }
        }
        if providers.len() != 1 {
            return None;
        }
        providers.pop()
    }

    fn same_binding(
        &self,
        file: &FileFacts,
        span: Span,
        declared: &FileFacts,
        binding: &ast::BindingFact,
        symbol: &SymbolId,
    ) -> bool {
        self.lookup
            .binding_at_reference(file.path.as_str(), span)
            .is_some_and(|(f, b, _)| f.path == declared.path && b.pattern == binding.pattern)
            || self.lookup.entities().at(file.path.as_str(), span) == Some(symbol)
            || (file.path == declared.path
                && file
                    .ast
                    .reference_declaration(span)
                    .is_some_and(|d| binding.names.iter().any(|n| n.span == d)))
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
        let Some(symbol) = self
            .lookup
            .entities()
            .at(file.path.as_str(), binding.names[0].span)
        else {
            return false;
        };
        for candidate in &self.facts.files {
            for id in candidate
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == ast::IdentifierRole::Reference)
            {
                if !self.same_binding(candidate, id.span, file, binding, symbol) {
                    continue;
                }
                let at = id.span;
                if candidate
                    .ast
                    .assignments
                    .iter()
                    .any(|a| a.target.contains(at))
                    || candidate.ast.deleted_targets.iter().any(|d| d.contains(at))
                {
                    return false;
                }
                let receiver = candidate.ast.members.iter().any(|m| {
                    candidate.ast.peel_ts_sugar_span(m.object) == at
                        && !candidate.ast.computed_members.contains(&m.span)
                });
                if receiver {
                    continue;
                }
                let provider = candidate.ast.jsx_elements.iter().any(|e| {
                    let target = e.member_object.unwrap_or(e.name.span);
                    self.lookup
                        .is_context_reference(candidate.path.as_str(), target)
                        && e.attributes.iter().any(|a| {
                            candidate.source_text(a.local_name) == Some("value")
                                && a.expression == Some(at)
                        })
                });
                if object {
                    if provider {
                        continue;
                    }
                    return false;
                }
                let called = candidate
                    .ast
                    .calls
                    .iter()
                    .any(|c| candidate.ast.peel_ts_sugar_span(c.callee) == at);
                let returned = candidate
                    .ast
                    .returns
                    .iter()
                    .filter_map(|r| r.argument)
                    .any(|r| candidate.ast.peel_ts_sugar_span(r) == at);
                let property = candidate
                    .ast
                    .object_properties
                    .iter()
                    .any(|p| p.data && p.value == at);
                if !called && !returned && !property && !provider {
                    return false;
                }
            }
        }
        true
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
