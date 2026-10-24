//! Per-file source discovery and the aggregate/merge types that combine files.

use super::*;

/// Whether a store-family creation is provably the value form
/// (`createStore(value)` / `createOptimisticStore(value)`), which never
/// builds a compute node. Runtime ground truth (probed, rc.0): `refresh()`
/// on such a store — or on any of its child records — throws
/// `INVALID_REFRESH_TARGET` in dev, while the function forms, projections,
/// and function-form optimistic stores all accept it. The runtime branches
/// on `typeof first === "function"`, so the proof is that argument 0 is a
/// non-callable value: a container or primitive literal, `null`, or
/// `undefined`. An identifier or other expression could still be a derive
/// function, so it stays unknown and refresh acceptance is preserved.
pub(super) fn store_is_value_form(
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
) -> bool {
    use solid_facts::ast::ArgumentValueKind;
    if !matches!(
        primitive,
        Some(Primitive::CreateStore | Primitive::CreateOptimisticStore)
    ) {
        return false;
    }
    call.arguments.first().is_some_and(|argument| {
        matches!(
            argument.value,
            ArgumentValueKind::Null | ArgumentValueKind::Undefined
        ) || argument.runtime_value_kind.is_data_literal()
    })
}

/// What a source-discovery fragment reads of the call creating a source that
/// a project function returns (ADR 0222): computed by the fragment and by the
/// cross-file digest that binds it (ADR 03XX-s6a), so the two cannot differ.
#[derive(Debug)]
pub(crate) struct ReturnedSourceFacts {
    pub(crate) owned_write: bool,
    pub(crate) value_form: bool,
    pub(crate) async_options: AsyncSourceOptions,
    pub(crate) async_source: bool,
}

pub(crate) fn returned_source_facts(
    lookup: &SemanticLookup<'_>,
    origin: &FileFacts,
    created: &solid_facts::ast::CallFact,
    primitive: Primitive,
) -> ReturnedSourceFacts {
    ReturnedSourceFacts {
        owned_write: created.owned_write_option,
        value_form: store_is_value_form(created, Some(primitive)),
        async_options: async_source_options(origin, created, Some(primitive), lookup.dialect),
        async_source: created
            .arguments
            .first()
            .is_some_and(|argument| computation_is_async(lookup, origin, argument.span)),
    }
}

pub(crate) fn discover_file_sources(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    ast_index: &CachedAstFileIndex,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> SourceDiscoveryContribution {
    let mut result = SourceDiscoveryContribution::default();
    for binding in &file.ast.bindings {
        let Some(initializer) = binding.call_initializer else {
            continue;
        };
        let Some(call) = ast_index.call_by_span(initializer) else {
            continue;
        };
        if crate::callback_return::callback_slot(lookup, file, call).is_some() {
            if let Some(returned) = crate::callback_return::resolve(lookup, file, call) {
                push_callback_return_sources(&mut result, lookup, file, entities, returned);
            }
            continue;
        }
        let primitive = crate::call_primitive_name_cached(
            lookup,
            file,
            call,
            entities,
            symbol_names,
            lookup.dialect,
        );
        let resolved = known_primitive(&primitive);
        if resolved.is_none()
            && binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
            && binding
                .initializer
                .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == call.span)
            && let Some(name) = binding.names.first()
            && !crate::indexes::binding_written(file, name.span)
            && let Some(symbol) = entities.at(file.path.as_str(), name.span)
            && let Some((origin_file, created, primitive)) =
                lookup.returned_source_at(file, call.span)
        {
            // The caller binding is this call's source, never the callee's
            // declaration symbol. Separate calls keep separate identities.
            let declaration = location(file.path.shared(), name.span);
            result.accessors.push((
                symbol.clone(),
                (
                    symbol_id(file.source_text(name.span).unwrap_or_default()),
                    declaration,
                ),
            ));
            result.source_kinds.push((
                symbol.clone(),
                if lookup.dialect.returns_store(primitive) {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                },
            ));
            result.source_phases.push((symbol.clone(), 1));
            if let Some(spelling) = lookup.dialect.name_of(primitive) {
                result
                    .source_primitives
                    .push((symbol.clone(), spelling.into()));
            }
            let facts = returned_source_facts(lookup, origin_file, created, primitive);
            result
                .source_owned_write
                .push((symbol.clone(), facts.owned_write));
            if facts.value_form {
                result.value_form_stores.push(symbol.clone());
            }
            if facts.async_options != AsyncSourceOptions::default() {
                result
                    .source_async_options
                    .push((symbol.clone(), facts.async_options));
            }
            if facts.async_source {
                result.async_sources.push(symbol.clone());
            }
            continue;
        }
        if resolved == Some(Primitive::Action) {
            if let Some(name) = binding.names.first() {
                let location = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&location) {
                    result.actions.push((
                        symbol.clone(),
                        (
                            symbol_id(file.source_text(name.span).unwrap_or_default()),
                            location,
                        ),
                    ));
                }
            }
            continue;
        }
        if resolved == Some(Primitive::Dynamic) {
            if let Some(name) = binding.names.first() {
                let declaration = location(file.path.shared(), name.span);
                if let Some(symbol) = entities.get(&declaration) {
                    result
                        .source_primitives
                        .push((symbol.clone(), "dynamic".into()));
                    if call
                        .arguments
                        .first()
                        .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
                    {
                        result.async_sources.push(symbol.clone());
                    }
                }
            }
            continue;
        }
        if !matches!(
            resolved,
            Some(primitive) if lookup.dialect.creates_reactive_source(primitive)
        ) {
            continue;
        }
        let source_kind = if matches!(
            resolved,
            Some(primitive) if lookup.dialect.returns_store(primitive)
        ) {
            ReactiveSourceKind::Store
        } else {
            ReactiveSourceKind::Accessor
        };
        let source_name = if binding.shape == solid_facts::ast::BindingShape::Array {
            binding.array_slots.first().and_then(Option::as_ref)
        } else {
            binding.names.first()
        };
        if let Some(name) = source_name {
            let declaration = location(file.path.shared(), name.span);
            if let Some(symbol) = entities.get(&declaration) {
                result.accessors.push((
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration,
                    ),
                ));
                // `Dialect::returns_reactive_tuple`, not a list: the list here
                // was 1.x's (`createResource` is 1.x-only) and so missed 2.0's
                // `createOptimistic` and `createOptimisticStore` entirely. That
                // method's own documentation names this as the thing it exists
                // to replace -- "shared code carried one hardcoded list that
                // was neither dialect's".
                let go_returned_source = binding.shape == solid_facts::ast::BindingShape::Array
                    && resolved
                        .is_some_and(|primitive| lookup.dialect.returns_reactive_tuple(primitive))
                    && binding_returns_reactive_source(binding, call);
                result.source_phases.push((
                    symbol.clone(),
                    if go_returned_source
                        && resolved.is_some_and(|primitive| lookup.dialect.returns_store(primitive))
                    {
                        2
                    } else if go_returned_source {
                        0
                    } else {
                        1
                    },
                ));
                if go_returned_source {
                    result.returned_source_symbols.push(symbol.clone());
                    result.summary_source_symbols.push(symbol.clone());
                }
                result.source_kinds.push((symbol.clone(), source_kind));
                if let Some(primitive) = primitive.as_deref() {
                    result
                        .source_primitives
                        .push((symbol.clone(), primitive.into()));
                }
                if store_is_value_form(call, resolved) {
                    result.value_form_stores.push(symbol.clone());
                }
                result
                    .source_owned_write
                    .push((symbol.clone(), call.owned_write_option));
                let options = async_source_options(file, call, resolved, lookup.dialect);
                if options != AsyncSourceOptions::default() {
                    result.source_async_options.push((symbol.clone(), options));
                }
                if call
                    .arguments
                    .first()
                    .is_some_and(|argument| computation_is_async(lookup, file, argument.span))
                {
                    result.async_sources.push(symbol.clone());
                }
            }
        }
        if resolved != Some(Primitive::CreateMemo)
            && let Some(name) = if binding.shape == solid_facts::ast::BindingShape::Array {
                binding.array_slots.get(1).and_then(Option::as_ref)
            } else {
                binding.names.get(1)
            }
        {
            let declaration = location(file.path.shared(), name.span);
            if let Some(symbol) = entities.get(&declaration) {
                result.setters.push((
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration,
                        call.owned_write_option,
                        source_kind,
                    ),
                ));
                if let Some(primitive) = primitive.as_deref() {
                    result
                        .source_primitives
                        .push((symbol.clone(), primitive.into()));
                }
            }
        }
    }
    for assignment in &file.ast.assignments {
        let (Some(initializer), Some(name)) = (
            assignment.call_initializer,
            assignment.array_slots.first().and_then(|slot| *slot),
        ) else {
            continue;
        };
        let Some(call) = ast_index.call_by_span(initializer) else {
            continue;
        };
        let symbol = entities.at(file.path.as_str(), name);
        let primitive = crate::call_primitive_name_cached(
            lookup,
            file,
            call,
            entities,
            symbol_names,
            lookup.dialect,
        );
        let resolved = known_primitive(&primitive);
        // Same seam as above, and the same reason.
        if !resolved.is_some_and(|primitive| lookup.dialect.returns_reactive_tuple(primitive)) {
            continue;
        }
        let Some(symbol) = symbol else {
            continue;
        };
        let declaration = location(file.path.shared(), name);
        let source_kind =
            if resolved.is_some_and(|primitive| lookup.dialect.returns_store(primitive)) {
                ReactiveSourceKind::Store
            } else {
                ReactiveSourceKind::Accessor
            };
        result.accessors.push((
            symbol.clone(),
            (
                symbol_id(file.source_text(name).unwrap_or_default()),
                declaration.clone(),
            ),
        ));
        result.source_kinds.push((symbol.clone(), source_kind));
        result.source_phases.push((
            symbol.clone(),
            if source_kind == ReactiveSourceKind::Store {
                2
            } else {
                0
            },
        ));
        result.returned_source_symbols.push(symbol.clone());
        result.summary_source_symbols.push(symbol.clone());
        if let Some(primitive) = primitive.as_deref() {
            result
                .source_primitives
                .push((symbol.clone(), primitive.into()));
        }
        if store_is_value_form(call, resolved) {
            result.value_form_stores.push(symbol.clone());
        }
        result
            .source_owned_write
            .push((symbol.clone(), call.owned_write_option));
        if let Some(setter) = assignment.array_slots.get(1).and_then(|slot| *slot)
            && let Some(setter_symbol) = entities.at(file.path.as_str(), setter)
        {
            result.setters.push((
                setter_symbol.clone(),
                (
                    symbol_id(file.source_text(setter).unwrap_or_default()),
                    location(file.path.shared(), setter),
                    call.owned_write_option,
                    source_kind,
                ),
            ));
            if let Some(primitive) = primitive.as_deref() {
                result
                    .source_primitives
                    .push((setter_symbol.clone(), primitive.into()));
            }
        }
    }
    result
}

pub(crate) struct SourceDiscoveryMergeTarget<'a> {
    pub(crate) accessors: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: &'a mut HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: &'a mut HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: &'a mut HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: &'a mut HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: &'a mut HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: &'a mut HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: &'a mut HashSet<SymbolId>,
    pub(crate) summary_source_symbols: &'a mut HashSet<SymbolId>,
    pub(crate) source_owned_write: &'a mut HashMap<SymbolId, bool>,
    pub(crate) async_sources: &'a mut HashSet<SymbolId>,
    pub(crate) source_async_options: &'a mut HashMap<SymbolId, AsyncSourceOptions>,
    pub(crate) value_form_stores: &'a mut HashSet<SymbolId>,
}

#[derive(Default)]
pub(crate) struct SourceDiscoveryAggregate {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: HashSet<SymbolId>,
    pub(crate) summary_source_symbols: HashSet<SymbolId>,
    pub(crate) source_owned_write: HashMap<SymbolId, bool>,
    pub(crate) async_sources: HashSet<SymbolId>,
    pub(crate) source_async_options: HashMap<SymbolId, AsyncSourceOptions>,
    pub(crate) value_form_stores: HashSet<SymbolId>,
}

impl SourceDiscoveryAggregate {
    pub(crate) fn merge(&mut self, contribution: &SourceDiscoveryContribution) {
        merge_source_discovery(
            contribution,
            SourceDiscoveryMergeTarget {
                accessors: &mut self.accessors,
                accessor_origins: &mut self.accessor_origins,
                setters: &mut self.setters,
                actions: &mut self.actions,
                source_kinds: &mut self.source_kinds,
                source_primitives: &mut self.source_primitives,
                source_phases: &mut self.source_phases,
                returned_source_symbols: &mut self.returned_source_symbols,
                summary_source_symbols: &mut self.summary_source_symbols,
                source_owned_write: &mut self.source_owned_write,
                async_sources: &mut self.async_sources,
                source_async_options: &mut self.source_async_options,
                value_form_stores: &mut self.value_form_stores,
            },
        );
    }

    pub(crate) fn append_to(self, target: SourceDiscoveryMergeTarget<'_>) {
        target.accessors.extend(self.accessors);
        target.accessor_origins.extend(self.accessor_origins);
        target.setters.extend(self.setters);
        target.actions.extend(self.actions);
        target.source_kinds.extend(self.source_kinds);
        target.source_primitives.extend(self.source_primitives);
        target.source_phases.extend(self.source_phases);
        target
            .returned_source_symbols
            .extend(self.returned_source_symbols);
        target
            .summary_source_symbols
            .extend(self.summary_source_symbols);
        target.source_owned_write.extend(self.source_owned_write);
        target.async_sources.extend(self.async_sources);
        target
            .source_async_options
            .extend(self.source_async_options);
        target.value_form_stores.extend(self.value_form_stores);
    }
}

pub(crate) fn merge_source_discovery(
    contribution: &SourceDiscoveryContribution,
    target: SourceDiscoveryMergeTarget<'_>,
) {
    target
        .accessors
        .extend(contribution.accessors.iter().cloned());
    target
        .accessor_origins
        .extend(contribution.accessor_origins.iter().cloned());
    target.setters.extend(contribution.setters.iter().cloned());
    target.actions.extend(contribution.actions.iter().cloned());
    target
        .source_kinds
        .extend(contribution.source_kinds.iter().cloned());
    target
        .source_primitives
        .extend(contribution.source_primitives.iter().cloned());
    target
        .source_phases
        .extend(contribution.source_phases.iter().cloned());
    target
        .returned_source_symbols
        .extend(contribution.returned_source_symbols.iter().cloned());
    target
        .summary_source_symbols
        .extend(contribution.summary_source_symbols.iter().cloned());
    target
        .source_owned_write
        .extend(contribution.source_owned_write.iter().cloned());
    target
        .async_sources
        .extend(contribution.async_sources.iter().cloned());
    target
        .source_async_options
        .extend(contribution.source_async_options.iter().cloned());
    target
        .value_form_stores
        .extend(contribution.value_form_stores.iter().cloned());
}

pub(crate) fn extend_source_discovery_symbols(
    symbols: &mut HashSet<SymbolId>,
    contribution: &SourceDiscoveryContribution,
) {
    symbols.extend(
        contribution
            .accessors
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .accessor_origins
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .setters
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .actions
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .source_kinds
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(
        contribution
            .source_primitives
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(contribution.async_sources.iter().cloned());
    symbols.extend(
        contribution
            .source_async_options
            .iter()
            .map(|(symbol, _)| symbol.clone()),
    );
    symbols.extend(contribution.value_form_stores.iter().cloned());
}

/// Owned reactive-source facts produced by the source-discovery stage and
/// consumed by the later interprocedural, static, and owner stages.
pub(crate) struct SourceDiscovery {
    pub(crate) accessors: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    /// The dialect primitive (canonical spelling) that created each reactive
    /// source, and, for a setter bound from the same tuple, the primitive that
    /// returned it: an owned-scope write guard can depend on which primitive
    /// made the setter (`Dialect::optimistic_store_setter_guarded`). A setter
    /// found only by its type has no entry.
    pub(crate) source_primitives: HashMap<SymbolId, SymbolId>,
    pub(crate) source_phases: HashMap<SymbolId, u8>,
    pub(crate) returned_source_symbols: HashSet<SymbolId>,
    pub(crate) summary_source_symbols: HashSet<SymbolId>,
    pub(crate) source_owned_write: HashMap<SymbolId, bool>,
    pub(crate) async_sources: HashSet<SymbolId>,
    pub(crate) source_async_options: HashMap<SymbolId, AsyncSourceOptions>,
    /// Store bindings proven to come from the value form
    /// (`createStore(value)` / `createOptimisticStore(value)`), which builds
    /// no compute node: `refresh()` on them (or a child record) throws
    /// `INVALID_REFRESH_TARGET` in dev (probed, rc.0). A store whose
    /// construction form is unknown is absent, keeping refresh acceptance.
    pub(crate) value_form_stores: HashSet<SymbolId>,
    /// ADR 0294: bindings and members whose only evidence of being a
    /// reactive source is a dialect type alias. Never in `accessors`.
    pub(crate) typed_sources: HashMap<SymbolId, TypedSource>,
    pub(crate) prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
    /// Prop roots whose enclosing function may be a component only by a
    /// dialect convention. Reads through them remain explicit proof
    /// obligations until a JSX call site or exact component type resolves the
    /// identity.
    pub(crate) uncertain_prop_sources: HashSet<SymbolId>,
    /// Caller-proven props reactivity per props declaration; empty (answering
    /// `Reactive` everywhere) for dialects that keep the upstream
    /// over-approximation.
    pub(crate) props_reactivity: PropsReactivityIndex,
    pub(crate) retained_source_paths: HashSet<String>,
    pub(crate) changed_source_symbols: HashSet<SymbolId>,
}

/// The stable, read-mostly environment threaded through every pipeline stage:
/// project facts, prebuilt indexes, the package boundary, and the semantic lookup.
#[derive(Clone, Copy)]
pub(crate) struct StageContext<'a> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) project_indexes: &'a ProjectIndexes<'a>,
    pub(crate) typescript_indexes: &'a CachedTypeScriptIndexes,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) source_declarations: &'a HashMap<SymbolId, Declaration>,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) semantic_lookup: &'a SemanticLookup<'a>,
    pub(crate) runtime: &'a crate::RuntimeEnvironment,
}

/// The spans of the function expressions that are `element`'s children value:
/// a `{...}` child expression container holding exactly one function (after
/// parentheses and type wrappers), or a `children={...}` attribute value.
/// Anything else lexically inside the element -- an attribute handler, a ref
/// callback, a nested element's own callbacks -- is not the children callback.
pub(super) fn jsx_children_function_spans(
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Vec<solid_facts::core::Span> {
    let mut candidates = Vec::new();
    for child in &element.children {
        let Some(text) = file.source_text(*child) else {
            continue;
        };
        let Some(inner) = text
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        else {
            continue;
        };
        let (Ok(leading), Ok(trailing)) = (
            u32::try_from(inner.len() - inner.trim_start().len()),
            u32::try_from(inner.len() - inner.trim_end().len()),
        ) else {
            continue;
        };
        let start = child.start + 1 + leading;
        let end = child.end - 1 - trailing;
        if start < end {
            candidates.push(solid_facts::core::Span::new(start, end));
        }
    }
    for attribute in &element.attributes {
        if attribute.namespace.is_none()
            && attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
            && file.source_text(attribute.name) == Some("children")
            && let Some(expression) = attribute.expression
        {
            candidates.push(expression);
        }
    }
    candidates
        .into_iter()
        .map(|candidate| file.ast.peel_ts_sugar_span(candidate))
        .collect()
}

/// Classifies a non-literal `keyed` attribute value.
///
/// Only a *proven function* selects the custom-key overload statically: an
/// inline function literal, or a value whose demanded type facts say
/// callable. Everything else — a boolean-typed expression, a member read, an
/// unresolved value — is a [`solid_dialect::KeyForm::DynamicFlag`], whose
/// runtime truthiness picks the keyed or unkeyed overload. RFC 03 tells
/// authors to "avoid dynamic boolean `keyed` values with function children"
/// and "prefer a literal `true`, literal `false`, or a custom key function";
/// until they do, the callback shape is ambiguous and the dialect tables
/// claim no accessor for it rather than fabricate a source for what may be a
/// raw value (see `children_accessor_parameters` in both dialects).
pub(super) fn dynamic_key_form(
    file: &FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    semantic_lookup: &SemanticLookup<'_>,
) -> solid_dialect::KeyForm {
    let expression = element.attributes.iter().find_map(|attribute| {
        (attribute.namespace.is_none() && file.source_text(attribute.local_name) == Some("keyed"))
            .then_some(attribute.expression)
            .flatten()
    });
    let Some(expression) = expression else {
        // A string or element value: truthy at runtime, but not a key
        // function — the ambiguous-flag stance applies.
        return solid_dialect::KeyForm::DynamicFlag;
    };
    if file
        .ast
        .functions
        .iter()
        .any(|function| function.span == expression)
    {
        return solid_dialect::KeyForm::CustomKey;
    }
    // Callability is trusted for plain identifier values only. At a call
    // expression's span the demanded entity answers for the *callee* — a
    // boolean-returning `keyed={cond()}` reads back `Callable` — so any
    // computed value stays in the ambiguous bucket.
    let identifier = file.ast.identifiers.iter().any(|identifier| {
        identifier.span == expression
            && identifier.role == solid_facts::ast::IdentifierRole::Reference
    });
    if identifier
        && matches!(
            semantic_lookup
                .entity_at(file.path.as_str(), expression)
                .and_then(|entity| entity.callability),
            Some(typefacts::Callability::Callable | typefacts::Callability::UntypedCallable)
        )
    {
        return solid_dialect::KeyForm::CustomKey;
    }
    // Boolean-typed, computed, or unresolved: refuse to fabricate.
    solid_dialect::KeyForm::DynamicFlag
}
