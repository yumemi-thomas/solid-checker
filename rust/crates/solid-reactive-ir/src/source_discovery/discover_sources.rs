//! Project-wide source discovery (`discover_sources`).

use super::*;

/// The source tables discovery fills, owned together so the passes that read
/// and extend them can each take just the tables they touch.
#[derive(Default)]
struct SourceTables {
    accessors: HashMap<SymbolId, (SymbolId, Location)>,
    accessor_origins: HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    setters: HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    actions: HashMap<SymbolId, (SymbolId, Location)>,
    source_kinds: HashMap<SymbolId, ReactiveSourceKind>,
    source_primitives: HashMap<SymbolId, SymbolId>,
    source_phases: HashMap<SymbolId, u8>,
    returned_source_symbols: HashSet<SymbolId>,
    summary_source_symbols: HashSet<SymbolId>,
    source_owned_write: HashMap<SymbolId, bool>,
    async_sources: HashSet<SymbolId>,
    source_async_options: HashMap<SymbolId, AsyncSourceOptions>,
    value_form_stores: HashSet<SymbolId>,
    typed_sources: HashMap<SymbolId, TypedSource>,
    prop_sources: HashMap<SymbolId, (SymbolId, Location)>,
    uncertain_prop_sources: HashSet<SymbolId>,
}

impl SourceTables {
    fn merge_target(&mut self) -> SourceDiscoveryMergeTarget<'_> {
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
        }
    }
}

pub(crate) fn discover_sources(
    ctx: &StageContext<'_>,
    source_discovery_cache: Option<&mut HashMap<SourcePath, CachedSourceDiscovery>>,
    typescript_unchanged: bool,
    build_timings: &mut BuildTimings,
    emit_timings: bool,
) -> SourceDiscovery {
    let mut clock = StageClock::new(emit_timings);
    let mut tables = SourceTables::default();
    let (retained_source_paths, changed_source_symbols) = merge_file_source_contributions(
        ctx,
        source_discovery_cache,
        typescript_unchanged,
        build_timings,
        &mut tables,
    );
    clock.finish(build_timings, ReactiveIrStage::SourceDiscovery);
    // Source construction/accepted-return facts collected above, before the
    // type-only fallback. A published Accessor/Store annotation alone cannot
    // witness a runtime reactive Get in an incoming prop expression.
    let mut runtime_sources = tables.accessors.keys().cloned().collect::<HashSet<_>>();
    let mut literal_member_slots =
        crate::literal_member_slots::LiteralMemberSlots::new(ctx.facts, ctx.semantic_lookup);
    discover_withdrawn_tuple_candidates(ctx, &runtime_sources, &mut tables.typed_sources);
    discover_typed_sources(
        ctx,
        &mut tables,
        &runtime_sources,
        &mut literal_member_slots,
    );
    discover_children_accessors(ctx, &mut runtime_sources, &mut tables.accessors);
    discover_effect_store_parameters(ctx, &mut tables);
    resolve_setter_action_aliases(ctx, &mut tables);
    resolve_store_view_aliases(ctx, &mut tables);
    discover_prop_sources(ctx, &mut tables);
    clock.finish(build_timings, ReactiveIrStage::TypedAccessorsAndPropRoots);
    propagate_prop_sources(ctx, &mut tables);
    let SourceTables {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        returned_source_symbols,
        summary_source_symbols,
        source_owned_write,
        async_sources,
        source_async_options,
        value_form_stores,
        typed_sources,
        prop_sources,
        uncertain_prop_sources,
    } = tables;
    let StageContext {
        facts,
        entities,
        semantic_lookup,
        runtime,
        ..
    } = *ctx;
    // A runtime source stays only while no file writes it. Collect every
    // written symbol once rather than asking each source of each file.
    let mut written: HashSet<&SymbolId> = HashSet::default();
    for file in &facts.files {
        written.extend(crate::value_identity::written_symbols(file, entities));
        for target in &file.ast.iteration_targets {
            for identifier in file.ast.identifiers_within(*target) {
                written.extend(entities.at(file.path.as_str(), identifier.span));
            }
        }
    }
    runtime_sources.retain(|symbol| !written.contains(symbol));
    let props_reactivity = classify_component_props(
        runtime,
        facts,
        semantic_lookup,
        entities,
        &accessors,
        &source_kinds,
        &prop_sources,
        &runtime_sources,
    );
    clock.finish(
        build_timings,
        ReactiveIrStage::PropPropagationAndControlFlow,
    );
    SourceDiscovery {
        accessors,
        accessor_origins,
        setters,
        actions,
        source_kinds,
        source_primitives,
        source_phases,
        returned_source_symbols,
        summary_source_symbols,
        source_owned_write,
        async_sources,
        source_async_options,
        value_form_stores,
        typed_sources,
        prop_sources,
        uncertain_prop_sources,
        props_reactivity,
        retained_source_paths,
        changed_source_symbols,
    }
}

fn merge_file_source_contributions(
    ctx: &StageContext<'_>,
    source_discovery_cache: Option<&mut HashMap<SourcePath, CachedSourceDiscovery>>,
    typescript_unchanged: bool,
    build_timings: &mut BuildTimings,
    tables: &mut SourceTables,
) -> (HashSet<String>, HashSet<SymbolId>) {
    let StageContext {
        facts,
        project_indexes,
        typescript_indexes,
        entities,
        symbol_names,
        semantic_lookup,
        ..
    } = *ctx;
    let mut retained_source_paths = HashSet::<String>::default();
    let mut changed_source_symbols = HashSet::<SymbolId>::default();
    match source_discovery_cache {
        None => {
            let contributions = parallel_file_results(&facts.files, |file| {
                discover_file_sources(
                    semantic_lookup,
                    file,
                    project_indexes
                        .ast_files_by_path
                        .get(file.path.as_str())
                        .expect("project index contains every source file"),
                    entities,
                    symbol_names,
                )
            });
            let mut aggregate = SourceDiscoveryAggregate::default();
            for contribution in &contributions {
                aggregate.merge(contribution);
            }
            aggregate.append_to(tables.merge_target());
        }
        Some(cache) => {
            let current_paths = facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache.retain(|path, _| current_paths.contains(path.as_str()));
            let mut identity_misses = 0;
            let reusable_paths = facts
                .files
                .iter()
                .filter_map(|file| {
                    cache
                        .get(file.path.as_str())
                        .is_some_and(|cached| {
                            // A returned-source proof's callee syntax is bound
                            // by `cross_file_proof_digest` (ADR 0222); its
                            // symbol resolution by the per-file delta.
                            let identity = source_discovery_identity_matches(
                                &cached.identity,
                                file.path.as_str(),
                                &file.source_hash,
                                typescript_unchanged,
                                typescript_indexes.source_discovery_delta.as_ref(),
                            );
                            identity_misses += u64::from(!identity);
                            identity
                                && cached.cross_file_proofs
                                    == semantic_lookup.cross_file_proof_digest()
                        })
                        .then_some(file.path.as_str())
                })
                .collect::<HashSet<_>>();
            build_timings.source_discovery_identity_misses = identity_misses;
            let recomputed = facts
                .files
                .iter()
                .filter(|file| !reusable_paths.contains(file.path.as_str()))
                .collect::<Vec<_>>();
            let discovered = parallel_slice_results(&recomputed, |file| {
                (
                    source_discovery_identity(file, project_indexes),
                    discover_file_sources(
                        semantic_lookup,
                        file,
                        project_indexes
                            .ast_files_by_path
                            .get(file.path.as_str())
                            .expect("project index contains every source file"),
                        entities,
                        symbol_names,
                    ),
                )
            });
            let mut discovered = discovered.into_iter();
            for file in &facts.files {
                if reusable_paths.contains(file.path.as_str()) {
                    build_timings.source_discovery_reused_files += 1;
                    retained_source_paths.insert(file.path.to_string());
                    continue;
                }
                let (identity, contribution) = discovered
                    .next()
                    .expect("recomputed source path has a fresh contribution");
                build_timings.source_discovery_recomputed_files += 1;
                if let Some(cached) = cache.get(file.path.as_str()) {
                    extend_source_discovery_symbols(
                        &mut changed_source_symbols,
                        &cached.contribution,
                    );
                }
                extend_source_discovery_symbols(&mut changed_source_symbols, &contribution);
                cache.insert(
                    file.path.clone(),
                    CachedSourceDiscovery {
                        identity,
                        cross_file_proofs: semantic_lookup.cross_file_proof_digest(),
                        contribution,
                    },
                );
            }
            debug_assert!(discovered.next().is_none());
            let cache = &*cache;
            for aggregate in parallel_file_chunk_results(&facts.files, |files| {
                let mut aggregate = SourceDiscoveryAggregate::default();
                for file in files {
                    if let Some(cached) = cache.get(file.path.as_str()) {
                        aggregate.merge(&cached.contribution);
                    }
                }
                aggregate
            }) {
                aggregate.append_to(tables.merge_target());
            }
        }
    }
    (retained_source_paths, changed_source_symbols)
}

fn discover_withdrawn_tuple_candidates(
    ctx: &StageContext<'_>,
    runtime_sources: &HashSet<SymbolId>,
    typed_sources: &mut HashMap<SymbolId, TypedSource>,
) {
    let StageContext {
        facts,
        project_indexes,
        entities,
        symbol_names,
        semantic_lookup,
        ..
    } = *ctx;
    // A tuple type describes the factory's result, not the values a
    // replaced iterator hands the binding. Preserve those former source
    // candidates even when the package's declared type is a plain function
    // or object, so a withdrawn proof cannot certify silence.
    if semantic_lookup.tuple_iteration_may_be_mutated() {
        for file in &facts.files {
            let ast_index = &project_indexes.ast_files_by_path[file.path.as_str()];
            for call in &file.ast.calls {
                for (name, value) in
                    crate::callback_return::withdrawn_tuple_candidates(semantic_lookup, file, call)
                {
                    let kind = match value.kind.as_str() {
                        "accessor" => ReactiveSourceKind::Accessor,
                        "store-path" => ReactiveSourceKind::Store,
                        _ => continue,
                    };
                    if let Some(symbol) = entities.at(file.path.as_str(), name) {
                        typed_sources.insert(
                            symbol.clone(),
                            TypedSource {
                                name: symbol_id(file.source_text(name).unwrap_or_default()),
                                declaration: location(file.path.shared(), name),
                                kind,
                                implementation_visible: false,
                                held_body_unfollowed: false,
                            },
                        );
                    }
                }
            }
            // Identify only former candidates under the pre-veto premise.
            // Never install that derivation as runtime source authority: a
            // primitive tuple consumed inside a wrapper can also be replaced.
            for binding in file.ast.bindings.iter().filter(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.len() == 1
            }) {
                let name = &binding.names[0];
                let Some(symbol) = entities.at(file.path.as_str(), name.span) else {
                    continue;
                };
                if runtime_sources.contains(symbol)
                    || crate::indexes::binding_written(file, name.span)
                {
                    continue;
                }
                let Some(call) = binding
                    .call_initializer
                    .and_then(|span| ast_index.call_by_span(span))
                else {
                    continue;
                };
                let Some((_, _, primitive)) = returned_source_through(
                    semantic_lookup,
                    file,
                    call,
                    entities,
                    symbol_names,
                    (RETURNED_SOURCE_FORWARDS, false),
                ) else {
                    continue;
                };
                typed_sources.insert(
                    symbol.clone(),
                    TypedSource {
                        name: symbol_id(file.source_text(name.span).unwrap_or_default()),
                        declaration: location(file.path.shared(), name.span),
                        kind: if semantic_lookup.dialect.returns_store(primitive) {
                            ReactiveSourceKind::Store
                        } else {
                            ReactiveSourceKind::Accessor
                        },
                        implementation_visible: false,
                        held_body_unfollowed: false,
                    },
                );
            }
        }
    }
}

fn discover_typed_sources(
    ctx: &StageContext<'_>,
    tables: &mut SourceTables,
    runtime_sources: &HashSet<SymbolId>,
    literal_member_slots: &mut crate::literal_member_slots::LiteralMemberSlots<'_, '_>,
) {
    let StageContext {
        facts,
        entities,
        source_declarations,
        semantic_lookup,
        ..
    } = *ctx;
    let SourceTables {
        accessors,
        setters,
        source_kinds,
        source_phases,
        typed_sources,
        ..
    } = tables;
    for entity in facts.typescript.entities() {
        let Some(descriptor) = &entity.type_descriptor else {
            continue;
        };
        if !semantic_lookup
            .dialect
            .owns_module(descriptor.origin_module.as_ref())
        {
            continue;
        }
        let Some(symbol) = entities.exact(&entity.location) else {
            continue;
        };
        let Some((role, type_declaration)) =
            descriptor
                .alias_declarations
                .iter()
                .find_map(|declaration| {
                    semantic_lookup
                        .dialect
                        .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                        .map(|role| (role, declaration.location.clone()))
                })
        else {
            continue;
        };
        if matches!(
            role,
            solid_dialect::TypeRole::Component | solid_dialect::TypeRole::Owner
        ) {
            continue;
        }
        if semantic_lookup
            .file_by_path(entity.location.path.as_ref())
            .is_some_and(|file| {
                let (Ok(start), Ok(end)) = (
                    u32::try_from(entity.location.start_byte),
                    u32::try_from(entity.location.end_byte),
                ) else {
                    return true;
                };
                crate::callback_return::binding_is_callback_result(
                    semantic_lookup,
                    file,
                    solid_facts::core::Span::new(start, end),
                )
            })
        {
            continue;
        }
        // ADR 0172: a tuple containing an accessor is not itself an accessor.
        // Demanded member/call spans may carry the member's type beside a
        // receiver symbol. Only an exact value's own callable type can
        // introduce the typed root; a member must also resolve separately
        // from its receiver. Nested alias declarations cannot do so.
        if matches!(
            role,
            solid_dialect::TypeRole::Accessor | solid_dialect::TypeRole::Signal
        ) && (entity.callability != Some(typefacts::Callability::Callable)
            || semantic_lookup
                .file_by_path(entity.location.path.as_ref())
                .is_some_and(|file| {
                    crate::value_identity::binding_has_write(file, entities, symbol)
                })
            || !semantic_lookup
                .file_by_path(entity.location.path.as_ref())
                .is_some_and(|file| {
                    file.ast.identifiers.iter().any(|identifier| {
                        u64::from(identifier.span.start) == entity.location.start_byte
                            && u64::from(identifier.span.end) == entity.location.end_byte
                    }) || file.ast.members.iter().any(|member| {
                        ((u64::from(member.span.start) == entity.location.start_byte
                            && u64::from(member.span.end) == entity.location.end_byte)
                            || (u64::from(member.property.start) == entity.location.start_byte
                                && u64::from(member.property.end) == entity.location.end_byte))
                            && !file.ast.computed_members.contains(&member.span)
                            && entities.at(file.path.as_str(), member.property) == Some(symbol)
                            && entities.at(file.path.as_str(), member.object) != Some(symbol)
                    })
                }))
        {
            continue;
        }
        // ADR 0286: a property an object literal declares is typed by what
        // the literal stored, which is not what its slot holds once the
        // object reached code that may replace the member.
        if literal_member_slots.replaceable(symbol) {
            continue;
        }
        let declaration = source_declarations.get(symbol);
        let (name, local_location) = declaration.map_or_else(
            || ("accessor".into(), entity.location.clone()),
            |declaration| (declaration.name.clone(), declaration.location.clone()),
        );
        let declaration_location = if type_declaration.path.is_empty() {
            local_location
        } else {
            type_declaration
        };
        // ADR 0294: the type names what the value may be, never that calling
        // or reading it reads reactive state. A proven source keeps what
        // proved it. Otherwise the value the slot holds decides: a source it
        // provably holds is a proven root, a literal it provably holds is
        // decided by that literal's own body or shape, and anything else is
        // a typed candidate, never a proven root.
        let typed_kind = match role {
            solid_dialect::TypeRole::Accessor | solid_dialect::TypeRole::Signal => {
                Some(ReactiveSourceKind::Accessor)
            }
            solid_dialect::TypeRole::Store => Some(ReactiveSourceKind::Store),
            _ => None,
        };
        if let Some(kind) = typed_kind {
            if accessors.contains_key(symbol) {
                continue;
            }
            match typed_implementation(
                semantic_lookup,
                entities,
                literal_member_slots,
                &entity.location,
                symbol,
                kind,
                runtime_sources,
                source_kinds,
            ) {
                TypedImplementation::Source(source) => {
                    let (source_name, source_declaration) = accessors[&source].clone();
                    accessors.insert(symbol.clone(), (source_name, source_declaration));
                    source_kinds.insert(symbol.clone(), kind);
                    source_phases.entry(symbol.clone()).or_insert(1);
                }
                TypedImplementation::Visible => {
                    typed_sources.insert(
                        symbol.clone(),
                        TypedSource {
                            name: symbol_id(name.as_ref()),
                            declaration: declaration_location,
                            kind,
                            implementation_visible: true,
                            held_body_unfollowed: false,
                        },
                    );
                }
                TypedImplementation::HeldUnfollowed => {
                    typed_sources.insert(
                        symbol.clone(),
                        TypedSource {
                            name: symbol_id(name.as_ref()),
                            declaration: declaration_location,
                            kind,
                            implementation_visible: false,
                            held_body_unfollowed: true,
                        },
                    );
                }
                TypedImplementation::Unknown => {
                    typed_sources.insert(
                        symbol.clone(),
                        TypedSource {
                            name: symbol_id(name.as_ref()),
                            declaration: declaration_location,
                            kind,
                            implementation_visible: false,
                            held_body_unfollowed: false,
                        },
                    );
                }
            }
            continue;
        }
        match role {
            solid_dialect::TypeRole::Accessor
            | solid_dialect::TypeRole::Signal
            | solid_dialect::TypeRole::Store => unreachable!(),
            solid_dialect::TypeRole::Setter | solid_dialect::TypeRole::StoreSetter => {
                let kind = if role == solid_dialect::TypeRole::StoreSetter {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                };
                setters.entry(symbol.clone()).or_insert((
                    symbol_id(name.as_ref()),
                    declaration_location,
                    false,
                    kind,
                ));
            }
            solid_dialect::TypeRole::Component | solid_dialect::TypeRole::Owner => unreachable!(),
        }
    }
}

fn discover_children_accessors(
    ctx: &StageContext<'_>,
    runtime_sources: &mut HashSet<SymbolId>,
    accessors: &mut HashMap<SymbolId, (SymbolId, Location)>,
) {
    let StageContext {
        facts,
        entities,
        symbol_names,
        semantic_lookup,
        ..
    } = *ctx;
    for file in &facts.files {
        for element in &file.ast.jsx_elements {
            let dialect = semantic_lookup.dialect;
            let primitive = jsx_primitive_name(file, element, entities, symbol_names, dialect);
            let keyed = element
                .boolean_properties
                .iter()
                .find(|property| file.source_text(property.name) == Some("keyed"))
                .map(|property| property.value);
            let key = match keyed {
                Some(true) => solid_dialect::KeyForm::Keyed,
                Some(false) => solid_dialect::KeyForm::Unkeyed,
                None if element
                    .properties
                    .iter()
                    .any(|property| file.source_text(*property) == Some("keyed")) =>
                {
                    dynamic_key_form(file, element, semantic_lookup)
                }
                None => solid_dialect::KeyForm::Absent,
            };
            // Which children parameters are accessors is the dialect's
            // question. The match this replaced knew `<For>` and not
            // `<Index>`, which are exact mirrors in 1.x, so every 1.x
            // `<Index>` item accessor would be invisible to source discovery.
            let parameter_indices = known_primitive(&primitive).map_or(&[][..], |primitive| {
                dialect.children_accessor_parameters(primitive, key)
            });
            if parameter_indices.is_empty() {
                continue;
            }
            // The accessor parameters belong to the function that IS this
            // element's children value -- the sole `{...}` child expression
            // or a `children={...}` attribute -- and to no other function that
            // happens to sit lexically inside the element (a `<For>` row
            // callback, an event handler, or a ref callback are plain-value
            // callbacks of their own).
            let children_functions = jsx_children_function_spans(file, element);
            for function in file
                .ast
                .functions
                .iter()
                .filter(|function| children_functions.contains(&function.span))
            {
                for index in parameter_indices {
                    let Some(parameter) = function
                        .parameters
                        .get(*index)
                        .and_then(|parameter| parameter.names.first())
                    else {
                        continue;
                    };
                    let declaration = location(file.path.shared(), parameter.span);
                    if let Some(symbol) = entities.get(&declaration) {
                        runtime_sources.insert(symbol.clone());
                        accessors.entry(symbol.clone()).or_insert((
                            symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                            declaration,
                        ));
                    }
                }
            }
        }
    }
}

fn discover_effect_store_parameters(ctx: &StageContext<'_>, tables: &mut SourceTables) {
    let StageContext {
        facts,
        entities,
        source_declarations,
        symbol_names,
        semantic_lookup,
        ..
    } = *ctx;
    let SourceTables {
        accessors,
        source_kinds,
        ..
    } = tables;
    for file in &facts.files {
        for call in &file.ast.calls {
            if !crate::call_primitive_name_cached(
                semantic_lookup,
                file,
                call,
                entities,
                symbol_names,
                semantic_lookup.dialect,
            )
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                matches!(
                    primitive,
                    Primitive::CreateEffect | Primitive::CreateRenderEffect
                )
            }) {
                continue;
            }
            let Some(compute) = call.arguments.first().and_then(|argument| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| argument.span.contains(function.span))
                    .max_by_key(|function| function.span.end - function.span.start)
            }) else {
                continue;
            };
            let returned = compute.expression_return.as_ref().or_else(|| {
                file.ast.returns.iter().find(|returned| {
                    compute.body.contains(returned.span)
                        && containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == compute.span)
                })
            });
            let Some(source_symbol) = returned
                .and_then(|returned| {
                    entities
                        .get(&location(file.path.shared(), returned.span))
                        .or_else(|| {
                            (returned.value == solid_facts::ast::ReturnValueKind::Identifier)
                                .then_some(returned.span)
                                .and_then(|span| file.source_text(span))
                                .and_then(|name| {
                                    source_declarations
                                        .iter()
                                        .find_map(|(symbol, declaration)| {
                                            (declaration.name == name.into()
                                                && declaration.location.path
                                                    == file.path.as_str().into())
                                            .then_some(symbol)
                                        })
                                })
                        })
                })
                // The second half of this test used to name `createStore` and
                // `createOptimisticStore`, and it could never change the
                // answer. `source_kinds` and `source_primitives` are written
                // together wherever a reactive source is discovered (the
                // `creates_reactive_source` path above and the tuple-binding
                // path), and the kind recorded there is `Store` exactly when
                // `returns_store` holds for that same primitive. So "the
                // primitive returns a store" implies "the kind is Store", for
                // any dialect, and only the kind is worth asking.
                .filter(|symbol| source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store))
            else {
                continue;
            };
            let Some(apply) = call.arguments.get(1).and_then(|argument| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| argument.span.contains(function.span))
                    .max_by_key(|function| function.span.end - function.span.start)
            }) else {
                continue;
            };
            let Some(parameter) = apply
                .parameters
                .first()
                .and_then(|parameter| parameter.names.first())
            else {
                continue;
            };
            let parameter_location = location(file.path.shared(), parameter.span);
            let Some(parameter_symbol) = entities.get(&parameter_location) else {
                continue;
            };
            let (display, declaration) =
                accessors.get(source_symbol).cloned().unwrap_or_else(|| {
                    (
                        symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                        location(file.path.shared(), parameter.span),
                    )
                });
            accessors.insert(parameter_symbol.clone(), (display, declaration));
            source_kinds.insert(parameter_symbol.clone(), ReactiveSourceKind::Store);
        }
    }
}

fn resolve_setter_action_aliases(ctx: &StageContext<'_>, tables: &mut SourceTables) {
    let StageContext {
        facts, entities, ..
    } = *ctx;
    let SourceTables {
        setters,
        actions,
        source_primitives,
        ..
    } = tables;
    loop {
        let mut setter_aliases = Vec::new();
        let mut action_aliases = Vec::new();
        // An alias of a setter was made by the same primitive, which an
        // owned-scope guard can depend on (`optimistic_*_setter_guarded`).
        let mut primitive_aliases = Vec::new();
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let Some(source_symbol) =
                    binding
                        .initializer_identifier
                        .as_ref()
                        .and_then(|identifier| {
                            entities.get(&location(file.path.shared(), identifier.span))
                        })
                else {
                    continue;
                };
                let setter = setters.get(source_symbol).cloned();
                let action = actions.get(source_symbol).cloned();
                if setter.is_none() && action.is_none() {
                    continue;
                }
                for name in &binding.names {
                    let declaration = location(file.path.shared(), name.span);
                    let Some(symbol) = entities.get(&declaration) else {
                        continue;
                    };
                    if let Some((_, source, owned_write, source_kind)) = &setter
                        && !setters.contains_key(symbol)
                    {
                        if binding.immutable
                            && let Some(primitive) = source_primitives.get(source_symbol)
                        {
                            primitive_aliases.push((symbol.clone(), primitive.clone()));
                        }
                        setter_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                                *owned_write,
                                *source_kind,
                            ),
                        ));
                    }
                    if let Some((_, source)) = &action
                        && !actions.contains_key(symbol)
                    {
                        action_aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                source.clone(),
                            ),
                        ));
                    }
                }
            }
        }
        if setter_aliases.is_empty() && action_aliases.is_empty() {
            break;
        }
        setters.extend(setter_aliases);
        actions.extend(action_aliases);
        for (symbol, primitive) in primitive_aliases {
            source_primitives.entry(symbol).or_insert(primitive);
        }
    }
}

fn resolve_store_view_aliases(ctx: &StageContext<'_>, tables: &mut SourceTables) {
    let StageContext {
        facts,
        entities,
        semantic_lookup,
        ..
    } = *ctx;
    let SourceTables {
        accessors,
        source_kinds,
        ..
    } = tables;
    // ADR 0323 (review round 10): an `omit` or `merge` view over a store reads
    // through to it (`viewSource(e)[key]`, `@solidjs/signals@2.0.0-rc.13`
    // `dist/prod/store/utils.js`), so a binding of one is a store alias, like
    // an effect apply's store parameter above.
    loop {
        let mut aliases = Vec::new();
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let Some(initializer) = binding.call_initializer else {
                    continue;
                };
                let Some(root) = view_root(
                    file,
                    initializer,
                    entities,
                    semantic_lookup,
                    &|symbol| {
                        source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
                            && accessors.contains_key(symbol)
                    },
                    0,
                ) else {
                    continue;
                };
                let Some((_, declaration)) = accessors.get(&root).cloned() else {
                    continue;
                };
                for name in &binding.names {
                    if let Some(symbol) = entities.get(&location(file.path.shared(), name.span))
                        && !accessors.contains_key(symbol)
                    {
                        aliases.push((
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                declaration.clone(),
                            ),
                        ));
                    }
                }
            }
        }
        if aliases.is_empty() {
            break;
        }
        for (symbol, entry) in aliases {
            accessors.insert(symbol.clone(), entry);
            source_kinds.insert(symbol, ReactiveSourceKind::Store);
        }
    }
}

fn discover_prop_sources(ctx: &StageContext<'_>, tables: &mut SourceTables) {
    let StageContext {
        facts,
        entities,
        semantic_lookup,
        ..
    } = *ctx;
    let SourceTables {
        prop_sources,
        uncertain_prop_sources,
        ..
    } = tables;
    for file in &facts.files {
        for function in &file.ast.functions {
            let component_status = semantic_lookup.function_component_status(file, function);
            if component_status == crate::indexes::ComponentStatus::No {
                continue;
            }
            let Some(parameter) = function
                .parameters
                .first()
                .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
                .and_then(|parameter| parameter.names.first())
            else {
                continue;
            };
            let declaration = location(file.path.shared(), parameter.span);
            if let Some(symbol) = entities.get(&declaration) {
                if component_status == crate::indexes::ComponentStatus::Uncertain {
                    uncertain_prop_sources.insert(symbol.clone());
                }
                prop_sources.insert(
                    symbol.clone(),
                    (
                        symbol_id(file.source_text(parameter.span).unwrap_or_default()),
                        declaration,
                    ),
                );
            }
        }
    }
}

fn propagate_prop_sources(ctx: &StageContext<'_>, tables: &mut SourceTables) {
    let StageContext {
        facts,
        entities,
        semantic_lookup,
        ..
    } = *ctx;
    let SourceTables {
        prop_sources,
        uncertain_prop_sources,
        ..
    } = tables;
    loop {
        let mut changed = false;
        for file in &facts.files {
            for binding in &file.ast.bindings {
                let source = binding
                    .initializer_identifier
                    .as_ref()
                    .and_then(|identifier| {
                        entities.get(&location(file.path.shared(), identifier.span))
                    })
                    .and_then(|symbol| {
                        prop_sources
                            .get(symbol)
                            .cloned()
                            .map(|source| (source, uncertain_prop_sources.contains(symbol)))
                    })
                    .or_else(|| {
                        let initializer = binding.call_initializer?;
                        let root = view_root(
                            file,
                            initializer,
                            entities,
                            semantic_lookup,
                            &|symbol| prop_sources.contains_key(symbol),
                            0,
                        )?;
                        prop_sources
                            .get(&root)
                            .cloned()
                            .map(|source| (source, uncertain_prop_sources.contains(&root)))
                    });
                let Some(((_, declaration), source_uncertain)) = source else {
                    continue;
                };
                for name in &binding.names {
                    let binding_location = location(file.path.shared(), name.span);
                    if let Some(symbol) = entities.get(&binding_location)
                        && !prop_sources.contains_key(symbol)
                    {
                        if source_uncertain {
                            uncertain_prop_sources.insert(symbol.clone());
                        }
                        prop_sources.insert(
                            symbol.clone(),
                            (
                                symbol_id(file.source_text(name.span).unwrap_or_default()),
                                declaration.clone(),
                            ),
                        );
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}
