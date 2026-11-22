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
    /// The positions that admitted each function: the spans whose role the
    /// function's [`NamedCallbackRole`] describes. A write role inherited from
    /// a function's entries counts a reference at one of these as accounted
    /// for ([`super::write_roles`]); a position that only names the function
    /// (an unproven callback slot, a result-access one) is not.
    pub(super) positions: HashMap<Span, Vec<Span>>,
    /// ADR 0933, 0936 (read by the named ends only): the argument spans that name each
    /// function at a position that tracks its reads (a memo's compute, an
    /// effect's tracked compute), the facts behind `tracked` and
    /// `effect_compute`. Nothing in an answer reads it.
    pub(super) tracking: HashMap<Span, Vec<Span>>,
    /// ADR 0517: the functions a callback position that runs in its call's
    /// own window (`createRoot(init)`, `runWithOwner(o, run)`) names by its
    /// binding, with that call and argument index when it is the function's
    /// sole entry (`by_name_position`), or `None` when another entry may run
    /// it. Such a position admits no role of its own (ADR 0323), so
    /// [`inherits_callback_role`] answers from the call.
    pub(super) inherits: HashMap<Span, Option<(Span, usize)>>,
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
    /// A `Show` or `Match` child of `length` 0: a value, run where it is read.
    pub(super) value_child: bool,
    pub(super) untracked_callback: bool,
    /// The function is a map function: untracked in no window (ADR 0443).
    pub(super) untracked_unwindowed: bool,
    pub(super) effect_apply: bool,
    /// Which effect primitive names this function at its apply position, for
    /// the read-context wording only.
    pub(super) effect_apply_primitive: EffectApplyPrimitive,
    pub(super) tracked: bool,
    /// Named, as the whole argument, at a proven position whose callback
    /// runs under a children-forbidden leaf owner
    /// ([`solid_dialect::CallbackOwner::Leaf`]: `createTrackedEffect`'s
    /// compute, `onSettled`'s callback). The runtime's write guard exempts
    /// such an owner (`@solidjs/signals` dist/dev.js:2046-2090, dev-shared.js
    /// :5619-5638), so the tracked read role the position gives says nothing
    /// against a write there: [`named_callback_write_execution_role`].
    pub(super) leaf: bool,
    /// Named, as the whole argument, at a tracked position whose callback
    /// runs in an owned computation the write guard rejects (a memo's or
    /// projection's compute): every tracked position that is not a leaf.
    pub(super) tracked_owned: bool,
    /// Named, as the whole argument, at an effect's tracked compute: a
    /// computation the effect owns, where a write is rejected as in a memo
    /// (ADR 0466). Reads are not admitted by it (see `admitted`).
    pub(super) effect_compute: bool,
    pub(super) deferred: bool,
    /// Named as an intrinsic element's event handler (`onScroll={handler}`):
    /// one of its `deferred` entries is the event's delivery, which a visible
    /// synchronous dispatch may run on its caller's stack (ADR 0356, 0934).
    pub(super) event_handler: bool,
    /// Named at a position whose callback runs when the call's returned
    /// object is read ([`Dialect::callback_runs_on_result_access`]). That
    /// time and scope are the reader's, which this index cannot see, so the
    /// function's own role is unknown and outranks every arm above.
    pub(super) result_access: bool,
    /// An intrinsic `ref` applies the function
    /// ([`crate::owners::ref_applied_element`]): named in the attribute
    /// (`ref={apply}`), or returned by the exact factory a two-phase directive
    /// calls (`ref={dir()}`). It runs while the element is created, which
    /// [`super::read_execution_role`] places from the element.
    pub(super) ref_applied: bool,
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

    fn admit_at(&mut self, function: Span, position: Span) {
        self.positions.entry(function).or_default().push(position);
    }

    /// The positions whose role this function's entry describes.
    pub(super) fn positions(&self, function: Span) -> &[Span] {
        self.positions.get(&function).map_or(&[], Vec::as_slice)
    }

    fn track_at(&mut self, function: Span, position: Span) {
        self.tracking.entry(function).or_default().push(position);
    }

    /// ADR 0933: the positions that track this function's reads.
    pub(super) fn tracking_positions(&self, function: Span) -> &[Span] {
        self.tracking.get(&function).map_or(&[], Vec::as_slice)
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
        let mut candidates = file
            .ast
            .functions
            .iter()
            .filter_map(|function| {
                Some((function.span, function_symbol(file, function, entities)?))
            })
            .collect::<Vec<_>>();
        // A named function expression its binding names (see
        // `named_expression_binding`) is a candidate under that binding too.
        for function in &file.ast.functions {
            if let Some(name) = named_expression_binding(file, function, lookup)
                && let Some(symbol) = entities.exact(&location(file.path.shared(), name.span))
                && !candidates
                    .iter()
                    .any(|(span, existing)| *span == function.span && *existing == symbol)
            {
                candidates.push((function.span, symbol));
            }
        }
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
        // An exact `const` copy of a function's binding names that function
        // (`const apply = run; createEffect(c, apply)`): `local_alias_bindings`
        // records only a non-exported `const` whose initializer is an exact
        // copy of the binding (ADR 0463, ADR 0515), so a reference to the copy
        // is the function's value -- once the copy's declarator has run. A
        // reference from a nested function may run before it (`const early =
        // () => createMemo(apply); early(); const apply = run;` throws a
        // ReferenceError, and nothing is written), which `tsc` does not
        // report. So a copy names the function only when every reference to it
        // is in the declarator's own function and after the declarator; any
        // other copy names nothing, as before. Chains follow, bounded.
        let initialized_before_every_use =
            |alias: &solid_facts::ast::LocalAliasBindingFact, local: &SymbolId| {
                let declared_in =
                    containing_ast_function(&file.ast, alias.local).map(|function| function.span);
                lookup
                    .symbol_references(local.as_str())
                    .iter()
                    .all(|reference| {
                        if reference.path.as_ref() != file.path.as_str() {
                            return false;
                        }
                        let (Ok(start), Ok(end)) = (
                            u32::try_from(reference.start_byte),
                            u32::try_from(reference.end_byte),
                        ) else {
                            return false;
                        };
                        let use_span = Span::new(start, end);
                        use_span == alias.local
                            || (use_span.start > alias.local.end
                                && containing_ast_function(&file.ast, use_span)
                                    .map(|function| function.span)
                                    == declared_in)
                    })
            };
        // Each round forwards every alias one step; a chain is no longer
        // than the alias list, so the fixpoint is reached within it.
        for _ in 0..=file.ast.local_alias_bindings.len() {
            let mut added = false;
            for alias in &file.ast.local_alias_bindings {
                let (Some(local), Some(source)) = (
                    entities.exact(&location(file.path.shared(), alias.local)),
                    entities.exact(&location(file.path.shared(), alias.source)),
                ) else {
                    continue;
                };
                if !initialized_before_every_use(alias, local) {
                    continue;
                }
                let Some(targets) = by_symbol.get(source.as_str()).cloned() else {
                    continue;
                };
                let entry = by_symbol.entry(local.as_str()).or_default();
                for target in targets {
                    if !entry.contains(&target) {
                        entry.push(target);
                        added = true;
                    }
                }
            }
            if !added {
                break;
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
    // Which span named each candidate: the argument itself or one of its
    // options-object properties.
    let mut named_spans = Vec::new();
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
            named_spans.clear();
            index.named_at(file, entities, argument.span, &mut named);
            named_spans.extend(named.iter().map(|candidate| (*candidate, argument.span)));
            for property in &argument.identifier_properties {
                let before = named.len();
                index.named_at(file, entities, property.span, &mut named);
                named_spans.extend(
                    named[before..]
                        .iter()
                        .map(|candidate| (*candidate, property.span)),
                );
            }
            named.sort_unstable();
            named.dedup();
            if named.is_empty() {
                continue;
            }
            // ADR 0517: a position whose mention may run before the binding's
            // declarator (`const early = () => createMemo(write); early();
            // const write = ...`) throws a `ReferenceError` there instead of
            // handing the function over, which `tsc` does not report. The
            // function may run at the position or not at all, so its role is
            // unknown, as a result-access position's is.
            let path = file.path.as_str();
            if named_spans
                .iter()
                .any(|(_, position)| lookup.may_precede_initialization(path, *position))
            {
                for candidate in &named {
                    let entry = roles.entry(index.functions[*candidate]);
                    entry.admitted = true;
                    entry.result_access = true;
                }
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
                    let function = index.functions[*candidate];
                    let entry = roles.entry(function);
                    entry.admitted |= tracked || deferred;
                    entry.tracked |= tracked;
                    entry.deferred |= deferred;
                    if tracked || deferred {
                        roles.admit_at(function, argument.span);
                    }
                    if tracked {
                        roles.track_at(function, argument.span);
                    }
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
            if inherits && proven {
                for candidate in index.identity_at(file, entities, argument.span) {
                    let function = index.functions[*candidate];
                    let sole = file
                        .ast
                        .function_at(function)
                        .and_then(|fact| super::write_roles::by_name_position(file, fact, lookup))
                        .filter(|position| {
                            position.sole
                                && position.call.span == call.span
                                && position.index == argument_index
                        })
                        .map(|_| (call.span, argument_index));
                    roles
                        .inherits
                        .entry(function)
                        .and_modify(|entry| *entry = None)
                        .or_insert(sole);
                }
            }
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
            let admits = proven && (untracked || effect_apply || tracked || deferred);
            for candidate in &named {
                let function = index.functions[*candidate];
                let entry = roles.entry(function);
                entry.admitted |= admits;
                let unwindowed =
                    dialect.callback_runs_untracked_without_window(primitive, argument_index);
                entry.untracked_callback |= proven && untracked && !unwindowed;
                entry.untracked_unwindowed |= proven && untracked && unwindowed;
                entry.effect_apply |= effect_apply;
                if effect_apply {
                    entry.effect_apply_primitive = entry.effect_apply_primitive.with(primitive);
                }
            }
            if admits {
                for (candidate, position) in &named_spans {
                    roles.admit_at(index.functions[*candidate], *position);
                }
            }
            // The tracked and deferred arms have always demanded that the whole
            // argument *be* the function, never that an options object mention
            // it.
            // The leaf owner is the inline path's question too
            // (`write_region_adjustment`): asked of the same call and
            // position, so a by-name callback answers as its literal would.
            let leaf = dialect.leaf_scopes_allow_writes()
                && callback_owner_at_call(file, call, primitive, argument_index, lookup)
                    == Some(solid_dialect::CallbackOwner::Leaf);
            for candidate in index.identity_at(file, entities, argument.span) {
                let function = index.functions[*candidate];
                let entry = roles.entry(function);
                entry.tracked |= proven && tracked;
                entry.leaf |= proven && leaf;
                entry.tracked_owned |= proven && tracked && !leaf;
                let effect_compute = proven
                    && is_effect(primitive)
                    && dialect
                        .callback_semantics_at(primitive, argument_index, count)
                        .tracks_reads;
                entry.effect_compute |= effect_compute;
                entry.deferred |= proven && deferred;
                if (proven && tracked) || effect_compute {
                    roles.track_at(function, argument.span);
                }
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
    // One denied delivery denies the function, whatever else it is bound to.
    let denied = file
        .compiler
        .callback_roles
        .iter()
        .filter(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && super::handler_event_denied(lookup, file, callback.span)
        })
        .flat_map(|callback| index.identity_at(file, entities, callback.span))
        .map(|candidate| index.functions[*candidate])
        .collect::<HashSet<_>>();
    for callback in &file.compiler.callback_roles {
        if callback.role != solid_facts::compiler::CallbackRoleKind::EventHandler
            || super::handler_event_denied(lookup, file, callback.span)
        {
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
            let function = index.functions[*candidate];
            if denied.contains(&function) {
                continue;
            }
            let entry = roles.entry(function);
            entry.admitted = true;
            entry.deferred = true;
            entry.event_handler = true;
            roles.admit_at(function, callback.span);
        }
    }
    // A function an intrinsic `ref` applies, by name or through the factory
    // of a two-phase directive. The attribute's own literal has its body
    // inside the compiler's `DirectiveApply` span and needs no entry here.
    for function in &file.ast.functions {
        if function.generator
            || file
                .compiler
                .callback_roles
                .iter()
                .any(|callback| callback.span.contains(function.span))
        {
            continue;
        }
        if let Some(element) = crate::owners::ref_applied_element(file, function, lookup) {
            let entry = roles.entry(function.span);
            entry.admitted = true;
            entry.ref_applied = true;
            // ADR 1160: a `ref` whose expression is the function's own name (`ref={rf1}`) is entered at that reference, so the
            // entry census accounts for it as this position; any other expression keeps the element as the position.
            let binding = function_binding_name(file, function).map(|name| name.span);
            let named = crate::owners::ref_applications_of(file, function, lookup)
                .into_iter()
                .map(|applied| file.ast.peel_ts_sugar_span(applied))
                .filter(|applied| {
                    binding.is_some()
                        && file.ast.identifiers.iter().any(|identifier| {
                            identifier.span == *applied
                                && identifier.role == solid_facts::ast::IdentifierRole::Reference
                        })
                        && file.ast.reference_declaration(*applied) == binding
                })
                .collect::<Vec<_>>();
            if named.is_empty() {
                roles.admit_at(function.span, element.name.span);
            }
            for applied in named {
                roles.admit_at(function.span, applied);
            }
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
                let function = index.functions[*candidate];
                // `Show` and `Match` call a function child only when its
                // `length` is above 0; else it is a value, run wherever it is
                // read: no untrack of theirs says anything about that run.
                let value =
                    known_primitive(&primitives.jsx[element_index]).is_some_and(|primitive| {
                        dialect.children_callback_requires_parameters(primitive)
                    }) && file
                        .ast
                        .function_at(function)
                        .is_some_and(|function| function_length(function) == 0);
                let entry = roles.entry(function);
                entry.admitted = true;
                entry.value_child |= value;
                roles.admit_at(function, identifier.span);
            }
        }
    }
    roles
}

/// A function's runtime `length`: the parameters before the first default or
/// the rest parameter.
pub(crate) fn function_length(function: &solid_facts::ast::FunctionFact) -> usize {
    function
        .parameters
        .iter()
        .take_while(|parameter| parameter.initializer.is_none())
        .count()
        .min(
            function
                .parameters
                .len()
                .saturating_sub(usize::from(function.rest_parameter)),
        )
}

/// The innermost function around `span` that a callback position of its file
/// names ([`NamedCallbackRoles`]): the function whose role
/// [`named_callback_execution_role`] reads.
pub(crate) fn named_callback_containing<'f>(
    file: &'f solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    let roles = lookup.named_callback_roles(file);
    file.ast
        .functions_body_containing(span)
        .find(|function| roles.get(function.span).admitted)
}

/// ADR 0515: the `const` binding that names a named function expression
/// (`const apply = function step() { ... }`) the way a binding names an arrow:
/// the expression is the binding's whole initializer and its own name `step`
/// is never referenced, so the function has no entry but the binding's. A
/// self-referencing expression (recursion through `step`) is entered by its
/// own name too, which no position of the binding accounts for: `None`.
pub(crate) fn named_expression_binding<'f>(
    file: &'f solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::NamedSpan> {
    let own = function.name.as_ref()?;
    if function.kind != solid_facts::ast::FunctionKind::Expression
        || function.generator
        || function.method_name.is_some()
    {
        return None;
    }
    let own_symbol = lookup.entities().at(file.path.as_str(), own.span)?;
    let unreferenced = lookup
        .symbol_references_if_present(own_symbol.as_str())
        .is_some_and(|references| {
            references.iter().all(|reference| {
                reference.path.as_ref() == file.path.as_str()
                    && u32::try_from(reference.start_byte)
                        .is_ok_and(|start| start == own.span.start)
            })
        });
    if !unreferenced {
        return None;
    }
    let binding = file
        .ast
        .bindings_initializer_containing(function.span)
        .find(|binding| {
            binding.immutable
                && binding.initializer_function
                && binding.initializer.is_some_and(|initializer| {
                    file.ast
                        .functions_within(initializer)
                        .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                        .is_some_and(|candidate| candidate.span == function.span)
                })
        })?;
    match binding.names.as_slice() {
        [name] => Some(name),
        _ => None,
    }
}

/// ADR 0516: whether the innermost function around `span` is named at a
/// tracked position (a memo's compute, an effect's tracked compute), where a
/// read tracks: the read is legal there whatever else names the function.
pub(crate) fn named_at_tracked_position(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // ADR 0874: its position says nothing while an impostor may run it.
    if runs_in_withheld_layer(file, span, lookup) {
        return false;
    }
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        let role = lookup.named_callback_roles(file).get(function.span);
        // ADR 1119 round 2 (H1): the tracked position says where the read runs only when it is every way into the function
        // (an effect's compute admits no role arm, so the census at the named-callback arm never asked for it):
        // `createEffect(cb, …); [cb].forEach((g) => g())` runs the read in the body too.
        // ADR 1124 round 2: the census runs in a frame at the read, so an arm it rests on (a `forEach`-carried entry) is named in
        // the proof log; the carried calls are classified after the frame closes, each its own decision.
        (role.tracked || role.effect_compute)
            && crate::role_log::decide_if_armed(
                "read",
                file.path.as_str(),
                span,
                || super::write_roles::role_entries_complete(file, function, lookup),
                || carried_calls_silent(file, function, lookup),
            )
    })
}

thread_local! {
    /// The functions whose carried calls [`carried_calls_silent`] is classifying on this thread: one reached again (it carries itself)
    /// is refused, never recursed into.
    static CLASSIFYING_CARRIED: std::cell::RefCell<HashSet<(String, Span)>> = std::cell::RefCell::new(HashSet::default());
}

/// ADR 1124: the census accepts a reference the standard `forEach` runs ([`super::write_roles::foreach_carried_call`]) as an entry,
/// which only holds when the run it stands for is silent where it runs: each such `forEach` call is classified as `cb()` written there,
/// with a read's own placement ([`super::read_execution_role`]: a `ref` an intrinsic applies runs in the body's rendering), and a role
/// that reports a read or a write, or none (unplaced), refuses the tracked position's silence. A direct call needs no such check here:
/// the summaries carry the function's rows to it.
fn carried_calls_silent(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let calls = super::write_roles::foreach_carried_calls(file, function, lookup);
    if calls.is_empty() {
        return true;
    }
    let allowed = super::read_context::allowed_callback_spans(file, lookup);
    calls.into_iter().all(|call| {
        let role = carried_call_role(
            file,
            function,
            call,
            &allowed,
            lookup.entities(),
            lookup.symbol_names(),
            lookup,
        );
        !role.reports_untracked_read()
            && !role.reports_disallowed_write()
            && !role.reports_unplaced()
    })
}

/// ADR 1124: the role of the run a carried `forEach` call stands for, as a read written at the call would take it
/// ([`super::read_execution_role`]); [`ExecutionRole::Unknown`] (unplaced, refused) when `function`'s carried calls are already being
/// classified on this thread (it carries itself), never a recursion.
pub(super) fn carried_call_role(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    call: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> ExecutionRole {
    let key = (file.path.to_string(), function.span);
    if !CLASSIFYING_CARRIED.with(|set| set.borrow_mut().insert(key.clone())) {
        return role_arm!(
            "named_callbacks.carried_call_role.reentered",
            ExecutionRole::Unknown
        );
    }
    let role = super::read_execution_role(file, call, allowed, entities, symbol_names, lookup);
    CLASSIFYING_CARRIED.with(|set| set.borrow_mut().remove(&key));
    role
}

thread_local! {
    /// The callbacks [`reporting_run_role`] is classifying the runs of on this thread: one reached again (a run of it is classified
    /// inside its own run) answers nothing, never recursed into.
    static CLASSIFYING_RUNS: std::cell::RefCell<HashSet<(String, Span)>> = std::cell::RefCell::new(HashSet::default());
}

/// ADR 1166: the reporting role of a run that provably executes the read at `span`, for a read left unplaced
/// ([`ExecutionRole::Unknown`]) because the named callback around it has an entry besides its by-name positions (ADR 0517, 1111,
/// 1119). A read rule asks whether *some* run of the read is untracked, so one such run is enough, whatever the other entries do:
/// - the read is written in the callback's own body (not a nested function, an instance field or a parameter default) and the
///   callback is neither `async` nor a generator, so every call of it runs the read synchronously on the caller's stack;
/// - the callback is the whole initializer of a `const` binding (`const cb = () => …`, `const cb = function inner() {…}`), so the
///   binding always holds it;
/// - the run is a direct call of that binding (`cb()`), or a `forEach`/`map` over a literal that carries it (ADR 1124,
///   [`super::write_roles::foreach_carried_call`]) whose parameter call runs on every completion of the inline arrow;
/// - the run is written in the function that declares the binding, after the declarator, and runs on every completion of that
///   function ([`solid_facts::ast::synchronously_unconditional_calls`]): no condition, loop or early exit (a dead `if (false) cb()`)
///   stands between the code that creates the callback and the run;
/// - a read written at the run would report ([`super::read_execution_role`] at the call reports an untracked read), and outside the
///   component body's rendering no call stands between the declarator and the run (a creation in an effect's apply throws first).
///
/// Anything else is `None` and the read stays unplaced.
pub(crate) fn reporting_run_role(
    file: &solid_facts::FileFacts,
    span: Span,
    allowed: &[Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let callback = crate::owners::containing_body_function(&file.ast, span)?;
    if callback.r#async
        || callback.generator
        || callback.method_name.is_some()
        || !callback.body.contains(span)
        || super::events::inside_instance_field(file, span)
    {
        return None;
    }
    let binding = file
        .ast
        .bindings_initializer_containing(callback.span)
        .find(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.initializer.is_some_and(|initializer| {
                    file.ast.peel_ts_sugar_span(initializer) == callback.span
                })
        })?;
    let [name] = binding.names.as_slice() else {
        return None;
    };
    let declarer = crate::owners::containing_body_function(&file.ast, binding.declaration)?;
    if declarer.r#async || declarer.generator {
        return None;
    }
    let symbol = lookup.entities().at(file.path.as_str(), name.span)?;
    let references = lookup.symbol_references_if_present(symbol.as_str())?;
    let key = (file.path.to_string(), callback.span);
    if !CLASSIFYING_RUNS.with(|set| set.borrow_mut().insert(key.clone())) {
        return None;
    }
    let mut unconditional: Option<Option<Vec<Span>>> = None;
    let mut found = None;
    for reference in references
        .iter()
        .filter(|reference| reference.path.as_ref() == file.path.as_str())
    {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            continue;
        };
        let at = Span::new(start, end);
        if at == name.span || at.start < binding.declaration.end {
            continue;
        }
        let run = file
            .ast
            .calls
            .iter()
            .find(|call| !call.construct && file.ast.peel_ts_sugar_span(call.callee) == at)
            .map(|call| call.span)
            .or_else(|| {
                super::write_roles::foreach_carried_call(file, at, lookup)
                    .filter(|carried| carried_run_unconditional(file, *carried, lookup))
            });
        let Some(run) = run else {
            continue;
        };
        if !crate::owners::containing_body_function(&file.ast, run)
            .is_some_and(|owner| owner.span == declarer.span)
        {
            continue;
        }
        let every = unconditional.get_or_insert_with(|| {
            solid_facts::ast::synchronously_unconditional_calls(
                std::path::Path::new(file.path.as_str()),
                &file.source,
                declarer.span,
                &[],
            )
        });
        // A source that does not parse proves no run unconditional.
        if !every.as_ref().is_some_and(|every| every.contains(&run)) {
            continue;
        }
        let role = super::read_execution_role(file, run, allowed, entities, symbol_names, lookup);
        // Outside the component body's rendering a creation throws (an effect's apply: PRIMITIVE_IN_EFFECT_CALLBACK, rc.21), so a
        // call there between the declarator and the run (`createMemo(f)`, a helper that creates) may stop the function first: the
        // run is proven only with no call between them.
        let interrupted = role != ExecutionRole::UntrackedRendering
            && file.ast.calls.iter().any(|call| {
                call.span.start >= binding.declaration.end
                    && call.span.end <= run.start
                    && crate::owners::containing_body_function(&file.ast, call.span)
                        .is_some_and(|owner| owner.span == declarer.span)
            });
        if role.reports_untracked_read()
            && !interrupted
            && !super::callee_callback_timing(file, run, role, lookup)
        {
            found = Some(role);
            break;
        }
    }
    CLASSIFYING_RUNS.with(|set| set.borrow_mut().remove(&key));
    found
}

/// ADR 1166: whether the inline arrow a carried `forEach`/`map` call ([`super::write_roles::foreach_carried_call`]) hands the
/// element to calls its parameter on every completion of the arrow (`(g) => g()`, not `(g) => { if (c) g(); }`).
fn carried_run_unconditional(
    file: &solid_facts::FileFacts,
    carried: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(call) = ast.calls.iter().find(|call| call.span == carried) else {
        return false;
    };
    let Some(arrow) = call.arguments.first().and_then(|argument| {
        ast.functions
            .iter()
            .find(|function| function.span == ast.peel_ts_sugar_span(argument.span))
    }) else {
        return false;
    };
    let Some(parameter) = arrow
        .parameters
        .first()
        .and_then(|parameter| parameter.names.first())
    else {
        return false;
    };
    let Some(every) = solid_facts::ast::synchronously_unconditional_calls(
        std::path::Path::new(file.path.as_str()),
        &file.source,
        arrow.span,
        &[],
    ) else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(file.path.as_str(), parameter.span) else {
        return false;
    };
    let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
        return false;
    };
    ast.calls.iter().any(|inner| {
        !inner.construct
            && every.contains(&inner.span)
            && references.iter().any(|reference| {
                reference.path.as_ref() == file.path.as_str()
                    && u64::from(ast.peel_ts_sugar_span(inner.callee).start) == reference.start_byte
                    && reference.start_byte != u64::from(parameter.span.start)
            })
    })
}

/// ADR 0933, 0936: the argument positions that name the innermost
/// function around `span` at a position that tracks its reads, the facts
/// [`named_at_tracked_position`] answers from.
pub(crate) fn naming_positions(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Vec<Span> {
    containing_ast_function(&file.ast, span)
        .map(|function| {
            lookup
                .named_callback_roles(file)
                .tracking_positions(function.span)
                .to_vec()
        })
        .unwrap_or_default()
}

/// Whether `span` is in a function nested in a named callback (not in the
/// callback's own body): code [`named_callback_execution_role`] places as
/// deferred unless the in-place layers between them place it at a call in the
/// callback's body.
pub(super) fn nested_in_named_callback(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    named_callback_containing(file, span, lookup).is_some_and(|callback| {
        lookup.function_component_status(file, callback) == ComponentStatus::No
            && containing_ast_function(&file.ast, span)
                .is_some_and(|owner| owner.span != callback.span)
    })
}

pub(crate) fn named_callback_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    named_callback_role(file, span, lookup, false)
}

/// [`named_callback_execution_role`] for a read (`write` false) or a write: the
/// two differ only in which dispatch veto withholds a named event handler's
/// deferral (ADR 0934).
fn named_callback_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    write: bool,
) -> Option<ExecutionRole> {
    if let Some(role) = inherits_callback_role(file, span, lookup) {
        return Some(role);
    }
    let roles = lookup.named_callback_roles(file);
    let callback = named_callback_containing(file, span, lookup)?;
    // ADR 0331: a callback position is one entry of the function. A function
    // a tag renders (or that some unfollowed entry may render) is also
    // entered as a component, which this role does not describe.
    if lookup.function_component_status(file, callback) != ComponentStatus::No {
        return None;
    }
    let owner = containing_ast_function(&file.ast, span)?;
    // ADR 0539: the cleanup a named apply returns runs on a rerun in the
    // apply's window, or in the owner's disposal outside it: unplaced.
    if owner.span != callback.span
        && super::write_roles::inside_rerun_effect_cleanup(file, span, lookup)
    {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.effect-cleanup-rerun",
            ExecutionRole::Unknown
        ));
    }
    let role = roles.get(callback.span);
    // ADR 0934: a named event handler is deferred only while no visible
    // synchronous dispatch can deliver its event on the caller's stack, as
    // an inline handler's `EventCallback` is (`body_roles`, ADR 0356): the
    // read veto for reads, the owner veto for writes. So is a closure nested
    // in it, which the handler's run may call.
    if role.event_handler
        && (if write {
            lookup.synchronous_event_dispatch_present()
        } else {
            lookup.synchronous_event_read_dispatch_present()
        })
    {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.event-handler-dispatch-veto",
            ExecutionRole::Unknown
        ));
    }
    if owner.span != callback.span {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.deferred-callback",
            ExecutionRole::DeferredCallback
        ));
    }
    if role.result_access {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.unknown",
            ExecutionRole::Unknown
        ));
    }
    if role.ref_applied {
        // Also named at a primitive's callback position: two entries, no one
        // role.
        return Some(
            if role.untracked_callback
                || role.untracked_unwindowed
                || role.effect_apply
                || role.tracked
                || role.deferred
            {
                role_arm!(
                    "named_callbacks.named_callback_execution_role.ref-applied-and-named",
                    ExecutionRole::Unknown
                )
            } else {
                role_arm!(
                    "named_callbacks.named_callback_execution_role.ref-applied",
                    ExecutionRole::DirectiveApply
                )
            },
        );
    }
    if role.untracked_callback {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.untracked-callback",
            ExecutionRole::UntrackedCallback
        ));
    }
    if role.untracked_unwindowed {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.untracked-unwindowed",
            ExecutionRole::UntrackedUnwindowed
        ));
    }
    if role.effect_apply {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.effect-apply",
            ExecutionRole::EffectApply
        ));
    }
    if role.tracked {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.tracked-jsx",
            ExecutionRole::TrackedJsx
        ));
    }
    if role.deferred {
        return Some(role_arm!(
            "named_callbacks.named_callback_execution_role.deferred-callback-2",
            ExecutionRole::DeferredCallback
        ));
    }
    if file
        .ast
        .jsx_containing(span)
        .any(|element| callback.body.contains(element.span))
    {
        Some(role_arm!(
            "named_callbacks.named_callback_execution_role.tracked-jsx-2",
            ExecutionRole::TrackedJsx
        ))
    } else if role.value_child {
        Some(role_arm!(
            "named_callbacks.named_callback_execution_role.value-child",
            ExecutionRole::Unknown
        ))
    } else {
        Some(role_arm!(
            "named_callbacks.named_callback_execution_role.untracked-rendering",
            ExecutionRole::UntrackedRendering
        ))
    }
}

/// [`named_callback_execution_role`] read for a write: the tracked role a
/// leaf position gives (`createTrackedEffect(cb)`) is a read fact, the
/// callback runs under a children-forbidden owner the write guard exempts, as
/// the inline callback's [`super::write_roles::WriteRegionAdjustment::LeafScope`]
/// says. That position is then as permissive as a deferred one, and the
/// function's other entries decide, as for a named effect apply. A function
/// also named at an owned tracked position (`createMemo(cb)`) keeps the
/// rejecting role: that position proves the violation.
pub(crate) fn named_callback_write_execution_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let role = named_callback_role(file, span, lookup, true)?;
    if role != ExecutionRole::TrackedJsx {
        return Some(role);
    }
    let callback = named_callback_containing(file, span, lookup)?;
    let entry = lookup.named_callback_roles(file).get(callback.span);
    Some(if entry.tracked && entry.leaf && !entry.tracked_owned {
        role_arm!(
            "named_callbacks.named_callback_write_execution_role.leaf-tracked",
            ExecutionRole::DeferredCallback
        )
    } else {
        role
    })
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

/// [`direct_callback_contains`] for a rule that reports what the callback runs
/// (ADR 0454): the callback's body runs `span`, so a site in a nested
/// function's parameter list is not (it runs when that function is called
/// without the argument, and the callback only builds the function), and neither
/// is one in the callback's own parameter list. The role lookups keep
/// [`direct_callback_contains`]: they place a list site by the code around it
/// and then withhold it (`parameter_default_owner`, `unrun_closure_within`, the
/// write guard).
pub(crate) fn direct_callback_runs(
    file: &solid_facts::FileFacts,
    argument: Span,
    span: Span,
) -> bool {
    direct_callback_contains(file, argument, span)
        && file
            .ast
            .functions_within(argument)
            .max_by_key(|function| function.span.end - function.span.start)
            .is_none_or(|callback| crate::owners::owned_by_body(&file.ast, callback, span))
}

/// ADR 0517: the role of code in a function named at a position that runs it
/// in its call's own window ([`NamedCallbackRoles::inherits`]). At the sole
/// proven position it is the inline callback's at that call (ADR 0323,
/// [`super::callee_roles::inheriting_call_role`]), JSX the callback writes
/// keeping the compiler's role (ADR 0470); at any other it is unknown, never
/// silence. Code in a layer the callback runs in place (an IIFE, a closure
/// called once, a helper's callback) climbs to that layer's call in the
/// callback's body first; a layer that does not run in place answers nothing
/// here.
fn inherits_callback_role(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let roles = lookup.named_callback_roles(file);
    if roles.inherits.is_empty() {
        return None;
    }
    let callback = file
        .ast
        .functions_body_containing(span)
        .find(|function| roles.inherits.contains_key(&function.span))?;
    // An async or generator callback runs its body past its first suspension
    // on another stack, which the call's role does not describe; it keeps the
    // paths it had (ADR 0516's after-await obligations among them).
    if callback.r#async || callback.generator {
        return None;
    }
    if lookup.function_component_status(file, callback) != ComponentStatus::No {
        return None;
    }
    // JSX written anywhere in the callback, a layer's included, takes the
    // compiler's role at the read itself (ADR 0470).
    if let Some(role) = super::callee_roles::own_jsx_compiler_role(file, span) {
        return Some(role);
    }
    let owner = containing_ast_function(&file.ast, span)?;
    if owner.span != callback.span {
        let climb = super::write_roles::climb_in_place_layers(file, span, lookup)?;
        if climb.bound() {
            return Some(ExecutionRole::Unknown);
        }
        if crate::owners::containing_body_function(&file.ast, climb.at)
            .map(|function| function.span)
            != Some(callback.span)
        {
            return None;
        }
        // A library callback may run zero times (ADR 0493 review): the read
        // is not proven to run at the layer's call.
        if super::write_roles::climb_crosses_library_callback(file, span, climb.at, lookup) {
            return Some(role_arm!(
                "named_callbacks.inherits_callback_role.unknown",
                ExecutionRole::Unknown
            ));
        }
        if climb.in_list.is_some() {
            return Some(role_arm!(
                "named_callbacks.inherits_callback_role.unknown-2",
                ExecutionRole::Unknown
            ));
        }
        // The layers' own awaits are the climb's; the layer's call in the
        // callback's body may follow one of the callback's.
        let at = climb.at;
        if climb.suspended_on_every_path_declared(file, lookup)
            || follows_unconditional_await(file, at)
            || follows_dominating_await(file, at)
            || follows_declared_try_head_await(file, at, lookup)
        {
            return Some(role_arm!(
                "named_callbacks.inherits_callback_role.unreported",
                ExecutionRole::Unreported
            ));
        }
        if climb.suspended.is_some()
            || follows_any_suspension(file, at)
            || may_follow_own_await(file, at)
        {
            return Some(role_arm!(
                "named_callbacks.inherits_callback_role.unknown-3",
                ExecutionRole::Unknown
            ));
        }
        return inherits_callback_role(file, at, lookup);
    }
    let Some((call_span, index)) = roles.inherits.get(&callback.span).copied().flatten() else {
        return Some(role_arm!(
            "named_callbacks.inherits_callback_role.unknown-4",
            ExecutionRole::Unknown
        ));
    };
    let call = file.ast.calls.iter().find(|call| call.span == call_span)?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    let clears_tracking = lookup
        .dialect
        .callback_clears_tracking_in_calling_window(primitive, index)
        .then(|| super::callee_roles::tracked_region_within(file, callback.span, span));
    // The call's role may be asked from inside this very callback (a function
    // named at its own `createRoot`, or a cycle of two): a callback already
    // being placed on this thread has no placement to give.
    let key = (file.path.to_string(), callback.span);
    if !INHERITS_PLACING.with(|placing| placing.borrow_mut().insert(key.clone())) {
        return Some(role_arm!(
            "named_callbacks.inherits_callback_role.unknown-5",
            ExecutionRole::Unknown
        ));
    }
    let allowed = super::allowed_callback_spans(file, lookup);
    let mut classifying = HashSet::default();
    let role = super::callee_roles::inheriting_call_role(
        file,
        call,
        clears_tracking,
        &allowed,
        lookup.entities(),
        lookup.symbol_names(),
        lookup,
        &mut classifying,
    );
    INHERITS_PLACING.with(|placing| placing.borrow_mut().remove(&key));
    Some(role)
}

/// The callbacks [`inherits_callback_role`] is placing on this thread, sorted:
/// part of the context a kept closure placement is reused in
/// (`body_roles::unrun_closure_within`).
pub(super) fn inherits_placing_snapshot() -> Vec<(String, Span)> {
    INHERITS_PLACING.with(|placing| {
        let mut held = placing.borrow().iter().cloned().collect::<Vec<_>>();
        held.sort_unstable();
        held
    })
}

thread_local! {
    /// The callbacks [`inherits_callback_role`] is placing on this thread.
    static INHERITS_PLACING: std::cell::RefCell<std::collections::HashSet<(String, Span)>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}
