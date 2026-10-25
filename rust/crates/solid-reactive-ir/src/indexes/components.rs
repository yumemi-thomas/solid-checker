//! Component and JSX call-site indexes: component status, rendered functions, prop render sites, and entity lookups.

use super::*;

/// A function's status when the census proves it no component: a dialect
/// naming convention only preserves the ambiguity, never proves one.
pub(crate) fn unproven_component_status(
    dialect: &dyn solid_dialect::Dialect,
    file: &FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> ComponentStatus {
    let possible = crate::owners::component_binding_name(file, function).is_some_and(|name| {
        dialect.component_name_may_be_component(file.source_text(name.span).unwrap_or_default())
    });
    if possible {
        ComponentStatus::Uncertain
    } else {
        ComponentStatus::No
    }
}

impl<'a> SemanticLookup<'a> {
    /// The type descriptor at `span`, falling back to the smallest typed entity
    /// contained in it for legacy consumers that deliberately query a region
    /// rather than one complete expression. Exact facts win: a descriptor on
    /// the demanded expression can never be replaced by an inner callee or
    /// member just because that entity happens to be smaller.
    pub(crate) fn smallest_contained_descriptor(
        &self,
        path: &str,
        span: Span,
    ) -> Option<&'a TypeDescriptor> {
        self.entity_at(path, span)
            .and_then(|entity| entity.type_descriptor.as_deref())
            .or_else(|| {
                self.smallest_contained(path, span, |entity| entity.type_descriptor.is_some())
                    .and_then(|entity| entity.type_descriptor.as_deref())
            })
            .or_else(|| {
                let symbol = self.entities.at(path, span)?;
                self.descriptors_by_symbol
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .entities()
                            .filter_map(|entity| {
                                (!entity.symbol.is_empty())
                                    .then_some(entity.symbol.as_ref())
                                    .zip(entity.type_descriptor.as_deref())
                            })
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
    }

    /// Whether a function is proven to be a Solid component by a JSX call
    /// site or by an exact compiler-resolved Solid component type alias.
    pub(crate) fn function_is_component(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.component_statuses_of(file.path.as_str())
            .0
            .contains(&(file.path.as_str(), function.span))
    }

    /// Three-outcome component identity. Exact JSX uses and Solid component
    /// types are proven; a dialect naming convention only preserves the
    /// ambiguity and must never become proof by itself.
    pub(crate) fn function_component_status(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> ComponentStatus {
        if self.function_is_component(file, function) {
            return ComponentStatus::Proven;
        }
        if self
            .component_statuses_of(file.path.as_str())
            .2
            .contains(&(file.path.as_str(), function.span))
        {
            return ComponentStatus::Uncertain;
        }
        unproven_component_status(self.dialect, file, function)
    }

    pub(crate) fn function_may_be_component(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.function_component_status(file, function) != ComponentStatus::No
    }

    pub(super) fn component_functions(&self) -> &HashSet<(&'a str, Span)> {
        &self.component_statuses().0
    }

    /// Functions that would be proven components but for a reference that
    /// enters them some other way. They are analyzed as components, so no
    /// finding is lost, and every violation in their own body is withdrawn to
    /// an obligation ([`Self::undecided_component_regions`]).
    pub(crate) fn escaped_component_functions(&self) -> &HashSet<(&'a str, Span)> {
        &self.component_statuses().1
    }

    /// Functions a resolved call enters as plain calls, whose other entries
    /// the census cannot follow, so one of them may render the function
    /// (ADR 0331). Neither execution model is selected for them: they answer
    /// [`ComponentStatus::Uncertain`] from
    /// [`Self::function_component_status`] without being components, so the
    /// call model keeps its findings and each consumer keeps the ambiguity.
    pub(super) fn possible_component_functions(&self) -> &HashSet<(&'a str, Span)> {
        &self.component_statuses().2
    }

    /// Every function's identity ([`super::FileComponentStatuses`]), gathered
    /// into the three sets by `(path, span)`: a path listed twice is one
    /// function per span.
    fn component_statuses(&self) -> &ComponentSets<'a> {
        crate::base::reads::note(crate::base::reads::Section::Components);
        self.component_sets()
    }

    /// The three sets, read only at keys whose path is `path`: the reader
    /// records that path's part of the census
    /// ([`super::census::ComponentSpansOf`]), or the whole census when the
    /// build holds no such path.
    pub(super) fn component_statuses_of(&self, path: &str) -> &ComponentSets<'a> {
        if !self.note_component_spans(path) {
            crate::base::reads::note(crate::base::reads::Section::Components);
        }
        self.component_sets()
    }

    fn component_sets(&self) -> &ComponentSets<'a> {
        self.component_functions
            .get_or_init(|| crate::base::reads::suppress(|| self.component_statuses_uncached()))
    }

    fn component_statuses_uncached(&self) -> ComponentSets<'a> {
        {
            let world = self.memo_world();
            let table = self.memos.table();
            let files = self.files();
            let slots = (0..files.len()).collect::<Vec<_>>();
            let answers = crate::pipeline::parallel_slice_results(&slots, |slot| {
                let id = table.slot_id(*slot);
                let rank = u32::try_from(table.slot_rank(*slot)).expect("fewer than 2^32 slots");
                world.answer::<super::FileComponentStatuses>(&(id, rank))
            });
            let mut proven = HashSet::default();
            let mut escaped = HashSet::default();
            let mut possible = HashSet::default();
            for (slot, identities) in answers.into_iter().enumerate() {
                let file = &files[slot];
                for (function, identity) in file.ast.functions.iter().zip(identities.iter()) {
                    let key = (file.path.as_str(), function.span);
                    match identity {
                        super::ComponentIdentity::Proven => {
                            proven.insert(key);
                        }
                        // Analyzed as a component, as before; findings in
                        // its own body are obligations (`UndecidedRegion`).
                        super::ComponentIdentity::Escaped => {
                            proven.insert(key);
                            escaped.insert(key);
                        }
                        super::ComponentIdentity::Possible => {
                            possible.insert(key);
                        }
                        super::ComponentIdentity::No => {}
                    }
                }
            }
            (proven, escaped, possible)
        }
    }

    /// Whether the binding's compiler-resolved type is an exact Solid
    /// component type alias.
    pub(super) fn binding_has_component_type(
        &self,
        file: &FileFacts,
        binding_name: Option<&solid_facts::ast::NamedSpan>,
    ) -> bool {
        let Some(name) = binding_name else {
            return false;
        };
        let Some(descriptor) = self.smallest_contained_descriptor(file.path.as_str(), name.span)
        else {
            return false;
        };
        descriptor.alias_declarations.iter().any(|declaration| {
            self.dialect
                .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                == Some(solid_dialect::TypeRole::Component)
        })
    }

    pub(super) fn project_has_component_type(&self) -> bool {
        *self.project_has_component_type.get_or_init(|| {
            self.facts.typescript.entities().any(|entity| {
                entity.type_descriptor.as_deref().is_some_and(|descriptor| {
                    descriptor.alias_declarations.iter().any(|declaration| {
                        self.dialect
                            .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                            == Some(solid_dialect::TypeRole::Component)
                    })
                })
            })
        })
    }

    /// Whether `span` executes inside a function proven to be a component.
    pub(crate) fn inside_component(&self, file: &FileFacts, span: Span) -> bool {
        file.ast
            .functions_body_containing(span)
            .any(|function| self.function_is_component(file, function))
    }

    /// Whether `span` is inside a function whose component identity is
    /// possible but not proven. Consumers use this to preserve an
    /// uncertifiable outcome instead of selecting either execution model.
    pub(crate) fn inside_possible_component(&self, file: &FileFacts, span: Span) -> bool {
        file.ast.functions_body_containing(span).any(|function| {
            self.function_component_status(file, function) == ComponentStatus::Uncertain
        })
    }

    /// The bodies of [`Self::escaped_component_functions`], each without the
    /// callbacks a dialect primitive places itself inside it
    /// (`<For>{(item) => …}</For>`, an effect's callbacks) and without nested
    /// functions a tag renders (`<Inner/>`): those run in a context of their
    /// own however the function is entered. An inline callback
    /// (`untrack(() => …)`) runs in the body's own context and stays in the
    /// region.
    pub(crate) fn undecided_component_regions(&self) -> Vec<crate::UndecidedRegion> {
        let mut regions = self
            .escaped_component_functions()
            .iter()
            .filter_map(|(path, span)| {
                let file = self.file_by_path(path)?;
                let function = file.ast.function_at(*span)?;
                Some(crate::UndecidedRegion {
                    body: crate::location(*path, function.body),
                    placed_callbacks: file
                        .ast
                        .functions_within(function.body)
                        .filter(|inner| {
                            (crate::execution_role::function_placed_by_dialect(
                                file, inner.span, self,
                            ) && !crate::execution_role::function_runs_inline(
                                file, inner.span, self,
                            ))
                                // A nested component a tag renders is a
                                // component however this body is entered.
                                || (self.jsx_call_site_loading(path, inner.span).any
                                    && !self
                                        .component_statuses_of(path)
                                        .1
                                        .contains(&(*path, inner.span)))
                        })
                        .map(|inner| crate::location(*path, inner.span))
                        .collect(),
                })
            })
            .collect::<Vec<_>>();
        regions.sort_by(|left, right| crate::location_order(&left.body, &right.body));
        regions
    }

    /// Compiler-derived callability at `span`, falling back to the smallest
    /// contained demanded entity only for callers that intentionally query a
    /// region, then to another demanded occurrence of the same symbol.
    pub(crate) fn smallest_contained_callability(
        &self,
        path: &str,
        span: Span,
    ) -> Option<Callability> {
        self.entity_at(path, span)
            .and_then(|entity| entity.callability)
            .or_else(|| {
                self.smallest_contained(path, span, |entity| entity.callability.is_some())
                    .and_then(|entity| entity.callability)
            })
            .or_else(|| {
                let symbol = self.entities.at(path, span)?;
                self.callability_by_symbol
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .entities()
                            .filter_map(|entity| {
                                (!entity.symbol.is_empty())
                                    .then_some(entity.symbol.as_ref())
                                    .zip(entity.callability)
                            })
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
    }

    /// Whether any JSX call site renders the function at `(path, function)`,
    /// and whether one of those call sites sits under a Loading boundary.
    pub(crate) fn jsx_call_site_loading(&self, path: &str, function: Span) -> CallSiteLoading {
        CallSiteLoading {
            any: self.memos.table().id_of(path).is_some_and(|id| {
                self.memo_world()
                    .answer::<super::TagRendered>(&id)
                    .binary_search(&function)
                    .is_ok()
            }),
        }
    }

    /// The memo key of the function at `span` in `path`.
    fn census_target(&self, path: &str, span: Span) -> Option<(crate::base::FileId, Span)> {
        Some((self.memos.table().id_of(path)?, span))
    }

    /// The call graph's entries of the function at `(path, function)`, each
    /// with the reference(s) its site spells ([`super::CallSitesOf`]).
    fn call_site_entries(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, super::CallSite)> {
        let Some(target) = self.census_target(path, function) else {
            return Vec::new();
        };
        let table = self.memos.table();
        let files = self.files();
        self.memo_world()
            .answer::<super::CallSitesOf>(&target)
            .iter()
            .filter_map(|site| {
                let slot = *table.slots_of(site.file).get(site.rank as usize)?;
                // A hand-out (`hand_outs`): each caller's file.
                self.note_slot_read(slot);
                Some((&files[slot], *site))
            })
            .collect()
    }

    /// Call sites whose Type Facts resolve to one project function: call
    /// expressions, and the JSX tags that render a component. Aliases,
    /// imports, and same-named locals follow canonical symbols. One entry per
    /// invocation — a closing tag is part of the render it closes, not a
    /// second call.
    ///
    /// Rendering `<Panel/>` invokes `Panel`: the tag is a call site, not a
    /// value escape, and the same exact resolution that proves which function a
    /// tag renders ([`super::TagRendered`], through the same callee index, so
    /// component identity and the call graph cannot disagree about which tags
    /// resolve) names the callee here. The edge's callee is the tag *name* span
    /// only — the component's own reference — so a component used as a value
    /// rather than rendered (`<Wrap child={Panel}/>`, `return Panel`,
    /// `apply(Panel)`) is still an escape. A tag whose exact declaration is
    /// unresolved — an untyped import, an ambiguous computed name — resolves
    /// to nothing and emits no edge, so consumers keep failing closed on it
    /// rather than gaining a caller the runtime does not have. A tag naming an
    /// exact alias proves identity only, and the graph, lacking its edge,
    /// keeps the alias as an escape.
    ///
    /// A dotted tag is not that case. `<ns.Panel/>` resolves: TypeScript
    /// reports the symbol at the whole `ns.Panel` name span, so an edge is
    /// emitted whose callee *is* that whole span — a member expression, not an
    /// identifier. Consumers that expect an identifier there fail closed on
    /// their own terms: `structural_parameter_member_symbols` finds no
    /// `CallFact` and marks the site unresolved, and the escape test's
    /// byte-exact span set does not match the `Panel` property reference it
    /// walks, so a dotted render stays an escape.
    ///
    /// Call expressions come first, then render sites: a function that is both
    /// called and rendered is asked about its call sites in that order, so a
    /// consumer that takes the first answer it can use
    /// (`semantic_write_execution_role_within` takes the first non-`Unknown`
    /// execution role) resolves the tie by an argued rule rather than by which
    /// file happens to hold the call. A call expression is the more direct
    /// evidence of the two — its own syntax names the invocation — so it wins.
    pub(crate) fn function_call_sites(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.call_site_entries(path, function)
            .into_iter()
            .map(|(file, site)| (file, site.callee))
            .collect()
    }

    /// Every reference to a project function that one of its call sites
    /// accounts for.
    ///
    /// The same sites as [`Self::function_call_sites`], plus the extra
    /// occurrence of the callee's name a single site can spell:
    /// `<Panel></Panel>` renders `Panel` once and writes its name twice, and
    /// TypeScript reports both occurrences as references to the same symbol.
    /// The escape test asks whether *every* reference to a function is
    /// accounted for, so it asks here; every other consumer counts
    /// invocations and asks above.
    ///
    /// A closing tag can never appear on its own: the extra span is stored on
    /// the edge that the *opening* tag's resolution created, so if the opening
    /// tag resolved to nothing there is no site to carry it.
    pub(crate) fn function_call_site_references(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.call_site_entries(path, function)
            .into_iter()
            .flat_map(|(file, site)| {
                std::iter::once((file, site.callee))
                    .chain(site.also_referenced.map(|span| (file, span)))
            })
            .collect()
    }

    /// The render sites a value reference reaches, when its value reaches
    /// only a dialect component's rendering prop:
    /// `createComponent(Dynamic, { get component() { return Selected(); } })`
    /// with `const Selected = createMemo(() => cond() ? Panel : Other)`
    /// invokes the value `Panel` holds as a component, from computations that
    /// render creates and nowhere else (ADR 0138). The sites are in the
    /// reference's file; `None` for any other reference.
    ///
    /// Keyed by the reference, not by a function: the call graph asks it of
    /// each compiler-resolved reference of a function's symbols, the walk its
    /// escape test already makes, so *which* function the value is comes from
    /// those references and never from a name or a demanded entity. Kept
    /// beside [`Self::function_render_call_sites`] and apart from
    /// [`Self::function_call_sites`] for the same reason: only the attribution
    /// call graph asks who can enter a function, and neither component
    /// identity nor execution-role inheritance has this answer for a value.
    ///
    /// The syntax fact ([`solid_facts::ast::component_value_flows`]) says
    /// only where the value can go. Every span it hands back must resolve
    /// here, or the reference has no entry:
    ///
    /// - each site's component is declared in one of the dialect's
    ///   primitive-defining packages under a name the dialect says renders
    ///   that prop ([`solid_dialect::Dialect::component_prop_renderers`]),
    ///   and a call form's callee renders its first argument
    ///   ([`solid_dialect::Dialect::renders_component_argument`]);
    /// - each holder call is a primitive whose accessor yields only its
    ///   compute ([`solid_dialect::Dialect::accessor_yields_only_its_compute`]).
    pub(crate) fn prop_render_sites_at(&self, path: &str, reference: Span) -> Option<Vec<Span>> {
        let id = self.memos.table().id_of(path)?;
        self.memo_world()
            .answer::<super::PropRenderSitesIn>(&id)
            .get(&reference)
            .cloned()
    }

    /// ADR 0313: the prop whose string value makes the component named at
    /// `name` render an intrinsic element
    /// ([`solid_dialect::Dialect::intrinsic_tag_renderers`]). The tag's
    /// symbol must have declarations, every one of them in one of the
    /// dialect's primitive-defining packages under that export's name.
    pub(crate) fn intrinsic_tag_prop(&self, file: &FileFacts, name: Span) -> Option<&'static str> {
        let component = self.entities.at(file.path.as_str(), name)?;
        let declarations = self.symbols_by_id().get(component.as_str())?.declarations();
        let packages = self.dialect.primitive_defining_packages();
        let (_, prop) = self
            .dialect
            .intrinsic_tag_renderers()
            .iter()
            .find(|(export, _)| {
                !declarations.is_empty()
                    && declarations.iter().all(|declaration| {
                        declared_in_package(declaration.location.path.as_ref(), packages)
                            && declaration.name.as_ref() == *export
                    })
            })?;
        Some(prop)
    }

    pub(super) fn symbols_by_id(&self) -> &HashMap<&'a str, solid_facts::TypeScriptSymbol<'a>> {
        self.symbols_by_id.get_or_init(|| {
            self.facts
                .typescript
                .symbols()
                .map(|candidate| (candidate.id(), candidate))
                .collect()
        })
    }

    fn smallest_contained(
        &self,
        path: &str,
        span: Span,
        predicate: impl Fn(&EntityFact) -> bool,
    ) -> Option<&'a EntityFact> {
        self.entities
            .locations
            .smallest_contained(path, span, predicate)
    }

    pub(super) fn functions_by_symbol(&self) -> &HashMap<&'a str, SymbolFunction> {
        self.functions_by_symbol.get_or_init(|| {
            // Each file's named functions, then its function-valued bindings,
            // read on several workers; the first in file order names a symbol.
            let slots = self.facts.files.iter().enumerate().collect::<Vec<_>>();
            let named = crate::pipeline::parallel_slice_results(&slots, |&(file_index, file)| {
                self.file_symbol_functions(file_index, file)
            });
            let mut map = HashMap::default();
            for (symbol, function) in named.into_iter().flatten() {
                map.entry(symbol).or_insert(function);
            }
            map
        })
    }

    /// One file's candidates for [`Self::functions_by_symbol`], in the order
    /// the first of a symbol wins.
    fn file_symbol_functions(
        &self,
        file_index: usize,
        file: &'a FileFacts,
    ) -> Vec<(&'a str, SymbolFunction)> {
        let path = file.path.as_str();
        let mut named = Vec::new();
        for (function_index, function) in file.ast.functions.iter().enumerate() {
            let own_method = || crate::owners::own_method_name(file, function);
            let Some(name) = function.name.as_ref().or_else(own_method) else {
                continue;
            };
            let Some(symbol) = self
                .entities
                .at(path, name.span)
                .map(SymbolId::as_str)
                .or_else(|| {
                    own_method().and_then(|method| self.method_symbol_ref(path, method.span))
                })
            else {
                continue;
            };
            named.push((
                symbol,
                SymbolFunction::Resolved {
                    file: file_index,
                    function: function_index,
                },
            ));
        }
        for binding in &file.ast.bindings {
            if !binding.initializer_function && binding.call_initializer.is_none() {
                continue;
            }
            if binding.call_initializer.is_some()
                && (binding.shape != solid_facts::ast::BindingShape::Identifier
                    || binding.names.len() != 1)
            {
                // A call result may name a wrapped function only when
                // one identifier owns the whole result. Tuple/object
                // bindings name slots, not closures nested in the
                // call's arguments (`createSignal(() => value)`).
                continue;
            }
            let mut outcome = None;
            for name in &binding.names {
                let Some(symbol) = self.entities.at(path, name.span) else {
                    continue;
                };
                let outcome = *outcome.get_or_insert_with(|| match binding.initializer {
                    None => Some(SymbolFunction::Aborted),
                    Some(initializer) => file
                        .ast
                        .functions
                        .iter()
                        .enumerate()
                        .filter(|(_, function)| initializer.contains(function.span))
                        .max_by_key(|(_, function)| function.span.end - function.span.start)
                        .map(|(function_index, _)| SymbolFunction::Resolved {
                            file: file_index,
                            function: function_index,
                        }),
                });
                if let Some(outcome) = outcome {
                    named.push((symbol.as_str(), outcome));
                }
            }
        }
        named
    }

    /// ADR 0331: the functions a tag renders through an exact alias only:
    /// `const P2 = Plain; <P2/>` renders `Plain` as a component, as `<Plain/>`
    /// does ([`super::AliasRendered`]). Identity only: the call graph keeps no
    /// edge for it, so its consumers still see the alias as an unaccounted
    /// reference and fail closed.
    pub(super) fn alias_rendered_functions(
        &self,
        table: &crate::base::FileTable,
    ) -> std::collections::BTreeSet<(crate::base::FileId, Span)> {
        let mut rendered = std::collections::BTreeSet::new();
        for file in &self.facts.files {
            for element in &file.ast.jsx_elements {
                if self
                    .function_called_at(file.path.as_str(), element.name.span)
                    .is_some()
                {
                    continue;
                }
                if let Some((target_file, target)) =
                    self.alias_rendered_function(file, element.name.span)
                    && let Some(id) = table.id_of(target_file.path.as_str())
                {
                    rendered.insert((id, target.span));
                }
            }
        }
        rendered
    }

    /// The project function a tag name holds through exact aliases: the tag
    /// names a binding [`exact_alias_initializer`] accepts, whose initializer
    /// names a project function or, transitively, another such binding.
    fn alias_rendered_function(
        &self,
        file: &FileFacts,
        name: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        let mut symbol = self.entities.value_symbol(file, name)?.as_str();
        for _ in 0..ALIAS_DEPTH {
            let (binding_file, binding) = self.binding_for_symbol(symbol)?;
            let initializer = exact_alias_initializer(binding_file, binding)?;
            symbol = self
                .entities
                .value_symbol(binding_file, initializer)?
                .as_str();
            if let Some((target_file, target)) = self.function_for_symbol(symbol) {
                // The alias holds what its initializer's binding held when it
                // ran; that is this function only if nothing writes it.
                return function_holds_its_binding(target_file, target)
                    .then_some((target_file, target));
            }
        }
        None
    }
}

/// How far a chain of aliases (`const A = Plain; const B = A`) is followed.
pub(super) const ALIAS_DEPTH: usize = 8;

/// The identifier initializing `binding` when the binding is an exact alias
/// of it: one plain name, bound by `const` or by a `let` nothing writes, to
/// an identifier (parentheses and TypeScript wrappers aside), so the binding
/// holds that value whenever it is read after its declarator.
pub(super) fn exact_alias_initializer(
    file: &FileFacts,
    binding: &solid_facts::ast::BindingFact,
) -> Option<Span> {
    let identifier = binding.initializer_identifier.as_ref()?;
    let [name] = binding.names.as_slice() else {
        return None;
    };
    (binding.shape == solid_facts::ast::BindingShape::Identifier
        && (binding.immutable || binding.runtime_let && !binding_written(file, name.span)))
    .then_some(identifier.span)
}

/// A reference inside a TypeScript type query (`typeof Plain`) is erased.
pub(super) fn reference_in_type_query(file: &FileFacts, span: Span) -> bool {
    file.ast
        .type_queries
        .iter()
        .any(|query| query.contains(span))
}

/// Whether the module exports the binding declared at `name` itself: an
/// exported declaration, `export { name }` or `export default name`.
/// Re-exports elsewhere need this export first.
pub(super) fn binding_exported(file: &FileFacts, name: Span) -> bool {
    file.ast
        .exports
        .iter()
        .filter(|export| export.module.is_none())
        .flat_map(|export| export.declarations.iter().chain(export.specifiers.iter()))
        .any(|specifier| {
            specifier.local.span == name
                || file.ast.reference_declaration(specifier.local.span) == Some(name)
        })
}

/// Whether the binding that names `function` holds it whenever it is read: a
/// function declaration nothing writes, or a `const` (or a `let` nothing
/// writes) initialized with the function itself. A call's result
/// (`withX(() => ...)`), a method, or a written binding holds no proven value.
fn function_holds_its_binding(file: &FileFacts, function: &solid_facts::ast::FunctionFact) -> bool {
    if let Some(name) = function.name.as_ref()
        && function.kind == solid_facts::ast::FunctionKind::Declaration
    {
        return !binding_written(file, name.span);
    }
    let Some(name) = crate::owners::function_binding_name(file, function) else {
        return false;
    };
    file.ast.bindings.iter().any(|binding| {
        binding.initializer_function
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && matches!(binding.names.as_slice(), [only] if only.span == name.span)
            && binding.initializer.is_some_and(|initializer| {
                file.ast.peel_ts_sugar_span(initializer) == function.span
            })
            && (binding.immutable || binding.runtime_let && !binding_written(file, name.span))
    })
}
