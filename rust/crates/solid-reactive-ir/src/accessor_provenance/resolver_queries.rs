//! Resolver queries: source origins, calls, array members, roles, and context-read arguments.

use super::*;

impl<'a, 'facts: 'a> Resolver<'a, 'facts> {
    pub fn has_source_origin(&self, symbol: &SymbolId) -> bool {
        self.source_origins.contains(symbol)
    }
    pub fn has_runtime_source_origin(&self, symbol: &SymbolId) -> bool {
        self.runtime_source_origins.contains(symbol)
    }
    pub fn has_extra_source_origins(&self) -> bool {
        !self.source_origins.is_empty()
    }
    pub fn source_call(&self, file: &'a FileFacts, callee: Span) -> Option<SymbolId> {
        <Self as Queries<'a>>::source_call(self, file, callee)
    }
    /// Every call's transport answer ([`Transport`]), by callee, in file
    /// order: the transported targets and the calls transport cannot follow.
    pub fn calls(&self) -> (HashMap<Location, Callable>, HashSet<Location>) {
        let mut calls = HashMap::default();
        let mut unavailable = HashSet::default();
        for call_file in &self.facts.files {
            let path = call_file.path.shared();
            fold_transport_rows(
                &mut calls,
                &mut unavailable,
                &path,
                &transport_rows(self, call_file),
                &|path, span| self.functions.contains(&(path.to_owned(), span)),
            );
        }
        (calls, unavailable)
    }

    /// What [`Self::read`] reads of the resolver for `symbol`: its display
    /// and declaration.
    pub fn read_entry(&self, symbol: &SymbolId) -> Option<&(SymbolId, Location)> {
        self.accessors
            .get(symbol)
            .or_else(|| self.declarations.get(symbol))
    }

    pub fn read(&self, symbol: &SymbolId, file: &'a FileFacts, span: Span) -> Option<SummaryRead> {
        let (display, declaration) = self.read_entry(symbol).cloned()?;
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
}

/// The resolver's call questions, over any [`World`].
pub(super) trait Queries<'w>: World<'w> {
    fn source_call(&self, file: &'w FileFacts, callee: Span) -> Option<SymbolId> {
        match self.value(file, callee, 24, &mut false)? {
            Value::Callable(Callable::Accessor(symbol)) => Some(symbol),
            _ => None,
        }
    }

    fn role(&self, file: &'w FileFacts, call: &'w ast::CallFact) -> Option<ContextRole> {
        let primitive = self.primitive(file, call)?;
        let dialect = self.dialect();
        dialect.context_role(dialect.name_of(primitive)?)
    }

    /// Every use of this exact parameter only reads a context, directly or
    /// through another current source function with the same complete census.
    /// A provider, mutation, alias, export, return or opaque handoff refuses.
    fn context_read_only_argument(
        &self,
        file: &'w FileFacts,
        call: &'w ast::CallFact,
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
        let Some((target_file, function)) =
            self.function_called_at(file, file.ast.peel_ts_sugar_span(call.callee))
        else {
            return false;
        };
        let Some(parameter) = function.parameters.get(position) else {
            return false;
        };
        if function.rest_parameter
            || function.r#async
            || function.generator
            || !crate::indexes::function_value_is_current_in(target_file, function)
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

impl<'w, T: World<'w> + ?Sized> Queries<'w> for T {}

/// What transport answers at one call: the callable it carries, or that the
/// call's transported value cannot be followed (a construct of a transported
/// callable, a vetoed tuple, an array member or function the graph has no
/// node for but whose body runs code).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Transport {
    Target(Callable),
    Unavailable,
}

/// A call's transport answer before the graph's function set decides it:
/// the targets that need a summary node, each with whether its body runs
/// code (a call or a member access; `None`: the target does not resolve).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TransportRow {
    Decided(Transport),
    ArrayMembers(Vec<(String, Span)>, Vec<Option<bool>>),
    Function(String, Span, Option<bool>),
}

/// Folds one file's rows into the build's tables, in the file's call order
/// (a later row of the same callee wins, as it always did). `functions`
/// answers whether the graph has a summary node for a function.
pub(crate) fn fold_transport_rows(
    calls: &mut HashMap<Location, Callable>,
    unavailable: &mut HashSet<Location>,
    path: &Arc<str>,
    rows: &[(Span, TransportRow)],
    functions: &impl Fn(&str, Span) -> bool,
) {
    for (callee, row) in rows {
        let transport = match row {
            TransportRow::Decided(transport) => transport.clone(),
            TransportRow::ArrayMembers(targets, runs_code) => {
                if targets.iter().all(|(path, span)| functions(path, *span)) {
                    Transport::Target(Callable::ArrayMembers(targets.clone()))
                } else if runs_code
                    .iter()
                    .any(|runs| runs.expect("resolved array member"))
                {
                    Transport::Unavailable
                } else {
                    continue;
                }
            }
            TransportRow::Function(path, span, runs_code) => {
                if functions(path, *span) {
                    Transport::Target(Callable::Function {
                        path: path.clone(),
                        span: *span,
                    })
                } else if runs_code.expect("resolved function") {
                    // Preserve the missing implementation obligation
                    // without admitting a synthetic node or inferring its
                    // summary.
                    Transport::Unavailable
                } else {
                    continue;
                }
            }
        };
        let callee = location(path.clone(), *callee);
        match transport {
            Transport::Target(callable) => {
                calls.insert(callee, callable);
            }
            Transport::Unavailable => {
                unavailable.insert(callee);
            }
        }
    }
}

/// The question transport asks beyond the resolver's world.
pub(super) trait TransportWorld<'w>: World<'w> {
    /// Whether the callback containing `span` of `file` has a named
    /// callback's execution role (`named_callback_execution_role`).
    fn named_callback(&self, file: &'w FileFacts, span: Span) -> bool;
}

impl<'a, 'facts: 'a> TransportWorld<'a> for Resolver<'a, 'facts> {
    fn named_callback(&self, file: &'a FileFacts, span: Span) -> bool {
        crate::execution_role::named_callback_execution_role(file, span, self.lookup).is_some()
    }
}

/// Whether the function at `span` of `path` runs code of its own: a call or
/// a member access in its body; `None` when it does not resolve.
fn body_runs_code<'w>(world: &impl World<'w>, path: &str, span: Span) -> Option<bool> {
    let file = world.file(path)?;
    let function = file.ast.function_at(span)?;
    Some(
        file.ast
            .calls
            .iter()
            .any(|c| function.body.contains(c.span))
            || file
                .ast
                .members
                .iter()
                .any(|m| function.body.contains(m.span)),
    )
}

/// The transport row of every call of `call_file` that has one, in call
/// order.
pub(super) fn transport_rows<'w>(
    world: &impl TransportWorld<'w>,
    call_file: &'w FileFacts,
) -> Vec<(Span, TransportRow)> {
    let mut rows = Vec::new();
    for call in &call_file.ast.calls {
        if let Some(targets) = array_members(world, call_file, call) {
            let runs_code = targets
                .iter()
                .map(|(path, span)| body_runs_code(world, path, *span))
                .collect();
            rows.push((call.callee, TransportRow::ArrayMembers(targets, runs_code)));
            continue;
        }
        let mut transported = false;
        let value = world.value(call_file, call.callee, 24, &mut transported);
        // Callable identity is not constructability. In particular,
        // a type assertion cannot turn a returned method/arrow into
        // a constructor whose body runs. Keep this transport surface
        // explicit rather than projecting its ordinary-call summary.
        if call.construct && transported {
            rows.push((call.callee, TransportRow::Decided(Transport::Unavailable)));
            continue;
        }
        if transported {
            match value {
                Some(Value::Callable(Callable::Function { path, span })) => {
                    let runs_code = body_runs_code(world, &path, span);
                    rows.push((call.callee, TransportRow::Function(path, span, runs_code)));
                }
                Some(Value::Callable(target)) => {
                    rows.push((
                        call.callee,
                        TransportRow::Decided(Transport::Target(target)),
                    ));
                }
                Some(Value::VetoedTuple) => {
                    rows.push((call.callee, TransportRow::Decided(Transport::Unavailable)));
                }
                _ => {}
            }
        }
    }
    rows
}

/// The exact element-member callbacks a built-in `map`/`forEach` call of an
/// array literal invokes, when every element's member is a synchronous arrow
/// invoked exactly once through the callback's own parameter.
fn array_members<'w>(
    world: &impl TransportWorld<'w>,
    file: &'w FileFacts,
    call: &'w ast::CallFact,
) -> Option<Vec<(String, Span)>> {
    let receiver = world.array_iteration_receiver(file, call)?;
    // A stored or named callback's witnessed use cannot establish its
    // other invocations. Leave those definitions and obligations alone;
    // only direct call-site execution and ordinary helper entry compose.
    // (The named-callback role is asked last: every check is a conjunct.)
    if super::super::runs_in_object_callable(file, call.span) {
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
    let Value::Array { path, elements } = world.value(file, receiver, 24, &mut false)? else {
        return None;
    };
    // The array and its element objects are created in `path`.
    if ["map", "forEach", name]
        .into_iter()
        .any(|member| world.member_vetoed(&path, member))
    {
        return None;
    }
    let array_file = world.file(&path)?;
    let mut targets = Vec::new();
    for element in elements {
        let properties = closed_properties(array_file, element)?;
        let property = properties.into_iter().find(|p| {
            array_file
                .source_text(p.key)
                .map(|s| s.trim_matches(['\'', '"']))
                == Some(name)
        })?;
        let Value::Callable(Callable::Function { path, span }) = world.value(
            array_file,
            property.shorthand_binding.unwrap_or(property.value),
            24,
            &mut false,
        )?
        else {
            return None;
        };
        let selected = world.file(&path)?.ast.function_at(span)?;
        if selected.kind != ast::FunctionKind::Arrow || selected.r#async || selected.generator {
            return None;
        }
        if !targets.contains(&(path.clone(), span)) {
            targets.push((path, span));
        }
    }
    if targets.is_empty() || world.named_callback(file, call.callee) {
        return None;
    }
    Some(targets)
}
