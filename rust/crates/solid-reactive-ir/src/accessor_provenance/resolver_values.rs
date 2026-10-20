//! Resolver value resolution: values, members, tuple slots, providers, and context origins.

use super::*;

impl Resolver<'_, '_> {
    pub(super) fn value(
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
                Value::Callable(_) | Value::Tuple { .. } | Value::VetoedTuple => false,
            };
            if !closed {
                return None;
            }
            *transported = true;
            return Some(value);
        }
        if let Some(function) = file.ast.function_at(span) {
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
        if let Some(member) = file.ast.member_at(span) {
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
            let function = target_file.ast.function_at(span)?;
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
        if let Some(properties) = closed_properties(file, span) {
            // Admitting a method must not let another field leak the holder
            // through its receiver. Close every callable channel, including
            // function-valued aliases; unknown values withhold transport.
            if properties.iter().any(|property| property.method)
                && (!module_scoped_source(file)
                    || self.receiver_channels_opaque
                    || !properties.iter().all(|property| {
                        if property.value_kind == ast::RuntimeValueKind::Primitive {
                            return true;
                        }
                        let value = file.ast.peel_ts_sugar_span(property.value);
                        if let Some(function) = file.ast.function_at(value) {
                            // Receiver closure is independent of when an async
                            // body runs. Its invocation remains outside ordinary
                            // callable transport; an unused sibling need not veto
                            // this object's other exact methods.
                            return function.kind == ast::FunctionKind::Arrow
                                || crate::object_members::receiver_inert(file, function);
                        }
                        match self.value(
                            file,
                            property.shorthand_binding.unwrap_or(property.value),
                            depth - 1,
                            &mut false,
                        ) {
                            Some(Value::Callable(Callable::Function { path, span })) => self
                                .lookup
                                .file_by_path(&path)
                                .and_then(|owner| {
                                    owner
                                        .ast
                                        .function_at(span)
                                        .map(|function| (owner, function))
                                })
                                .is_some_and(|(owner, function)| {
                                    module_scoped_source(owner)
                                        && (function.kind == ast::FunctionKind::Arrow
                                            || crate::object_members::receiver_inert(
                                                owner, function,
                                            ))
                                }),
                            Some(Value::Callable(Callable::Accessor(symbol))) => {
                                self.accessors.contains_key(&symbol)
                            }
                            Some(
                                Value::Object { .. }
                                | Value::Array { .. }
                                | Value::Tuple { .. }
                                | Value::VetoedTuple,
                            ) => true,
                            _ => false,
                        }
                    }))
            {
                return None;
            }
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
            && !self.dependency_member_stable(file.path.as_str(), object, name)
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
            let selected = self.lookup.file_by_path(path)?.ast.function_at(*span)?;
            if selected.kind != ast::FunctionKind::Arrow
                && (!module_scoped_source(self.lookup.file_by_path(path)?)
                    || self.receiver_channels_opaque
                    || !crate::object_members::receiver_inert(
                        self.lookup.file_by_path(path)?,
                        selected,
                    ))
            {
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
        if bound_file.path != file.path || !simple_array_binding(binding) {
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
        if !*transported {
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
        // Retain the transported-call obligation after establishing this is
        // a tuple closure. A mutation withdraws its identity, never certifies
        // silence or substitutes another violating implementation.
        if self.member_stability.tuple_iteration {
            return Some(Value::VetoedTuple);
        }
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
        // ADR 0300 excludes unseen providers of selected-package contexts.
        // Visible providers, escapes and writes still participate below.
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
    pub(super) fn context_origin(
        &self,
        file: &FileFacts,
        span: Span,
        depth: usize,
    ) -> Option<Location> {
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
}
