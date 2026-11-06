//! Resolver dependency getters: closed dependency objects, stable members, derived objects, and accessor creation.

use super::*;

impl<'a, 'facts: 'a> Resolver<'a, 'facts> {
    /// ADR 0300: every visible value must carry the same reactive member
    /// claim. Unlike `dependency_getters`, this walk refuses open literals,
    /// unknown providers, mutable bindings and visible member/descriptor
    /// writes. The package integrity premise replaces the receiver escape
    /// census only for objects created by a selected package.
    pub fn proven_dependency_getters(
        &self,
        file: &'a FileFacts,
        span: Span,
        name: &str,
    ) -> Option<Vec<Location>> {
        let objects = self.closed_dependency_objects(file, span, 32)?;
        let mut getters = Vec::new();
        for (path, object) in objects {
            self.facts.dependency_source_root(&path)?;
            if !self.dependency_member_stable(&path, object, name) {
                return None;
            }
            let file = self.lookup.file_by_path(&path)?;
            let shape = file
                .ast
                .object_get_shapes
                .iter()
                .find(|s| s.span == object && s.closed)?;
            let property = shape.properties.iter().find(|p| p.key.as_str() == name)?;
            let getter = property.getter?;
            // The normalized entry call executes before any branch or user
            // operation, not merely somewhere in the getter's body.
            let call = file.ast.call_at(property.entry_call?)?;
            let source = self.source_call(file, call.callee)?;
            if !self.has_runtime_source_origin(&source) {
                return None;
            }
            let site = location(file.path.shared(), getter);
            if !getters.contains(&site) {
                getters.push(site);
            }
        }
        (!getters.is_empty()).then_some(getters)
    }

    fn closed_dependency_objects(
        &self,
        file: &'a FileFacts,
        span: Span,
        depth: usize,
    ) -> Option<Vec<(String, Span)>> {
        if depth == 0 {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        if let Some(choice) = file.ast.conditional_at(span) {
            // Both runtime alternatives must close. A missing alternative is
            // not permission to keep only the known package object.
            let mut values = self.closed_dependency_objects(file, choice.consequent, depth - 1)?;
            for value in self.closed_dependency_objects(file, choice.alternate, depth - 1)? {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            return (!values.is_empty()).then_some(values);
        }
        if let Some(shape) = file.ast.object_get_shapes.iter().find(|s| s.span == span) {
            return shape.closed.then(|| vec![(file.path.to_string(), span)]);
        }
        if let Some(member) = file.ast.member_at(span) {
            if file.ast.computed_members.contains(&span) {
                return None;
            }
            let name = file.source_text(member.property)?;
            let mut values = Vec::new();
            for (path, object) in self.closed_dependency_objects(file, member.object, depth - 1)? {
                if !self.dependency_member_stable(&path, object, name) {
                    return None;
                }
                let object_file = self.lookup.file_by_path(&path)?;
                let property = direct_data_properties(object_file, object)
                    .into_iter()
                    .find(|p| {
                        object_file
                            .source_text(p.key)
                            .map(|s| s.trim_matches(['\'', '"']))
                            == Some(name)
                    })?;
                values.extend(self.closed_dependency_objects(
                    object_file,
                    property.shorthand_binding.unwrap_or(property.value),
                    depth - 1,
                )?);
            }
            return (!values.is_empty()).then_some(values);
        }
        if let Some(call) = file.ast.call_at(span) {
            // This proves the value if this expression is evaluated, not
            // that the containing function always invokes it. A preceding
            // guard/throw cannot supply a different successful return value.
            if call.construct {
                return None;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
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
                self.facts
                    .dependency_source_root(context_file.path.as_str())?;
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
                if self.uses.get(&key).is_some_and(|uses| uses.context_escaped) {
                    return None;
                }
                let providers = self.providers.get(&key)?;
                let mut values = Vec::new();
                for provider in providers {
                    let (path, value) = provider.as_ref()?;
                    values.extend(self.closed_dependency_objects(
                        self.lookup.file_by_path(path)?,
                        *value,
                        depth - 1,
                    )?);
                }
                return (!values.is_empty()).then_some(values);
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let (target_file, function) = self
                .lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| self.lookup.namespace_member_function(file, callee))?;
            if function.r#async
                || function.generator
                || !self.lookup.function_value_is_current(target_file, function)
            {
                return None;
            }
            let returned = single_return(target_file, function)?;
            let returned = target_file.ast.peel_ts_sugar_span(returned);
            if let Some(position) =
                target_file
                    .ast
                    .reference_declaration(returned)
                    .and_then(|declaration| {
                        function.parameters.iter().position(|p| {
                            p.pattern == declaration && p.shape == ast::BindingShape::Identifier
                        })
                    })
            {
                let parameter = &function.parameters[position];
                if parameter.initializer.is_some()
                    || function.rest_parameter
                    || crate::indexes::binding_written(target_file, parameter.pattern)
                    || target_file.ast.identifiers.iter().any(|id| {
                        function.body.contains(id.span)
                            && id.role == ast::IdentifierRole::Reference
                            && matches!(
                                target_file.source_text(id.span),
                                Some("arguments" | "eval")
                            )
                    })
                {
                    return None;
                }
                let argument = call.arguments.get(position).filter(|a| !a.spread)?;
                if call.arguments.iter().take(position).any(|a| a.spread) {
                    return None;
                }
                return self.closed_dependency_objects(file, argument.span, depth - 1);
            }
            return self.closed_dependency_objects(target_file, returned, depth - 1);
        }
        if !exact_identifier(file, span) {
            return None;
        }
        let (bound_file, binding, symbol) =
            self.lookup.binding_at_reference(file.path.as_str(), span)?;
        if !binding.immutable
            || binding.names.len() != 1
            || crate::indexes::binding_written(bound_file, binding.names[0].span)
            || crate::value_identity::binding_has_write(bound_file, self.lookup.entities(), &symbol)
        {
            return None;
        }
        if binding.shape == ast::BindingShape::Object {
            let [slot] = binding.object_slots.as_slice() else {
                return None;
            };
            if !slot.plain_identifier_key || slot.local.span != binding.names[0].span {
                return None;
            }
            let mut values = Vec::new();
            for (path, object) in
                self.closed_dependency_objects(bound_file, binding.initializer?, depth - 1)?
            {
                // A package-owned data descriptor is stable under the same
                // integrity premise as an ordinary member projection. No
                // project receiver escape census is waived here.
                self.facts.dependency_source_root(&path)?;
                if !self.dependency_member_stable(&path, object, slot.property.as_str()) {
                    return None;
                }
                let owner = self.lookup.file_by_path(&path)?;
                let property = direct_data_properties(owner, object)
                    .into_iter()
                    .find(|p| {
                        owner
                            .source_text(p.key)
                            .map(|key| key.trim_matches(['\'', '"']))
                            == Some(slot.property.as_str())
                    })?;
                values.extend(self.closed_dependency_objects(
                    owner,
                    property.shorthand_binding.unwrap_or(property.value),
                    depth - 1,
                )?);
            }
            return (!values.is_empty()).then_some(values);
        }
        if binding.shape != ast::BindingShape::Identifier {
            return None;
        }
        let values = self.closed_dependency_objects(bound_file, binding.initializer?, depth - 1)?;
        // Project-owned values retain ADR 0273's ordinary receiver census.
        if values
            .iter()
            .any(|(path, _)| self.facts.dependency_source_root(path).is_none())
            && !self.binding_is_closed(bound_file, binding, true)
        {
            return None;
        }
        Some(values)
    }

    /// ADR 0289: derivation only, like [`Self::context_origin`]. The getters
    /// a Get of `name` on the value at `span` may run, when a package analyzed
    /// as source (ADR 0284) defines them and they read an exact accessor.
    ///
    /// The walk follows exact bindings, a resolved function's every return
    /// (a returned parameter continues at this call's argument), the data
    /// members of object literals and every provider of an exact context. It
    /// ignores every closure, census and member-stability requirement, so it
    /// is never a proof: its one consumer is an uncertifiable obligation.
    pub fn dependency_getters(&self, file: &'a FileFacts, span: Span, name: &str) -> Vec<Location> {
        let mut objects = Vec::new();
        let mut visited = HashSet::default();
        self.derived_objects(file, span, 24, &mut visited, &mut objects, &mut Vec::new());
        let mut getters = Vec::new();
        // Past the walk's bound the value may be any object: the site itself
        // stands for what was not walked.
        if objects.iter().any(|(path, _)| path.is_empty()) {
            objects.retain(|(path, _)| !path.is_empty());
            getters.push(location(file.path.shared(), span));
        }
        for (path, object) in objects {
            if self.facts.dependency_source_root(&path).is_none() {
                continue;
            }
            let Some(object_file) = self.package_file(&path) else {
                continue;
            };
            let Some(shape) = object_file
                .ast
                .object_get_shapes
                .iter()
                .find(|shape| shape.span == object)
            else {
                continue;
            };
            for getter in shape
                .properties
                .iter()
                .filter(|property| property.key.as_str() == name)
                .filter_map(|property| property.getter)
            {
                let site = location(object_file.path.shared(), getter);
                if !getters.contains(&site) && self.getter_reads_accessor(object_file, getter) {
                    getters.push(site);
                }
            }
        }
        getters
    }

    /// Whether the getter function at `getter` calls an exact accessor
    /// directly in its body: the read a Get of it may perform.
    fn getter_reads_accessor(&self, file: &'a FileFacts, getter: Span) -> bool {
        let Some(function) = file.ast.function_at(getter) else {
            return false;
        };
        file.ast.calls.iter().any(|call| {
            function.body.contains(call.span)
                && crate::owners::written_directly_in(&file.ast, function, call.span)
                && self.source_call(file, call.callee).is_some()
        })
    }

    /// The object literals the value at `span` may be, for
    /// [`Self::dependency_getters`]. `visited` breaks recursion through
    /// mutually recursive bindings and functions.
    fn derived_objects(
        &self,
        file: &'a FileFacts,
        span: Span,
        depth: usize,
        visited: &mut HashSet<Location>,
        objects: &mut Vec<(String, Span)>,
        open: &mut Vec<Location>,
    ) {
        let span = file.ast.peel_ts_sugar_span(span);
        if depth == 0 {
            objects.push((String::new(), span));
            return;
        }
        if !visited.insert(location(file.path.shared(), span)) {
            return;
        }
        if let Some(choice) = file.ast.conditional_at(span) {
            // May-provenance retains each known alternative even when its
            // sibling cannot supply the complete proof above.
            self.derived_objects(file, choice.consequent, depth - 1, visited, objects, open);
            self.derived_objects(file, choice.alternate, depth - 1, visited, objects, open);
            return;
        }
        let declaration = file.ast.reference_declaration(span).unwrap_or(span);
        if let Some((path, argument)) = self
            .parameter_inputs
            .get(&location(file.path.shared(), declaration))
        {
            if let Some(argument_file) = self.lookup.file_by_path(path) {
                self.derived_objects(argument_file, *argument, depth - 1, visited, objects, open);
            }
            return;
        }
        if file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == span)
        {
            objects.push((file.path.to_string(), span));
            return;
        }
        if let Some(member) = file.ast.member_at(span) {
            if file.ast.computed_members.contains(&span) {
                self.note_open(file, span, open);
                return;
            }
            let Some(name) = file.source_text(member.property) else {
                return;
            };
            let mut receivers = Vec::new();
            self.derived_objects(
                file,
                member.object,
                depth - 1,
                visited,
                &mut receivers,
                open,
            );
            for (path, object) in receivers {
                if path.is_empty() {
                    objects.push((path, object));
                    continue;
                }
                let Some(object_file) = self.lookup.file_by_path(&path) else {
                    continue;
                };
                for property in direct_data_properties(object_file, object) {
                    if object_file
                        .source_text(property.key)
                        .map(|key| key.trim_matches(['\'', '"']))
                        == Some(name)
                    {
                        self.derived_objects(
                            object_file,
                            property.shorthand_binding.unwrap_or(property.value),
                            depth - 1,
                            visited,
                            objects,
                            open,
                        );
                    }
                }
            }
            return;
        }
        if let Some(call) = file.ast.call_at(span) {
            if call.construct {
                self.note_open(file, span, open);
                return;
            }
            if self.role(file, call) == Some(ContextRole::Read) {
                let Some(argument) = call
                    .arguments
                    .first()
                    .filter(|argument| call.arguments.len() == 1 && !argument.spread)
                    .map(|argument| file.ast.peel_ts_sugar_span(argument.span))
                    .filter(|argument| exact_identifier(file, *argument))
                else {
                    return;
                };
                let Some(key) = self.binding_key(file, argument) else {
                    return;
                };
                for (path, value) in self.providers.get(&key).into_iter().flatten().flatten() {
                    if let Some(provider_file) = self.lookup.file_by_path(path) {
                        self.derived_objects(
                            provider_file,
                            *value,
                            depth - 1,
                            visited,
                            objects,
                            open,
                        );
                    }
                }
                return;
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let Some((target_file, function)) = self
                .lookup
                .function_called_at(file.path.as_str(), callee)
                .or_else(|| self.lookup.namespace_member_function(file, callee))
            else {
                self.note_open(file, span, open);
                return;
            };
            if function.r#async || function.generator {
                return;
            }
            let returns = function
                .expression_return
                .iter()
                .chain(target_file.ast.returns.iter().filter(|r| {
                    containing_ast_function(&target_file.ast, r.span)
                        .is_some_and(|f| f.span == function.span)
                }))
                .filter_map(|r| r.argument)
                .collect::<Vec<_>>();
            for returned in returns {
                let returned = target_file.ast.peel_ts_sugar_span(returned);
                // A returned parameter is this call's argument.
                let position =
                    target_file
                        .ast
                        .reference_declaration(returned)
                        .and_then(|declaration| {
                            function.parameters.iter().position(|parameter| {
                                parameter.pattern == declaration
                                    && parameter.shape == ast::BindingShape::Identifier
                            })
                        });
                match position {
                    Some(position) => {
                        if let Some(argument) = call
                            .arguments
                            .get(position)
                            .filter(|argument| !argument.spread)
                        {
                            self.derived_objects(
                                file,
                                argument.span,
                                depth - 1,
                                visited,
                                objects,
                                open,
                            );
                        }
                    }
                    None => {
                        self.derived_objects(
                            target_file,
                            returned,
                            depth - 1,
                            visited,
                            objects,
                            open,
                        );
                    }
                }
            }
            return;
        }
        if !exact_identifier(file, span) {
            if !file.source_text(span).is_some_and(plain_value_text) {
                self.note_open(file, span, open);
            }
            return;
        }
        let Some((bound_file, binding, _)) =
            self.lookup.binding_at_reference(file.path.as_str(), span)
        else {
            if self
                .lookup
                .function_called_at(file.path.as_str(), span)
                .is_none()
            {
                self.note_open(file, span, open);
            }
            return;
        };
        if binding.shape == ast::BindingShape::Object
            && binding.names.len() == 1
            && let [slot] = binding.object_slots.as_slice()
            && slot.plain_identifier_key
            && slot.local.span == binding.names[0].span
            && let Some(initializer) = binding.initializer
        {
            let mut receivers = Vec::new();
            self.derived_objects(
                bound_file,
                initializer,
                depth - 1,
                visited,
                &mut receivers,
                open,
            );
            for (path, object) in receivers {
                let Some(owner) = self.lookup.file_by_path(&path) else {
                    continue;
                };
                for property in direct_data_properties(owner, object) {
                    if owner
                        .source_text(property.key)
                        .map(|key| key.trim_matches(['\'', '"']))
                        == Some(slot.property.as_str())
                    {
                        self.derived_objects(
                            owner,
                            property.shorthand_binding.unwrap_or(property.value),
                            depth - 1,
                            visited,
                            objects,
                            open,
                        );
                    }
                }
            }
            return;
        }
        if binding.shape == ast::BindingShape::Identifier
            && binding.names.len() == 1
            && let Some(initializer) = binding.initializer
        {
            self.derived_objects(bound_file, initializer, depth - 1, visited, objects, open);
        } else {
            self.note_open(bound_file, binding.pattern, open);
        }
    }

    /// A file of a selected package, by path.
    pub fn package_file(&self, path: &str) -> Option<&'a FileFacts> {
        self.lookup.file_by_path(path)
    }

    /// ADR 0822: a value a selected package produces that the walk ends on.
    fn note_open(&self, file: &'a FileFacts, span: Span, open: &mut Vec<Location>) {
        if self
            .facts
            .dependency_source_root(file.path.as_str())
            .is_some()
        {
            open.push(location(file.path.shared(), span));
        }
    }

    /// ADR 0822: where a Get of `name` on the value at `span` may run what the
    /// package installs and no shape states: an accessor installed at run time
    /// on an object the value may be, or a value the package makes in a way
    /// the walk does not follow. `None` when the walk closed.
    pub fn dependency_open_value(
        &self,
        file: &'a FileFacts,
        span: Span,
        name: &str,
    ) -> Option<Location> {
        let mut objects = Vec::new();
        let mut open = Vec::new();
        let mut visited = HashSet::default();
        self.derived_objects(file, span, 24, &mut visited, &mut objects, &mut open);
        for (path, object) in &objects {
            if path.is_empty() || self.facts.dependency_source_root(path).is_none() {
                continue;
            }
            let Some(object_file) = self.package_file(path) else {
                continue;
            };
            if let Some(site) = self.installed_accessor(object_file, *object, name) {
                return Some(site);
            }
        }
        open.into_iter().find(|site| {
            let start = u32::try_from(site.start_byte).ok();
            let end = u32::try_from(site.end_byte).ok();
            let (Some(start), Some(end), Some(owner)) =
                (start, end, self.package_file(site.path.as_ref()))
            else {
                return true;
            };
            self.open_site_may_hold(owner, Span::new(start, end), name)
        })
    }

    /// A call of `Object.defineProperty`, `defineProperties` or their `Reflect`
    /// twins whose target may be `object` and whose descriptor may define
    /// `name` with a getter.
    fn installed_accessor(
        &self,
        file: &'a FileFacts,
        object: Span,
        name: &str,
    ) -> Option<Location> {
        file.ast
            .module_hazards
            .iter()
            .filter(|hazard| hazard.kind == ast::ModuleHazardKind::RuntimeAccessorInstallation)
            .filter_map(|hazard| {
                file.ast
                    .calls
                    .iter()
                    .find(|call| file.ast.peel_ts_sugar_span(call.callee) == hazard.span)
            })
            .find(|call| {
                let Some(target) = call.arguments.first() else {
                    return false;
                };
                let mut objects = Vec::new();
                self.derived_objects(
                    file,
                    target.span,
                    24,
                    &mut HashSet::default(),
                    &mut objects,
                    &mut Vec::new(),
                );
                objects
                    .iter()
                    .any(|(path, found)| *found == object && path == file.path.as_str())
                    && descriptor_may_define(file, call, name)
            })
            .map(|call| location(file.path.shared(), call.span))
    }

    /// Whether a value the walk ended on may run something on a Get of `name`.
    /// A standard container's instance does not, nor does an exact class
    /// with no `extends` and no getter under that name; anything else may.
    fn open_site_may_hold(&self, file: &'a FileFacts, span: Span, name: &str) -> bool {
        let Some(call) = file.ast.call_at(span).filter(|call| call.construct) else {
            return true;
        };
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if matches!(
            file.source_text(callee),
            Some("Map" | "Set" | "WeakMap" | "WeakSet" | "Date" | "URL" | "RegExp" | "Error")
        ) && file.ast.reference_declaration(callee).is_none()
        {
            return false;
        }
        let Some(declaration) = file.ast.reference_declaration(callee) else {
            return true;
        };
        let Some(class) = file
            .ast
            .classes
            .iter()
            .find(|class| class.name.as_ref().is_some_and(|n| n.span == declaration))
        else {
            return true;
        };
        class.heritage.is_some()
            || class.elements.iter().any(|element| {
                element.kind == ast::ClassElementKind::Getter
                    && element.key.is_none_or(|key| {
                        file.source_text(key)
                            .is_none_or(|text| text.trim_matches(['\'', '"']) == name)
                    })
            })
    }

    /// ADR 0822: whether a package function, run from outside it, may read a
    /// source: it calls one, a function that does, or a callee nothing
    /// resolves. A standard call does not.
    pub fn may_read_source(&self, file: &'a FileFacts, function: &'a ast::FunctionFact) -> bool {
        let mut seen = HashSet::default();
        let mut stack = vec![(file, function)];
        while let Some((in_file, holder)) = stack.pop() {
            if !seen.insert((in_file.path.as_str().to_owned(), holder.span)) {
                continue;
            }
            for call in in_file.ast.calls_within(holder.body) {
                if self.source_call(in_file, call.callee).is_some() {
                    return true;
                }
                let callee = in_file.ast.peel_ts_sugar_span(call.callee);
                if let Some(next) = self
                    .lookup
                    .function_called_at(in_file.path.as_str(), callee)
                {
                    stack.push(next);
                } else if self
                    .lookup
                    .resolved_callee_call(in_file, call.callee)
                    .is_none()
                {
                    return true;
                }
            }
        }
        false
    }
}

/// The resolver's accessor-creation and closure questions, over any [`World`].
pub(super) trait Dependencies<'w>: World<'w> {
    fn created_accessor(&self, file: &'w FileFacts, span: Span, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(call) = file.ast.call_at(file.ast.peel_ts_sugar_span(span)) else {
            return false;
        };
        if call.construct || !file.ast.straight_line_calls.contains(&call.span) {
            return false;
        }
        if let Some(primitive) = self.primitive(file, call) {
            let dialect = self.dialect();
            return dialect.creates_reactive_source(primitive)
                && !dialect.returns_reactive_tuple(primitive)
                && !dialect.returns_store(primitive);
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let target = if file.ast.identifiers.iter().any(|id| id.span == callee) {
            self.function_called_at(file, callee)
        } else {
            self.namespace_member_function(file, callee)
        };
        let Some((target_file, function)) = target else {
            return false;
        };
        !function.r#async
            && !function.generator
            && crate::indexes::function_value_is_current_in(target_file, function)
            && single_return(target_file, function)
                .is_some_and(|returned| self.created_accessor(target_file, returned, depth - 1))
    }

    fn binding_is_closed(
        &self,
        file: &'w FileFacts,
        binding: &'w ast::BindingFact,
        object: bool,
    ) -> bool {
        let key = location(file.path.shared(), binding.pattern);
        self.uses_of(&key).is_none_or(|uses| {
            if object {
                !uses.object_escaped
            } else {
                !uses.callable_escaped
            }
        })
    }
}

impl<'w, T: World<'w> + ?Sized> Dependencies<'w> for T {}

/// A literal's text: no value a Get runs anything on. Decides values, never types.
pub(crate) fn plain_value_text(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '"' | '\'' | '`' | '['))
        || matches!(text, "null" | "undefined" | "true" | "false")
}

/// Whether the descriptor arguments of an installing call may give `name` a
/// getter. A literal key names one member; a closed literal descriptor with
/// no `get` holds a value. Anything else may do either.
fn descriptor_may_define(file: &FileFacts, call: &ast::CallFact, name: &str) -> bool {
    let closed_without_get = |span: Span| {
        let span = file.ast.peel_ts_sugar_span(span);
        file.ast.object_get_shapes.iter().any(|shape| {
            shape.span == span
                && shape.closed
                && !shape.properties.iter().any(|p| p.key.as_str() == "get")
        })
    };
    match call.arguments.as_slice() {
        [_, key, descriptor] if !key.spread && !descriptor.spread => {
            let text = file
                .source_text(file.ast.peel_ts_sugar_span(key.span))
                .unwrap_or_default();
            let quoted = text.len() > 1 && (text.starts_with('"') || text.starts_with('\''));
            (!quoted || text.trim_matches(['\'', '"']) == name)
                && !closed_without_get(descriptor.span)
        }
        [_, descriptors] if !descriptors.spread => {
            let span = file.ast.peel_ts_sugar_span(descriptors.span);
            file.ast
                .object_get_shapes
                .iter()
                .find(|shape| shape.span == span && shape.closed)
                .is_none_or(|shape| shape.properties.iter().any(|p| p.key.as_str() == name))
        }
        _ => true,
    }
}

/// ADR 0300: whether no visible write can replace `name` of the object
/// literal at `object` in `path`: no named write anywhere, and every unknown
/// receiver and reflective wildcard target an exactly disjoint identity.
pub(super) fn dependency_member_stable(
    lookup: &SemanticLookup<'_>,
    stability: &MemberStability,
    path: &str,
    object: Span,
    name: &str,
) -> bool {
    // Named writes anywhere retain their veto. Every visible unknown
    // target needs an exact disjoint identity before the integrity proof.
    if stability.written.contains_key(name) {
        return false;
    }
    let object = location(path, object);
    // Visible syntax cannot disappear when receiver derivation is
    // incomplete. In particular a package's unknown receiver can be an
    // object from another selected package, or one handed it by the app.
    if !stability
        .unknown_receivers
        .iter()
        .chain(
            stability
                .dependencies
                .values()
                .flat_map(|p| &p.unknown_receivers),
        )
        .all(|receiver| {
            let Some(file) = lookup.file_by_path(receiver.path.as_ref()) else {
                return false;
            };
            let (Ok(start), Ok(end)) = (
                u32::try_from(receiver.start_byte),
                u32::try_from(receiver.end_byte),
            ) else {
                return false;
            };
            mutation_target_is_distinct(
                file,
                Span::new(start, end),
                &object,
                lookup,
                32,
                &mut HashSet::default(),
            )
        })
    {
        return false;
    }
    stability
        .wildcard_sites
        .iter()
        .chain(
            stability
                .dependencies
                .values()
                .flat_map(|package| &package.wildcard_sites),
        )
        .all(|site| {
            let Some(file) = lookup.file_by_path(site.path.as_ref()) else {
                return false;
            };
            let Some(call) = file.ast.calls.iter().find(|call| {
                u64::from(call.span.start) == site.start_byte
                    && u64::from(call.span.end) == site.end_byte
            }) else {
                return false;
            };
            // A target spread can supply the getter object itself.
            let Some(target) = call.arguments.first().filter(|a| !a.spread) else {
                return false;
            };
            mutation_target_is_distinct(
                file,
                target.span,
                &object,
                lookup,
                32,
                &mut HashSet::default(),
            )
        })
}
