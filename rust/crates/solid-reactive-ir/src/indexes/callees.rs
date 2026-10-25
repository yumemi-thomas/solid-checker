//! Callee resolution: callee symbols, method and namespace-member functions, and class instance exactness.

use super::*;

impl<'a> SemanticLookup<'a> {
    pub(crate) fn standard_library_declaration(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        kind: typefacts::CallKind,
    ) -> Option<&'a typefacts::ResolvedDeclaration> {
        standard_library_declaration_of(self.resolved_callee_call(file, call.callee), kind)
    }

    /// ADR 0211: the instance method `property` of exactly `class`, declared
    /// in its own body: one plain method, not static, not an accessor.
    pub(crate) fn class_method_symbol(
        &self,
        class_file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
        property: &str,
    ) -> Option<&'a SymbolId> {
        use solid_facts::ast::ClassElementKind;
        if !self.class_instance_is_exact(class_file, class, property, 4) {
            return None;
        }
        let mut methods = class.elements.iter().filter(|element| {
            !element.r#static
                && element.key.and_then(|key| class_file.source_text(key)) == Some(property)
        });
        let method = methods.next()?;
        if methods.next().is_some() || method.kind != ClassElementKind::Method {
            return None;
        }
        let function = class_file
            .ast
            .functions
            .iter()
            .find(|function| Some(function.span) == method.value)?;
        self.entities.at(
            class_file.path.as_str(),
            function.method_name.as_ref()?.span,
        )
    }

    /// Whether the value a project function's symbol names now is that
    /// function: a declaration whose name nothing writes, a method, or the
    /// direct initializer of a `const`. A `let` that is reassigned, or a
    /// binding initialized by a call that merely contains the function
    /// (`const f = wrap(() => …)`), proves nothing.
    pub(crate) fn function_value_is_current(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        function_value_is_current_in(file, function)
    }

    /// Whether `new C(…)` of this class evaluates to an instance whose
    /// `property` is found on its prototype chain (ADR 0209, 0211 as
    /// amended): no constructor in the chain returns another object, and no
    /// class in the chain defines `property` as an own field, a parameter
    /// property, or under a computed key. Every ancestor must be a project
    /// class this resolves; an unresolved `extends` proves nothing.
    pub(crate) fn class_instance_is_exact(
        &self,
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
        property: &str,
        depth: usize,
    ) -> bool {
        super::callee_world::class_instance_is_exact_in(self, file, class, property, depth)
    }

    pub(crate) fn binding_at_reference(
        &self,
        path: &str,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::BindingFact, SymbolId)> {
        // A hand-out (`hand_outs`): the answer, and the file it names.
        self.note_binding_at(path);
        let resolution = self.binding_resolution_at(path, span)?;
        self.note_slot_read(resolution.file);
        let file = &self.facts.files[resolution.file];
        Some((
            file,
            &file.ast.bindings[resolution.binding],
            resolution.symbol.clone(),
        ))
    }

    /// The slot of the file and the ordinal of the binding that the reference
    /// at `span` of `path` resolves to: [`Self::binding_at_reference`]'s
    /// answer as positions.
    pub(super) fn binding_resolution_at(
        &self,
        path: &str,
        span: Span,
    ) -> Option<&BindingResolution> {
        let references = self.bindings_by_reference().get(path)?;
        self.resolution_in(references, self.file_by_path(path), span)
    }

    /// The symbol [`Self::binding_at_reference`] names at each of `spans` of
    /// `path`, finding the path's resolutions once.
    pub(crate) fn binding_symbols_at(&self, path: &str, spans: &[Span]) -> Vec<Option<SymbolId>> {
        let Some(references) = self.bindings_by_reference().get(path) else {
            return vec![None; spans.len()];
        };
        let file = self.file_by_path(path);
        spans
            .iter()
            .map(|span| {
                self.resolution_in(references, file, *span)
                    .map(|resolution| resolution.symbol.clone())
            })
            .collect()
    }

    fn resolution_in<'t>(
        &self,
        references: &'t HashMap<(u64, u64), BindingResolution>,
        file: Option<&FileFacts>,
        span: Span,
    ) -> Option<&'t BindingResolution> {
        let key = |span: Span| (u64::from(span.start), u64::from(span.end));
        // Transparent TypeScript wrappers, however nested, preserve the value
        // of the identifier they wrap. A missed wrapped span resolves as that
        // identifier, and only when it peels to exactly one identifier.
        let resolution = references.get(&key(span)).or_else(|| {
            let file = file?;
            let peeled = file.ast.peel_ts_sugar_span(span);
            let identifier = file
                .ast
                .identifiers
                .binary_search_by_key(&peeled, |id| id.span)
                .is_ok()
                || file.ast.reference_declaration(peeled).is_some();
            (peeled != span && identifier)
                .then(|| references.get(&key(peeled)))
                .flatten()
        })?;
        Some(resolution)
    }

    pub(super) fn bindings_by_reference(&self) -> &BindingsByReference {
        self.bindings_by_reference.get_or_init(|| {
            let symbols = self.symbols_by_id();
            // Each file's binding names, and the spans each resolves, read on
            // several workers in the order the table is written: a key's
            // first writer in that order is its resolution.
            let slots = self.facts.files.iter().enumerate().collect::<Vec<_>>();
            let names = crate::pipeline::parallel_slice_results(&slots, |&(_, file)| {
                let path = file.path.as_str();
                let mut names = Vec::new();
                let mut spans = Vec::new();
                for (binding_index, binding) in file.ast.bindings.iter().enumerate() {
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(path, name.span) else {
                            continue;
                        };
                        let name_index = names.len();
                        names.push((binding_index, symbol));
                        spans.push((
                            name_index,
                            path,
                            (u64::from(name.span.start), u64::from(name.span.end)),
                        ));
                        if let Some(candidate) = symbols.get(symbol.as_str()) {
                            spans.extend(candidate.references().map(|reference| {
                                (
                                    name_index,
                                    reference.path.as_ref(),
                                    (reference.start_byte, reference.end_byte),
                                )
                            }));
                        }
                    }
                }
                (names, spans)
            });
            let mut by_symbol = HashMap::<&SymbolId, BindingResolution>::default();
            for (file_index, (names, _)) in names.iter().enumerate() {
                for (binding_index, symbol) in names {
                    by_symbol
                        .entry(symbol)
                        .or_insert_with(|| BindingResolution {
                            file: file_index,
                            binding: *binding_index,
                            symbol: (*symbol).clone(),
                        });
                }
            }
            // The paths are split among shards, each written on a worker in
            // the same order.
            const SHARDS: usize = 64;
            let shard_of = |path: &str| {
                use std::hash::BuildHasher;
                (rustc_hash::FxBuildHasher.hash_one(path) >> 58) as usize % SHARDS
            };
            let mut shards = vec![Vec::new(); SHARDS];
            for (file_index, (_, spans)) in names.iter().enumerate() {
                let mut last: Option<(&str, usize)> = None;
                for &(name_index, path, span) in spans {
                    let shard = match last {
                        Some((previous, shard)) if std::ptr::eq(previous, path) => shard,
                        _ => {
                            let shard = shard_of(path);
                            last = Some((path, shard));
                            shard
                        }
                    };
                    shards[shard].push((file_index, name_index, path, span));
                }
            }
            let shards = crate::pipeline::parallel_balanced_results(&shards, |spans| {
                let mut by_path =
                    HashMap::<&str, HashMap<(u64, u64), BindingResolution>>::default();
                for &(file_index, name_index, path, span) in spans {
                    by_path
                        .entry(path)
                        .or_default()
                        .entry(span)
                        .or_insert_with(|| {
                            let (binding, symbol) = names[file_index].0[name_index];
                            BindingResolution {
                                file: file_index,
                                binding,
                                symbol: symbol.clone(),
                            }
                        });
                }
                by_path
            });
            let mut by_path = BindingsByReference::default();
            for shard in shards {
                by_path.extend(
                    shard
                        .into_iter()
                        .map(|(path, references)| (path.to_owned(), references)),
                );
            }
            // The symbol-reference table proves aliases even when no entity
            // was demanded at a use. Conversely, exact entity facts can exist
            // at a use omitted from that reference projection. Retain both
            // compiler proofs, matching the former direct-or-reference query.
            //
            // An entity demanded at a composite expression carries the symbol
            // of the token it begins with (`ctx.a() === "x"` names `ctx`), not
            // the expression's value. Those keys are retained because several
            // proofs withhold findings through them; a consumer needing value
            // identity must first require an exact identifier span (accessor
            // provenance's `exact_identifier`).
            //
            // Each key of a path is written at most once here, so the paths
            // are independent: each is completed on a worker.
            let paths = self
                .entities
                .by_path
                .iter()
                .map(|(path, entities)| {
                    (
                        path.as_str(),
                        std::sync::Mutex::new(by_path.remove(path.as_str()).unwrap_or_default()),
                        entities,
                    )
                })
                .collect::<Vec<_>>();
            let by_symbol = &by_symbol;
            crate::pipeline::parallel_slice_results(&paths, |(_, references, entities)| {
                let mut references = references.lock().expect("one worker per path");
                for ((start, end), symbol) in entities.iter() {
                    if let Some(resolution) = by_symbol.get(symbol) {
                        references
                            .entry((*start, *end))
                            .or_insert_with(|| resolution.clone());
                    }
                }
            });
            by_path.extend(paths.into_iter().filter_map(|(path, references, _)| {
                let references = references.into_inner().expect("one worker per path");
                (!references.is_empty()).then(|| (path.to_owned(), references))
            }));
            by_path
        })
    }

    /// Whether the last reference in this JSX member-object span resolves to
    /// a binding initialized by the dialect's `createContext` primitive
    /// anywhere in the project.
    ///
    /// Context providers are routinely declared in one module and rendered
    /// in another. Indexing canonical TypeScript symbols keeps that ordinary
    /// import/re-export boundary from erasing the runtime identity while
    /// still rejecting objects that merely expose a property named
    /// `Provider`. Matching the reference that ends with the object span also
    /// supports `<contexts.ValueContext.Provider>` without confusing it with
    /// `<ValueContext.someObject.Provider>`.
    pub(crate) fn is_context_reference(&self, path: &str, span: Span) -> bool {
        // ADR 0289: under runtime redirects (ADR 0186, ADR 0284) the compiler
        // binds an import to the declaration file beside the runtime one, so
        // it lists the reference under the declaration's symbol. The entity
        // at the span carries the redirected runtime binding, which decides.
        if !self.facts.runtime_symbol_redirects.is_empty()
            && self
                .entities
                .at(path, span)
                .is_some_and(|symbol| self.context_symbols().contains(symbol.as_str()))
        {
            return true;
        }
        self.context_symbols().iter().any(|symbol| {
            self.symbols_by_id
                .get_or_init(|| {
                    self.facts
                        .typescript
                        .symbols()
                        .map(|candidate| (candidate.id(), candidate))
                        .collect()
                })
                .get(symbol)
                .is_some_and(|candidate| {
                    candidate.references().any(|reference| {
                        reference.path.as_ref() == path
                            && reference.start_byte >= u64::from(span.start)
                            && reference.end_byte == u64::from(span.end)
                    })
                })
        })
    }

    fn context_symbols(&self) -> &HashSet<&'a str> {
        self.context_symbols.get_or_init(|| {
            self.facts
                .files
                .iter()
                .flat_map(|file| {
                    let primitives = self.primitives(file);
                    file.ast.bindings.iter().filter_map(move |binding| {
                        let initializer = binding.call_initializer?;
                        let call = file
                            .ast
                            .calls
                            .iter()
                            .position(|call| call.span == initializer)?;
                        (crate::known_primitive(&primitives.calls[call])
                            == Some(solid_dialect::Primitive::CreateContext))
                        .then(|| {
                            binding.names.iter().find_map(|name| {
                                self.entities
                                    .at(file.path.as_str(), name.span)
                                    .map(|symbol| symbol.as_str())
                            })
                        })
                        .flatten()
                    })
                })
                .collect()
        })
    }

    pub(crate) fn call_by_callee(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&solid_facts::ast::CallFact> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.call_by_callee(callee))
    }

    /// The compiler-selected signature and argument mapping for this call.
    ///
    /// Resolved-call facts are demanded at the complete callee expression,
    /// unlike member declaration identity which lives on its property span.
    pub(crate) fn resolved_callee_call(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a ResolvedCall> {
        resolved_callee_call_in(
            file,
            self.ast_indexes.get(file.path.as_str()),
            callee,
            |span| self.entity_at(file.path.as_str(), span),
        )
    }

    pub(crate) fn ast_file_index(&self, path: &str) -> Option<&'a CachedAstFileIndex> {
        self.ast_indexes.get(path)
    }

    pub(crate) fn function_called_at(
        &self,
        path: &str,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        // The called value is the identifier the callee names, read through its
        // TypeScript wrappers. A callee that is a call or a member names its own
        // value through other facts, so the leading token's function is not this
        // callee's (ADR 0346).
        let file = self.file_by_path(path)?;
        let symbol = self.entities.value_symbol(file, callee)?;
        self.function_for_symbol(symbol)
    }

    pub(super) fn method_symbol_ref(&self, path: &str, span: Span) -> Option<&'a str> {
        self.declaration_symbols
            .get_or_init(|| declaration_symbols(&self.facts.typescript))
            .get(path)?
            .get(&(u64::from(span.start), u64::from(span.end)))
            .copied()
    }

    /// A static member of an actual namespace import names its exported
    /// value, not a structurally compatible method. Resolve the property
    /// symbol itself; a selected call signature need not name that value.
    pub(crate) fn namespace_member_function(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        super::callee_world::namespace_member_function_in(self, file, callee)
    }

    /// The exported value a static member of a namespace import names: what
    /// [`Self::namespace_member_function`] reads of `file` alone. Must agree
    /// with `callee_world::namespace_member_function_in`, which that method
    /// delegates to (S7a's kept discovery rows read the two halves apart).
    pub(crate) fn namespace_member_symbol(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a SymbolId> {
        let property = super::callee_world::namespace_member_property(file, callee)?;
        self.entities.at(file.path.as_str(), property)
    }

    /// [`Self::namespace_member_function`]'s answer from the member
    /// symbol's function: what it reads of the function's file.
    pub(crate) fn namespace_member_target(
        &self,
        found: Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)>,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        found.filter(|(target_file, function)| {
            super::callee_world::namespace_member_value_is_current(target_file, function)
        })
    }

    pub(crate) fn function_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        // A hand-out (`hand_outs`): the answer, and the file it names.
        self.note_function_for_symbol(symbol);
        match self.functions_by_symbol().get(symbol)? {
            SymbolFunction::Resolved { file, function } => {
                self.note_slot_read(*file);
                let file = &self.facts.files[*file];
                Some((file, &file.ast.functions[*function]))
            }
            SymbolFunction::Aborted => None,
        }
    }

    pub(crate) fn entity_at(&self, path: &str, span: Span) -> Option<&'a EntityFact> {
        self.entities.locations.at(path, span)
    }

    pub(crate) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        self.facts.typescript.file(path)
    }

    /// The symbol a callee span resolves to. Transparent wrappers are peeled
    /// because they preserve the called value; arbitrary contained symbols are
    /// never substituted, since `handlers[i]()` and `wrapper.value()` must not
    /// inherit the identity of `i` or `wrapper` when the complete callee has no
    /// semantic fact.
    pub(crate) fn callee_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'a str> {
        self.redirected_callee_symbol(file, callee).or_else(|| {
            let path = file.path.as_str();
            plain_callee_symbol(
                file,
                self.ast_indexes.get(path),
                callee,
                |span| self.resolved_declaration_symbol(file, span),
                |span| self.entities.at(path, span),
                |span| self.entities.value_symbol(file, span),
            )
        })
    }

    /// ADR 0322: the redirected export a member of a namespace import names,
    /// when a runtime redirect binds it to selected source: the entity at the
    /// property carries it, the resolved declaration names the declaration
    /// file the compiler bound instead. `None` without redirects, and for every
    /// other callee ([`Self::callee_symbol`] then answers
    /// [`plain_callee_symbol`]).
    pub(crate) fn redirected_callee_symbol(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a str> {
        if self.facts.runtime_symbol_redirects.is_empty() {
            return None;
        }
        let callee = file.ast.peel_ts_sugar_span(callee);
        let property = self
            .ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(callee))?;
        if file.ast.computed_members.contains(&callee)
            || self.namespace_member_function(file, callee).is_none()
        {
            return None;
        }
        let symbol = self.entities.at(file.path.as_str(), property)?;
        self.facts
            .runtime_symbol_redirects
            .values()
            .any(|target| target.as_str() == symbol.as_str())
            .then_some(symbol.as_str())
    }

    /// Exact callable identities for one call site.
    ///
    /// A normal TypeScript call fact supplies one selected declaration. A
    /// composite/union call deliberately supplies none; in that case this
    /// method only expands aliases and object values whose exact property
    /// declarations are present in the semantic table. It never turns a
    /// property spelling into a project-wide method lookup. Callers must
    /// compare the returned candidates' summaries before using more than one.
    pub(crate) fn callee_symbols(&self, file: &FileFacts, callee: Span) -> Vec<SymbolId> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        let member_property = self
            .ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(callee));
        // For an identifier call the entity at the complete callee is the
        // value binding (including a function parameter). A resolved
        // FunctionType declaration is not the runtime function identity and
        // must not replace it.
        if member_property.is_none()
            && let Some(symbol) = self.entities.value_symbol(file, callee)
        {
            // Whether a binding naming the symbol can replace its identity:
            // those bindings are its declarations' (`hand_outs`).
            self.note_symbol_declarations(symbol.as_str());
            if !self.direct_value_aliases().contains(symbol) {
                return vec![symbol.clone()];
            }
            let mut visited = HashSet::default();
            let aliases = self.direct_value_symbols(file, callee, &mut visited);
            if !aliases.is_empty() {
                return aliases;
            }
            return vec![symbol.clone()];
        }
        if let Some(call) = self.resolved_callee_call(file, callee)
            && call.validity == ResolvedCallValidity::Valid
            && let Some(declaration) = call.declaration.as_ref()
            && !declaration.symbol.is_empty()
        {
            let symbol = SymbolId::from(declaration.symbol.as_ref());
            // A selected declaration is sufficient when it names an
            // analyzed implementation. A TypeScript interface/signature
            // declaration without one is only a structural type, so continue
            // to exact receiver/call-site dispatch below.
            if self.function_for_symbol(symbol.as_str()).is_some() {
                return vec![symbol];
            }
        }
        let Some(member_property) = member_property else {
            return self
                .entities
                .at(file.path.as_str(), callee)
                .cloned()
                .into_iter()
                .collect();
        };
        if file.ast.computed_members.binary_search(&callee).is_ok() {
            return Vec::new();
        }
        let Some(property_name) = file.source_text(member_property) else {
            return Vec::new();
        };
        let member = file.ast.member_at(callee);
        let Some(member) = member else {
            return Vec::new();
        };
        let mut symbols = Vec::new();
        let mut visited_spans = HashSet::default();
        self.member_value_symbols(
            file,
            member.object,
            property_name,
            &mut visited_spans,
            &mut symbols,
        );
        if symbols.is_empty() {
            self.structural_parameter_member_symbols(
                file,
                member.object,
                property_name,
                &mut symbols,
            );
        }
        symbols.sort_unstable();
        symbols.dedup();
        symbols
    }

    /// The root symbol of a non-computed member callee, and the access path
    /// reaching it from that root.
    ///
    /// `reader.read(value)` answers `(reader, ["read"])`, and
    /// `parsed.modifiers.includes(m)` answers `(parsed, ["modifiers",
    /// "includes"])` -- **not** `(parsed, ["includes"])`. Rooting a whole
    /// chain at its last segment names a property the receiver does not have,
    /// and a consumer matches a recorded path as a *prefix* of the observed
    /// access (`type_facts::parameter_value_source_matches`), so a wrong first
    /// segment is a demand no runtime can witness.
    ///
    /// A computed member or a callee with no member fact answers `None`, the
    /// same conservative gate [`Self::callee_symbols`] applies -- `handlers[i]()`
    /// must never be read as a property named `i`.
    ///
    /// **The chain's root must be a plain identifier.** `EntitySymbols::at`
    /// answers a symbol for any span the compiler emitted an entity at, and at
    /// a conditional, sequence, logical, or call expression that symbol is some
    /// *operand's* -- not the value the chain walks through. Trusting it
    /// attaches properties of the chain's result to a binding that never has
    /// them: `(k ? options.a : options.b).c.slice(n)` would be reported as `k`
    /// reading `["c", "slice"]`. Those are refused, and so is
    /// `options().slice(n)`, whose `slice` is a property of the call's result.
    ///
    /// Segments *inside* the chain fail closed to the longest exact prefix
    /// rather than being guessed or skipped: `props[key].values()` cannot name
    /// anything and answers an empty path, and `props.of[key].values()` answers
    /// `["of"]`. A chain longer than [`MEMBER_CALLEE_PATH_LIMIT`] is cut the
    /// same way. With the root pinned to an identifier, a shorter path is
    /// always a true prefix of the real access, and so a strictly weaker claim
    /// under both the prefix matcher and the exact one
    /// (`type_facts::parameter_value_source_matches` and its `_exact` sibling
    /// compare a witness against *this* stated path, never against the access
    /// it was cut from). An empty path claims only "read through this
    /// parameter".
    pub(crate) fn member_callee_receiver(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(SymbolId, Vec<String>)> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        if file.ast.computed_members.binary_search(&callee).is_ok() {
            return None;
        }
        let ast_index = self.ast_indexes.get(file.path.as_str())?;
        // The callee must itself be a named member. `notify(callback)` is a
        // plain identifier: it is a call *of* a value, not a call *through* a
        // property of one, and answering it with a zero-segment path would
        // make every ordinary call read as a member access on its callee.
        let leaf = member_at(file, callee)?;
        let leaf_property = ast_index
            .member_property(callee)
            .and_then(|property| file.source_text(property))?
            .to_owned();
        // Walk leaf -> root, then reverse: the model orders a path outwards
        // from the parameter. An unnameable segment discards everything
        // collected so far -- those sit *outside* it, and only the segments
        // still to be walked form an exact prefix of the real access.
        let mut segments = vec![leaf_property];
        let mut current = file.ast.peel_ts_sugar_span(leaf.object);
        let root = loop {
            let Some(member) = member_at(file, current) else {
                break current;
            };
            if file.ast.computed_members.binary_search(&current).is_ok() {
                segments.clear();
            } else if let Some(name) = ast_index
                .member_property(current)
                .and_then(|property| file.source_text(property))
            {
                segments.push(name.to_owned());
            } else {
                segments.clear();
            }
            let next = file.ast.peel_ts_sugar_span(member.object);
            // A member's object is a strict sub-span of the member itself, so
            // every step shrinks the span and the walk terminates. The depth
            // it terminates at bounds the *path*, not the walk: cutting the
            // walk short would lose the root and force a drop, and a dropped
            // row is the negative claim this must never make. A fact table
            // that does not shrink is malformed, and refused rather than
            // walked forever.
            if next.start < current.start || next.end >= current.end {
                return None;
            }
            current = next;
        };
        // Refuse a root that is not a plain identifier: see the doc comment.
        // Everything collected above sits on the chain's *result*, so a
        // symbol answered at a compound span would be given a path of
        // properties it does not have.
        if file
            .ast
            .identifiers
            .binary_search_by_key(&root, |identifier| identifier.span)
            .is_err()
        {
            return None;
        }
        segments.reverse();
        segments.truncate(MEMBER_CALLEE_PATH_LIMIT);
        let receiver = self.entities.at(file.path.as_str(), root)?;
        Some((receiver.clone(), segments))
    }

    /// Every exact implementation `value.property` may resolve to.
    ///
    /// This retains a finite runtime union rather than silently selecting one
    /// candidate. Consumers may certify it only after proving every candidate
    /// has equivalent behavior; an empty result is missing evidence, not
    /// safety.
    ///
    /// Records in `consulted` every function whose source the answer read
    /// (factories, callees an object is handed to), so a cached consumer can
    /// depend on them.
    pub(crate) fn member_value_symbols_at_tracked(
        &self,
        file: &FileFacts,
        value: Span,
        property_name: &str,
        consulted: &mut Vec<(String, Span)>,
    ) -> Vec<SymbolId> {
        // ADR 0321: an object literal written as the argument itself is held
        // only by the callee's parameter, which the caller proved is only
        // member-read: no other code can reach it to replace a member.
        let fresh_argument = file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == file.ast.peel_ts_sugar_span(value));
        if !fresh_argument && self.own_member_may_be_replaced(property_name) {
            return Vec::new();
        }
        let mut visited = HashSet::default();
        let mut symbols = self
            .member_value_candidates(file, value, property_name, &mut visited, true, consulted)
            .unwrap_or_default();
        symbols.sort_unstable();
        symbols.dedup();
        symbols
    }

    /// Follow a nested member path through fresh, closed data literals. The
    /// argument and every held intermediate have a single consumption channel;
    /// the receiving parameter may only call this exact path. In particular,
    /// neither a const declaration nor a structural signature proves stability.
    pub(crate) fn nested_member_value_symbols_at(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        parameter: usize,
        path: &[String],
    ) -> Vec<SymbolId> {
        let resolve = || {
            if path.len() < 2
                || path.len() > 8
                || call.construct
                || call.arguments.iter().any(|argument| argument.spread)
                || self.program_has_eval()
            {
                return None;
            }
            let (callee_file, callee, exact) =
                crate::execution_role::project_callee_at_parameter(file, call, parameter, self)?;
            let input = callee.parameters.get(parameter)?;
            if !exact
                || input.shape != solid_facts::ast::BindingShape::Identifier
                || input.names.len() != 1
                || callee.rest_parameter
                || binding_written(callee_file, input.names[0].span)
                || callee_file.ast.identifiers.iter().any(|id| {
                    callee.span.contains(id.span)
                        && callee_file.source_text(id.span) == Some("arguments")
                })
            {
                return None;
            }
            let symbol = self
                .entities
                .at(callee_file.path.as_str(), input.names[0].span)?;
            let mut seen = false;
            for (reference, declaration) in &callee_file.ast.reference_declarations {
                if *declaration != input.names[0].span {
                    continue;
                }
                seen = true;
                // Extracting, returning, assigning, or forwarding any prefix
                // exposes the receiver. Only an immediate call of the full
                // selected path keeps the fresh object's channel closed.
                if !callee_file.ast.calls.iter().any(|inner| {
                    !inner.construct
                        // Read summaries do not instantiate the selected
                        // method's callback parameters at this indirect edge.
                        && inner.arguments.is_empty()
                        && inner.callee.contains(*reference)
                        // member_callee_receiver intentionally returns an
                        // exact prefix at computed segments. A prefix cannot
                        // stand for this whole runtime dispatch.
                        && !callee_file.ast.computed_members.iter()
                            .any(|member| inner.callee.contains(*member))
                        && self
                            .member_callee_receiver(callee_file, inner.callee)
                            .is_some_and(|(root, selected)| &root == symbol && selected == path)
                }) {
                    return None;
                }
            }
            if !seen {
                return None;
            }
            let argument = call.arguments.get(parameter)?;
            // Resolving a nonempty path proves the supplied argument is a
            // fresh object (possibly through closed aliases), never undefined.
            // An identifier parameter's default therefore cannot replace it.
            let (owner, value) = self.nested_member_value(file, argument.span, path, 16)?;
            let function = owner.ast.function_at(value);
            let symbol = if let Some(function) = function {
                let name = function_binding_name(owner, function)?;
                self.entities.at(owner.path.as_str(), name.span)?
            } else {
                if !owner.ast.identifiers.iter().any(|id| id.span == value) {
                    return None;
                }
                self.entities.at(owner.path.as_str(), value).or_else(|| {
                    let declaration = owner.ast.reference_declaration(value)?;
                    self.entities.at(owner.path.as_str(), declaration)
                })?
            };
            let (function_file, function) = self.function_for_symbol(symbol)?;
            (self.function_value_is_current(function_file, function)
                && crate::object_members::receiver_inert(function_file, function))
            .then(|| vec![symbol.clone()])
        };
        resolve().unwrap_or_default()
    }
}

/// A call's resolved declaration when it is a valid, single-target call of
/// `kind` to a standard-library declaration.
pub(crate) fn standard_library_declaration_of(
    resolved: Option<&ResolvedCall>,
    kind: typefacts::CallKind,
) -> Option<&typefacts::ResolvedDeclaration> {
    resolved
        .filter(|resolved| {
            resolved.validity == ResolvedCallValidity::Valid
                && resolved.kind == kind
                && resolved.targets.is_none()
        })
        .and_then(|resolved| resolved.declaration.as_ref())
        .filter(|declaration| declaration.standard_library)
}

/// [`SemanticLookup::resolved_callee_call`] over one file's own facts: its
/// AST index and its entities (`entity_at` answers for this file only).
pub(crate) fn resolved_callee_call_in<'e>(
    file: &FileFacts,
    ast_index: Option<&CachedAstFileIndex>,
    callee: Span,
    entity_at: impl Fn(Span) -> Option<&'e EntityFact>,
) -> Option<&'e ResolvedCall> {
    let call_by_callee = |span| ast_index.and_then(|index| index.call_by_callee(span));
    let original_callee = callee;
    let callee = file.ast.peel_ts_sugar_span(callee);
    let outer = call_by_callee(original_callee).or_else(|| call_by_callee(callee));
    let call_span = outer.map_or(original_callee, |call| call.span);
    // Review of ADR 0368: a callee that is itself a call (`get()(x)`)
    // has, at its own span, the inner call's fact. Only the outer call's
    // exact fact (demanded at its whole span) answers for the outer call;
    // without it there is no fact.
    if outer.is_some() && file.ast.call_at(callee).is_some() {
        return entity_at(call_span).and_then(|entity| entity.resolved_call.as_deref());
    }
    // A wrapped callee has its own exact call demand. Prefer that fact:
    // the peeled expression may also be another invocation's location
    // (a factory result, for example), or may have only a symbol demand.
    entity_at(original_callee)
        .and_then(|entity| entity.resolved_call.as_deref())
        .or_else(|| entity_at(callee).and_then(|entity| entity.resolved_call.as_deref()))
        .or_else(|| entity_at(call_span).and_then(|entity| entity.resolved_call.as_deref()))
}

/// The symbol a callee span resolves to, runtime redirects aside
/// ([`SemanticLookup::redirected_callee_symbol`]). Transparent wrappers are
/// peeled because they preserve the called value; arbitrary contained symbols
/// are never substituted, since `handlers[i]()` and `wrapper.value()` must not
/// inherit the identity of `i` or `wrapper` when the complete callee has no
/// semantic fact. A member names its resolved declaration, or else (not
/// computed) its property's symbol; an identifier its own symbol. A call or
/// another expression names no value through its leading token (ADR 0346):
/// the token at `make(…)(…)`'s callee is `make`, not what it returns.
///
/// Every input is the callee's own file (its AST, its entity symbols and its
/// entities' resolved calls), so a per-file contribution can answer it
/// ([`crate::base::contributions::callees`]).
pub(crate) fn plain_callee_symbol<'s>(
    file: &FileFacts,
    ast_index: Option<&CachedAstFileIndex>,
    callee: Span,
    resolved_declaration: impl Fn(Span) -> Option<&'s str>,
    symbol_at: impl Fn(Span) -> Option<&'s SymbolId>,
    value_symbol: impl Fn(Span) -> Option<&'s SymbolId>,
) -> Option<&'s str> {
    let callee = file.ast.peel_ts_sugar_span(callee);
    let member_property = ast_index.and_then(|index| index.member_property(callee));
    let computed_member = file.ast.computed_members.contains(&callee);
    match member_property {
        Some(property) => resolved_declaration(callee).or_else(|| {
            if computed_member {
                None
            } else {
                symbol_at(property).map(SymbolId::as_str)
            }
        }),
        None => value_symbol(callee).map(SymbolId::as_str),
    }
}

/// [`SemanticLookup::function_value_is_current`]: it reads the function's
/// own file only.
pub(crate) fn function_value_is_current_in(
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    // A class element's or an object literal method's own function. A
    // function nested inside a method inherits its `method_name`, so that
    // field alone says nothing.
    let element_function = file.ast.classes.iter().any(|class| {
        class
            .elements
            .iter()
            .any(|element| element.value == Some(function.span))
    }) || file.ast.object_properties.iter().any(|property| {
        !property.data && file.ast.peel_ts_sugar_span(property.value) == function.span
    });
    if element_function {
        return true;
    }
    if function.kind == solid_facts::ast::FunctionKind::Declaration {
        return function
            .name
            .as_ref()
            .is_some_and(|name| !binding_written(file, name.span));
    }
    file.ast.bindings.iter().any(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.initializer.is_some_and(|initializer| {
                file.ast.peel_ts_sugar_span(initializer) == function.span
            })
    })
}
