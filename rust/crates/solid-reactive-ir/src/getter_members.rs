//! Plain returned objects whose scalar getters perform reads (ADR 0272).
use super::*;
use solid_facts::ast::{BindingShape, IdentifierRole};
use solid_facts::core::Span;

impl LocalAccessContext<'_, '_> {
    fn getter_builtin_call(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<&str> {
        let declaration =
            self.lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)?;
        let (global, property, library) = match declaration.qualified_name.as_ref() {
            "ObjectConstructor.keys" => ("Object", "keys", "lib.es5.d.ts"),
            "ObjectConstructor.values" => ("Object", "values", "lib.es5.d.ts"),
            "ObjectConstructor.entries" => ("Object", "entries", "lib.es5.d.ts"),
            "ObjectConstructor.assign" => ("Object", "assign", "lib.es5.d.ts"),
            "ObjectConstructor.hasOwn" => ("Object", "hasOwn", "lib.es5.d.ts"),
            "Reflect.ownKeys" => ("Reflect", "ownKeys", "lib.es2015.reflect.d.ts"),
            "Reflect.has" => ("Reflect", "has", "lib.es2015.reflect.d.ts"),
            "JSON.stringify" => ("JSON", "stringify", "lib.es5.d.ts"),
            _ => return None,
        };
        let member = file.ast.members.iter().find(|member| {
            member.span == file.ast.peel_ts_sugar_span(call.callee)
                && !file.ast.computed_members.contains(&member.span)
                && file.source_text(member.property) == Some(property)
        })?;
        let receiver = file.ast.peel_ts_sugar_span(member.object);
        if file.ast.reference_declaration(receiver).is_some()
            || !file
                .ast
                .identifiers
                .iter()
                .any(|id| id.span == receiver && id.role == IdentifierRole::Reference)
        {
            return None;
        }
        let entity = self.lookup.entity_at(file.path.as_str(), receiver)?;
        if entity.symbol_unresolved || entity.symbol.is_empty() {
            return None;
        }
        let symbol = self.lookup.symbol_at(file.path.as_str(), receiver)?;
        let expected = std::path::Path::new(declaration.source_file.as_ref())
            .parent()?
            .join(library);
        // The signature and runtime receiver are independent identities.
        // A local/aliased/augmented ObjectConstructor is not the builtin.
        if !symbol.alias_target().is_empty()
            || symbol.declarations().iter().any(|binding| {
                self.lookup
                    .file_by_path(binding.location.path.as_ref())
                    .is_some()
            })
        {
            return None;
        }
        let bindings = symbol
            .declarations()
            .iter()
            .filter(|binding| {
                binding.name.as_ref() == global
                    && (global == "Reflect" || binding.kind.as_ref() == "variable")
            })
            .collect::<Vec<_>>();
        if bindings.len() != 1
            || std::path::Path::new(bindings[0].location.path.as_ref()) != expected
            || self.lookup.member_name_may_be_reassigned(property)
            || self.lookup.member_name_may_be_reassigned(global)
        {
            return None;
        }
        // Reuse the scalar-builtin proof's project-wide global escape/write veto.
        let current = self.lookup.files().iter().all(|other| {
            other.ast.identifiers.iter().all(|id| {
                if id.role != IdentifierRole::Reference
                    || other.ast.reference_declaration(id.span).is_some()
                    || other
                        .ast
                        .type_queries
                        .iter()
                        .any(|query| query.contains(id.span))
                {
                    return true;
                }
                let Some(reference) = self.lookup.entity_at(other.path.as_str(), id.span) else {
                    return false;
                };
                if reference.symbol_unresolved || reference.symbol.is_empty() {
                    return false;
                }
                if reference.symbol != entity.symbol {
                    return true;
                }
                if other
                    .ast
                    .calls
                    .iter()
                    .any(|call| other.ast.peel_ts_sugar_span(call.callee) == id.span)
                {
                    return true;
                }
                let Some(access) = other
                    .ast
                    .members
                    .iter()
                    .find(|member| other.ast.peel_ts_sugar_span(member.object) == id.span)
                else {
                    return false;
                };
                !other
                    .ast
                    .assignments
                    .iter()
                    .map(|assignment| assignment.target)
                    .chain(other.ast.deleted_targets.iter().copied())
                    .chain(other.ast.iteration_targets.iter().copied())
                    .any(|target| target.contains(access.span))
            })
        });
        current.then_some(declaration.qualified_name.as_ref())
    }

    fn getter_store_binding(
        &self,
        file: &solid_facts::FileFacts,
        root: Span,
        declaration: &Location,
        result: &mut LocalAccessResult,
    ) -> bool {
        let refers = |span| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(span))
                == Some(root)
        };
        let unsafe_use = crate::indexes::binding_written(file, root)
            || file.ast.identifiers.iter().any(|id| {
                id.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(id.span).is_none()
                    && file.source_text(id.span) == file.source_text(root)
                    && !file.ast.transparent_wrappers.iter().any(|wrapper| {
                        wrapper.span.contains(id.span) && !wrapper.inner.contains(id.span)
                    })
            })
            || file.ast.identifiers.iter().any(|id| {
                id.role == IdentifierRole::Reference
                    && refers(id.span)
                    && !file
                        .ast
                        .members
                        .iter()
                        .any(|member| file.ast.peel_ts_sugar_span(member.object) == id.span)
                    && !file.ast.transparent_wrappers.iter().any(|wrapper| {
                        wrapper.span.contains(id.span) && !wrapper.inner.contains(id.span)
                    })
            })
            || file
                .ast
                .assignments
                .iter()
                .map(|assignment| assignment.target)
                .chain(file.ast.deleted_targets.iter().copied())
                .chain(file.ast.iteration_targets.iter().copied())
                .any(|target| {
                    file.ast
                        .members
                        .iter()
                        .any(|member| target.contains(member.span) && refers(member.object))
                })
            || file
                .ast
                .exports
                .iter()
                .filter(|export| !export.type_only)
                .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
                .any(|export| export.local.span == root || refers(export.local.span));
        if unsafe_use {
            return false;
        }
        let mut exact = true;
        for member in &file.ast.members {
            if !refers(member.object) {
                continue;
            }
            if file.ast.computed_members.contains(&member.span) {
                exact = false;
            } else {
                self.getter_read(file, member.span, declaration, true, result);
            }
        }
        exact
    }

    fn getter_read(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        declaration: &Location,
        store: bool,
        result: &mut LocalAccessResult,
    ) {
        let allowed = allowed_callback_spans(file, self.lookup);
        let execution = semantic_execution_role(
            file,
            span,
            &allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        if execution == ExecutionRole::DiscardedRendering {
            return;
        }
        // Match the existing store/props timing boundary. A local helper's
        // definition does not prove the tracking context of its callers.
        let non_strict = matches!(
            execution,
            ExecutionRole::TrackedJsx
                | ExecutionRole::EventCallback
                | ExecutionRole::EffectApply
                | ExecutionRole::UntrackedCallback
        ) || execution == ExecutionRole::DeferredCallback
            && !host_callback_timing(file, span, execution, self.lookup)
            && !callee_callback_timing(file, span, execution, self.lookup);
        if (inside_non_component_function(file, span, self.lookup)
            || inside_unclassified_callback(file, span))
            && named_callback_execution_role(file, span, self.lookup).is_none()
            && direct_control_flow_body_role(
                file,
                span,
                self.entities,
                self.symbol_names,
                self.lookup.dialect,
            )
            .is_none()
            && !non_strict
        {
            result.dispatch_obligations.push(crate::StaticDefect {
                kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: "returned getter execution".into(),
                    member: None,
                },
                location: location(file.path.shared(), span),
                analysis_context:
                    "the helper or callback's getter execution context is not established".into(),
                fixes: vec![],
                uncertain: true,
            });
            return;
        }
        result.reads.push(Arc::new(ReactiveRead {
            package_internal: false,
            summary_attributed: false,
            kind: if store {
                "store-path"
            } else {
                "reactive-getter"
            }
            .into(),
            accessor: file.source_text(span).unwrap_or("returned getter").into(),
            location: location(file.path.shared(), span),
            declaration: declaration.clone(),
            execution,
            context: read_analysis_context(file, span, execution, self.lookup).into(),
            via: "returned object Get".into(),
            origin: Some(declaration.clone()),
            origin_context: "package member contract".into(),
            uncertain: self.lookup.inside_possible_component(file, span),
            missing_jsx_census: missing_jsx_census(file, span, execution),
            host_callback_timing: host_callback_timing(file, span, execution, self.lookup),
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: callee_callback_timing(file, span, execution, self.lookup),
        }));
        if counts_as_strict_read_root(file, span, execution, self.lookup) {
            result.strict_read_obligations += 1;
        }
    }

    pub(super) fn getter_member_accesses(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        let obligation = |span| {
            crate::StaticDefect {
            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                callee: "returned getter object".into(), member: None,
            },
            location: location(file.path.shared(), span),
            analysis_context: "returned object Get requires an exact unchanged receiver, member and execution; escape or reflection is unproven".into(),
            fixes: vec![], uncertain: true,
        }
        };
        for factory in &file.ast.calls {
            let Some((recipe, declaration)) = self
                .lookup
                .callee_symbol(file, factory.callee)
                .and_then(|symbol| self.contract_returns.get(symbol))
                .filter(|(returned, _)| returned.kind == crate::contracts::GETTER_OBJECT)
            else {
                continue;
            };
            let binding = file.ast.bindings.iter().find(|binding| {
                binding
                    .initializer
                    .is_some_and(|span| file.ast.peel_ts_sugar_span(span) == factory.span)
            });
            let reactive = recipe
                .properties
                .values()
                .any(|member| member.kind == "reactive-getter");
            if let Some(binding) = binding.filter(|binding| binding.shape == BindingShape::Object) {
                if binding.object_slots.len() != binding.names.len() || !binding.immutable {
                    result
                        .dispatch_obligations
                        .push(obligation(binding.pattern));
                    continue;
                }
                for slot in &binding.object_slots {
                    match recipe.properties.get(slot.property.as_str()) {
                        Some(member) if member.kind == "reactive-getter" => {
                            self.getter_read(file, binding.pattern, declaration, false, result)
                        }
                        Some(member) if member.kind == "plain" => {}
                        Some(member)
                            if member.kind == "store-path"
                                && self.getter_store_binding(
                                    file,
                                    slot.local.span,
                                    declaration,
                                    result,
                                ) => {}
                        _ => result
                            .dispatch_obligations
                            .push(obligation(binding.pattern)),
                    }
                }
                continue;
            }
            let root = binding
                .filter(|binding| {
                    binding.immutable
                        && binding.shape == BindingShape::Identifier
                        && binding.names.len() == 1
                })
                .map(|binding| binding.names[0].span);
            if binding.is_some() && root.is_none() {
                result.dispatch_obligations.push(obligation(factory.span));
                continue;
            }
            let refers = |span| {
                let span = file.ast.peel_ts_sugar_span(span);
                root.map_or(span == factory.span, |root| {
                    file.ast.reference_declaration(span) == Some(root)
                })
            };
            // Classify only exact standard-library calls. Shadowed methods and
            // ambiguous/computed dispatch cannot establish traversal or purity.
            let traversal = |call: &solid_facts::ast::CallFact| {
                if call.construct
                    || call.arguments.iter().any(|argument| argument.spread)
                    || !file.ast.straight_line_calls.contains(&call.span)
                {
                    return None;
                }
                match self.getter_builtin_call(file, call)? {
                    "ObjectConstructor.keys" | "Reflect.ownKeys" if call.arguments.len() == 1 => {
                        Some(false)
                    }
                    "ObjectConstructor.values" | "ObjectConstructor.entries" | "JSON.stringify"
                        if call.arguments.len() == 1 =>
                    {
                        Some(true)
                    }
                    "Reflect.has" | "ObjectConstructor.hasOwn"
                        if call.arguments.len() == 2
                            && matches!(
                                call.arguments[1].runtime_value_kind,
                                solid_facts::ast::RuntimeValueKind::Primitive
                            ) =>
                    {
                        Some(false)
                    }
                    "ObjectConstructor.assign"
                        if call.arguments.len() == 2
                            && call.arguments[0].exact_object_literal
                            && call.arguments[0].property_names.is_empty() =>
                    {
                        Some(true)
                    }
                    _ => None,
                }
            };
            let destructures = file
                .ast
                .bindings
                .iter()
                .filter(|binding| {
                    binding.shape == BindingShape::Object && binding.initializer.is_some_and(refers)
                })
                .collect::<Vec<_>>();
            let spreads = file
                .ast
                .spreads
                .iter()
                .filter(|spread| {
                    refers(spread.argument)
                        && file
                            .ast
                            .object_get_shapes
                            .iter()
                            .any(|shape| shape.span.contains(spread.span))
                })
                .collect::<Vec<_>>();
            let calls = file
                .ast
                .calls
                .iter()
                .filter(|call| {
                    call.arguments.iter().enumerate().any(|(index, argument)| {
                        refers(argument.span)
                            && (index == 0 || index == 1 && call.arguments.len() == 2)
                    }) && traversal(call).is_some()
                })
                .collect::<Vec<_>>();
            let mutated = root.is_some_and(|root| crate::indexes::binding_written(file, root))
                || file
                    .ast
                    .assignments
                    .iter()
                    .map(|assignment| assignment.target)
                    .chain(file.ast.deleted_targets.iter().copied())
                    .chain(file.ast.iteration_targets.iter().copied())
                    .any(|target| {
                        file.ast
                            .members
                            .iter()
                            .any(|member| target.contains(member.span) && refers(member.object))
                    });
            let escaped = root.is_some_and(|root| {
                file.ast
                    .exports
                    .iter()
                    .filter(|export| !export.type_only)
                    .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
                    .any(|export| export.local.span == root || refers(export.local.span))
                    || file.ast.identifiers.iter().any(|id| {
                        if id.role != IdentifierRole::Reference
                            || file.ast.transparent_wrappers.iter().any(|wrapper| {
                                wrapper.span.contains(id.span) && !wrapper.inner.contains(id.span)
                            })
                        {
                            return false;
                        }
                        if file.ast.reference_declaration(id.span).is_none()
                            && file.source_text(id.span) == file.source_text(root)
                        {
                            return true;
                        }
                        refers(id.span)
                            && !file
                                .ast
                                .members
                                .iter()
                                .any(|member| file.ast.peel_ts_sugar_span(member.object) == id.span)
                            && !destructures.iter().any(|binding| {
                                binding
                                    .initializer
                                    .is_some_and(|span| span.contains(id.span))
                            })
                            && !spreads
                                .iter()
                                .any(|spread| spread.argument.contains(id.span))
                            && !calls.iter().any(|call| {
                                call.arguments
                                    .iter()
                                    .any(|argument| argument.span.contains(id.span))
                            })
                    })
            });
            if mutated || escaped {
                result.dispatch_obligations.push(obligation(factory.span));
            }
            let mut modeled = root.is_some();
            for member in &file.ast.members {
                if !refers(member.object) {
                    continue;
                }
                modeled = true;
                if file.ast.computed_members.contains(&member.span) || mutated || escaped {
                    result.dispatch_obligations.push(obligation(member.span));
                    continue;
                }
                let key = file.source_text(member.property).unwrap_or_default();
                match recipe.properties.get(key) {
                    Some(value) if value.kind == "reactive-getter" => {
                        self.getter_read(file, member.span, declaration, false, result)
                    }
                    Some(value) if value.kind == "plain" => {}
                    Some(value) if value.kind == "store-path" => {
                        // Retrieving the data property is inert; a Get on its
                        // returned proxy reads reactively. No whole-object brand.
                        let nested = file
                            .ast
                            .members
                            .iter()
                            .filter(|nested| {
                                file.ast.peel_ts_sugar_span(nested.object) == member.span
                            })
                            .collect::<Vec<_>>();
                        for nested in &nested {
                            if file.ast.computed_members.contains(&nested.span) {
                                result.dispatch_obligations.push(obligation(nested.span));
                            } else {
                                self.getter_read(file, nested.span, declaration, true, result);
                            }
                        }
                        // Other transports of the proxy retain an obligation.
                        let bindings = file
                            .ast
                            .bindings
                            .iter()
                            .filter(|binding| {
                                binding.initializer.is_some_and(|span| {
                                    file.ast.peel_ts_sugar_span(span) == member.span
                                })
                            })
                            .collect::<Vec<_>>();
                        let retained = bindings.iter().any(|binding| {
                            !binding.immutable
                                || binding.shape != BindingShape::Identifier
                                || binding.names.len() != 1
                                || !self.getter_store_binding(
                                    file,
                                    binding.names[0].span,
                                    declaration,
                                    result,
                                )
                        });
                        if retained
                            || (nested.is_empty() && bindings.is_empty())
                            || file.ast.calls.iter().any(|call| {
                                call.arguments.iter().any(|argument| {
                                    file.ast.peel_ts_sugar_span(argument.span) == member.span
                                })
                            })
                        {
                            result.dispatch_obligations.push(obligation(member.span));
                        }
                    }
                    _ => result.dispatch_obligations.push(obligation(member.span)),
                }
            }
            for binding in destructures {
                modeled = true;
                if mutated || escaped || binding.object_slots.len() != binding.names.len() {
                    result
                        .dispatch_obligations
                        .push(obligation(binding.pattern));
                    continue;
                }
                for slot in &binding.object_slots {
                    match recipe.properties.get(slot.property.as_str()) {
                        Some(member) if member.kind == "reactive-getter" => {
                            self.getter_read(file, binding.pattern, declaration, false, result)
                        }
                        Some(member) if member.kind == "plain" => {}
                        Some(member)
                            if member.kind == "store-path"
                                && binding.immutable
                                && self.getter_store_binding(
                                    file,
                                    slot.local.span,
                                    declaration,
                                    result,
                                ) => {}
                        _ => result
                            .dispatch_obligations
                            .push(obligation(binding.pattern)),
                    }
                }
            }
            for span in spreads.iter().map(|spread| spread.span).chain(
                calls
                    .iter()
                    .filter(|call| traversal(call) == Some(true))
                    .map(|call| call.span),
            ) {
                modeled = true;
                if mutated || escaped {
                    result.dispatch_obligations.push(obligation(span));
                } else if reactive {
                    self.getter_read(file, span, declaration, false, result);
                }
            }
            modeled |= !calls.is_empty();
            // A copy runs the getters but preserves data members' values.
            // Until those copied proxies have provenance, allow only exact
            // scalar-result uses; transporting the copy is an obligation.
            let scalar_copy_use = |span| {
                let scalar = |member: &solid_facts::ast::MemberFact| {
                    !file.ast.computed_members.contains(&member.span)
                        && recipe
                            .properties
                            .get(file.source_text(member.property).unwrap_or_default())
                            .is_some_and(|value| {
                                matches!(value.kind.as_str(), "plain" | "reactive-getter")
                            })
                };
                if file.ast.members.iter().any(|member| {
                    file.ast.peel_ts_sugar_span(member.object) == span && scalar(member)
                }) {
                    return true;
                }
                let Some(binding) = file.ast.bindings.iter().find(|binding| {
                    binding.immutable
                        && binding.shape == BindingShape::Identifier
                        && binding.names.len() == 1
                        && binding
                            .initializer
                            .is_some_and(|initial| file.ast.peel_ts_sugar_span(initial) == span)
                }) else {
                    return false;
                };
                let root = binding.names[0].span;
                !crate::indexes::binding_written(file, root)
                    && !file
                        .ast
                        .exports
                        .iter()
                        .filter(|export| !export.type_only)
                        .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
                        .any(|export| {
                            export.local.span == root
                                || file.ast.reference_declaration(export.local.span) == Some(root)
                        })
                    && !file.ast.identifiers.iter().any(|id| {
                        id.role == IdentifierRole::Reference
                            && file.ast.reference_declaration(id.span).is_none()
                            && file.source_text(id.span) == file.source_text(root)
                    })
                    && file
                        .ast
                        .identifiers
                        .iter()
                        .filter(|id| {
                            id.role == IdentifierRole::Reference
                                && file.ast.reference_declaration(id.span) == Some(root)
                        })
                        .all(|id| {
                            file.ast.members.iter().any(|member| {
                                file.ast.peel_ts_sugar_span(member.object) == id.span
                                    && scalar(member)
                            })
                        })
            };
            for spread in &spreads {
                if let Some(shape) = file
                    .ast
                    .object_get_shapes
                    .iter()
                    .find(|shape| shape.span.contains(spread.span))
                    && !scalar_copy_use(shape.span)
                {
                    result.dispatch_obligations.push(obligation(shape.span));
                }
            }
            for call in &calls {
                let kind = self.getter_builtin_call(file, call);
                if matches!(
                    kind,
                    Some("ObjectConstructor.values" | "ObjectConstructor.entries")
                ) || kind == Some("ObjectConstructor.assign") && !scalar_copy_use(call.span)
                {
                    result.dispatch_obligations.push(obligation(call.span));
                }
            }
            if !modeled && !factory.result_discarded {
                result.dispatch_obligations.push(obligation(factory.span));
            }
        }
    }
}
