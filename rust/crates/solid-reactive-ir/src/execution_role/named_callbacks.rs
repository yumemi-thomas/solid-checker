//! Named callback index: the roles of named callbacks and the apply-primitive classification.

use super::*;

/// How this file's primitive calls and control-flow JSX name each of its own
/// functions as a callback.
///
/// Every flag is a property of the *function*, not of the read being
/// classified: which argument positions name it, and how the dialect executes
/// those positions. Deriving them once per file replaces the whole-file call and
/// JSX scans [`named_callback_execution_role`] used to run for every read it was
/// asked about -- five of them per read in the worst case.
#[derive(Default)]
pub(crate) struct NamedCallbackRoles {
    pub(super) by_function: HashMap<Span, NamedCallbackRole>,
}

/// The classification a named callback's positions support, in the order
/// [`named_callback_execution_role`] consults them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct NamedCallbackRole {
    /// Some position naming this function is one an arm below can truthfully
    /// classify. An effect's tracked compute and an inline adapter argument
    /// (1.x `on`'s deps) satisfy none of them -- admitting those would fall
    /// through to the rendering tail and misreport `createEffect(namedCompute)`
    /// as untracked rendering. Answering None instead defers such reads to
    /// compiler facts and the returned-adapter classifiers.
    pub(super) admitted: bool,
    pub(super) untracked_callback: bool,
    pub(super) effect_apply: bool,
    /// Which effect primitive names this function at its apply position, for
    /// the read-context wording only.
    pub(super) effect_apply_primitive: EffectApplyPrimitive,
    pub(super) tracked: bool,
    pub(super) deferred: bool,
    /// Named at a position whose callback runs when the call's returned
    /// object is read ([`Dialect::callback_runs_on_result_access`]). That
    /// time and scope are the reader's, which this index cannot see, so the
    /// function's own role is unknown and outranks every arm above.
    pub(super) result_access: bool,
}

/// The effect primitive(s) whose apply position names a function.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum EffectApplyPrimitive {
    #[default]
    None,
    One(Primitive),
    /// Named by the apply positions of two different primitives: the wording
    /// names neither.
    Several,
}

impl EffectApplyPrimitive {
    fn with(self, primitive: Primitive) -> Self {
        match self {
            Self::None => Self::One(primitive),
            Self::One(existing) if existing == primitive => self,
            Self::One(_) | Self::Several => Self::Several,
        }
    }

    pub(super) const fn single(self) -> Option<Primitive> {
        match self {
            Self::One(primitive) => Some(primitive),
            Self::None | Self::Several => None,
        }
    }
}

impl NamedCallbackRoles {
    fn entry(&mut self, function: Span) -> &mut NamedCallbackRole {
        self.by_function.entry(function).or_default()
    }

    pub(super) fn get(&self, function: Span) -> NamedCallbackRole {
        self.by_function.get(&function).copied().unwrap_or_default()
    }
}

/// Every way this file's callback positions can name one of its own functions.
///
/// The three maps are the *semantic* alternatives to comparing source text: an
/// exact demanded entity, a proven TypeScript reference to the function's
/// symbol, and -- for a function whose symbol carries a canonical Solid name --
/// that name. Admitting a function because some identifier in a callback
/// position happens to be spelled the same, which is what these replace,
/// accepts a same-spelled binding from any other scope or module.
pub(super) struct NamedCallbackIndex<'a> {
    /// This file's functions that resolve to a symbol, in AST order.
    pub(super) functions: Vec<Span>,
    pub(super) by_symbol: HashMap<&'a str, Vec<usize>>,
    pub(super) by_reference: HashMap<Span, Vec<usize>>,
    pub(super) by_canonical_name: HashMap<&'a str, Vec<usize>>,
}

impl<'a> NamedCallbackIndex<'a> {
    fn new(
        file: &solid_facts::FileFacts,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolId>,
        lookup: &SemanticLookup<'_>,
    ) -> Self {
        let candidates = file
            .ast
            .functions
            .iter()
            .filter_map(|function| {
                Some((function.span, function_symbol(file, function, entities)?))
            })
            .collect::<Vec<_>>();
        let mut by_symbol = HashMap::<&str, Vec<usize>>::default();
        let mut by_reference = HashMap::<Span, Vec<usize>>::default();
        let mut by_canonical_name = HashMap::<&str, Vec<usize>>::default();
        for (index, (_, symbol)) in candidates.iter().enumerate() {
            by_symbol.entry(symbol.as_str()).or_default().push(index);
            if let Some(name) = symbol_names.get(*symbol) {
                by_canonical_name
                    .entry(name.as_str())
                    .or_default()
                    .push(index);
            }
            for reference in lookup.symbol_references(symbol.as_str()) {
                if reference.path.as_ref() != file.path.as_str() {
                    continue;
                }
                let (Ok(start), Ok(end)) = (
                    u32::try_from(reference.start_byte),
                    u32::try_from(reference.end_byte),
                ) else {
                    continue;
                };
                by_reference
                    .entry(Span::new(start, end))
                    .or_default()
                    .push(index);
            }
        }
        Self {
            functions: candidates.iter().map(|(span, _)| *span).collect(),
            by_symbol,
            by_canonical_name,
            by_reference,
        }
    }

    /// The functions an exact demanded entity at `span` names -- the whole of a
    /// bare `createEffect(compute, applyValue)` argument, say.
    fn identity_at(
        &self,
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        span: Span,
    ) -> &[usize] {
        entities
            .at(file.path.as_str(), span)
            .and_then(|symbol| self.by_symbol.get(symbol.as_str()))
            .map_or(&[], Vec::as_slice)
    }

    /// The functions a use at `span` names, by any of the three proofs.
    fn named_at(
        &self,
        file: &solid_facts::FileFacts,
        entities: &EntitySymbols,
        span: Span,
        matched: &mut Vec<usize>,
    ) {
        matched.extend(self.identity_at(file, entities, span));
        if let Some(references) = self.by_reference.get(&span) {
            matched.extend(references.iter().copied());
        }
        if let Some(text) = file.source_text(span)
            && let Some(named) = self.by_canonical_name.get(text)
        {
            matched.extend(named.iter().copied());
        }
    }
}

/// Derive [`NamedCallbackRoles`] for one file. Memoized by
/// [`SemanticLookup::named_callback_roles`]; nothing else should call it.
pub(crate) fn named_callback_roles(
    file: &solid_facts::FileFacts,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> NamedCallbackRoles {
    let dialect = lookup.dialect;
    let index = NamedCallbackIndex::new(file, entities, symbol_names, lookup);
    let mut roles = NamedCallbackRoles::default();
    if index.functions.is_empty() {
        return roles;
    }
    let primitives = lookup.primitives(file);
    let mut named = Vec::new();
    for (call_index, call) in file.ast.calls.iter().enumerate() {
        let direct_primitive = known_primitive(&primitives.calls[call_index]);
        let returned_primitive = direct_primitive
            .is_none()
            .then(|| returned_primitive_invocation(file, call, lookup))
            .flatten();
        if direct_primitive.is_none() && returned_primitive.is_none() {
            continue;
        }
        let count = call.arguments.len();
        for (argument_index, argument) in call.arguments.iter().enumerate() {
            // The argument itself, plus the callbacks an options object names
            // through its `effect`/`error` properties.
            named.clear();
            index.named_at(file, entities, argument.span, &mut named);
            for property in &argument.identifier_properties {
                index.named_at(file, entities, property.span, &mut named);
            }
            named.sort_unstable();
            named.dedup();
            if named.is_empty() {
                continue;
            }
            if let Some((_primitive, _result_slot)) = returned_primitive {
                let execution =
                    returned_callback_execution_at_call(file, call, argument_index, lookup);
                let tracked = execution == Some(Execution::Tracked);
                let deferred = execution == Some(Execution::Deferred);
                // Returned-function summaries describe the argument itself,
                // never a callback named inside an options object. Require
                // exact TypeScript identity for the admitted function. Inline
                // returned callbacks inherit the individual call site's role
                // and therefore cannot be collapsed into this per-function
                // index.
                for candidate in index.identity_at(file, entities, argument.span) {
                    let entry = roles.entry(index.functions[*candidate]);
                    entry.admitted |= tracked || deferred;
                    entry.tracked |= tracked;
                    entry.deferred |= deferred;
                }
                continue;
            }
            let primitive = direct_primitive.expect("one primitive kind is proven above");
            if dialect.callback_runs_on_result_access(primitive, argument_index, count) {
                for candidate in index.identity_at(file, entities, argument.span) {
                    let entry = roles.entry(index.functions[*candidate]);
                    entry.admitted = true;
                    entry.result_access = true;
                }
                continue;
            }
            let proven = callback_execution_at_call(file, call, primitive, argument_index, lookup)
                .is_some()
                || dialect
                    .effect_apply_defer_options(primitive, argument_index)
                    .is_some();
            // ADR 0323: a callback that runs in its call's own window has no
            // role of its own: it follows the call (the flow walk's `Call`).
            let inherits = dialect.callback_inherits_calling_window(primitive, argument_index);
            let untracked =
                !inherits && dialect.reports_untracked_reads_at(primitive, argument_index, count);
            // The role describes the phase if invoked. ADR 0302 gates every
            // diagnostic on the separate call-site invocation premise.
            let effect_apply =
                effect_apply_argument(dialect, primitive, count) == Some(argument_index);
            let tracked = !is_effect(primitive)
                && dialect
                    .callback_semantics_at(primitive, argument_index, count)
                    .tracks_reads;
            let deferred = !inherits
                && callback_runs_outside_tracking(dialect, primitive, argument_index, count);
            for candidate in &named {
                let entry = roles.entry(index.functions[*candidate]);
                entry.admitted |= proven && (untracked || effect_apply || tracked || deferred);
                entry.untracked_callback |= proven && untracked;
                entry.effect_apply |= effect_apply;
                if effect_apply {
                    entry.effect_apply_primitive = entry.effect_apply_primitive.with(primitive);
                }
            }
            // The tracked and deferred arms have always demanded that the whole
            // argument *be* the function, never that an options object mention
            // it.
            for candidate in index.identity_at(file, entities, argument.span) {
                let entry = roles.entry(index.functions[*candidate]);
                entry.tracked |= proven && tracked;
                entry.deferred |= proven && deferred;
            }
        }
    }
    // A *named* function bound to a JSX event handler -- `onPointerDown={onPointerDown}`
    // with the handler declared in the component body.
    //
    // The compiler censuses the attribute, so `callback_roles` carries an
    // `EventHandler` span covering the attribute value; an *inline* arrow gets
    // its role from that span directly, because the arrow's body lies inside
    // it. A named handler's body does not, so nothing classified it and its
    // reads took the enclosing component's `UntrackedRendering` role. A read
    // written in such a body was then silent only because `local_access` gates
    // reads inside an unproven helper, while the same read propagated through
    // one call was reported as a proven untracked read -- 24 of SC1001's 71
    // violations on the consumer corpus, including `ref()` read inside a
    // handler two lines below a silent direct read of the same accessor.
    //
    // `deferred`, which is what an event handler is: it runs after setup,
    // outside the tracking phase, and reads current values when it fires.
    // Identity-exact, like the tracked/deferred arms above -- the attribute
    // value must *be* the function, so `onClick={() => handler()}` keeps
    // classifying the arrow rather than `handler`.
    for callback in &file.compiler.callback_roles {
        if callback.role != solid_facts::compiler::CallbackRoleKind::EventHandler {
            continue;
        }
        // The attribute value must be a bare identifier reference.
        // `entities.at` answers a *call* span with the callee's symbol, so
        // without this `onClick={makeHandler()}` would admit `makeHandler`
        // itself and silence the setup-time writes in its body -- which is a
        // pinned positive in `fixtures/reactive-ir/directive-phases`.
        if !file.ast.identifiers.iter().any(|identifier| {
            identifier.span == callback.span
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        }) {
            continue;
        }
        for candidate in index.identity_at(file, entities, callback.span) {
            let entry = roles.entry(index.functions[*candidate]);
            entry.admitted = true;
            entry.deferred = true;
        }
    }
    for (element_index, element) in file.ast.jsx_elements.iter().enumerate() {
        if !known_primitive(&primitives.jsx[element_index])
            .is_some_and(|primitive| dialect.renders_children_through_callback(primitive))
        {
            continue;
        }
        for identifier in file.ast.identifiers_within(element.span) {
            // The identifier must *be* a children expression
            // (`<For each={xs}>{renderItem}</For>`). Merely mentioning a
            // helper elsewhere in the element -- `when={visible()}`,
            // `each={rows()}`, a `fallback` -- passes it nothing to invoke as a
            // render callback.
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || !element
                    .children
                    .iter()
                    .any(|child| jsx_child_expression(file, *child) == Some(identifier.span))
                || file
                    .ast
                    .jsx_containing(identifier.span)
                    .any(|nested| nested.span != element.span && element.span.contains(nested.span))
            {
                continue;
            }
            named.clear();
            index.named_at(file, entities, identifier.span, &mut named);
            for candidate in &named {
                roles.entry(index.functions[*candidate]).admitted = true;
            }
        }
    }
    roles
}

pub(crate) fn named_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let roles = lookup.named_callback_roles(file);
    let callback = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)?;
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        return Some(ExecutionRole::DeferredCallback);
    }
    let role = roles.get(callback.span);
    if role.result_access {
        return Some(ExecutionRole::Unknown);
    }
    if role.untracked_callback {
        return Some(ExecutionRole::UntrackedCallback);
    }
    if role.effect_apply {
        return Some(ExecutionRole::EffectApply);
    }
    if role.tracked {
        return Some(ExecutionRole::TrackedJsx);
    }
    if role.deferred {
        return Some(ExecutionRole::DeferredCallback);
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|element| callback.body.contains(element.span))
    {
        Some(ExecutionRole::TrackedJsx)
    } else {
        Some(ExecutionRole::UntrackedRendering)
    }
}

pub(crate) fn function_symbol<'a>(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &'a EntitySymbols,
) -> Option<&'a SymbolId> {
    let name = function
        .name
        .as_ref()
        .or_else(|| function_binding_name(file, function))?;
    entities.exact(&location(file.path.shared(), name.span))
}

pub(crate) fn argument_references_callback_symbol(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    symbol: &str,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> bool {
    entities
        .value_symbol(file, argument.span)
        .map(SymbolId::as_str)
        == Some(symbol)
        || argument.identifier_properties.iter().any(|property| {
            entities
                .exact(&location(file.path.shared(), property.span))
                .map(SymbolId::as_str)
                == Some(symbol)
                || symbol_names.get(symbol).map(SymbolId::as_str) == file.source_text(property.span)
        })
}

pub(crate) fn direct_callback_contains(
    file: &solid_facts::FileFacts,
    argument: Span,
    span: Span,
) -> bool {
    if !argument.contains(span) {
        return false;
    }
    let callback = file
        .ast
        .functions_within(argument)
        .max_by_key(|function| function.span.end - function.span.start);
    let owner = containing_ast_function(&file.ast, span);
    match (callback, owner) {
        (Some(callback), Some(owner)) => callback.span == owner.span,
        (None, None) => true,
        _ => false,
    }
}
