//! The resolver's world: every question its walks ask of the program and of
//! the resolver's own tables (S7 design §3, "The world trait").
//!
//! Two worlds answer them. The resolver itself ([`Resolver`]) answers from
//! the build's lookup and the tables it holds: a fresh build, and every
//! query after construction. The memo world
//! ([`super::resolver_memos::MemoWorld`]) answers inside a memo
//! computation, each answer a recorded read. The walks
//! ([`super::resolver_values::Values`] and its siblings) are written once,
//! over this trait.

use std::borrow::Cow;

use super::*;

/// One context's providers in file order: the value each renders, or `None`
/// for one whose value is hidden, spread or repeated.
pub(super) type ProviderValues = [Option<(String, Span)>];

pub(super) trait World<'w> {
    fn dialect(&self) -> &'w dyn solid_dialect::Dialect;
    fn program_closed(&self) -> bool;
    /// The analyzed file at `path` ([`SemanticLookup::file_by_path`]).
    fn file(&self, path: &str) -> Option<&'w FileFacts>;
    /// The entity symbol at exactly `span` of `file` (`EntitySymbols::at`).
    fn symbol_at(&self, file: &'w FileFacts, span: Span) -> Option<&'w SymbolId>;
    fn binding_at_reference(
        &self,
        file: &'w FileFacts,
        span: Span,
    ) -> Option<(&'w FileFacts, &'w ast::BindingFact, SymbolId)>;
    /// The project function the callee symbol at `span` names
    /// (`callee_symbol`, then `function_for_symbol`).
    fn callee_function(
        &self,
        file: &'w FileFacts,
        span: Span,
    ) -> Option<(&'w FileFacts, &'w ast::FunctionFact)>;
    fn function_called_at(
        &self,
        file: &'w FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w ast::FunctionFact)>;
    fn namespace_member_function(
        &self,
        file: &'w FileFacts,
        callee: Span,
    ) -> Option<(&'w FileFacts, &'w ast::FunctionFact)>;
    /// The dialect primitive `call` resolves to (`call_primitive_name`).
    fn primitive(
        &self,
        file: &'w FileFacts,
        call: &'w ast::CallFact,
    ) -> Option<solid_dialect::Primitive>;
    /// [`crate::value_identity::binding_has_write`].
    fn binding_has_write(&self, file: &'w FileFacts, symbol: &SymbolId) -> bool;
    /// [`slot_closure`].
    fn slot_closure(&self, file: &'w FileFacts, factory: Span, element: Span) -> Option<Span>;
    /// [`source_has_runtime_witness`] over the build's accessors.
    fn source_has_runtime_witness(&self, symbol: &SymbolId) -> bool;
    /// Whether member stability vetoes `name` of an object created in `owner`.
    fn member_vetoed(&self, owner: &str, name: &str) -> bool;
    /// [`dependency_member_stable`].
    fn dependency_member_stable(&self, path: &str, object: Span, name: &str) -> bool;
    /// ADR 0282: a visible mutation may change the array iteration protocol.
    fn tuple_iteration(&self) -> bool;
    /// `eval` or a code-evaluating call makes every receiver channel opaque.
    fn receiver_channels_opaque(&self) -> bool;
    /// Whether source discovery found an accessor for `symbol`.
    fn accessor(&self, symbol: &SymbolId) -> bool;
    fn source_kind(&self, symbol: &SymbolId) -> Option<crate::ReactiveSourceKind>;

    /// The exact argument a parameter declared at `declaration` receives:
    /// its file (none when the lookup lacks it) and span.
    fn parameter_input(
        &self,
        file: &'w FileFacts,
        declaration: Span,
    ) -> Option<(Option<&'w FileFacts>, Span)>;
    fn binding_location(&self, symbol: &SymbolId) -> Option<Location>;
    /// The joined use census of a binding key.
    fn uses_of(&self, key: &Location) -> Option<ClosedUses>;
    fn providers_of(&self, key: &Location) -> Option<Cow<'_, ProviderValues>>;
    /// Whether some context provider is not attributed to one binding.
    fn unkeyed_provider(&self) -> bool;
    /// Whether some context has a provider.
    fn any_providers(&self) -> bool;
    /// Whether the receiver census blocked a context (empty during
    /// construction: the origins see no blocked context).
    fn blocked(&self, key: &Location) -> bool;

    /// The census's questions.
    fn is_context_reference(&self, file: &'w FileFacts, span: Span) -> bool;
    fn array_iteration_receiver(
        &self,
        file: &'w FileFacts,
        call: &'w ast::CallFact,
    ) -> Option<Span>;
    /// The parameter-input census's questions.
    fn function_call_sites(
        &self,
        file: &'w FileFacts,
        function: Span,
    ) -> Vec<(&'w FileFacts, Span)>;
    /// The Type Facts half of a parameter channel: the function is not
    /// exported per TypeScript, its one call (in `caller`, a slot of the
    /// same path) resolves validly, and the compiler's reference census of
    /// its name is exactly that callee.
    fn parameter_typefacts_admit(
        &self,
        file: &'w FileFacts,
        function: &'w ast::FunctionFact,
        name: Span,
        caller: &'w FileFacts,
        call: &'w ast::CallFact,
        callee: Span,
    ) -> bool;
}

/// The Type Facts half of [`World::parameter_typefacts_admit`], over the
/// lookup: what the census asked of it in place.
pub(super) fn parameter_typefacts_admit_in(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    function: &ast::FunctionFact,
    name: Span,
    caller: &FileFacts,
    call: &ast::CallFact,
    callee: Span,
) -> bool {
    let exported = lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript| {
            typescript.functions.iter().any(|f| {
                f.exported
                    && f.body.start_byte == u64::from(function.body.start)
                    && f.body.end_byte == u64::from(function.body.end)
            })
        });
    !exported
        && lookup
            .resolved_callee_call(caller, call.callee)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
        // Include the producer reference census too: a reference in
        // excluded/missing source is never evidence of closure.
        && lookup
            .symbol_at(file.path.as_str(), name)
            .is_some_and(|symbol| {
                symbol.references().all(|reference| {
                    reference.path.as_ref() == caller.path.as_str()
                        && reference.start_byte == u64::from(callee.start)
                        && reference.end_byte == u64::from(callee.end)
                })
            })
}

impl<'a, 'facts: 'a> World<'a> for Resolver<'a, 'facts> {
    fn dialect(&self) -> &'a dyn solid_dialect::Dialect {
        self.lookup.dialect
    }

    fn program_closed(&self) -> bool {
        self.lookup.program_closed
    }

    fn file(&self, path: &str) -> Option<&'a FileFacts> {
        self.lookup.file_by_path(path)
    }

    fn symbol_at(&self, file: &'a FileFacts, span: Span) -> Option<&'a SymbolId> {
        self.lookup.entities().at(file.path.as_str(), span)
    }

    fn binding_at_reference(
        &self,
        file: &'a FileFacts,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a ast::BindingFact, SymbolId)> {
        self.lookup.binding_at_reference(file.path.as_str(), span)
    }

    fn callee_function(
        &self,
        file: &'a FileFacts,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a ast::FunctionFact)> {
        self.lookup
            .callee_symbol(file, span)
            .and_then(|symbol| self.lookup.function_for_symbol(symbol))
    }

    fn function_called_at(
        &self,
        file: &'a FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a ast::FunctionFact)> {
        self.lookup.function_called_at(file.path.as_str(), callee)
    }

    fn namespace_member_function(
        &self,
        file: &'a FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a ast::FunctionFact)> {
        self.lookup.namespace_member_function(file, callee)
    }

    fn primitive(
        &self,
        file: &'a FileFacts,
        call: &'a ast::CallFact,
    ) -> Option<solid_dialect::Primitive> {
        crate::call_primitive_name_cached(
            self.lookup,
            file,
            call,
            self.lookup.entities(),
            self.names,
            self.lookup.dialect,
        )?
        .primitive()
    }

    fn binding_has_write(&self, file: &'a FileFacts, symbol: &SymbolId) -> bool {
        crate::value_identity::binding_has_write(file, self.lookup.entities(), symbol)
    }

    fn slot_closure(&self, file: &'a FileFacts, factory: Span, element: Span) -> Option<Span> {
        slot_closure(file, factory, element, self.lookup)
    }

    fn source_has_runtime_witness(&self, symbol: &SymbolId) -> bool {
        source_has_runtime_witness(
            symbol,
            self.accessors,
            self.lookup.entities(),
            self.names,
            self.lookup,
        )
    }

    fn member_vetoed(&self, owner: &str, name: &str) -> bool {
        match self.member_stability.state_of(self.facts, owner, name) {
            Stability::Vetoed(_site) => true,
            Stability::Assumed => false,
        }
    }

    fn dependency_member_stable(&self, path: &str, object: Span, name: &str) -> bool {
        dependency_member_stable(self.lookup, self.member_stability, path, object, name)
    }

    fn tuple_iteration(&self) -> bool {
        self.member_stability.tuple_iteration
    }

    fn receiver_channels_opaque(&self) -> bool {
        self.receiver_channels_opaque
    }

    fn accessor(&self, symbol: &SymbolId) -> bool {
        self.accessors.contains_key(symbol)
    }

    fn source_kind(&self, symbol: &SymbolId) -> Option<crate::ReactiveSourceKind> {
        self.source_kinds.get(symbol).copied()
    }

    fn parameter_input(
        &self,
        file: &'a FileFacts,
        declaration: Span,
    ) -> Option<(Option<&'a FileFacts>, Span)> {
        self.parameter_inputs
            .get(&location(file.path.shared(), declaration))
            .map(|(path, span)| (self.lookup.file_by_path(path), *span))
    }

    fn binding_location(&self, symbol: &SymbolId) -> Option<Location> {
        self.binding_locations.get(symbol).cloned()
    }

    fn uses_of(&self, key: &Location) -> Option<ClosedUses> {
        self.uses.get(key).copied()
    }

    fn providers_of(&self, key: &Location) -> Option<Cow<'_, ProviderValues>> {
        self.providers
            .get(key)
            .map(|providers| Cow::Borrowed(providers.as_slice()))
    }

    fn unkeyed_provider(&self) -> bool {
        self.unkeyed_provider
    }

    fn any_providers(&self) -> bool {
        !self.providers.is_empty()
    }

    fn blocked(&self, key: &Location) -> bool {
        self.blocked_contexts.contains(key)
    }

    fn is_context_reference(&self, file: &'a FileFacts, span: Span) -> bool {
        self.lookup.is_context_reference(file.path.as_str(), span)
    }

    fn array_iteration_receiver(
        &self,
        file: &'a FileFacts,
        call: &'a ast::CallFact,
    ) -> Option<Span> {
        array_iteration_receiver(file, call, self.lookup)
    }

    fn function_call_sites(
        &self,
        file: &'a FileFacts,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.lookup
            .function_call_sites(file.path.as_str(), function)
    }

    fn parameter_typefacts_admit(
        &self,
        file: &'a FileFacts,
        function: &'a ast::FunctionFact,
        name: Span,
        caller: &'a FileFacts,
        call: &'a ast::CallFact,
        callee: Span,
    ) -> bool {
        parameter_typefacts_admit_in(self.lookup, file, function, name, caller, call, callee)
    }
}
