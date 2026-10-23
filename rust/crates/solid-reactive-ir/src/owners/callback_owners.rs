//! Callback owners: the owner and execution at a callback call, returned callbacks, and their invocation sites.

use super::*;

/// The callback execution fact for one concrete primitive call, after proving
/// any lazy returned adapter is actually consumed.
///
/// Solid 1.x `mapArray` and `indexArray` merely allocate and return a function;
/// their list and mapper arguments run only when that returned function runs.
/// The dialect owns that runtime distinction, while this helper proves the
/// use from canonical TypeScript symbols and the enclosing AST call shape.
pub(crate) fn callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    if crate::effect_execution::at_call(file, call, primitive, argument, lookup)
        != crate::effect_execution::ApplyExecution::Proven
    {
        return None;
    }
    let semantics = lookup
        .dialect
        .callback_semantics_at(primitive, argument, call.arguments.len());
    if semantics.requires_return_invocation
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    semantics.execution
}

/// The callback-owner fact for one concrete primitive call, after proving
/// that callbacks implemented by a returned lazy adapter can run.
///
/// Execution, reachability, cleanup, and both owner-graph passes must ask the
/// same call-site question. Otherwise a discarded adapter can disappear from
/// read analysis while still manufacturing an impossible owner diagnostic.
pub(crate) fn callback_owner_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Primitive,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    use crate::effect_execution::ApplyExecution;
    match crate::effect_execution::at_call(file, call, primitive, argument, lookup) {
        ApplyExecution::Absent => return None,
        ApplyExecution::Unknown => return Some(solid_dialect::CallbackOwner::Conditional),
        ApplyExecution::Proven => {}
    }
    if lookup
        .dialect
        .effect_apply_defer_options(primitive, argument)
        .is_some_and(|options| {
            crate::call_option_literal(file, call, options, "defer")
                == solid_dialect::OptionLiteral::True
        })
    {
        // A deferred apply has no inherited first run, including render effects.
        return Some(solid_dialect::CallbackOwner::None);
    }
    let semantics = lookup
        .dialect
        .callback_semantics_at(primitive, argument, call.arguments.len());
    if semantics.requires_return_invocation
        && !returned_callback_result_is_invoked(file, call, lookup)
    {
        return None;
    }
    if primitive == Primitive::RunWithOwner && argument == 1 {
        return run_with_owner_callback_owner(file, call, lookup);
    }
    semantics.owner
}

/// `runWithOwner` is the one ownership primitive whose owner is data. Both
/// dialects accept `Owner | null`; 2.0 makes the null spelling the documented
/// way to detach a root. Preserve a definite answer where the call site proves
/// one and keep a conditional edge everywhere else so certification cannot
/// silently assume a nullable owner exists.
pub(crate) fn run_with_owner_callback_owner(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::CallbackOwner> {
    let owner = call.arguments.first()?;
    if owner.value == solid_facts::ast::ArgumentValueKind::Undefined
        || owner.value == solid_facts::ast::ArgumentValueKind::Null
    {
        return Some(solid_dialect::CallbackOwner::None);
    }
    // `void expression` has no dedicated AST argument kind yet. This text
    // check is conservative (it can only detach an owner) and remains local
    // to the one syntax shape not represented by the fact protocol.
    let source = file.source_text(owner.span)?.trim();
    if source.starts_with("void ") {
        return Some(solid_dialect::CallbackOwner::None);
    }

    // 2.0's createOwner() is non-null by construction. Resolve the nested
    // call semantically so a local function with the same spelling proves
    // nothing.
    if lookup.primitive_at_call(file, owner.span) == Some(Primitive::CreateOwner) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }
    if component_owner_binding(file, owner.span, lookup) {
        return Some(solid_dialect::CallbackOwner::Creates);
    }

    if let Some(descriptor) = lookup
        .entity_at(file.path.as_str(), owner.span)
        .and_then(|entity| entity.type_descriptor.as_deref())
    {
        // TypeScript may preserve a user alias's name rather than render its
        // nullable expansion, so absence of the word `null` is not proof.
        // The compiler-resolved alias declaration and origin module must be
        // the dialect's own Owner export; a user-local type with the same
        // spelling has no such role. Nullable/union types remain Creates here
        // only when the selected type itself is the Solid Owner branch, and
        // the existing call-site analysis keeps unresolved values conditional.
        if descriptor.alias_declarations.iter().any(|declaration| {
            lookup
                .dialect
                .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                == Some(solid_dialect::TypeRole::Owner)
        }) {
            return Some(solid_dialect::CallbackOwner::Creates);
        }
    }

    Some(solid_dialect::CallbackOwner::Conditional)
}

/// ADR 0206: whether `value` names a `const` bound to `getOwner()` written
/// directly in a proven component's body.
///
/// A proven component's body runs under an owner: the owner graph seeds it
/// owned (`owner_node`), as its render does. `getOwner()` called directly
/// there, not in a nested function or a default, therefore returns that
/// owner, never `null`, and a `const` keeps it. The owner may have been
/// disposed by the time a later callback hands it to `runWithOwner`; that is
/// not the null owner this question is about.
pub(super) fn component_owner_binding(
    file: &solid_facts::FileFacts,
    value: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let value = file.ast.peel_ts_sugar_span(value);
    let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), value)
    else {
        return false;
    };
    let Some(initializer) = binding.call_initializer else {
        return false;
    };
    if !binding.immutable
        || binding.shape != solid_facts::ast::BindingShape::Identifier
        || lookup.primitive_at_call(binding_file, initializer) != Some(Primitive::GetOwner)
    {
        return false;
    }
    let Some(component) = containing_ast_function(&binding_file.ast, initializer) else {
        return false;
    };
    !component.r#async
        && !component.generator
        && written_directly_in(&binding_file.ast, component, initializer)
        && lookup.function_component_status(binding_file, component)
            == crate::indexes::ComponentStatus::Proven
}

/// Resolve the primitive whose returned function is the callee of `call`.
///
/// Canonical TypeScript symbols prove identity, AST binding facts prove the
/// initializer/alias chain, and the dialect's resolved primitive index proves
/// that the initializer is the actual Solid export rather than a same-spelled
/// local function.
pub(crate) fn returned_primitive_invocation(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Option<(Primitive, Option<usize>)> {
    let mut reference_path = file.path.as_str();
    let mut reference_span = call.callee;
    let mut seen = HashSet::default();
    loop {
        // Callee result values do not always receive an entity fact of their
        // own (tuple slots are the motivating case). Resolve them from the
        // canonical reference set of each demanded binding declaration, then
        // follow local or cross-file aliases one binding at a time.
        let (binding_file, binding, symbol) =
            lookup.binding_at_reference(reference_path, reference_span)?;
        if !seen.insert(symbol.clone()) {
            return None;
        }
        if let Some(initializer) = binding.call_initializer {
            let call_index = lookup.call_index(binding_file, initializer)?;
            let result_slot = match binding.shape {
                solid_facts::ast::BindingShape::Array => {
                    Some(binding.array_slots.iter().position(|slot| {
                        slot.as_ref().and_then(|name| {
                            lookup.entities().at(binding_file.path.as_str(), name.span)
                        }) == Some(&symbol)
                    })?)
                }
                _ => None,
            };
            return known_primitive(&lookup.primitives(binding_file).calls[call_index])
                .map(|primitive| (primitive, result_slot));
        }
        let initializer = binding.initializer_identifier.as_ref()?;
        reference_path = binding_file.path.as_str();
        reference_span = initializer.span;
    }
}

/// The execution semantics of one callback argument accepted by a proven
/// returned function (including a specific destructured tuple slot).
pub(crate) fn returned_callback_execution_at_call(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    argument: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<solid_dialect::Execution> {
    // Solid 2.0 answers `None` for every returned-callback question, so the
    // binding-chain resolution below cannot produce an answer there. Skip it
    // rather than walk it once per call and discard the result.
    if !lookup.models_returned_callbacks() {
        return None;
    }
    let (primitive, result_slot) = returned_primitive_invocation(file, call, lookup)?;
    lookup
        .dialect
        .returned_callback_semantics_at(primitive, result_slot, argument, call.arguments.len())
        .execution
}

pub(crate) fn returned_callback_result_is_invoked(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    !returned_callback_invocation_sites_memo(file, factory_call, lookup).is_empty()
}

/// Proven project invocations of the function returned by `factory_call`.
///
/// TypeScript symbols establish that calls, JSX tags, and `.preload()` uses
/// refer to the exact factory result. AST facts establish how the value is
/// consumed. For values passed to another known callback-taking primitive,
/// the invocation inherits that callback's owner; direct calls and
/// JSX uses inherit their containing function's owner.
pub(crate) fn returned_callback_invocation_sites(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ReturnedCallbackInvocationSite> {
    returned_callback_invocation_sites_memo(file, factory_call, lookup)
        .as_ref()
        .clone()
}

/// [`returned_callback_invocation_sites`], computed once per factory call: the
/// compute scans every file's calls, and execution, owner and standing passes
/// each ask about the same call.
fn returned_callback_invocation_sites_memo(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> std::sync::Arc<Vec<ReturnedCallbackInvocationSite>> {
    lookup.returned_invocation_sites_memo(file.path.as_str(), factory_call.span, || {
        compute_returned_callback_invocation_sites(file, factory_call, lookup)
    })
}

fn compute_returned_callback_invocation_sites(
    file: &solid_facts::FileFacts,
    factory_call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<ReturnedCallbackInvocationSite> {
    let direct_factory_value = |argument: Span| {
        file.ast
            .calls_within(argument)
            .max_by_key(|nested| nested.span.end - nested.span.start)
            .is_some_and(|nested| nested.span == factory_call.span)
    };
    let mut sites = Vec::new();
    for outer in file
        .ast
        .calls_with_callee_containing(factory_call.span)
        .filter(|outer| {
            outer.span != factory_call.span
            && file
                .ast
                .calls_within(outer.callee)
                .filter(|nested| nested.span != outer.span)
                .max_by_key(|nested| nested.span.end - nested.span.start)
                .is_some_and(|nested| nested.span == factory_call.span)
            // `factory(...).member()` invokes the member, not the returned
            // function, so it proves nothing about the factory's callbacks.
            // `preload` is the one member the runtime routes to the loader,
            // matching the binding-based preload proof below.
            && lookup
                .member_property_at(file, outer.callee)
                .is_none_or(|property| file.source_text(property) == Some("preload"))
        })
    {
        sites.push(ReturnedCallbackInvocationSite {
            path: file.path.to_string(),
            span: outer.span,
            inherited_execution: None,
            inherited_owner: OwnerEdgeKind::Preserve,
        });
    }

    for (outer, index) in file.ast.arguments_containing(factory_call.span) {
        let primitive = lookup.primitive_at_call(file, outer.span);
        if direct_factory_value(outer.arguments[index].span)
            && primitive.is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, outer.arguments.len())
                    .execution
                    .is_some()
            })
        {
            let inherited_owner = primitive
                .and_then(|primitive| {
                    callback_owner_at_call(file, outer, primitive, index, lookup)
                        .map(|owner| callback_owner_edge_kind_at(lookup, primitive, owner))
                })
                .unwrap_or(OwnerEdgeKind::Preserve);
            sites.push(ReturnedCallbackInvocationSite {
                path: file.path.to_string(),
                span: outer.span,
                inherited_execution: primitive.and_then(|primitive| {
                    lookup
                        .dialect
                        .callback_semantics_at(primitive, index, outer.arguments.len())
                        .execution
                }),
                inherited_owner,
            });
        }
    }

    let mut factory_bindings = file
        .ast
        .bindings
        .iter()
        .filter(|binding| binding.call_initializer == Some(factory_call.span))
        .flat_map(|binding| &binding.names)
        .filter_map(|name| lookup.entities().at(file.path.as_str(), name.span).cloned())
        .collect::<HashSet<_>>();
    if factory_bindings.is_empty() {
        sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
        sites.dedup();
        return sites;
    }

    // Follow identifier aliases in every project file. The compiler's symbol
    // reference table bridges imports/re-exports; local AST bindings bridge a
    // fresh `const Alias = Imported` symbol introduced after that boundary.
    loop {
        let references = returned_binding_references(lookup, &factory_bindings);
        let aliases = lookup
            .files()
            .iter()
            .flat_map(|candidate| {
                candidate
                    .ast
                    .bindings
                    .iter()
                    .filter_map(|binding| {
                        let initializer = binding.initializer_identifier.as_ref()?;
                        returned_binding_reference(
                            candidate,
                            initializer.span,
                            lookup,
                            &factory_bindings,
                            &references,
                        )
                        .then_some(binding)
                    })
                    .flat_map(move |binding| {
                        binding.names.iter().filter_map(|name| {
                            lookup
                                .entities()
                                .at(candidate.path.as_str(), name.span)
                                .cloned()
                        })
                    })
            })
            .collect::<Vec<_>>();
        let before = factory_bindings.len();
        factory_bindings.extend(aliases);
        if factory_bindings.len() == before {
            break;
        }
    }

    // Group the reference locations by file once, so each file's test of a
    // span is two lookups with no allocation (`returned_binding_reference`
    // builds an owned path key for every span it is asked about).
    let mut references_by_path: HashMap<&str, HashSet<(u64, u64)>> = HashMap::default();
    let references = returned_binding_references(lookup, &factory_bindings);
    for (path, start, end) in &references {
        references_by_path
            .entry(path.as_str())
            .or_default()
            .insert((*start, *end));
    }
    for use_file in lookup.files() {
        let file_entities = lookup.entities().by_path.get(use_file.path.as_str());
        let file_references = references_by_path.get(use_file.path.as_str());
        let returned = |span: Span| {
            let key = (u64::from(span.start), u64::from(span.end));
            file_entities
                .and_then(|entities| entities.get(&key))
                .is_some_and(|symbol| factory_bindings.contains(symbol))
                || file_references.is_some_and(|references| references.contains(&key))
        };
        for outer in &use_file.ast.calls {
            if returned(outer.callee) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: outer.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
            for (index, argument) in outer.arguments.iter().enumerate() {
                if !returned(argument.span) {
                    continue;
                }
                let primitive = lookup.primitive_at_call(use_file, outer.span);
                if primitive.is_some_and(|primitive| {
                    lookup
                        .dialect
                        .callback_semantics_at(primitive, index, outer.arguments.len())
                        .execution
                        .is_some()
                }) {
                    let inherited_owner = primitive
                        .and_then(|primitive| {
                            callback_owner_at_call(use_file, outer, primitive, index, lookup)
                                .map(|owner| callback_owner_edge_kind_at(lookup, primitive, owner))
                        })
                        .unwrap_or(OwnerEdgeKind::Preserve);
                    sites.push(ReturnedCallbackInvocationSite {
                        path: use_file.path.to_string(),
                        span: outer.span,
                        inherited_execution: primitive.and_then(|primitive| {
                            lookup
                                .dialect
                                .callback_semantics_at(primitive, index, outer.arguments.len())
                                .execution
                        }),
                        inherited_owner,
                    });
                }
            }

            // `lazyResult.preload()` resolves the property as a distinct
            // method; prove the receiver from the member-expression AST.
            if use_file.ast.members_within(outer.callee).any(|member| {
                use_file.source_text(member.property) == Some("preload") && returned(member.object)
            }) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: outer.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
        }

        for element in &use_file.ast.jsx_elements {
            if returned(element.name.span) {
                sites.push(ReturnedCallbackInvocationSite {
                    path: use_file.path.to_string(),
                    span: element.span,
                    inherited_execution: None,
                    inherited_owner: OwnerEdgeKind::Preserve,
                });
            }
        }
    }

    sites.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
    sites.dedup();
    sites
}
