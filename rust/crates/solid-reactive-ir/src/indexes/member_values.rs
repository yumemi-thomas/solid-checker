//! Member value resolution: the symbols and candidates a member access can denote.

use super::*;

impl<'a> SemanticLookup<'a> {
    pub(super) fn nested_member_value(
        &self,
        file: &FileFacts,
        value: Span,
        path: &[String],
        depth: usize,
    ) -> Option<(&'a FileFacts, Span)> {
        if depth == 0 {
            return None;
        }
        let file = self.file_by_path(file.path.as_str())?;
        let value = file.ast.peel_ts_sugar_span(value);
        // Resolve bindings before literals, but never chase a compound span's
        // root entity. A binding must be a const or runtime lexical let with
        // no writes, initialized before its sole reference, and consumed in
        // its own frame (no captured alias). Ambient/namespace lets supply
        // no runtime initialization proof.
        if !path.is_empty() && file.ast.identifiers.iter().any(|id| id.span == value) {
            let declaration = file.ast.reference_declaration(value)?;
            let owner = file;
            let binding = owner
                .ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
            let declaration = binding.names.first()?.span;
            let initializer = binding.initializer?;
            let frame = |file: &FileFacts, span| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| function.body.contains(span))
                    .min_by_key(|function| function.span.end - function.span.start)
                    .map(|function| function.span)
            };
            if !(binding.immutable || binding.runtime_let)
                || binding.shape != solid_facts::ast::BindingShape::Identifier
                || binding.names.len() != 1
                || binding_written(owner, declaration)
                || initializer.end > value.start
                || frame(owner, declaration).is_none()
                || frame(owner, declaration) != frame(file, value)
                || owner
                    .ast
                    .reference_declarations
                    .iter()
                    .filter(|(_, bound)| *bound == declaration)
                    .any(|(reference, _)| *reference != value)
            {
                return None;
            }
            return self.nested_member_value(owner, initializer, path, depth - 1);
        }
        let Some((property, rest)) = path.split_first() else {
            return Some((file, value));
        };
        let properties = crate::interproc::accessor_provenance::closed_properties(file, value)?;
        let selected = properties.into_iter().find(|candidate| {
            file.source_text(candidate.key)
                .is_some_and(|key| key.trim_matches(['\'', '"']) == property)
        })?;
        self.nested_member_value(file, selected.value, rest, depth - 1)
    }

    /// Resolve `parameter.member()` through the exact project call sites of
    /// the containing function. This is the structural-interface seam: the
    /// member declaration itself may be a composite TypeScript signature,
    /// while each analyzed call site supplies an exact runtime value. Every
    /// call site must resolve to a concrete property value; one missing or
    /// ambiguous site makes the whole dispatch uncertifiable.
    pub(super) fn structural_parameter_member_symbols(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        symbols: &mut Vec<SymbolId>,
    ) {
        let object = file.ast.peel_ts_sugar_span(object);
        let Some(receiver_symbol) = self.entities.at(file.path.as_str(), object) else {
            return;
        };
        let Some(function) = file
            .ast
            .functions_body_containing(object)
            .min_by_key(|function| function.body.end - function.body.start)
        else {
            return;
        };
        let Some(parameter_index) = function.parameters.iter().position(|parameter| {
            parameter.names.iter().any(|name| {
                self.entities
                    .at(file.path.as_str(), name.span)
                    .is_some_and(|symbol| symbol == receiver_symbol)
            })
        }) else {
            return;
        };
        let Some(function_name) = function_binding_name(file, function) else {
            return;
        };
        if self
            .entities
            .at(file.path.as_str(), function_name.span)
            .is_none()
        {
            return;
        }
        let call_sites = self.function_call_sites(file.path.as_str(), function.span);
        if call_sites.is_empty() {
            return;
        }
        let mut unresolved_site = false;
        for (caller_file, callee) in call_sites {
            let Some(call) = self.call_by_callee(caller_file, callee) else {
                unresolved_site = true;
                continue;
            };
            let Some(argument) = call.arguments.get(parameter_index) else {
                unresolved_site = true;
                continue;
            };
            let mut site_symbols = Vec::new();
            let mut visited = HashSet::default();
            self.member_value_symbols(
                caller_file,
                argument.span,
                property_name,
                &mut visited,
                &mut site_symbols,
            );
            site_symbols.sort_unstable();
            site_symbols.dedup();
            if site_symbols.is_empty() {
                unresolved_site = true;
            } else {
                symbols.extend(site_symbols);
            }
        }
        if unresolved_site {
            symbols.clear();
        }
    }

    pub(super) fn member_value_symbols(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        visited_spans: &mut HashSet<Span>,
        symbols: &mut Vec<SymbolId>,
    ) {
        self.member_value_symbols_in(file, object, property_name, visited_spans, symbols, false);
    }

    /// [`Self::member_value_symbols`]; `factories` also follows the result of
    /// a project factory call ([`Self::factory_member_value_candidates`]).
    /// Only the per-call-site parameter member resolution asks for that (ADR
    /// 0314).
    ///
    /// ADR 0320: the candidates are trusted only when every part of the value
    /// resolved. An alternative that does not resolve -- one arm of a
    /// conditional, a binding without an inspectable initializer, a spread
    /// that may supply or override the member, a value the program may
    /// rewrite -- withholds the whole value, and nothing is added. A resolved
    /// union keeps every alternative's candidates.
    fn member_value_symbols_in(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        _visited_spans: &mut HashSet<Span>,
        symbols: &mut Vec<SymbolId>,
        factories: bool,
    ) {
        if self.own_member_may_be_replaced(property_name) {
            return;
        }
        let mut visited = HashSet::default();
        let mut consulted = Vec::new();
        if let Some(found) = self.member_value_candidates(
            file,
            object,
            property_name,
            &mut visited,
            factories,
            &mut consulted,
        ) && consulted
            .iter()
            .all(|(path, _)| path.as_str() == file.path.as_str())
        {
            // An untracked consumer's cached answer depends on this file's
            // source only, so only an answer read from it is kept.
            symbols.extend(found);
        }
    }

    /// Whether some code may replace an own member `name` of an object it did
    /// not create: an assignment of that name or through an unknown computed
    /// key, or a reflective definition of it. Code copying unknown keys into
    /// an object (`Object.assign`) cannot reach one the resolution proves
    /// unshared ([`Self::binding_stays_closed`], a fresh literal or factory
    /// result whose methods never use their receiver).
    pub(super) fn own_member_may_be_replaced(&self, name: &str) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || assigned.names.contains(name)
            || self.property_may_be_defined_reflectively_as(name)
    }

    /// ADR 0320: whether any code may copy keys into an existing object: a
    /// member named `assign`, `setPrototypeOf`, `defineProperty` or
    /// `defineProperties` that is not the callee of a call whose first
    /// argument is a fresh literal (read, aliased, or called on anything
    /// else); any `Reflect` other than the object of a member it does not
    /// name `set`, `defineProperty` or `setPrototypeOf` directly on a fresh
    /// literal; or a destructuring that takes one of those names.
    pub(super) fn objects_may_be_assigned_into(&self) -> bool {
        const COPIERS: [&str; 4] = [
            "assign",
            "setPrototypeOf",
            "defineProperty",
            "defineProperties",
        ];
        *self.objects_assigned_into.get_or_init(|| {
            // An assignment through a key nothing names (`t[key] = v`) may
            // write any member of whatever `t` is, unless `t` is proven a
            // built-in value (`xs[i] = v` on an array), which no project
            // instance is.
            let unknown_key_write = self.files().iter().any(|file| {
                file.ast.assignments.iter().any(|assignment| {
                    let target = file.ast.peel_ts_sugar_span(assignment.target);
                    file.ast.computed_members.binary_search(&target).is_ok()
                        && file
                            .ast
                            .literal_computed_members
                            .iter()
                            .all(|literal| literal.span != target)
                        && file.ast.member_at(target).is_some_and(|member| {
                            constant_string_key(file, member.property).is_none()
                                && !matches!(
                                    self.value_origin_assuming(
                                        file,
                                        member.object,
                                        solid_facts::ast::RuntimeValueKind::Unknown,
                                        6,
                                        None,
                                    ),
                                    Some(ValueOrigin::Builtin)
                                )
                        })
                })
            });
            unknown_key_write
                || self.files().iter().any(|file| {
                    let fresh_first = |member: Span| {
                        file.ast.calls.iter().any(|call| {
                            file.ast.peel_ts_sugar_span(call.callee) == member
                                && call.arguments.first().is_some_and(|argument| {
                                    let target = file.ast.peel_ts_sugar_span(argument.span);
                                    !argument.spread
                                        && (file
                                            .ast
                                            .object_get_shapes
                                            .iter()
                                            .any(|shape| shape.span == target)
                                            || file
                                                .ast
                                                .array_literals
                                                .binary_search(&target)
                                                .is_ok()
                                            || file
                                                .ast
                                                .functions
                                                .iter()
                                                .any(|function| function.span == target))
                                })
                        })
                    };
                    let unquoted = |span: Span| {
                        file.source_text(span)
                            .map(|text| text.trim_matches(['\'', '"', '`']).to_owned())
                    };
                    file.ast.members.iter().any(|member| {
                        let Some(name) = unquoted(member.property) else {
                            return false;
                        };
                        let reflect = file.source_text(file.ast.peel_ts_sugar_span(member.object))
                            == Some("Reflect");
                        (COPIERS.contains(&name.as_str()) || (reflect && name == "set"))
                            && !fresh_first(member.span)
                    }) || file.ast.identifiers.iter().any(|identifier| {
                        file.source_text(identifier.span) == Some("Reflect")
                            && !file.ast.members.iter().any(|member| {
                                file.ast.peel_ts_sugar_span(member.object) == identifier.span
                            })
                    }) || file.ast.bindings.iter().any(|binding| {
                        binding.object_slots.iter().any(|slot| {
                            COPIERS.contains(&slot.property.as_str()) || slot.property == "set"
                        })
                    })
                })
        })
    }

    /// The exact implementations `object.property` resolves to, or `None`
    /// when some part of the value does not resolve.
    pub(super) fn member_value_candidates(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        visited: &mut HashSet<(String, Span)>,
        factories: bool,
        consulted: &mut Vec<(String, Span)>,
    ) -> Option<Vec<SymbolId>> {
        let object = file.ast.peel_ts_sugar_span(object);
        if !visited.insert((file.path.to_string(), object)) {
            return None;
        }
        if let Some(conditional) = file.ast.conditional_at(object) {
            let mut both = self.member_value_candidates(
                file,
                conditional.consequent,
                property_name,
                visited,
                factories,
                consulted,
            )?;
            both.extend(self.member_value_candidates(
                file,
                conditional.alternate,
                property_name,
                visited,
                factories,
                consulted,
            )?);
            return Some(both);
        }

        let direct_symbol = self.entities.at(file.path.as_str(), object);
        let identifier_reference = direct_symbol.is_none()
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == object
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            });
        let binding = direct_symbol
            .and_then(|symbol| {
                self.binding_for_symbol(symbol)
                    .map(|(binding_file, binding)| (binding_file, binding, symbol.clone()))
            })
            .or_else(|| {
                identifier_reference
                    .then(|| self.binding_at_reference(file.path.as_str(), object))?
            });
        if let Some((binding_file, binding, symbol)) = binding {
            // ADR 0314: only a `const` naming its whole initializer holds that
            // value. A `let` may hold another by now, and a destructured name
            // holds a member of the initializer, not the initializer. A
            // parameter's default holds only when no argument is passed.
            let [name] = binding.names.as_slice() else {
                return None;
            };
            if !binding.immutable
                || binding.shape != solid_facts::ast::BindingShape::Identifier
                || binding_file.ast.functions.iter().any(|function| {
                    function
                        .parameters
                        .iter()
                        .any(|parameter| parameter.names.iter().any(|n| n.span == name.span))
                })
                || !self.binding_stays_closed(
                    binding_file,
                    name.span,
                    &symbol,
                    file,
                    object,
                    consulted,
                )
            {
                return None;
            }
            let initializer = binding.initializer?;
            return self.member_value_candidates(
                binding_file,
                initializer,
                property_name,
                visited,
                factories,
                consulted,
            );
        }

        // ADR 0314: the object is exactly an object literal, and only its own
        // properties and spreads count: not a property of a literal nested in
        // it, nor of a literal anywhere inside a call or function that is not
        // one (`createFoo({ run })` holds no `run` of its result).
        let Some(literal) = file
            .ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == object)
        else {
            return if factories {
                self.factory_member_value_candidates(
                    file,
                    object,
                    property_name,
                    visited,
                    consulted,
                )
            } else {
                None
            };
        };
        let direct = |span: Span| {
            literal.span.contains(span)
                && !file.ast.object_get_shapes.iter().any(|inner| {
                    inner.span != literal.span
                        && literal.span.contains(inner.span)
                        && inner.span.contains(span)
                })
        };
        // A function in the literal that uses its receiver may hand the
        // object to code that replaces the member (ADR 0320).
        if file.ast.functions.iter().any(|function| {
            direct(function.span)
                && function.kind != solid_facts::ast::FunctionKind::Arrow
                && file
                    .source_text(function.span)
                    .is_none_or(crate::execution_role::mentions_receiver)
        }) {
            return None;
        }
        // A getter's value is whatever it returns when read, not itself; a
        // key without a static name (computed, or a `__proto__:` prototype
        // setter) may name this property or supply it.
        if literal
            .properties
            .iter()
            .any(|property| property.key.as_str() == property_name && property.getter.is_some())
            || file.ast.object_properties.iter().any(|property| {
                direct(property.span)
                    && (property.computed
                        || crate::execution_role::property_static_key(file, property).is_none())
            })
        {
            return None;
        }
        let spreads = file
            .ast
            .spreads
            .iter()
            .filter(|spread| direct(spread.span))
            .collect::<Vec<_>>();
        let Some(properties) = file
            .ast
            .object_properties
            .iter()
            .filter(|property| {
                direct(property.span)
                    && crate::execution_role::property_static_key(file, property).as_deref()
                        == Some(property_name)
            })
            .max_by_key(|property| property.span.start)
        else {
            // No own property: the member is whatever the one spread copies.
            let [spread] = spreads.as_slice() else {
                return None;
            };
            return self.member_value_candidates(
                file,
                spread.argument,
                property_name,
                visited,
                factories,
                consulted,
            );
        };
        // A spread after the property may replace it.
        if spreads
            .iter()
            .any(|spread| spread.span.start > properties.span.start)
        {
            return None;
        }

        // ADR 0321: a function literal that is exactly the property's value
        // is its own summary node, identified by its span.
        if let Some(function) = file.ast.functions.iter().find(|function| {
            function.span == file.ast.peel_ts_sugar_span(properties.value)
                && function.method_name.is_none()
        }) && let Some(symbol) =
            crate::interproc::literal_property_function_symbol(file, function)
        {
            return Some(vec![symbol]);
        }
        // The property key itself is a semantic identity once demanded by
        // the backend. This works for class/object methods and for function
        // properties copied through an exact object spread.
        let mut symbols = Vec::new();
        if let Some(symbol) = self.entities.at(file.path.as_str(), properties.key) {
            symbols.push(symbol.clone());
        }
        for function in file.ast.functions.iter().filter(|function| {
            properties.value.contains(function.span)
                && function.method_name.as_ref().is_some_and(|name| {
                    name.span == properties.key
                        && self.entities.at(file.path.as_str(), name.span).is_some()
                })
        }) {
            if let Some(name) = function.method_name.as_ref()
                && let Some(symbol) = self.entities.at(file.path.as_str(), name.span)
            {
                symbols.push(symbol.clone());
            }
        }
        if symbols.is_empty() {
            return self.member_value_candidates(
                file,
                properties.value,
                property_name,
                visited,
                factories,
                consulted,
            );
        }
        Some(symbols)
    }

    /// ADR 0320: whether the object a `const` binding holds stays the one its
    /// initializer created, as far as `object` (the reference being resolved,
    /// in `file`) can tell: every other reference anywhere is the object of a
    /// non-computed member access, or an argument of an exact project callee
    /// that only reads members of it, so nothing hands it to code that could
    /// replace a member. An exported binding of an open program, or a
    /// script's global, may be referenced where this build cannot see.
    pub(super) fn binding_stays_closed(
        &self,
        binding_file: &FileFacts,
        name: Span,
        symbol: &SymbolId,
        file: &FileFacts,
        object: Span,
        consulted: &mut Vec<(String, Span)>,
    ) -> bool {
        let module_level = binding_file
            .ast
            .functions
            .iter()
            .all(|function| !function.body.contains(name));
        // An exported binding is referenced from other files, whose edits a
        // cached answer here would not see; a script's binding is global.
        if module_level
            && (!crate::execution_role::file_is_module(binding_file)
                || crate::execution_role::flow_binding_exported(binding_file, name))
        {
            return false;
        }
        let Some(references) = self.symbol_references_if_present(symbol.as_str()) else {
            return false;
        };
        references.iter().all(|reference| {
            let path = reference.path.as_ref();
            let span = Span::new(
                u32::try_from(reference.start_byte).unwrap_or(u32::MAX),
                u32::try_from(reference.end_byte).unwrap_or(u32::MAX),
            );
            if (path == binding_file.path.as_str() && span == name)
                || (path == file.path.as_str() && span == object)
            {
                return true;
            }
            let Some(reference_file) = self.file_by_path(path) else {
                return false;
            };
            // Handed as an argument to an exact project callee that only
            // reads members of it.
            if let Some((call, index)) =
                reference_file
                    .ast
                    .arguments_containing(span)
                    .find(|(call, index)| {
                        reference_file
                            .ast
                            .peel_ts_sugar_span(call.arguments[*index].span)
                            == span
                    })
                && !call.arguments[index].spread
                && self.parameter_only_member_accessed_consulting(
                    reference_file,
                    call,
                    index,
                    consulted,
                )
            {
                return true;
            }
            reference_file.ast.members.iter().any(|member| {
                reference_file.ast.peel_ts_sugar_span(member.object) == span
                    && reference_file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_err()
                    && !reference_file
                        .ast
                        .assignments
                        .iter()
                        .any(|assignment| assignment.target.contains(member.span))
            })
        })
    }

    /// ADR 0320: whether the callee of `call` uses its parameter `index` only
    /// as the object of non-computed member accesses it does not assign: it
    /// hands the caller's object to nothing that could replace a member
    /// before the member runs.
    ///
    /// Records every callee whose source the answer read.
    pub(crate) fn parameter_only_member_accessed_consulting(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        consulted: &mut Vec<(String, Span)>,
    ) -> bool {
        self.parameter_only_member_accessed_within(file, call, index, 4, consulted)
    }

    /// [`Self::parameter_only_member_accessed_consulting`], where the callee may also
    /// hand the parameter, whole, to another exact callee of which the same
    /// holds, `depth` calls deep.
    fn parameter_only_member_accessed_within(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        depth: usize,
        consulted: &mut Vec<(String, Span)>,
    ) -> bool {
        if depth == 0 {
            return false;
        }
        let Some((callee_file, callee, exact)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, self)
        else {
            return false;
        };
        consulted.push((callee_file.path.to_string(), callee.span));
        let Some(parameter) = callee.parameters.get(index) else {
            return false;
        };
        let [name] = parameter.names.as_slice() else {
            return false;
        };
        exact
            && parameter.shape == solid_facts::ast::BindingShape::Identifier
            && !(callee.rest_parameter && index + 1 == callee.parameters.len())
            && !binding_written(callee_file, name.span)
            && !callee_file.ast.identifiers.iter().any(|identifier| {
                callee.span.contains(identifier.span)
                    && matches!(
                        callee_file.source_text(identifier.span),
                        Some("arguments" | "eval")
                    )
            })
            && callee_file
                .ast
                .reference_declarations
                .iter()
                .filter(|(reference, declaration)| {
                    *declaration == name.span && *reference != name.span
                })
                .all(|(reference, _)| {
                    let forwarded = callee_file
                        .ast
                        .arguments_containing(*reference)
                        .find(|(inner, position)| {
                            callee_file
                                .ast
                                .peel_ts_sugar_span(inner.arguments[*position].span)
                                == *reference
                        })
                        .is_some_and(|(inner, position)| {
                            !inner.arguments[position].spread
                                && self.parameter_only_member_accessed_within(
                                    callee_file,
                                    inner,
                                    position,
                                    depth - 1,
                                    consulted,
                                )
                        });
                    forwarded
                        || callee_file.ast.members.iter().any(|member| {
                            callee_file.ast.peel_ts_sugar_span(member.object) == *reference
                                && callee_file
                                    .ast
                                    .computed_members
                                    .binary_search(&member.span)
                                    .is_err()
                                && !callee_file
                                    .ast
                                    .assignments
                                    .iter()
                                    .any(|assignment| assignment.target.contains(member.span))
                        })
                })
    }

    /// ADR 0314: the implementations `factory(…).property` may resolve to.
    ///
    /// The callee is one exact, current, synchronous project function called
    /// plainly (a method call is selected by its receiver), and every value it
    /// returns is an object literal written in the return itself -- a fresh
    /// object nothing else holds when the call returns. Each such literal must
    /// resolve the property; one that does not leaves the union incomplete,
    /// so nothing is resolved.
    fn factory_member_value_candidates(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        visited: &mut HashSet<(String, Span)>,
        consulted: &mut Vec<(String, Span)>,
    ) -> Option<Vec<SymbolId>> {
        let call = file.ast.call_at(object)?;
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if call.construct
            || self.is_member_span(file, callee)
            || file.ast.computed_members.binary_search(&callee).is_ok()
        {
            return None;
        }
        let (function_file, function) = self
            .callee_symbol(file, call.callee)
            .and_then(|symbol| self.function_for_symbol(symbol))
            .filter(|(function_file, function)| {
                self.function_value_is_current(function_file, function)
            })?;
        if function.r#async || function.generator {
            return None;
        }
        consulted.push((function_file.path.to_string(), function.span));
        let literal = |span: Span| {
            function_file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == span)
        };
        let body = function_file.ast.peel_ts_sugar_span(function.body);
        let returned = if literal(body) {
            vec![body]
        } else {
            let mut returned = Vec::new();
            for value in function_file.ast.own_returns(function) {
                returned.push(function_file.ast.peel_ts_sugar_span(value.argument?));
            }
            returned
        };
        if returned.is_empty() {
            return None;
        }
        let mut found = Vec::new();
        for value in returned {
            if !literal(value) {
                return None;
            }
            found.extend(self.member_value_candidates(
                function_file,
                value,
                property_name,
                visited,
                true,
                consulted,
            )?);
        }
        Some(found)
    }

    pub(super) fn binding_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::BindingFact)> {
        let resolution = self.bindings_by_symbol().get(symbol)?;
        let file = &self.facts.files[resolution.file];
        Some((file, &file.ast.bindings[resolution.binding]))
    }

    /// Bindings whose initializer can replace the binding's callable identity.
    ///
    /// Most call sites name an import, a function, a signal tuple slot, or the
    /// direct result of another call. Their entity symbol is already the exact
    /// runtime identity. Only identifier aliases, object destructures, and
    /// conditional initializers need the recursive value proof.
    pub(super) fn direct_value_aliases(&self) -> &HashSet<SymbolId> {
        self.direct_value_aliases.get_or_init(|| {
            let mut aliases = HashSet::default();
            for file in &self.facts.files {
                for binding in &file.ast.bindings {
                    let conditional_initializer = binding.initializer.is_some_and(|initializer| {
                        let initializer = file.ast.peel_ts_sugar_span(initializer);
                        file.ast
                            .conditional_expressions
                            .iter()
                            .any(|conditional| conditional.span == initializer)
                    });
                    if binding.initializer_identifier.is_none()
                        && binding.shape != solid_facts::ast::BindingShape::Object
                        && !conditional_initializer
                    {
                        continue;
                    }
                    for name in &binding.names {
                        if let Some(symbol) = self.entities.at(file.path.as_str(), name.span) {
                            aliases.insert(symbol.clone());
                        }
                    }
                }
            }
            aliases
        })
    }

    fn bindings_by_symbol(&self) -> &HashMap<SymbolId, BindingResolution> {
        self.bindings_by_symbol.get_or_init(|| {
            let mut bindings = HashMap::default();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (binding_index, binding) in file.ast.bindings.iter().enumerate() {
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        bindings.entry(symbol.clone()).or_insert(BindingResolution {
                            file: file_index,
                            binding: binding_index,
                            symbol: symbol.clone(),
                        });
                    }
                }
            }
            bindings
        })
    }

    pub(super) fn direct_value_symbols(
        &self,
        file: &FileFacts,
        value: Span,
        visited: &mut HashSet<Span>,
    ) -> Vec<SymbolId> {
        let value = file.ast.peel_ts_sugar_span(value);
        if !visited.insert(value) {
            return Vec::new();
        }
        if let Some(conditional) = file.ast.conditional_at(value) {
            let mut symbols = self.direct_value_symbols(file, conditional.consequent, visited);
            symbols.extend(self.direct_value_symbols(file, conditional.alternate, visited));
            symbols.sort_unstable();
            symbols.dedup();
            return symbols;
        }
        let direct_symbol = self.entities.at(file.path.as_str(), value);
        let identifier_reference = direct_symbol.is_none()
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == value
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            });
        let Some(symbol) = direct_symbol.cloned().or_else(|| {
            identifier_reference.then(|| {
                self.binding_at_reference(file.path.as_str(), value)
                    .map(|(_, _, symbol)| symbol)
            })?
        }) else {
            return Vec::new();
        };
        let Some((binding_file, binding)) = direct_symbol
            .and_then(|symbol| self.binding_for_symbol(symbol))
            .or_else(|| {
                identifier_reference.then(|| {
                    self.binding_at_reference(file.path.as_str(), value)
                        .map(|(binding_file, binding, _)| (binding_file, binding))
                })?
            })
        else {
            return vec![symbol.clone()];
        };
        if binding.initializer_function {
            return vec![symbol.clone()];
        }
        // A destructuring pattern is not an alias of its initializer. `const
        // { href } = props` binds `props.href`; `const [first] = pair` binds
        // `pair[0]`. Carrying the initializer's identity across the pattern
        // made every destructured local *be* the whole object, which is how
        // `@solidjs/router`'s `Navigate` published a callback claim saying the
        // props object itself is invoked -- a claim no adapter can verify,
        // because `href({ navigate })` invokes a member. Only an identifier
        // binding may inherit its initializer's callable identity; the object
        // slot below resolves a destructured property on its own evidence, and
        // a slot with no inspectable value stays this binding's own symbol so
        // the claim is never made.
        let inherits_initializer_identity =
            binding.shape == solid_facts::ast::BindingShape::Identifier;
        if inherits_initializer_identity && let Some(initializer) = &binding.initializer_identifier
        {
            let aliases = self.direct_value_symbols(binding_file, initializer.span, visited);
            if !aliases.is_empty() {
                return aliases;
            }
        }
        if binding.shape == solid_facts::ast::BindingShape::Object
            && let Some(initializer) = binding.initializer
            && let Some(slot) = binding.object_slots.iter().find(|slot| {
                self.entities
                    .at(binding_file.path.as_str(), slot.local.span)
                    .is_some_and(|candidate| candidate == &symbol)
            })
        {
            let mut aliases = Vec::new();
            let mut visited_objects = HashSet::default();
            self.member_value_symbols(
                binding_file,
                initializer,
                slot.property.as_str(),
                &mut visited_objects,
                &mut aliases,
            );
            aliases.sort_unstable();
            aliases.dedup();
            if !aliases.is_empty() {
                return aliases;
            }
        }
        if inherits_initializer_identity && let Some(initializer) = binding.initializer {
            let aliases = self.direct_value_symbols(binding_file, initializer, visited);
            if aliases.len() > 1 {
                return aliases;
            }
        }
        vec![symbol.clone()]
    }

    pub(super) fn resolved_declaration_symbol(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a str> {
        self.resolved_callee_call(file, callee)
            .and_then(|call| call.declaration.as_ref())
            .map(|declaration| declaration.symbol.as_ref())
            .filter(|symbol| !symbol.is_empty())
    }
}
