//! Component and JSX call-site indexes: component status, rendered functions, prop render sites, and entity lookups.

use super::*;

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
        self.component_functions()
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
            .possible_component_functions()
            .contains(&(file.path.as_str(), function.span))
        {
            return ComponentStatus::Uncertain;
        }
        let possible = crate::owners::component_binding_name(file, function).is_some_and(|name| {
            self.dialect
                .component_name_may_be_component(file.source_text(name.span).unwrap_or_default())
        });
        if possible {
            ComponentStatus::Uncertain
        } else {
            ComponentStatus::No
        }
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
    pub(super) fn escaped_component_functions(&self) -> &HashSet<(&'a str, Span)> {
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

    fn component_statuses(&self) -> &ComponentSets<'a> {
        self.component_functions.get_or_init(|| {
            let mut proven = HashSet::default();
            let mut escaped = HashSet::default();
            let mut possible = HashSet::default();
            for file in &self.facts.files {
                for function in &file.ast.functions {
                    let key = (file.path.as_str(), function.span);
                    match self.compute_function_component_status(file, function) {
                        Identity::Proven => {
                            proven.insert(key);
                        }
                        // Analyzed as a component, as before; findings in
                        // its own body are obligations (`UndecidedRegion`).
                        Identity::Escaped => {
                            proven.insert(key);
                            escaped.insert(key);
                        }
                        Identity::Possible => {
                            possible.insert(key);
                        }
                        Identity::No => {}
                    }
                }
            }
            (proven, escaped, possible)
        })
    }

    /// `Proven` or `No`; `Escaped` when only an escaping reference withholds
    /// the direct-JSX-return proof; `Possible` when a call would make it `No`
    /// but the census of its entries is incomplete.
    fn compute_function_component_status(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> Identity {
        if self
            .jsx_call_site_loading(file.path.as_str(), function.span)
            .any
        {
            return Identity::Proven;
        }
        // ADR 0334/0335: a function written as an audited router's component
        // value is rendered there, and an anonymous inline value has no other
        // reference (a named function expression has its own name in its
        // body, which can hand it on: `invoke = Inline`).
        if function.name.is_none() && self.route_component_value(file.path.as_str(), function.span)
        {
            return Identity::Proven;
        }
        // ADR 0342: an anonymous function stored in a compound component that
        // some JSX member tag renders has no other reference.
        if function.name.is_none() && self.compound_value(file.path.as_str(), function.span) {
            return Identity::Proven;
        }
        let binding_name = crate::owners::component_binding_name(file, function);
        let directly_contains_jsx = |span: Span| {
            file.ast.jsx_within(span).any(|element| {
                crate::owners::containing_ast_function(&file.ast, element.span)
                    .is_some_and(|owner| owner.span == function.span)
            }) || file.ast.jsx_fragments.iter().any(|fragment| {
                span.contains(*fragment)
                    && crate::owners::containing_ast_function(&file.ast, *fragment)
                        .is_some_and(|owner| owner.span == function.span)
            })
        };
        let directly_returns_jsx = function
            .expression_return
            .as_ref()
            .and_then(|returned| returned.argument)
            .is_some_and(directly_contains_jsx)
            || file.ast.returns_within(function.body).any(|returned| {
                returned.argument.is_some_and(directly_contains_jsx)
                    && crate::owners::containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|owner| owner.span == function.span)
            });
        // Every remaining runtime proof below only decides whether a direct
        // JSX return is used as a component value or as an ordinary callback.
        // Without such a return, only an exact compiler-resolved Component
        // type can still establish component identity. Avoid building the
        // project-wide reference and call-site indexes for ordinary helpers.
        if !directly_returns_jsx {
            return if self.project_has_component_type()
                && self.binding_has_component_type(file, binding_name)
            {
                Identity::Proven
            } else {
                Identity::No
            };
        }
        // Component identity is about the function value itself, not anything
        // lexically nested below a callback expression. The largest function
        // inside a callback/argument is the direct value; nested declarations
        // remain independently classifiable.
        let direct_function_value = |candidate: &FileFacts, container: Span| {
            candidate
                .ast
                .functions_within(container)
                .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                .is_some_and(|candidate| candidate.span == function.span)
        };
        let used_as_callback_value = |candidate: &FileFacts| {
            candidate
                .compiler
                .callback_roles
                .iter()
                .any(|role| direct_function_value(candidate, role.span))
                || candidate
                    .ast
                    .arguments_containing(function.span)
                    .any(|(call, index)| {
                        direct_function_value(candidate, call.arguments[index].span)
                            && self
                                .primitive_at_call(candidate, call.span)
                                .map(|primitive| {
                                    self.dialect.callback_semantics_at(
                                        primitive,
                                        index,
                                        call.arguments.len(),
                                    )
                                })
                                .is_some_and(|semantics| {
                                    semantics.execution.is_some() || semantics.stores_as_value
                                })
                    })
        };
        let used_as_any_call_argument = file
            .ast
            .arguments_containing(function.span)
            .any(|(call, index)| direct_function_value(file, call.arguments[index].span));
        let reference_span = |reference: &typefacts::Location| {
            let candidate = self.file_by_path(reference.path.as_ref())?;
            let start = u32::try_from(reference.start_byte).ok()?;
            let end = u32::try_from(reference.end_byte).ok()?;
            Some((candidate, Span::new(start, end)))
        };
        let reference_is_callback_value = |candidate: &FileFacts, span: Span| {
            candidate.compiler.callback_roles.iter().any(|role| {
                role.span.contains(span)
                    && candidate.ast.functions_within(role.span).next().is_none()
            }) || candidate
                .ast
                .arguments_containing(span)
                .any(|(call, index)| {
                    candidate
                        .ast
                        .functions_within(call.arguments[index].span)
                        .next()
                        .is_none()
                        && self
                            .primitive_at_call(candidate, call.span)
                            .map(|primitive| {
                                self.dialect.callback_semantics_at(
                                    primitive,
                                    index,
                                    call.arguments.len(),
                                )
                            })
                            .is_some_and(|semantics| {
                                semantics.execution.is_some() || semantics.stores_as_value
                            })
                })
        };
        // A value written at a prop spelled `component` is analyzed as a
        // component value, as it always was. It renders as one only where the
        // dialect says the receiving export renders that prop
        // (`<Dynamic component={…}/>`, ADR 0138), resolved through its
        // declaration: only that accounts for the reference in the census
        // below; a project component's `component` prop is just a prop.
        let component_value_operation = |candidate: &FileFacts, span: Span| {
            candidate.compiler.jsx_operations.iter().any(|operation| {
                if operation.kind == "component-property" && operation.span.contains(span) {
                    return candidate.ast.jsx_elements.iter().any(|element| {
                        element.attributes.iter().any(|attribute| {
                            attribute.value.is_some_and(|value| value.contains(span))
                                && candidate.source_text(attribute.local_name) == Some("component")
                        })
                    });
                }
                operation.kind == "component-spread"
                    && operation.span.contains(span)
                    && candidate.ast.object_properties.iter().any(|property| {
                        operation.span.contains(property.span)
                            && property.value.contains(span)
                            && candidate.source_text(property.key) == Some("component")
                    })
            })
        };
        let rendered_by_dialect_prop = |candidate: &FileFacts, span: Span| {
            self.prop_render_sites_at(candidate.path.as_str(), span)
                .is_some()
        };
        let used_as_render_value = file.compiler.jsx_operations.iter().any(|operation| {
            matches!(
                operation.kind.as_str(),
                "component-property" | "component-spread" | "component-child"
            ) && operation.span.contains(function.span)
                && direct_function_value(file, operation.span)
                && !component_value_operation(file, function.span)
        });
        let mut referenced_as_callback = false;
        let mut referenced_as_component_property = false;
        // Solid runs a function as a component only when `createComponent`
        // calls it (a tag, a dialect renderer prop, `lazy`); called directly,
        // its body runs in the caller's scope, tracked inside JSX. A direct JSX
        // return therefore proves identity only when every reference to the
        // function is accounted for: its declaration, its module surface, a
        // dialect callback value, a dialect renderer's prop (ADR 0138), a
        // dialect loader's `default` (ADR 0292), an audited router's route
        // option (ADR 0334) or a member of a compound component that only JSX
        // member tags reach (ADR 0342). Any other use hands it to
        // code that may call it plainly: `items().map(Row)`, `{ badge: Row }`,
        // an alias, a router config the analysis does not read. So does a
        // function with no resolvable binding, a reference in a file not
        // analyzed, and an object or class method, reached through its object,
        // whose aliases its name's references do not enumerate
        // (`renderers.badge()` is absent from them). Each leaves identity
        // undecided rather than proven.
        let method =
            function.name.is_none() && crate::owners::own_method_name(file, function).is_some();
        let directly_called = !self
            .function_call_sites(file.path.as_str(), function.span)
            .is_empty();
        // ADR 0331: a visible call proves the function is not a component
        // only when it is every entry. So a called function's references are
        // a census too: each must be a resolved call site, a reference the
        // test above accounts for, a type query, or an exact alias whose own
        // references are calls, tags, type queries or surface. Anything
        // else (an argument, a store, a method's receiver, a dotted tag) is
        // an entry nobody enumerated, which may render it.
        let call_references = if directly_called {
            self.function_call_site_references(file.path.as_str(), function.span)
                .into_iter()
                .map(|(caller, span)| (caller.path.as_str(), span))
                .collect::<HashSet<_>>()
        } else {
            HashSet::default()
        };
        let mut alias_entries = AliasEntries::default();
        let mut reference_escapes = true;
        if !method
            && let Some(symbol) = binding_name
                .and_then(|name| self.entities.at(file.path.as_str(), name.span))
                .and_then(|symbol| {
                    self.symbols_by_id
                        .get_or_init(|| {
                            self.facts
                                .typescript
                                .symbols()
                                .map(|candidate| (candidate.id(), candidate))
                                .collect()
                        })
                        .get(symbol.as_str())
                        .copied()
                })
        {
            reference_escapes = false;
            alias_entries.unfollowed = false;
            for reference in symbol.references() {
                let Some((candidate, span)) = reference_span(reference) else {
                    reference_escapes = true;
                    alias_entries.unfollowed = true;
                    continue;
                };
                let callback = reference_is_callback_value(candidate, span);
                let component_value = component_value_operation(candidate, span);
                referenced_as_callback |= callback;
                referenced_as_component_property |= component_value;
                let escapes = !(callback && !component_value)
                    && !rendered_by_dialect_prop(candidate, span)
                    && !reference_is_declaration_or_module_surface(candidate, span)
                    && !self.lazy_default_value(candidate, function, span)
                    && !self.route_component_value(candidate.path.as_str(), span)
                    && !self.compound_value(candidate.path.as_str(), span);
                reference_escapes |= escapes;
                if !escapes
                    || call_references.contains(&(candidate.path.as_str(), span))
                    || reference_in_type_query(candidate, span)
                {
                    continue;
                }
                match self.exact_alias_of_reference(candidate, span) {
                    Some(alias) => self.alias_entries(alias, ALIAS_DEPTH, &mut alias_entries),
                    None => alias_entries.unfollowed = true,
                }
            }
        } else {
            alias_entries.unfollowed = true;
        }
        // An open program's unseen importers of an exported function are
        // entries no census enumerates (ADR 0331 treats the module surface as
        // no entry only for the proof that it *is* a component). A closed
        // program has none, but a visible namespace read can still hand the
        // export out with no reference naming it (ADR 0347): a computed or
        // otherwise unnamed escape, or a named member or dynamic-import read.
        if directly_called
            && (binding_name.is_some_and(|name| binding_exported(file, name.span))
                && (!self.program_closed || self.escapes_unnamed(file.path.as_str()))
                || self.hands_out_by_name(file.path.as_str(), function.span))
        {
            alias_entries.unfollowed = true;
        }
        let hoc_bound = binding_name.is_some()
            && crate::owners::function_binding_name(file, function).is_none();
        let direct_return_proof = directly_returns_jsx
            && (self.dialect.direct_jsx_return_is_component()
                || hoc_bound
                || referenced_as_component_property)
            && !directly_called
            && (!used_as_callback_value(file) || hoc_bound)
            && (!used_as_any_call_argument || hoc_bound)
            && !used_as_render_value
            && (!referenced_as_callback || referenced_as_component_property);
        if direct_return_proof && !reference_escapes
            || self.binding_has_component_type(file, binding_name)
        {
            Identity::Proven
        } else if direct_return_proof {
            Identity::Escaped
        } else if directly_called && (alias_entries.unfollowed || alias_entries.rendered) {
            // ADR 0331: called, but entered some way the census cannot
            // follow, so the call is no proof of every entry. (An alias tag
            // made it a component above, through `jsx_call_site_loading`.)
            Identity::Possible
        } else {
            Identity::No
        }
    }

    /// Whether the binding's compiler-resolved type is an exact Solid
    /// component type alias.
    fn binding_has_component_type(
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

    /// Whether `reference` is the `default` value of a module object a
    /// dialect component loader renders, naming `function` (ADR 0292): that
    /// render is the reference's only use. A reflective prototype write
    /// anywhere vetoes it, as it vetoes the render witness.
    fn lazy_default_value(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        reference: Span,
    ) -> bool {
        file.ast.calls.iter().any(|call| {
            crate::owners::lazy_default_render_candidate(file, call, self).is_some_and(
                |(component, _, value)| component.span == function.span && value == reference,
            )
        }) && !crate::interproc::accessor_provenance::reflective_prototype_write(self)
    }

    fn project_has_component_type(&self) -> bool {
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
                                        .escaped_component_functions()
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
        self.jsx_call_sites()
            .get(&(path, function))
            .copied()
            .unwrap_or_default()
    }

    /// Call sites whose Type Facts resolve to one project function: call
    /// expressions, and the JSX tags that render a component. Aliases,
    /// imports, and same-named locals follow canonical symbols. One entry per
    /// invocation — a closing tag is part of the render it closes, not a
    /// second call.
    pub(crate) fn function_call_sites(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.all_function_call_sites()
            .get(&(path, function))
            .into_iter()
            .flatten()
            .map(|site| (&self.facts.files[site.file], site.callee))
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
        self.all_function_call_sites()
            .get(&(path, function))
            .into_iter()
            .flatten()
            .flat_map(|site| {
                let file = &self.facts.files[site.file];
                std::iter::once((file, site.callee))
                    .chain(site.also_referenced.map(|span| (file, span)))
            })
            .collect()
    }

    /// The argument a call of `symbol` renders, when `symbol` is declared by
    /// one of the dialect's own packages under a rendering name.
    fn rendered_argument_of(&self, symbol: &str) -> Option<usize> {
        let packages = self.dialect.primitive_defining_packages();
        self.symbols_by_id()
            .get(symbol)?
            .declarations()
            .iter()
            .filter(|declaration| declared_in_package(declaration.location.path.as_ref(), packages))
            .find_map(|declaration| {
                self.dialect
                    .renders_component_argument(declaration.name.as_ref())
            })
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
        self.prop_render_sites().get(&(path, reference)).cloned()
    }

    fn prop_render_sites(&self) -> &PropRenderSites<'a> {
        self.prop_render_sites.get_or_init(|| {
            let mut map = PropRenderSites::default();
            let renderers = self.dialect.component_prop_renderers();
            if renderers.is_empty() {
                return map;
            }
            let mut props = renderers.iter().map(|(_, prop)| *prop).collect::<Vec<_>>();
            props.sort_unstable();
            props.dedup();
            let facts: &'a ProjectFacts = self.facts;
            for file in &facts.files {
                // Only a module that spells a rendering component and its prop
                // can hold one of these flows; the rest are not parsed again.
                if !renderers.iter().any(|(component, prop)| {
                    file.source.contains(component) && file.source.contains(prop)
                }) {
                    continue;
                }
                let Some(flows) = solid_facts::ast::component_value_flows(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    &props,
                ) else {
                    continue;
                };
                for flow in flows {
                    if flow
                        .holder_calls
                        .iter()
                        .all(|call| self.holder_call_is_exact(file, *call))
                        && flow.sites.iter().all(|site| self.renders_prop(file, site))
                    {
                        map.insert(
                            (file.path.as_str(), flow.reference),
                            flow.sites.iter().map(|site| site.site).collect(),
                        );
                    }
                }
            }
            map
        })
    }

    /// Whether the call at `call` is a primitive whose accessor hands its
    /// compute's results to nothing but its own reads.
    fn holder_call_is_exact(&self, file: &FileFacts, call: Span) -> bool {
        self.primitive_at_call(file, call)
            .is_some_and(|primitive| self.dialect.accessor_yields_only_its_compute(primitive))
    }

    /// Whether `site` renders its component with the dialect's semantics for
    /// its prop: the component is exactly a dialect export that renders that
    /// prop, and a call form's callee is exactly a dialect renderer of its
    /// first argument.
    fn renders_prop(&self, file: &FileFacts, site: &solid_facts::ast::ComponentPropSite) -> bool {
        if let Some(renderer) = site.renderer
            && self
                .callee_symbol(file, renderer)
                .and_then(|symbol| self.rendered_argument_of(symbol))
                != Some(0)
        {
            return false;
        }
        let Some(component) = self.entities.at(file.path.as_str(), site.component) else {
            return false;
        };
        let packages = self.dialect.primitive_defining_packages();
        let renderers = self.dialect.component_prop_renderers();
        self.symbols_by_id()
            .get(component.as_str())
            .into_iter()
            .flat_map(|symbol| symbol.declarations().iter())
            .filter(|declaration| declared_in_package(declaration.location.path.as_ref(), packages))
            .any(|declaration| {
                renderers.iter().any(|(name, prop)| {
                    *name == declaration.name.as_ref() && *prop == site.prop.as_str()
                })
            })
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
        let start = u64::from(span.start);
        let end = u64::from(span.end);
        let entities = self.contained_entities_by_path().get(path)?;
        let first = entities.partition_point(|entity| entity.location.start_byte < start);
        let last = entities.partition_point(|entity| entity.location.start_byte <= end);
        entities[first..last]
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                (entity.location.end_byte <= end && predicate(entity)).then_some((index, *entity))
            })
            .min_by_key(|(index, entity)| {
                (
                    entity.location.end_byte - entity.location.start_byte,
                    *index,
                )
            })
            .map(|(_, entity)| entity)
    }

    fn contained_entities_by_path(&self) -> &HashMap<&'a str, Vec<&'a EntityFact>> {
        self.contained_entities_by_path.get_or_init(|| {
            let mut by_path = HashMap::<&str, Vec<&EntityFact>>::default();
            for entity in self.facts.typescript.entities() {
                by_path
                    .entry(entity.location.path.as_ref())
                    .or_default()
                    .push(entity);
            }
            for entities in by_path.values_mut() {
                entities.sort_by_key(|entity| entity.location.start_byte);
            }
            by_path
        })
    }

    pub(super) fn functions_by_symbol(&self) -> &HashMap<&'a str, SymbolFunction> {
        self.functions_by_symbol.get_or_init(|| {
            let mut map = HashMap::default();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (function_index, function) in file.ast.functions.iter().enumerate() {
                    let own_method = crate::owners::own_method_name(file, function);
                    let Some(name) = function.name.as_ref().or(own_method) else {
                        continue;
                    };
                    let Some(symbol) = self
                        .entities
                        .at(file.path.as_str(), name.span)
                        .map(SymbolId::as_str)
                        .or_else(|| {
                            own_method.and_then(|method| {
                                self.method_symbol_ref(file.path.as_str(), method.span)
                            })
                        })
                    else {
                        continue;
                    };
                    map.entry(symbol).or_insert(SymbolFunction::Resolved {
                        file: file_index,
                        function: function_index,
                    });
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
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        if map.contains_key(symbol.as_str()) {
                            continue;
                        }
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
                            map.insert(symbol.as_str(), outcome);
                        }
                    }
                }
            }
            map
        })
    }

    /// Every JSX element whose tag name resolves, exactly, to one project
    /// function, paired with the function it renders.
    ///
    /// The single authority on "this tag renders that function". Two indexes
    /// need that answer — [`Self::jsx_call_sites`], which decides component
    /// identity and Loading placement, and [`Self::all_function_call_sites`],
    /// which puts the render edge in the call graph — and they must agree: the
    /// render edges are invisible to component identity only because a
    /// rendered function is already `jsx_call_site_loading(..).any`. Two copies
    /// of the resolution could drift apart on the next filter added to either;
    /// one iterator cannot. The one asymmetry is deliberate: a tag naming an
    /// exact alias ([`Self::alias_rendered_function`]) proves identity only,
    /// and the graph, lacking its edge, keeps the alias as an escape.
    fn jsx_rendered_functions(
        &self,
    ) -> impl Iterator<
        Item = (
            usize,
            &'a FileFacts,
            &'a solid_facts::ast::JsxElementFact,
            &'a FileFacts,
            &'a solid_facts::ast::FunctionFact,
        ),
    > + '_ {
        self.facts
            .files
            .iter()
            .enumerate()
            .flat_map(move |(file_index, caller_file)| {
                caller_file
                    .ast
                    .jsx_elements
                    .iter()
                    .filter_map(move |element| {
                        let (target_file, target) =
                            self.function_called_at(caller_file.path.as_str(), element.name.span)?;
                        Some((file_index, caller_file, element, target_file, target))
                    })
            })
    }

    fn jsx_call_sites(&self) -> &HashMap<(&'a str, Span), CallSiteLoading> {
        self.jsx_call_sites.get_or_init(|| {
            let mut map = HashMap::<(&'a str, Span), CallSiteLoading>::default();
            for (_, _, _, target_file, target) in self.jsx_rendered_functions() {
                let entry = map
                    .entry((target_file.path.as_str(), target.span))
                    .or_default();
                entry.any = true;
            }
            // ADR 0331: `const P2 = Plain; <P2/>` renders `Plain` as a
            // component, as `<Plain/>` does. Identity only: the call graph
            // keeps no edge for it, so its consumers still see the alias as
            // an unaccounted reference and fail closed.
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
                    {
                        map.entry((target_file.path.as_str(), target.span))
                            .or_default()
                            .any = true;
                    }
                }
            }
            map
        })
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

    /// How the references of an exact alias binding enter the value it holds
    /// (ADR 0331), transitively through aliases of the alias.
    fn alias_entries(&self, alias: &str, depth: usize, entries: &mut AliasEntries) {
        let Some(symbol) = self.symbols_by_id().get(alias).copied() else {
            entries.unfollowed = true;
            return;
        };
        if depth == 0 {
            entries.unfollowed = true;
            return;
        }
        for reference in symbol.references() {
            let Some((candidate, span)) = self.reference_location(reference) else {
                entries.unfollowed = true;
                continue;
            };
            if reference_is_declaration_or_module_surface(candidate, span)
                || reference_in_type_query(candidate, span)
            {
                continue;
            }
            if candidate
                .ast
                .jsx_elements
                .iter()
                .any(|element| element.name.span == span || element.closing_name == Some(span))
            {
                entries.rendered = true;
                continue;
            }
            if self
                .call_by_callee(candidate, span)
                .is_some_and(|call| call.callee == span)
            {
                continue;
            }
            if let Some(next) = self.exact_alias_of_reference(candidate, span) {
                self.alias_entries(next, depth - 1, entries);
                continue;
            }
            entries.unfollowed = true;
        }
    }

    /// The symbol of the exact alias binding whose initializer is the
    /// reference at `span`: `const Alias = Value`.
    fn exact_alias_of_reference(&self, file: &FileFacts, span: Span) -> Option<&'a str> {
        let binding = file.ast.bindings.iter().find(|binding| {
            binding
                .initializer_identifier
                .as_ref()
                .is_some_and(|identifier| identifier.span == span)
        })?;
        exact_alias_initializer(file, binding)?;
        // An open program's unseen importers may use an exported alias any
        // way at all; that is no exact alias of anything we can follow.
        if !self.program_closed && binding_exported(file, binding.names[0].span) {
            return None;
        }
        let symbol = self
            .entities
            .at(file.path.as_str(), binding.names[0].span)?;
        self.symbols_by_id()
            .get_key_value(symbol.as_str())
            .map(|(id, _)| *id)
    }

    fn reference_location(&self, reference: &typefacts::Location) -> Option<(&'a FileFacts, Span)> {
        let candidate = self.file_by_path(reference.path.as_ref())?;
        let start = u32::try_from(reference.start_byte).ok()?;
        let end = u32::try_from(reference.end_byte).ok()?;
        Some((candidate, Span::new(start, end)))
    }

    /// Every entry the graph can name for each project function.
    ///
    /// Rendering `<Panel/>` invokes `Panel`: the tag is a call site, not a
    /// value escape, and the same exact resolution that proves which function a
    /// tag renders ([`Self::jsx_rendered_functions`], shared with
    /// `jsx_call_sites` so component identity and the call graph cannot
    /// disagree about which tags resolve) names the callee here. The edge's
    /// callee is the tag *name* span only — the component's own reference — so
    /// a component used as a value rather than rendered (`<Wrap
    /// child={Panel}/>`, `return Panel`, `apply(Panel)`) is still an escape;
    /// `component_value_operation` keeps that distinction. A tag whose exact
    /// declaration is unresolved — an untyped import, an ambiguous computed
    /// name — resolves to nothing and emits no edge, so consumers keep failing
    /// closed on it rather than gaining a caller the runtime does not have.
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
    pub(super) fn all_function_call_sites(
        &self,
    ) -> &HashMap<(&'a str, Span), Vec<FunctionCallSite>> {
        self.function_call_sites.get_or_init(|| {
            let mut map = HashMap::<(&str, Span), Vec<FunctionCallSite>>::default();
            for (file_index, caller_file) in self.facts.files.iter().enumerate() {
                for call in &caller_file.ast.calls {
                    // ADR 0339: a call that may run before its callee's
                    // declarator is no call site.
                    if self.may_precede_initialization(caller_file.path.as_str(), call.callee) {
                        continue;
                    }
                    let Some(symbol) = self.callee_symbol(caller_file, call.callee) else {
                        continue;
                    };
                    let Some((target_file, target)) = self.function_for_symbol(symbol) else {
                        continue;
                    };
                    map.entry((target_file.path.as_str(), target.span))
                        .or_default()
                        .push(FunctionCallSite {
                            file: file_index,
                            callee: call.callee,
                            also_referenced: None,
                        });
                }
            }
            for (file_index, caller_file, element, target_file, target) in
                self.jsx_rendered_functions()
            {
                if self.may_precede_initialization(caller_file.path.as_str(), element.name.span) {
                    continue;
                }
                map.entry((target_file.path.as_str(), target.span))
                    .or_default()
                    .push(FunctionCallSite {
                        file: file_index,
                        callee: element.name.span,
                        // One render, one edge. `<Panel></Panel>` writes the
                        // tag name twice and TypeScript reports both
                        // occurrences as references to `Panel`, so the escape
                        // test has to account for the closing one — but it is
                        // the same invocation, so it rides on this edge
                        // instead of minting a second one. Carried here rather
                        // than re-derived by the consumer so it can only ever
                        // be the closing name of the element whose opening tag
                        // resolved to this function.
                        also_referenced: element.closing_name,
                    });
            }
            map
        })
    }

    pub(super) fn entities_by_location(&self) -> &HashMap<(&'a str, u64, u64), &'a EntityFact> {
        self.entities_by_location.get_or_init(|| {
            let mut map = HashMap::default();
            for entity in self.facts.typescript.entities() {
                map.entry((
                    entity.location.path.as_ref(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ))
                .or_insert(entity);
            }
            map
        })
    }
}

/// How far a chain of aliases (`const A = Plain; const B = A`) is followed.
const ALIAS_DEPTH: usize = 8;

/// The entries an exact alias's references give the value it holds.
#[derive(Default)]
struct AliasEntries {
    /// A JSX tag names the alias: a component render.
    rendered: bool,
    /// Some reference is none of these, nor a call whose whole callee is the
    /// alias (read attribution follows those calls), a declaration, module
    /// surface, type query or a further exact alias.
    unfollowed: bool,
}

/// The identifier initializing `binding` when the binding is an exact alias
/// of it: one plain name, bound by `const` or by a `let` nothing writes, to
/// an identifier (parentheses and TypeScript wrappers aside), so the binding
/// holds that value whenever it is read after its declarator.
fn exact_alias_initializer(
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
fn reference_in_type_query(file: &FileFacts, span: Span) -> bool {
    file.ast
        .type_queries
        .iter()
        .any(|query| query.contains(span))
}

/// Whether the module exports the binding declared at `name` itself: an
/// exported declaration, `export { name }` or `export default name`.
/// Re-exports elsewhere need this export first.
fn binding_exported(file: &FileFacts, name: Span) -> bool {
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

/// What [`SemanticLookup::compute_function_component_status`] decides.
enum Identity {
    Proven,
    Escaped,
    Possible,
    No,
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
