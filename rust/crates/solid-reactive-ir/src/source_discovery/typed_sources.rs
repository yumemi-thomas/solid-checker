//! Typed sources: sources established by Type Facts descriptors and the value judge that reads them.

use super::*;

/// The symbol an identifier reference names. Type Facts keeps no entity at an
/// identifier under a TypeScript wrapper (`(props as P)`, `props!`), so the
/// binder and the declaration the reference resolves to answer in its place.
/// Any other expression names nothing (ADR 0376): every one of these lookups
/// answers a compound span with its leading token.
pub(crate) fn reference_symbol(
    file: &FileFacts,
    span: solid_facts::core::Span,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> Option<SymbolId> {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .identifiers
        .binary_search_by_key(&span, |identifier| identifier.span)
        .ok()?;
    entities
        .get(&location(file.path.shared(), span))
        .cloned()
        .or_else(|| {
            lookup
                .binding_at_reference(file.path.as_str(), span)
                .map(|(_, _, symbol)| symbol)
        })
        .or_else(|| {
            file.ast
                .reference_declaration(span)
                .and_then(|declaration| {
                    entities
                        .get(&location(file.path.shared(), declaration))
                        .cloned()
                })
        })
}

/// Whether a symbol is a source of the kind a caller asks about.
pub(crate) type SourceTest<'a> = &'a dyn Fn(&SymbolId) -> bool;

/// ADR 0323 (review round 10): the source symbol an `omit` or `merge` view
/// expression reads through to: the expression itself when it names a source,
/// else the view call's first argument (`omit`) or first resolvable argument
/// (`merge`), however deeply the views nest. Inline views and bound ones are
/// the same expression. A spread argument leaves the source unknown.
///
/// A source is an identifier, or for `is_chain_source` (stores, whose members
/// are nested proxies) a chain of static members rooted at one
/// (`merge(store.items[0], …)`). Any other argument roots nothing, whatever
/// its leading token names (ADR 0376): `store.open ? a : b` is `a` or `b`.
pub(crate) fn view_root(
    file: &FileFacts,
    expression: solid_facts::core::Span,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
    (is_source, is_chain_source): (SourceTest<'_>, SourceTest<'_>),
    depth: usize,
) -> Option<SymbolId> {
    if depth > 8 {
        return None;
    }
    let expression = file.ast.peel_ts_sugar_span(expression);
    if let Some(symbol) = reference_symbol(file, expression, entities, lookup)
        && is_source(&symbol)
    {
        return Some(symbol);
    }
    if file.ast.member_at(expression).is_some() {
        return member_chain_identifier_root(file, expression)
            .and_then(|root| reference_symbol(file, root, entities, lookup))
            .filter(|symbol| is_chain_source(symbol));
    }
    let call = file.ast.call_at(expression)?;
    let primitive = lookup.primitive_at_call(file, call.span)?;
    if lookup.dialect.splits_props(primitive) {
        // A spread among the hidden keys hides nothing nameable; the source is
        // still the first argument, which a spread cannot be.
        let first = call.arguments.first().filter(|first| !first.spread)?;
        return view_root(
            file,
            first.span,
            entities,
            lookup,
            (is_source, is_chain_source),
            depth + 1,
        );
    }
    if !lookup.dialect.merges_props_reactivity(primitive) {
        return None;
    }
    call.arguments.iter().find_map(|argument| {
        if argument.spread {
            return None;
        }
        view_root(
            file,
            argument.span,
            entities,
            lookup,
            (is_source, is_chain_source),
            depth + 1,
        )
    })
}

/// The identifier a chain of members starts at (`store.items[0]` ->
/// `store`), through TypeScript wrappers; `None` when the chain bottoms out
/// at anything else (a call, a parenthesised expression).
pub(crate) fn member_chain_identifier_root(
    file: &FileFacts,
    span: solid_facts::core::Span,
) -> Option<solid_facts::core::Span> {
    let mut current = file.ast.peel_ts_sugar_span(span);
    for _ in 0..64 {
        match file.ast.member_at(current) {
            Some(member) => current = file.ast.peel_ts_sugar_span(member.object),
            None => {
                return file
                    .ast
                    .identifiers
                    .binary_search_by_key(&current, |identifier| identifier.span)
                    .is_ok()
                    .then_some(current);
            }
        }
    }
    None
}

/// ADR 0294: a binding or member typed with a dialect source alias
/// (`Accessor<T>`, `Store<T>`) that nothing else proves to be a source.
/// `Accessor<T>` is `() => T`, which every plain function satisfies, so the
/// type alone never supplies the reactive-read premise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedSource {
    pub(crate) name: SymbolId,
    /// The alias declaration the type names.
    pub(crate) declaration: Location,
    pub(crate) kind: ReactiveSourceKind,
    /// The value the binding or slot holds is a literal written in project
    /// source, so that literal's own body or shape decides what a call or
    /// Get does and the type adds nothing. Otherwise the implementation is
    /// unknown and a read in a reporting role is an obligation.
    pub(crate) implementation_visible: bool,
    /// Review round 1 of ADR 0294: the member holds function literals
    /// written in project source, but no summary follows a call of the
    /// member into them, and not every one is proven to run nothing
    /// (`local_access::function_literal_runs_nothing`). Not visible, so a
    /// read in a reporting role is an obligation; a call in a helper is an
    /// undecided summary row (`interproc::typed_candidate_contributions`).
    pub(crate) held_body_unfollowed: bool,
}

/// What a typed binding or member provably holds.
pub(super) enum TypedImplementation {
    /// A proven source of the same kind: the root is that source's.
    Source(SymbolId),
    /// A literal written in project source.
    Visible,
    /// Function literals held by a closed literal member that no summary
    /// follows and that are not all proven to run nothing.
    HeldUnfollowed,
    Unknown,
}

/// The value every read of the typed `symbol` at `entity` observes, judged
/// from source, not from its type. Judged are: an unwritten `const`
/// identifier binding (its initializer, through `const` aliases), an
/// unwritten `const` array-pattern slot bound from one exact synchronous
/// project function every return of which writes that slot, and a member
/// whose every read is Closed over project literals (ADR 0286,
/// `LiteralMemberSlots::closed_values`). A parameter, an import, a
/// `declare`d value or a class field is Unknown.
#[allow(clippy::too_many_arguments)]
pub(super) fn typed_implementation(
    lookup: &SemanticLookup<'_>,
    entities: &EntitySymbols,
    literal_member_slots: &mut crate::literal_member_slots::LiteralMemberSlots<'_, '_>,
    entity: &Location,
    symbol: &SymbolId,
    kind: ReactiveSourceKind,
    runtime_sources: &HashSet<SymbolId>,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
) -> TypedImplementation {
    use solid_facts::{ast::RuntimeValueKind, core::Span};
    let Some(file) = lookup.file_by_path(entity.path.as_ref()) else {
        return TypedImplementation::Unknown;
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(entity.start_byte),
        u32::try_from(entity.end_byte),
    ) else {
        return TypedImplementation::Unknown;
    };
    let span = Span::new(start, end);
    let visible = |value_kind: RuntimeValueKind| match kind {
        ReactiveSourceKind::Accessor => value_kind == RuntimeValueKind::Function,
        ReactiveSourceKind::Store => {
            matches!(
                value_kind,
                RuntimeValueKind::Object | RuntimeValueKind::Array
            )
        }
    };
    let judge = TypedValueJudge {
        lookup,
        entities,
        kind,
        runtime_sources,
        source_kinds,
    };
    let declaration = file.ast.reference_declaration(span).unwrap_or(span);
    if let Some(binding) = typed_identifier_binding(file, declaration) {
        let Some(initializer) = binding
            .initializer
            .filter(|_| binding.immutable && !crate::indexes::binding_written(file, declaration))
        else {
            return TypedImplementation::Unknown;
        };
        if visible(binding.initializer_value_kind) {
            return TypedImplementation::Visible;
        }
        if let Some(source) = judge.value(file, initializer, 8) {
            return TypedImplementation::Source(source);
        }
        // A plain call of one exact project function every return of which
        // is a literal: that literal is what the binding holds.
        return match judge.factory_returns(file, initializer) {
            Some((_, returns))
                if !returns.is_empty()
                    && returns
                        .iter()
                        .all(|returned| visible(returned.runtime_value_kind)) =>
            {
                TypedImplementation::Visible
            }
            _ => TypedImplementation::Unknown,
        };
    }
    if let Some(source) = judge.array_slot(file, declaration) {
        return TypedImplementation::Source(source);
    }
    let Some(values) = literal_member_slots.closed_values(symbol) else {
        return TypedImplementation::Unknown;
    };
    if values.is_empty() {
        return TypedImplementation::Unknown;
    }
    if values
        .iter()
        .all(|(_, property)| visible(property.value_kind))
    {
        // Review round 1: a call of the member is not followed into the
        // function it holds, so that body decides only when it provably
        // reads nothing.
        if kind == ReactiveSourceKind::Accessor
            && !values.iter().all(|(file, property)| {
                crate::local_access::function_literal_runs_nothing(
                    file,
                    file.ast.peel_ts_sugar_span(property.value),
                )
            })
        {
            return TypedImplementation::HeldUnfollowed;
        }
        return TypedImplementation::Visible;
    }
    let sources = values
        .iter()
        .map(|(file, property)| match property.shorthand_binding {
            Some(binding) => judge.declaration(file, binding, 8),
            None => judge.value(file, property.value, 8),
        })
        .collect::<Option<Vec<_>>>();
    match sources.as_deref() {
        Some([first, rest @ ..]) if rest.iter().all(|other| other == first) => {
            TypedImplementation::Source(first.clone())
        }
        _ => TypedImplementation::Unknown,
    }
}

/// The identifier binding declared exactly at `declaration`.
pub(super) fn typed_identifier_binding(
    file: &FileFacts,
    declaration: solid_facts::core::Span,
) -> Option<&solid_facts::ast::BindingFact> {
    file.ast.bindings.iter().find(|binding| {
        binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding.names[0].span == declaration
    })
}

/// Which proven source of one kind a value expression is, through `const`
/// aliases only (ADR 0294).
pub(super) struct TypedValueJudge<'j, 'a> {
    pub(super) lookup: &'j SemanticLookup<'a>,
    pub(super) entities: &'j EntitySymbols,
    pub(super) kind: ReactiveSourceKind,
    pub(super) runtime_sources: &'j HashSet<SymbolId>,
    pub(super) source_kinds: &'j HashMap<SymbolId, ReactiveSourceKind>,
}

impl TypedValueJudge<'_, '_> {
    fn proven(&self, symbol: Option<SymbolId>) -> Option<SymbolId> {
        symbol.filter(|symbol| {
            self.runtime_sources.contains(symbol)
                && self.source_kinds.get(symbol) == Some(&self.kind)
        })
    }

    /// The source the expression at `value` is: an identifier naming one,
    /// or naming an unwritten `const` whose initializer is one.
    fn value(
        &self,
        file: &FileFacts,
        value: solid_facts::core::Span,
        depth: u8,
    ) -> Option<SymbolId> {
        let value = file.ast.peel_ts_sugar_span(value);
        if let Some(source) = self.proven(self.entities.at(file.path.as_str(), value).cloned()) {
            return Some(source);
        }
        let declaration = file.ast.reference_declaration(value)?;
        self.declaration(file, declaration, depth)
    }

    /// The source the binding declared at `declaration` holds.
    fn declaration(
        &self,
        file: &FileFacts,
        declaration: solid_facts::core::Span,
        depth: u8,
    ) -> Option<SymbolId> {
        if let Some(source) = self.proven(
            self.entities
                .get(&crate::location(file.path.shared(), declaration))
                .cloned(),
        ) {
            return Some(source);
        }
        let binding = typed_identifier_binding(file, declaration)?;
        if depth == 0 || !binding.immutable || crate::indexes::binding_written(file, declaration) {
            return None;
        }
        self.value(file, binding.initializer?, depth - 1)
    }

    /// The source an unwritten `const` array-pattern slot holds when its
    /// initializer is a plain call of one exact synchronous project function
    /// every return of which is an array literal holding one source there.
    fn array_slot(
        &self,
        file: &FileFacts,
        declaration: solid_facts::core::Span,
    ) -> Option<SymbolId> {
        // The same destructuring protocol as ADR 0282's closure slots. A
        // declared accessor type cannot restore an identity the iterator may
        // have replaced; retain the ordinary typed candidate instead.
        if self.lookup.tuple_iteration_may_be_mutated() {
            return None;
        }
        let binding = file.ast.bindings.iter().find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Array
                && binding
                    .array_slots
                    .iter()
                    .flatten()
                    .any(|slot| slot.span == declaration)
        })?;
        if !binding.immutable || crate::indexes::binding_written(file, declaration) {
            return None;
        }
        let index = binding
            .array_slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.span == declaration))?;
        let (factory_file, returns) = self.factory_returns(file, binding.initializer?)?;
        let sources = returns
            .into_iter()
            .map(|returned| {
                // Review round 1: a syntactic element is the runtime slot of
                // the same index only when nothing before it is a spread
                // (which may fill any number of slots) or a hole, which the
                // facts record alike as `None`.
                let elements = returned.elements();
                if elements.get(..index)?.iter().any(Option::is_none) {
                    return None;
                }
                let element = (*elements.get(index)?)?;
                self.value(factory_file, element, 8)
            })
            .collect::<Option<Vec<_>>>()?;
        match sources.as_slice() {
            [first, rest @ ..] if rest.iter().all(|other| other == first) => Some(first.clone()),
            _ => None,
        }
    }

    /// Every return of the one exact synchronous project function the plain
    /// call at `initializer` runs.
    fn factory_returns<'f>(
        &'f self,
        file: &FileFacts,
        initializer: solid_facts::core::Span,
    ) -> Option<(&'f FileFacts, Vec<&'f solid_facts::ast::ReturnFact>)> {
        let initializer = file.ast.peel_ts_sugar_span(initializer);
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == initializer && !call.construct)?;
        if call.arguments.iter().any(|argument| argument.spread) {
            return None;
        }
        let (factory_file, factory) = self
            .lookup
            .callee_symbol(file, call.callee)
            .and_then(|symbol| self.lookup.function_for_symbol(symbol))?;
        if factory.r#async
            || factory.generator
            || factory.method_name.is_some()
            || !self.lookup.function_value_is_current(factory_file, factory)
        {
            return None;
        }
        let returns = factory.expression_return.as_ref().map_or_else(
            || {
                factory_file
                    .ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        crate::owners::containing_ast_function(&factory_file.ast, returned.span)
                            .is_some_and(|owner| owner.span == factory.span)
                    })
                    .collect::<Vec<_>>()
            },
            |returned| vec![returned],
        );
        Some((factory_file, returns))
    }
}
