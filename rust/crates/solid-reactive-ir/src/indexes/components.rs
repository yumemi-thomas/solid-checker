//! Component and JSX call-site indexes: component status, rendered functions, prop render sites, and entity lookups.

use super::*;

/// The reference census of one function ([`SemanticLookup::reference_census`]).
struct ReferenceCensus {
    /// Some reference is not accounted for.
    escapes: bool,
    as_callback: bool,
    as_component_property: bool,
    /// Some accounted reference is a render `createComponent` makes (a tag,
    /// a tag-only alias, a `lazy` default, a compound member tag, an
    /// `@solidjs/router` route), whose body runs in a strict-read window.
    create_component_render: bool,
    /// Some accounted reference is a render through `Dynamic` (a dialect
    /// renderer prop, a TanStack route option), whose body does not.
    dynamic_render: bool,
}

impl ReferenceCensus {
    /// Every render the census accounts for runs through `Dynamic`.
    fn only_dynamic_renders(&self) -> bool {
        self.dynamic_render && !self.create_component_render
    }
}

/// A value written at a prop spelled `component` is analyzed as a component
/// value, as it always was. It renders as one only where the dialect says the
/// receiving export renders that prop (`<Dynamic component={…}/>`, ADR 0138),
/// resolved through its declaration: only that accounts for the reference in
/// the census; a project component's `component` prop is just a prop.
fn component_value_operation(candidate: &FileFacts, span: Span) -> bool {
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

    /// ADR 0373: whether no code of a closed program can render or call
    /// `function`, so it receives no props at all (ADR 0214's vacuous case).
    ///
    /// The binding through which code reaches it is its declaration's name,
    /// or the `const` an unnamed function or arrow initializes (a named
    /// function expression's own name is visible only inside it, so its
    /// outer binding is not known here). Type Facts must know that binding's
    /// complete reference list, and every reference must be the declaration,
    /// a use inside the function itself, or the module surface (an export
    /// specifier or an import statement). The module census must be complete
    /// and leave the module unreached by any namespace (`namespace_exposed_modules`:
    /// namespace imports and re-exports, dynamic loads, `import =`, through
    /// barrels), and no file may hold a direct `eval` or an `import.meta.glob`,
    /// which load or name what no reference shows. A dependency-source file
    /// answers `false`.
    pub(crate) fn component_never_rendered(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        let Some(exposed) = self.unrendered_census.get_or_init(|| {
            let opaque = self.facts.files.iter().any(|file| {
                !file.ast.lazy_import_globs.is_empty()
                    || !file.ast.in_place_import_globs.is_empty()
                    || file
                        .ast
                        .module_hazards
                        .iter()
                        .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
            });
            if !self.program_closed || opaque {
                return None;
            }
            crate::object_members::namespace_exposed_modules(self.facts, self)
        }) else {
            return false;
        };
        let path = file.path.as_str();
        if exposed.contains(path)
            || self.facts.dependency_source_root(path).is_some()
            || self.hands_out_by_name(path, function.span)
        {
            return false;
        }
        let binding = match function.kind {
            solid_facts::ast::FunctionKind::Declaration => function.name.as_ref(),
            _ if function.name.is_none()
                && crate::owners::own_method_name(file, function).is_none() =>
            {
                crate::owners::function_binding_name(file, function)
            }
            _ => None,
        };
        let Some(binding) = binding else {
            return false;
        };
        let Some(references) = self
            .entities
            .at(path, binding.span)
            .and_then(|symbol| self.symbol_references_if_present(symbol.as_str()))
        else {
            return false;
        };
        references.iter().all(|reference| {
            let (Some(candidate), Ok(start), Ok(end)) = (
                self.file_by_path(reference.path.as_ref()),
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let span = Span::new(start, end);
            (candidate.path.as_str() == path
                && (span == binding.span || function.span.contains(span)))
                || super::reference_is_declaration_or_module_surface(candidate, span)
        })
    }

    pub(super) fn component_functions(&self) -> &HashSet<(&'a str, Span)> {
        &self.component_statuses().0
    }

    /// Proven components every accounted render of which runs through
    /// `Dynamic` (ADR 0138): rc.17's `dynamicCore` calls the component as
    /// `untrack(() => component(props))` inside a memo, with no strict-read
    /// label (`solid-js` `dist/solid.dev.js:1515`, static form `:1541`),
    /// where `createComponent` passes one (`observedComponent`, `:65`), and
    /// `untrack` opens the window only for a label (`@solidjs/signals`
    /// `dist/dev-shared.js:5099-5117`). A strict-read claim in such a body is
    /// an obligation ([`Self::windowless_component_regions`]).
    pub(super) fn windowless_component_functions(&self) -> &HashSet<(&'a str, Span)> {
        &self.component_statuses().2
    }

    /// Functions that would be proven components but for a reference that
    /// enters them some other way. They are analyzed as components, so no
    /// finding is lost, and every violation in their own body is withdrawn to
    /// an obligation ([`Self::undecided_component_regions`]).
    pub(super) fn escaped_component_functions(&self) -> &HashSet<(&'a str, Span)> {
        &self.component_statuses().1
    }

    fn component_statuses(&self) -> &ComponentSets<'a> {
        self.component_functions.get_or_init(|| {
            let mut proven = HashSet::default();
            let mut escaped = HashSet::default();
            let mut windowless = HashSet::default();
            for file in &self.facts.files {
                for function in &file.ast.functions {
                    let (status, only_dynamic) =
                        self.compute_function_component_status(file, function);
                    match status {
                        ComponentStatus::Proven => {
                            proven.insert((file.path.as_str(), function.span));
                            if only_dynamic {
                                windowless.insert((file.path.as_str(), function.span));
                            }
                        }
                        // Analyzed as a component, as before; findings in
                        // its own body are obligations (`UndecidedRegion`).
                        ComponentStatus::Uncertain => {
                            proven.insert((file.path.as_str(), function.span));
                            escaped.insert((file.path.as_str(), function.span));
                        }
                        ComponentStatus::No => {}
                    }
                }
            }
            (proven, escaped, windowless)
        })
    }

    /// `Proven` or `No`, or `Uncertain` when only an escaping reference
    /// withholds the direct-JSX-return proof; and whether every render the
    /// census accounts for runs through `Dynamic`.
    fn compute_function_component_status(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> (ComponentStatus, bool) {
        // ADR 0334/0335: a function written as an audited router's component
        // value is rendered there, and an anonymous inline value has no other
        // reference (a named function expression has its own name in its
        // body, which can hand it on: `invoke = Inline`).
        if function.name.is_none()
            && let Some(through_dynamic) = self.route_render(file.path.as_str(), function.span)
        {
            return (ComponentStatus::Proven, through_dynamic);
        }
        // ADR 0342: an anonymous function stored in a compound component that
        // some JSX member tag renders has no other reference.
        if function.name.is_none() && self.compound_value(file.path.as_str(), function.span) {
            return (ComponentStatus::Proven, false);
        }
        let binding_name = crate::owners::component_binding_name(file, function);
        // ADR 0331: a tag proves identity only when no other reference can
        // produce a plain call. A function a tag renders and some other
        // reference hands on (`items().map(Row)`, an alias that is called, a
        // plain call) may also run as a plain call, whose body runs in the
        // caller's scope; it stays analyzed as a component and its violations
        // become obligations. A reference whose every use is a render (an
        // object value only `<Dynamic component={icons[k]}/>` reads, an alias
        // only tags name) is no such escape. An exact Component type does not
        // change that: it says nothing about how the function is invoked, and
        // `tsc` accepts `items().map(Row)` for it.
        if self
            .jsx_call_site_loading(file.path.as_str(), function.span)
            .any
        {
            // A tag is a `createComponent` render, so the body has a
            // strict-read window whatever else renders it.
            return if !self
                .reference_census(file, function, binding_name, true)
                .escapes
            {
                (ComponentStatus::Proven, false)
            } else {
                (ComponentStatus::Uncertain, false)
            };
        }
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
        // type can still establish component identity, and only when no
        // other reference escapes. Avoid building the project-wide reference
        // and call-site indexes for ordinary helpers.
        let has_component_type = self.project_has_component_type()
            && self.binding_has_component_type(file, binding_name);
        if !directly_returns_jsx {
            if !has_component_type {
                return (ComponentStatus::No, false);
            }
            let census = self.reference_census(file, function, binding_name, false);
            return if census.escapes {
                (ComponentStatus::Uncertain, false)
            } else {
                (ComponentStatus::Proven, census.only_dynamic_renders())
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
        let used_as_render_value = file.compiler.jsx_operations.iter().any(|operation| {
            matches!(
                operation.kind.as_str(),
                "component-property" | "component-spread" | "component-child"
            ) && operation.span.contains(function.span)
                && direct_function_value(file, operation.span)
                && !component_value_operation(file, function.span)
        });
        let census = self.reference_census(file, function, binding_name, false);
        let only_dynamic = census.only_dynamic_renders();
        let ReferenceCensus {
            escapes: reference_escapes,
            as_callback: referenced_as_callback,
            as_component_property: referenced_as_component_property,
            ..
        } = census;
        let hoc_bound = binding_name.is_some()
            && crate::owners::function_binding_name(file, function).is_none();
        let directly_called = !self
            .function_call_sites(file.path.as_str(), function.span)
            .is_empty();
        let direct_return_proof = directly_returns_jsx
            && (self.dialect.direct_jsx_return_is_component()
                || hoc_bound
                || referenced_as_component_property)
            && !directly_called
            && (!used_as_callback_value(file) || hoc_bound)
            && (!used_as_any_call_argument || hoc_bound)
            && !used_as_render_value
            && (!referenced_as_callback || referenced_as_component_property);
        if (direct_return_proof || has_component_type) && !reference_escapes {
            (ComponentStatus::Proven, only_dynamic)
        } else if direct_return_proof || has_component_type {
            (ComponentStatus::Uncertain, false)
        } else {
            (ComponentStatus::No, false)
        }
    }

    /// Every reference to the function, sorted into the uses that account
    /// for it and the ones that do not.
    ///
    /// Solid runs a function as a component only when `createComponent`
    /// calls it (a tag, a dialect renderer prop, `lazy`); called directly,
    /// its body runs in the caller's scope, tracked inside JSX. Component
    /// identity is therefore proven only when every reference to the
    /// function is accounted for: its declaration, its module surface, a JSX
    /// tag that renders it, a reference inside a type query
    /// (`ComponentProps<typeof Row>`, erased at runtime), a dialect callback
    /// value, a dialect renderer's prop (ADR 0138), a dialect loader's
    /// `default` (ADR 0292), an audited router's route option (ADR 0334) or a
    /// member of a compound component that only JSX member tags reach
    /// (ADR 0342). Any other use hands it to
    /// code that may call it plainly: `items().map(Row)`, `{ badge: Row }`,
    /// an alias, a router config the analysis does not read. So does a
    /// function with no resolvable binding, a reference in a file not
    /// analyzed, and an object or class method, reached through its object,
    /// whose aliases its name's references do not enumerate
    /// (`renderers.badge()` is absent from them). Each leaves identity
    /// undecided rather than proven.
    ///
    /// For a function a tag renders (`tagged`), two holders are read through
    /// as well, because every use they allow is a render: a closed object
    /// literal only a dialect renderer prop reads through its members
    /// (`<Dynamic component={icons[name]}/>`, ADR 0138's flow), and a `const`
    /// alias whose every reference is a plain tag ([`Self::tag_only_alias`]).
    /// A function with no tag keeps ADR 0331's census unchanged.
    fn reference_census(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        binding_name: Option<&solid_facts::ast::NamedSpan>,
        tagged: bool,
    ) -> ReferenceCensus {
        let mut census = ReferenceCensus {
            escapes: true,
            as_callback: false,
            as_component_property: false,
            create_component_render: false,
            dynamic_render: false,
        };
        let method =
            function.name.is_none() && crate::owners::own_method_name(file, function).is_some();
        if method {
            return census;
        }
        let Some(symbol) = binding_name
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
        else {
            return census;
        };
        census.escapes = false;
        for reference in symbol.references() {
            let Some(candidate) = self.file_by_path(reference.path.as_ref()) else {
                census.escapes = true;
                continue;
            };
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                census.escapes = true;
                continue;
            };
            let span = Span::new(start, end);
            let callback = self.reference_is_callback_value(candidate, span);
            let component_value = component_value_operation(candidate, span);
            census.as_callback |= callback;
            census.as_component_property |= component_value;
            let dynamic_prop = self.rendered_by_dialect_prop(candidate, span, tagged);
            let route = self.route_render(candidate.path.as_str(), span);
            let created = self.jsx_tag_reference(file, function, candidate, span)
                || self.lazy_default_value(candidate, function, span)
                || route == Some(false)
                || self.compound_value(candidate.path.as_str(), span)
                || (tagged && self.tag_only_alias(candidate, span));
            let through_dynamic = dynamic_prop || route == Some(true);
            census.create_component_render |= created;
            census.dynamic_render |= through_dynamic;
            census.escapes |= !(callback && !component_value)
                && !through_dynamic
                && !created
                && !reference_is_declaration_or_module_surface(candidate, span)
                && !candidate
                    .ast
                    .type_queries
                    .iter()
                    .any(|query| query.contains(span));
        }
        census
    }

    /// Whether `span` in `candidate` is a tag name that renders `function`:
    /// the opening or closing name, or the final property of a dotted one.
    fn jsx_tag_reference(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        candidate: &FileFacts,
        span: Span,
    ) -> bool {
        self.jsx_tags_by_function()
            .get(&(file.path.as_str(), function.span))
            .is_some_and(|tags| {
                tags.iter().any(|(path, element)| {
                    *path == candidate.path.as_str()
                        && (element.name.span == span
                            || element.closing_name == Some(span)
                            || (element.member_property.is_some()
                                && (element.member_property == Some(span)
                                    || element.closing_name.is_some_and(|closing| {
                                        closing.contains(span) && closing.end == span.end
                                    }))))
                })
            })
    }

    fn jsx_tags_by_function(&self) -> &JsxTagsByFunction<'a> {
        self.jsx_tags_by_function.get_or_init(|| {
            let mut map = JsxTagsByFunction::default();
            for (_, caller_file, element, target_file, target) in self.jsx_rendered_functions() {
                map.entry((target_file.path.as_str(), target.span))
                    .or_default()
                    .push((caller_file.path.as_str(), element));
            }
            map
        })
    }

    fn reference_is_callback_value(&self, candidate: &FileFacts, span: Span) -> bool {
        candidate.compiler.callback_roles.iter().any(|role| {
            role.span.contains(span) && candidate.ast.functions_within(role.span).next().is_none()
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
    }

    /// Whether `span` is a value a dialect export renders as a component
    /// (`<Dynamic component={…}/>`, ADR 0138). A value that reaches it
    /// through an object holder's member read counts only for a function a
    /// tag renders (`tagged`).
    fn rendered_by_dialect_prop(&self, candidate: &FileFacts, span: Span, tagged: bool) -> bool {
        self.prop_render_sites()
            .get(&(candidate.path.as_str(), span))
            .is_some_and(|(_, through_object)| tagged || !through_object)
    }

    /// Whether `span` is the whole initializer of `const Alias = F` (through
    /// TypeScript wrappers) whose every reference is its declaration, a
    /// reference inside a type query or a plain JSX tag (`<Alias/>`, its
    /// closing name): `createComponent(Alias, props)` is then the only use
    /// of the value. The alias is unexported, a single identifier, and its
    /// module has no direct `eval`, which could read it with no reference; a
    /// reference in another file, a call, an argument, a member or a dotted
    /// tag withholds it.
    fn tag_only_alias(&self, candidate: &FileFacts, span: Span) -> bool {
        let ast = &*candidate.ast;
        let Some(binding) = ast.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding
                    .initializer
                    .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == span)
        }) else {
            return false;
        };
        let name = binding.names[0].span;
        let exported = ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .any(|specifier| specifier.local.span == name)
        });
        let whole = Span::new(0, u32::try_from(candidate.source.len()).unwrap_or(u32::MAX));
        if exported || crate::interproc::property_gets::direct_eval_in(candidate, whole) {
            return false;
        }
        let Some(symbol) = self
            .entities
            .at(candidate.path.as_str(), name)
            .and_then(|symbol| self.symbols_by_id().get(symbol.as_str()))
        else {
            return false;
        };
        symbol.references().all(|reference| {
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let use_ = Span::new(start, end);
            reference.path.as_ref() == candidate.path.as_str()
                && (use_ == name
                    || ast.type_queries.iter().any(|query| query.contains(use_))
                    || ast.jsx_elements.iter().any(|element| {
                        element.member_property.is_none()
                            && (element.name.span == use_ || element.closing_name == Some(use_))
                    }))
        })
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
        self.component_regions(self.escaped_component_functions())
    }

    /// The bodies of [`Self::windowless_component_functions`], with the same
    /// carve-outs as [`Self::undecided_component_regions`]: a callback a
    /// primitive places (an effect's) and a nested tag-rendered component
    /// open windows of their own.
    pub(crate) fn windowless_component_regions(&self) -> Vec<crate::UndecidedRegion> {
        self.component_regions(self.windowless_component_functions())
    }

    fn component_regions(
        &self,
        functions: &HashSet<(&'a str, Span)>,
    ) -> Vec<crate::UndecidedRegion> {
        let mut regions = functions
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
        self.prop_render_sites()
            .get(&(path, reference))
            .map(|(sites, _)| sites.clone())
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
            // A computed read of an object holder (`icons[name]`) can fall
            // through to an inherited property: an accessor a reflective
            // prototype write installs would run with the holder as `this`
            // (ADR 0292's veto, read once and only when needed).
            let prototype_written = std::cell::OnceCell::new();
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
                        && (!flow.through_object
                            || !*prototype_written.get_or_init(|| {
                                crate::interproc::accessor_provenance::reflective_prototype_write(
                                    self,
                                )
                            }))
                    {
                        map.insert(
                            (file.path.as_str(), flow.reference),
                            (
                                flow.sites.iter().map(|site| site.site).collect(),
                                flow.through_object,
                            ),
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
    /// one iterator cannot.
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
            map
        })
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
