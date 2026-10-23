//! ADR 0372: the own keys a function's returned object can hold, read from
//! one module's syntax.
//!
//! A spread of a call's result (`<div {...attrs(x)} />`) hands the element
//! whatever keys the returned object holds. When every value a function can
//! return is an object this module builds from static keys and no other code
//! can reach before the return, those keys are a fixed set. Anything else
//! (a parameter, a spread, a computed key, a getter, an object handed to a
//! call, a binding a nested function captures) answers `None`.

use std::collections::BTreeSet;

use compact_str::CompactString;

use crate::core::Span;

use super::{AstFacts, BindingShape, FunctionFact, ImportFact, ImportKind};

/// How many module-local functions a `return f(…)` is followed through.
const RETURN_THROUGH_DEPTH: usize = 4;

impl AstFacts {
    /// The import a call's callee names, with the export it reads: a named or
    /// default import binding called directly (`attrs(x)`), or a static member
    /// of a namespace import (`stylex.attrs(x)`). Resolution is the binder's
    /// own (`reference_declaration`); a type-only import names nothing.
    #[must_use]
    pub fn imported_callee(
        &self,
        source: &str,
        callee: Span,
    ) -> Option<(&ImportFact, CompactString)> {
        let callee = self.peel_ts_sugar_span(callee);
        let import_binding = |reference: Span| {
            let declaration = self.reference_declaration(reference)?;
            self.imports
                .iter()
                .filter(|import| !import.type_only)
                .find_map(|import| {
                    import
                        .bindings
                        .iter()
                        .find(|binding| !binding.type_only && binding.local.span == declaration)
                        .map(|binding| (import, binding))
                })
        };
        if let Some(member) = self.member_at(callee) {
            if self.computed_members.binary_search(&member.span).is_ok()
                || self.optional_members.binary_search(&member.span).is_ok()
            {
                return None;
            }
            let (import, binding) = import_binding(self.peel_ts_sugar_span(member.object))?;
            if binding.kind != ImportKind::Namespace {
                return None;
            }
            let name = source.get(member.property.start as usize..member.property.end as usize)?;
            return Some((import, name.into()));
        }
        let (import, binding) = import_binding(callee)?;
        match binding.kind {
            ImportKind::Named | ImportKind::Default => Some((import, binding.imported.clone()?)),
            ImportKind::Namespace | ImportKind::SideEffect => None,
        }
    }

    /// The function this module exports under `name`, when the export is a
    /// local binding no assignment in the module rewrites: a function
    /// declaration, or a `const` whose initializer is a function or arrow.
    /// A re-export (`export { f } from "./x"`) answers `None`.
    #[must_use]
    pub fn exported_function(&self, name: &str) -> Option<&FunctionFact> {
        let local = self.module_level_exports().find_map(|export| {
            (export.module.is_none() && !export.type_only)
                .then(|| {
                    export
                        .specifiers
                        .iter()
                        .chain(&export.declarations)
                        .find(|item| !item.type_only && item.exported == name)
                })
                .flatten()
        })?;
        let declaration = self
            .reference_declaration(local.local.span)
            .unwrap_or(local.local.span);
        self.local_function(declaration)
    }

    /// The function the binding declared at `declaration` holds for good.
    fn local_function(&self, declaration: Span) -> Option<&FunctionFact> {
        if self.binding_rewritten(declaration) {
            return None;
        }
        if let Some(function) = self.functions.iter().find(|function| {
            function.kind == super::FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
        }) {
            return Some(function);
        }
        let binding = self.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declaration
        })?;
        let initializer = self.peel_ts_sugar_span(binding.initializer?);
        self.functions
            .iter()
            .find(|function| function.span == initializer)
    }

    /// Whether any assignment in the module writes the binding declared at
    /// `declaration` (an `=`, a compound assignment or an update).
    fn binding_rewritten(&self, declaration: Span) -> bool {
        self.assignments.iter().any(|assignment| {
            self.reference_declaration(self.peel_ts_sugar_span(assignment.target))
                == Some(declaration)
        })
    }

    /// The keys every object `function` can return may hold as own
    /// properties, or `None` when some return is not such an object.
    ///
    /// A return reads as one of: no value (`return;`, spreading `undefined`
    /// adds nothing); a closed object literal (static, unduplicated,
    /// non-`__proto__`, non-integer keys, no spread, getter or setter); a
    /// `const` of this function's own frame initialized by such a literal,
    /// every other reference to which is the object of a static-key member
    /// assignment written directly in this function (`result.class = …`,
    /// `result["data-x"] = …`), so its keys are the literal's and the written
    /// ones; or a direct call of a module-local function no assignment
    /// rewrites, whose own returns read the same way (four functions deep).
    /// A function that falls off its end returns `undefined`, and a throw
    /// returns nothing; neither adds a key. Async functions and generators
    /// return a promise or an iterator and answer `None`.
    #[must_use]
    pub fn returned_object_keys(
        &self,
        source: &str,
        function: &FunctionFact,
    ) -> Option<BTreeSet<CompactString>> {
        self.returned_keys_within(source, function, RETURN_THROUGH_DEPTH)
    }

    fn returned_keys_within(
        &self,
        source: &str,
        function: &FunctionFact,
        depth: usize,
    ) -> Option<BTreeSet<CompactString>> {
        if function.r#async || function.generator {
            return None;
        }
        let mut keys = BTreeSet::new();
        if function.expression_body {
            let returned = function.expression_return.as_ref()?;
            keys.extend(self.value_keys(source, function, returned.argument?, depth)?);
            return Some(keys);
        }
        for returned in self.own_returns(function) {
            if let Some(argument) = returned.argument {
                keys.extend(self.value_keys(source, function, argument, depth)?);
            }
        }
        Some(keys)
    }

    /// The own keys of the value `function` returns at `argument`.
    fn value_keys(
        &self,
        source: &str,
        function: &FunctionFact,
        argument: Span,
        depth: usize,
    ) -> Option<BTreeSet<CompactString>> {
        let argument = self.peel_ts_sugar_span(argument);
        if let Some(keys) = self.literal_keys(argument) {
            return Some(keys);
        }
        if let Some(call) = self.call_at(argument) {
            if call.construct || depth == 0 {
                return None;
            }
            let declaration = self.reference_declaration(self.peel_ts_sugar_span(call.callee))?;
            let callee = self.local_function(declaration)?;
            if callee.span == function.span {
                return None;
            }
            return self.returned_keys_within(source, callee, depth - 1);
        }
        self.built_object_keys(source, function, argument)
    }

    /// The keys of the closed object literal at `span`.
    fn literal_keys(&self, span: Span) -> Option<BTreeSet<CompactString>> {
        let shape = self
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == span)?;
        if !shape.closed
            || shape
                .properties
                .iter()
                .any(|property| property.getter.is_some())
        {
            return None;
        }
        Some(
            shape
                .properties
                .iter()
                .map(|property| property.key.clone())
                .collect(),
        )
    }

    /// The keys of the `const` returned at `reference`, built in `function`
    /// from a closed literal and static-key member assignments only.
    fn built_object_keys(
        &self,
        source: &str,
        function: &FunctionFact,
        reference: Span,
    ) -> Option<BTreeSet<CompactString>> {
        let declaration = self.reference_declaration(reference)?;
        let binding = self.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declaration
        })?;
        if !function.body.contains(binding.declaration)
            || self.innermost_function(binding.declaration) != Some(function.span)
        {
            return None;
        }
        let mut keys = self.literal_keys(self.peel_ts_sugar_span(binding.initializer?))?;
        let returned = |span: Span| {
            self.own_returns(function).any(|fact| {
                fact.argument
                    .is_some_and(|argument| self.peel_ts_sugar_span(argument) == span)
            })
        };
        for (use_span, _) in self
            .reference_declarations
            .iter()
            .filter(|(_, target)| *target == declaration)
        {
            if self.innermost_function(*use_span) != Some(function.span) {
                return None;
            }
            if returned(*use_span) {
                continue;
            }
            keys.insert(self.written_key(source, *use_span)?);
        }
        Some(keys)
    }

    /// The static key of the member assignment whose object is the
    /// identifier at `object`: `object.key = …` or `object["key"] = …`.
    fn written_key(&self, source: &str, object: Span) -> Option<CompactString> {
        let member = self
            .members
            .iter()
            .find(|member| self.peel_ts_sugar_span(member.object) == object)?;
        if !self
            .assignments
            .iter()
            .any(|assignment| assignment.target == member.span)
            || self.optional_members.binary_search(&member.span).is_ok()
        {
            return None;
        }
        let key: CompactString = if self.computed_members.binary_search(&member.span).is_ok() {
            self.literal_computed_members
                .iter()
                .find(|literal| literal.span == member.span)?
                .key
                .clone()
        } else {
            source
                .get(member.property.start as usize..member.property.end as usize)?
                .into()
        };
        (key != "__proto__").then_some(key)
    }

    /// The span of the innermost function whose span contains `span`.
    fn innermost_function(&self, span: Span) -> Option<Span> {
        self.functions
            .iter()
            .filter(|function| function.span.contains(span))
            .min_by_key(|function| function.span.end - function.span.start)
            .map(|function| function.span)
    }
}

#[cfg(test)]
mod tests;
