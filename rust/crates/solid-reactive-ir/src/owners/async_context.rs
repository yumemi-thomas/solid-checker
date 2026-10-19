//! Async and loading-cover context: Loading boundaries, async reads, and non-component function containment.

use super::*;

/// Whether a `Loading` boundary above a render position is proven, proven
/// absent, or cannot be resolved from the analyzed project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoadingCover {
    /// A boundary encloses the position, directly or through the call sites
    /// that render the function containing it.
    Covered,
    /// Every call-site chain was followed to a mount root (a `render` or
    /// `hydrate` argument) and none passed through a boundary.
    Uncovered,
    /// A chain ends at a function nothing in the project renders -- an
    /// exported component, a route handed to a router, a lazy page -- so the
    /// boundary above it is decided by code the analysis cannot see. Missing
    /// is not absent.
    Unresolved,
}

pub(crate) fn read_loading_cover(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> LoadingCover {
    loading_cover_at(lookup, file, span, symbol_names, &mut HashSet::new())
}

pub(super) fn loading_cover_at(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    visited: &mut HashSet<(String, Span)>,
) -> LoadingCover {
    let entities = lookup.entities();
    if file.ast.jsx_containing(span).any(|element| {
        jsx_element_is_loading(file, element, entities, symbol_names, lookup.dialect)
    }) {
        return LoadingCover::Covered;
    }
    if file.ast.jsx_containing(span).any(|element| {
        jsx_target_function(lookup, file, element).is_some_and(|(target_file, target)| {
            target_file.ast.jsx_within(target.body).any(|candidate| {
                jsx_element_is_loading(
                    target_file,
                    candidate,
                    entities,
                    symbol_names,
                    lookup.dialect,
                )
            })
        })
    }) {
        return LoadingCover::Covered;
    }
    // A wrapper the site is a child of, with no `Loading` of its own, that
    // reads its `children`: it may hand them to another component that has
    // one (`<Layout><Loading>{props.children}</Loading></Layout>` behind a
    // `Shell`), which this walk does not follow. Not proven uncovered (review
    // round 4).
    if file.ast.jsx_containing(span).any(|element| {
        element.children.iter().any(|child| child.contains(span))
            && jsx_target_function(lookup, file, element).is_some_and(|(target_file, target)| {
                target_file.ast.members.iter().any(|member| {
                    target.body.contains(member.span)
                        && target_file.source_text(member.property) == Some("children")
                })
            })
    }) {
        return LoadingCover::Unresolved;
    }
    // Walk outward from the innermost function. A closure nothing in the
    // project calls (a `<For>` row callback, a render prop) is part of the
    // function around it; the first function the project does render or call
    // is the one whose call sites decide.
    let mut enclosing = file.ast.functions_body_containing(span).collect::<Vec<_>>();
    enclosing.sort_by_key(|function| function.body.end - function.body.start);
    for function in enclosing {
        if !visited.insert((file.path.as_str().to_owned(), function.span)) {
            // Already being resolved on this chain: it adds no information.
            return LoadingCover::Uncovered;
        }
        let call_sites = lookup.function_call_sites(file.path.as_str(), function.span);
        if !call_sites.is_empty() {
            if file.ast.jsx_within(function.body).any(|candidate| {
                jsx_element_is_loading(file, candidate, entities, symbol_names, lookup.dialect)
            }) {
                return LoadingCover::Covered;
            }
            let mut result = LoadingCover::Uncovered;
            for (caller_file, callee) in call_sites {
                match loading_cover_at(lookup, caller_file, callee, symbol_names, visited) {
                    LoadingCover::Covered => return LoadingCover::Covered,
                    LoadingCover::Unresolved => result = LoadingCover::Unresolved,
                    LoadingCover::Uncovered => {}
                }
            }
            return result;
        }
        if function_is_mount_root(lookup, file, function) {
            return LoadingCover::Uncovered;
        }
    }
    LoadingCover::Unresolved
}

/// Whether `function` is what a `render`/`hydrate` call mounts: the function
/// literal passed as its first argument, or a function it names.
pub(super) fn function_is_mount_root(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.ast.calls.iter().any(|call| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| lookup.dialect.mounts_component_tree(primitive))
            && call.arguments.first().is_some_and(|argument| {
                let argument = file.ast.peel_ts_sugar_span(argument.span);
                argument == function.span
                    || lookup
                        .function_called_at(file.path.as_str(), argument)
                        .is_some_and(|(target_file, target)| {
                            target_file.path == file.path && target.span == function.span
                        })
            })
    })
}

/// Whether a compiler ref/directive application role at `span` belongs to an
/// intrinsic element. The runtime applies an intrinsic element's `ref` through
/// `ref()` with no owner; a component (a capitalized or member tag) receives
/// `ref` as an ordinary prop and invokes it whenever its own code does, often
/// in its owned body. By JSX semantics a lowercase, non-member tag is
/// intrinsic, so a callback written as an attribute value of any other element
/// is not a proven ownerless application.
pub(crate) fn ref_application_on_intrinsic(file: &solid_facts::FileFacts, span: Span) -> bool {
    !file.ast.jsx_elements.iter().any(|element| {
        let tag = file
            .source
            .get(element.name.span.start as usize..element.name.span.end as usize)
            .unwrap_or_default();
        let component = tag.contains('.') || tag.starts_with(|c: char| c.is_ascii_uppercase());
        component
            && element.attributes.iter().any(|attribute| {
                attribute
                    .expression
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value).contains(span))
            })
    })
}

/// The role a pending async read plays for the missing-boundary claim: a
/// tracked read inside a non-render computation (a memo, a user effect's
/// compute, a derived signal) is not a render of the value, so it is not
/// [`ExecutionRole::TrackedJsx`] for that claim. The pending state flows on to
/// whichever render effect consumes the computation, and that consumer is the
/// one whose boundary matters.
pub(crate) fn async_read_role(
    file: &solid_facts::FileFacts,
    span: Span,
    role: crate::ExecutionRole,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> crate::ExecutionRole {
    if role != crate::ExecutionRole::TrackedJsx {
        return role;
    }
    // A function that is a JSX attribute's value (an event handler, a callback
    // prop) runs when something calls it, not while the JSX renders. Nothing
    // proves a read inside it is rendered, so it is not a render read. A
    // `children` attribute is the element's own children and keeps its role.
    if file.ast.functions_body_containing(span).any(|function| {
        file.ast.jsx_containing(function.span).any(|element| {
            element.attributes.iter().any(|attribute| {
                attribute.value_kind == solid_facts::ast::JsxAttributeValueKind::Expression
                    && attribute
                        .expression
                        .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == function.span)
                    && file
                        .source
                        .get(attribute.name.start as usize..attribute.name.end as usize)
                        != Some("children")
            })
        })
    }) {
        return crate::ExecutionRole::Unknown;
    }
    let dialect = lookup.dialect;
    let in_computation = file.ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        matches!(
            argument.value,
            solid_facts::ast::ArgumentValueKind::Identifier
                | solid_facts::ast::ArgumentValueKind::Function
                | solid_facts::ast::ArgumentValueKind::AsyncFunction
        ) && call_primitive_name(file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                callback_execution_at_call(file, call, primitive, index, lookup).is_some()
                    && dialect
                        .callback_semantics_at(primitive, index, call.arguments.len())
                        .tracks_reads
                    && !dialect.computation_read_is_render(primitive)
            })
            // A JSX region written inside the computation is a render again.
            && !file
                .compiler
                .tracked_regions
                .iter()
                .any(|region| argument.span.contains(region.span) && region.span.contains(span))
    });
    if in_computation {
        crate::ExecutionRole::Unknown
    } else {
        role
    }
}

pub(crate) fn jsx_element_is_loading(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    dialect: &dyn Dialect,
) -> bool {
    jsx_primitive_name(file, element, entities, symbol_names, dialect)
        .as_deref()
        .is_some_and(|tag| dialect.is_async_boundary(tag))
}

pub(crate) fn jsx_target_function<'a>(
    lookup: &SemanticLookup<'a>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Option<(
    &'a solid_facts::FileFacts,
    &'a solid_facts::ast::FunctionFact,
)> {
    lookup.function_called_at(file.path.as_str(), element.name.span)
}

pub(crate) fn computation_is_async(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    argument: Span,
) -> bool {
    // The computation is the argument's own function. An async function that
    // is merely nested inside it (`createMemo(() => async () => ...)`, an async
    // IIFE) is a value the computation returns or runs, not the computation:
    // the memo's result is a function and can never be pending.
    let argument = file.ast.peel_ts_sugar_span(argument);
    if lookup
        .typescript_file(file.path.as_str())
        .is_some_and(|typescript_file| {
            typescript_file.async_functions.iter().any(|function| {
                function.can_return_async
                    && u64::from(argument.start) == function.expression.start_byte
                    && function.expression.end_byte == u64::from(argument.end)
            })
        })
    {
        return true;
    }
    file.ast
        .functions
        .iter()
        .find(|function| function.span == argument)
        .is_some_and(|function| function.r#async)
}

pub(crate) fn inside_non_component_function(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span.contains(span))
    {
        return false;
    }
    // An argument to a primitive that takes a callback is not "some helper
    // function's body", whatever the helper is called. Asked of the dialect
    // rather than listed: the seven names this had were 2.0's, three of which
    // 1.x does not have, and it was missing every 1.x callback-taker outside
    // the four it shares.
    if file.ast.arguments_containing(span).any(|(call, index)| {
        lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup
                    .dialect
                    .callback_semantics_at(primitive, index, call.arguments.len())
                    .execution
                    .is_some()
            })
    }) {
        return false;
    }
    file.ast.functions_body_containing(span).any(|function| {
        function_binding_name(file, function).is_some()
            && !object_method_escapes(file, function, lookup)
            && !lookup.function_may_be_component(file, function)
    })
}

/// ADR 0320: whether `function` is an object-literal method whose literal
/// goes anywhere other than a member dispatch the analysis resolves: handed
/// to unknown code, enumerated, returned, stored. Such a method is a stored
/// callback like a function-expression property, not a named helper whose
/// reads are claimed where it is called: it may be called where nothing
/// claims them.
pub(crate) fn object_method_escapes(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(name) = own_method_name(file, function) else {
        return false;
    };
    // An object literal's own method; a class's is selected by its instance.
    if !file
        .ast
        .object_properties
        .iter()
        .any(|property| property.key == name.span && property.method)
    {
        return false;
    }
    let Some(literal) = file
        .ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(name.span))
        .min_by_key(|shape| shape.span.end - shape.span.start)
    else {
        return true;
    };
    !object_literal_only_dispatched(file, literal.span, lookup)
}

/// Whether every way the object literal at `literal` is used is a member
/// call on it: it is the receiver of one, or a `const` in this module whose
/// every reference is, or an argument of an exact callee in this file that
/// only reads members of its parameter.
pub(super) fn object_literal_only_dispatched(
    file: &solid_facts::FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let called_member_of = |value: Span| {
        file.ast.members.iter().any(|member| {
            file.ast.peel_ts_sugar_span(member.object) == value
                && file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                && file
                    .ast
                    .calls
                    .iter()
                    .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span)
        })
    };
    let closed_argument = |value: Span| {
        file.ast
            .arguments_containing(value)
            .find(|(call, index)| file.ast.peel_ts_sugar_span(call.arguments[*index].span) == value)
            .is_some_and(|(call, index)| {
                let mut consulted = Vec::new();
                !call.arguments[index].spread
                    && lookup.parameter_only_member_accessed_consulting(
                        file,
                        call,
                        index,
                        &mut consulted,
                    )
                    && consulted
                        .iter()
                        .all(|(path, _)| path.as_str() == file.path.as_str())
            })
    };
    if called_member_of(literal) || closed_argument(literal) {
        return true;
    }
    let Some(binding) = file.ast.bindings.iter().find(|binding| {
        binding
            .initializer
            .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == literal)
    }) else {
        return false;
    };
    let [name] = binding.names.as_slice() else {
        return false;
    };
    binding.immutable
        && binding.shape == solid_facts::ast::BindingShape::Identifier
        && crate::execution_role::file_is_module(file)
        && !crate::execution_role::flow_binding_exported(file, name.span)
        && file
            .ast
            .reference_declarations
            .iter()
            .filter(|(reference, declaration)| *declaration == name.span && *reference != name.span)
            .all(|(reference, _)| called_member_of(*reference) || closed_argument(*reference))
}

/// Whether `span` is inside a syntactically direct function argument that a
/// known primitive provably never invokes.
///
/// Two positive proofs qualify, and nothing else: the dialect declares the
/// position a stored value (Solid 1.x `createSignal(() => value)`), or the
/// position's modelled callback belongs to a returned lazy adapter that this
/// call never consumes (`mapArray` whose result is discarded). A primitive
/// with no callback model at all — 2.0 `children`, `onCleanup` — proves
/// nothing: those callbacks do run, so their reads must not be treated as
/// dormant. Requiring the argument itself to be a function keeps an IIFE
/// passed as a value from being mistaken for dormant code.
pub(crate) fn inside_known_value_function_argument(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // Both proofs are dialect answers that Solid 2.0 leaves at their negative
    // default, so under 2.0 this question is provably always "no" and the
    // containment query below need not run at all.
    if !lookup.models_stored_function_arguments() && !lookup.models_returned_callbacks() {
        return false;
    }
    // Only the calls whose arguments actually contain `span` can answer. The
    // former whole-file scan asked every call in the file, once per call being
    // classified, which is quadratic in a file's call count.
    file.ast
        .arguments_containing(span)
        .any(|(call, argument_index)| {
            let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
                return false;
            };
            let argument = &call.arguments[argument_index];
            matches!(
                argument.value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && ({
                let semantics = lookup.dialect.callback_semantics_at(
                    primitive,
                    argument_index,
                    call.arguments.len(),
                );
                semantics.stores_as_value
                    || (semantics.requires_return_invocation
                        && callback_execution_at_call(
                            file,
                            call,
                            primitive,
                            argument_index,
                            lookup,
                        )
                        .is_none())
            })
        })
}
