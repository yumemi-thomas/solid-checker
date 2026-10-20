//! Resolver queries: source origins, calls, array members, roles, and context-read arguments.

use super::*;

impl Resolver<'_, '_> {
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
        let mut calls = HashMap::default();
        let mut unavailable = HashSet::default();
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
                        let function = file.ast.function_at(*span).expect("resolved array member");
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
                // Callable identity is not constructability. In particular,
                // a type assertion cannot turn a returned method/arrow into
                // a constructor whose body runs. Keep this transport surface
                // explicit rather than projecting its ordinary-call summary.
                if call.construct && transported {
                    unavailable.insert(location(call_file.path.shared(), call.callee));
                    continue;
                }
                if transported {
                    match value {
                        Some(Value::Callable(Callable::Function { path, span }))
                            if !self.functions.contains(&(path.clone(), span)) =>
                        {
                            let file = self
                                .lookup
                                .file_by_path(&path)
                                .expect("resolved function file");
                            let function =
                                file.ast.function_at(span).expect("resolved function span");
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
                        Some(Value::VetoedTuple) => {
                            unavailable.insert(location(call_file.path.shared(), call.callee));
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
        if super::super::runs_in_object_callable(file, call.span)
            || crate::execution_role::named_callback_execution_role(file, call.callee, self.lookup)
                .is_some()
        {
            return None;
        }
        if call.arguments.len() != 1 || call.arguments[0].spread {
            return None;
        }
        let callback_span = file.ast.peel_ts_sugar_span(call.arguments[0].span);
        let callback = file.ast.function_at(callback_span)?;
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
            || !super::super::body_site_runs_during_call(file, callback, invocation.span)
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
            let selected = self.lookup.file_by_path(&path)?.ast.function_at(span)?;
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
            symbol: symbol.clone(),
            display,
            kind: Some("accessor".into()),
            declaration,
            origin: location(file.path.shared(), span),
            origin_context: crate::owners::enclosing_function_label(file, span),
            owner: None,
            local: false,
            conditional: false,
            undecided: false,
        })
    }

    pub(super) fn role(&self, file: &FileFacts, call: &ast::CallFact) -> Option<ContextRole> {
        let primitive = crate::call_primitive_name_cached(
            self.lookup,
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

    /// Every use of this exact parameter only reads a context, directly or
    /// through another current source function with the same complete census.
    /// A provider, mutation, alias, export, return or opaque handoff refuses.
    pub(super) fn context_read_only_argument(
        &self,
        file: &FileFacts,
        call: &ast::CallFact,
        position: usize,
        depth: usize,
    ) -> bool {
        if depth == 0
            || call.construct
            || call.arguments.iter().take(position + 1).any(|a| a.spread)
        {
            return false;
        }
        if self.role(file, call) == Some(ContextRole::Read) {
            return position == 0 && call.arguments.len() == 1;
        }
        let Some((target_file, function)) = self
            .lookup
            .function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
        else {
            return false;
        };
        let Some(parameter) = function.parameters.get(position) else {
            return false;
        };
        if function.rest_parameter
            || function.r#async
            || function.generator
            || !self.lookup.function_value_is_current(target_file, function)
            || parameter.initializer.is_some()
            || parameter.shape != ast::BindingShape::Identifier
            || parameter.names.len() != 1
            || crate::indexes::binding_written(target_file, parameter.pattern)
            || target_file.ast.identifiers.iter().any(|id| {
                function.body.contains(id.span)
                    && id.role == ast::IdentifierRole::Reference
                    && matches!(target_file.source_text(id.span), Some("arguments" | "eval"))
            })
        {
            return false;
        }
        let mut references = target_file
            .ast
            .identifiers
            .iter()
            .filter(|id| {
                id.role == ast::IdentifierRole::Reference
                    && target_file.ast.reference_declaration(id.span)
                        == Some(parameter.names[0].span)
            })
            .peekable();
        references.peek().is_some()
            && references.all(|id| {
                target_file.ast.calls.iter().any(|inner| {
                    inner
                        .arguments
                        .iter()
                        .enumerate()
                        .any(|(position, argument)| {
                            target_file.ast.peel_ts_sugar_span(argument.span) == id.span
                                && self.context_read_only_argument(
                                    target_file,
                                    inner,
                                    position,
                                    depth - 1,
                                )
                        })
                })
            })
    }
}
