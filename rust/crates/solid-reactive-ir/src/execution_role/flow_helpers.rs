//! Helpers shared by the invocation-flow walk: receivers, static keys, origins, and exported bindings.

use super::*;

/// ADR 0323: the role a function value handed to argument `index` of the
/// primitive call `call` runs in, when it is one that keeps a read out of the
/// render window: a tracked computation or a deferred (explicitly untracked,
/// registered, scheduled) callback. The reporting slots (an effect's apply, a
/// hydrate/render entry, an invalidation callback), a slot that stores the
/// function, runs it only on result access or needs a returned invoker, and
/// any slot the dialect does not model answer `None`.
pub(super) fn primitive_callback_slot_role(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    index: usize,
    lookup: &SemanticLookup<'_>,
) -> Option<ExecutionRole> {
    let argument = &call.arguments[index];
    if argument.spread
        || !lookup
            .resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| resolved.validity == typefacts::ResolvedCallValidity::Valid)
    {
        return None;
    }
    let primitive = lookup.primitive_at_call(file, call.span)?;
    let dialect = lookup.dialect;
    let semantics = dialect.callback_semantics_at(primitive, index, call.arguments.len());
    let execution = semantics.execution?;
    if semantics.requires_return_invocation
        || semantics.stores_as_value
        || dialect.callback_runs_on_result_access(primitive, index, call.arguments.len())
        || dialect.reports_untracked_reads_at(primitive, index, call.arguments.len())
        || dialect.apply_callback_argument(primitive) == Some(index)
    {
        return None;
    }
    if semantics.tracks_reads {
        return Some(ExecutionRole::TrackedJsx);
    }
    (execution == Execution::Deferred || dialect.runs_callback_deferred(primitive))
        .then_some(ExecutionRole::DeferredCallback)
}

/// Whether a role, or the mix of them [`InvocationFlow::function_role`]
/// answers as [`ExecutionRole::Unreported`], keeps a read out of every
/// reporting role. [`ExecutionRole::Unknown`] is accepted as before, and keeps
/// the merge it enters unknown.
pub(super) fn non_reporting_mix(role: ExecutionRole) -> bool {
    flow_safe_role(role) || matches!(role, ExecutionRole::Unknown | ExecutionRole::Unreported)
}

/// Whether a role keeps a read out of the render window and untracked
/// reporting: an event, a deferred callback, or a tracked computation.
pub(super) fn flow_safe_role(role: ExecutionRole) -> bool {
    matches!(
        role,
        ExecutionRole::EventCallback | ExecutionRole::DeferredCallback | ExecutionRole::TrackedJsx
    )
}

/// The innermost object literal whose own property key is at `key`.
pub(super) fn innermost_object_literal(
    file: &solid_facts::FileFacts,
    key: Span,
) -> Option<&solid_facts::ast::ObjectGetShapeFact> {
    file.ast
        .object_get_shapes
        .iter()
        .filter(|shape| shape.span.contains(key))
        .min_by_key(|shape| shape.span.end - shape.span.start)
}

/// Whether any non-arrow function written in `span` may use `this`: a method
/// of an object literal there receives the object.
pub(super) fn receiver_used_within(file: &solid_facts::FileFacts, span: Span) -> bool {
    file.ast.functions.iter().any(|function| {
        span.contains(function.span)
            && function.kind != solid_facts::ast::FunctionKind::Arrow
            && function_uses_receiver(file, function)
    })
}

/// Whether source text mentions `this` or `super` as a word.
pub(crate) fn mentions_receiver(text: &str) -> bool {
    mentions_this(text)
}

pub(super) fn mentions_this(text: &str) -> bool {
    ["this", "super"].iter().any(|word| {
        text.match_indices(word).any(|(at, _)| {
            let before = text[..at].chars().next_back();
            let after = text[at + word.len()..].chars().next();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$' || c == '.')
                && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
        })
    })
}

/// Whether a non-arrow function may use the object it is called on: its
/// text (parameter defaults included) mentions `this` or `super` (a method's
/// `super.m()` runs the prototype's `m` with the same `this`).
pub(super) fn function_uses_receiver(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.source_text(function.span).is_none_or(mentions_this)
}

/// Whether a source file is an ECMAScript module: it has an import or an
/// export. Anything else is a script, whose top-level declarations are
/// globals shared with every other script and reachable from any module.
pub(crate) fn file_is_module(file: &solid_facts::FileFacts) -> bool {
    !file.ast.imports.is_empty() || file.ast.module_level_exports().next().is_some()
}

/// Whether the member at `member` is a template tag, or reached through
/// (`p.g.call(…)`), which can hand its object over as `this`.
pub(super) fn flow_tagged_or_chained(file: &solid_facts::FileFacts, member: Span) -> bool {
    let next = file
        .source
        .get(member.end as usize..)
        .map(str::trim_start)
        .unwrap_or_default();
    next.starts_with(['`', '(']) || next.starts_with("?.(")
}

/// An element's children as the JSX transform keeps them: whitespace-only
/// text that spans a line break is dropped.
pub(super) fn jsx_children(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> Vec<Span> {
    element
        .children
        .iter()
        .copied()
        .filter(|child| {
            !file
                .source_text(*child)
                .is_some_and(|text| text.trim().is_empty() && text.contains('\n'))
        })
        .collect()
}

/// The runtime property key a literal key token spells: an identifier name,
/// a quoted string, a no-substitution template (computed only) or a canonical
/// array index. Anything else, or any escape sequence, has no static name
/// here. A non-computed numeric key is normalized as JavaScript does only for
/// plain decimal integers; `1.0`, `0x1`, `1e3` refuse.
pub(super) fn static_key_text(text: &str, computed: bool) -> Option<String> {
    let text = text.trim();
    if text.contains('\\') {
        return None;
    }
    let quoted = |open: char| {
        text.strip_prefix(open)
            .and_then(|inner| inner.strip_suffix(open))
            .filter(|inner| !inner.contains(open))
            .map(str::to_owned)
    };
    if let Some(inner) = quoted('"').or_else(|| quoted('\'')) {
        return Some(inner);
    }
    if computed && let Some(inner) = quoted('`').filter(|inner| !inner.contains("${")) {
        return Some(inner);
    }
    if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
        let canonical = text == "0" || !text.starts_with('0');
        return (canonical && text.parse::<i32>().is_ok()).then(|| text.to_owned());
    }
    let identifier = !computed
        && text
            .chars()
            .next()
            .is_some_and(|first| !first.is_ascii_digit())
        && text
            .chars()
            .all(|character| character == '_' || character == '$' || character.is_alphanumeric());
    identifier.then(|| text.to_owned())
}

/// The static key of an object-literal property, or `None`. A non-computed,
/// non-shorthand `__proto__: v` (quoted or not) is no own property at all: it
/// sets the literal's prototype (ECMA-262 `PropertyDefinitionEvaluation`), so
/// it has no key here. The computed `["__proto__"]: v` and the shorthand
/// `{ __proto__ }` do define an own property.
pub(crate) fn property_static_key(
    file: &solid_facts::FileFacts,
    property: &solid_facts::ast::ObjectPropertyFact,
) -> Option<String> {
    let key = static_key_text(file.source_text(property.key)?, property.computed)?;
    let shorthand = property.key == property.value;
    (property.computed || shorthand || key != "__proto__").then_some(key)
}

/// The static key a member access reads: its identifier, or the literal key
/// of a computed member (`o["run"]`, `o[1]`); `None` for any other computed
/// member.
pub(super) fn member_static_key(
    file: &solid_facts::FileFacts,
    member: &solid_facts::ast::MemberFact,
) -> Option<String> {
    if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        return file
            .ast
            .literal_computed_members
            .binary_search_by_key(&member.span, |fact| fact.span)
            .ok()
            .map(|index| file.ast.literal_computed_members[index].key.to_string());
    }
    static_key_text(file.source_text(member.property)?, false)
}

/// The objects an origin names, innermost first: a `seen` key component, so
/// a holder reached with a different origin is followed (and its receiver
/// proofs run) again.
pub(super) fn origin_identity(origin: &FlowOrigin<'_>) -> Vec<(String, Span)> {
    let mut identity = Vec::new();
    let mut origin = origin;
    loop {
        match origin {
            FlowOrigin::Unknown => return identity,
            FlowOrigin::Object(file, span) => {
                identity.push((file.path.to_string(), *span));
                return identity;
            }
            FlowOrigin::Element(file, span, enclosing) => {
                identity.push((file.path.to_string(), *span));
                origin = enclosing;
            }
        }
    }
}

/// The declaration span of a component's props parameter: its first
/// parameter, a plain identifier that is never reassigned.
pub(super) fn props_parameter(
    file: &solid_facts::FileFacts,
    component: &solid_facts::ast::FunctionFact,
) -> Option<Span> {
    let parameter = component.parameters.first()?;
    let [name] = parameter.names.as_slice() else {
        return None;
    };
    (parameter.shape == solid_facts::ast::BindingShape::Identifier
        && parameter.initializer.is_none()
        && !crate::indexes::binding_written(file, name.span))
    .then_some(name.span)
}

/// The expression a JSX attribute value or child at `value` holds: a
/// container's braces peeled to the member or function that is the whole of
/// what they enclose (anything else stays the container, which no proof
/// recognizes).
pub(super) fn jsx_value_expression(file: &solid_facts::FileFacts, value: Span) -> Span {
    let value = file.ast.peel_ts_sugar_span(value);
    let Some(inner) = file
        .source_text(value)
        .and_then(|text| text.strip_prefix('{'))
        .and_then(|text| text.strip_suffix('}'))
        .map(str::trim)
    else {
        return value;
    };
    file.ast
        .members
        .iter()
        .map(|member| member.span)
        .chain(file.ast.functions.iter().map(|function| function.span))
        .find(|candidate| value.contains(*candidate) && file.source_text(*candidate) == Some(inner))
        .map_or(value, |candidate| file.ast.peel_ts_sugar_span(candidate))
}

/// Whether member `key` of the object `origin` describes is a value that
/// cannot use that object as `this` when called: an arrow (or a `const`
/// bound to one), a non-arrow function that never mentions `this`, a
/// non-function literal, or, for a component's props, an attribute
/// forwarding such a value from the enclosing component's props. An absent
/// member proves nothing: `Object.prototype` may carry one.
pub(super) fn origin_member_ignores_receiver(
    origin: &FlowOrigin<'_>,
    key: &str,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    match origin {
        FlowOrigin::Unknown => false,
        FlowOrigin::Object(file, object) => {
            let Some(property) = file
                .ast
                .object_properties
                .iter()
                .filter(|property| {
                    object.contains(property.span)
                        && property_static_key(file, property).as_deref() == Some(key)
                        && innermost_object_literal(file, property.key)
                            .is_some_and(|inner| inner.span == *object)
                })
                .max_by_key(|property| property.span.start)
            else {
                return false;
            };
            // The property must be the member's final value: no spread and no
            // key without a static name after it may replace it, and a getter
            // or setter is no plain value.
            let own = |span: Span| {
                innermost_object_literal(file, span).is_some_and(|inner| inner.span == *object)
            };
            if (!property.data && !property.method)
                || file.ast.spreads.iter().any(|spread| {
                    object.contains(spread.span)
                        && spread.span.start > property.span.start
                        && file
                            .ast
                            .object_get_shapes
                            .iter()
                            .filter(|shape| {
                                shape.span.contains(spread.span) && shape.span != spread.span
                            })
                            .min_by_key(|shape| shape.span.end - shape.span.start)
                            .is_some_and(|shape| shape.span == *object)
                })
                || file.ast.object_properties.iter().any(|other| {
                    object.contains(other.span)
                        && other.span.start > property.span.start
                        && own(other.key)
                        && property_static_key(file, other).is_none()
                })
            {
                return false;
            }
            if property.method {
                return file
                    .ast
                    .functions
                    .iter()
                    .find(|function| {
                        function
                            .method_name
                            .as_ref()
                            .is_some_and(|name| name.span == property.key)
                    })
                    .is_some_and(|function| !function_uses_receiver(file, function));
            }
            value_ignores_receiver(file, property.value, &FlowOrigin::Unknown, lookup, depth)
        }
        FlowOrigin::Element(file, element, enclosing) => {
            let Some(element) = file.ast.jsx_element_at(*element) else {
                return false;
            };
            if !element.spreads.is_empty() {
                return false;
            }
            let children = jsx_children(file, element);
            if key == "children" && !children.is_empty() {
                let [child] = children.as_slice() else {
                    return false;
                };
                return value_ignores_receiver(file, *child, enclosing, lookup, depth);
            }
            match element
                .attributes
                .iter()
                .rev()
                .find(|attribute| file.source_text(attribute.name) == Some(key))
            {
                // No attribute gives the props object that key, and no
                // assignment or reflective definition anywhere can give it
                // one or reach the built-in prototype it would otherwise be
                // read from: the read is `undefined`, or an
                // `Object.prototype` built-in that invokes nothing it holds.
                None => {
                    !lookup.member_may_be_reassigned_on_builtin(key)
                        && !lookup.member_may_be_reassigned_on_builtin("__proto__")
                        && !lookup.property_may_be_defined_reflectively_as(key)
                }
                Some(attribute) => match attribute.value_kind {
                    solid_facts::ast::JsxAttributeValueKind::String
                    | solid_facts::ast::JsxAttributeValueKind::Boolean => true,
                    solid_facts::ast::JsxAttributeValueKind::Expression => {
                        attribute.expression.is_some_and(|value| {
                            value_ignores_receiver(file, value, enclosing, lookup, depth)
                        })
                    }
                    _ => false,
                },
            }
        }
    }
}

/// ADR 0319: whether the `const` `binding` names, at `declaration`, a function
/// a Solid primitive returned that ignores its receiver: the binding is the
/// primitive call's own result (`const busy = createMemo(...)`) or a plain
/// identifier slot of an array pattern destructuring that result in the same
/// declaration (`const [state, setState] = createSignal(...)`), and the
/// dialect read that slot as a bound function or an arrow. A held tuple, an
/// alias, a wrapper call and a defaulted slot are not the result. Premises:
/// the program replaces neither `Function.prototype.bind`, which the runtime
/// builds the accessors with, nor the array iteration protocol the pattern
/// uses (ADR 0349's tripwire).
fn primitive_result_ignores_receiver(
    file: &solid_facts::FileFacts,
    binding: &solid_facts::ast::BindingFact,
    declaration: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(call) = binding
        .initializer
        .and_then(|initializer| file.ast.call_at(file.ast.peel_ts_sugar_span(initializer)))
    else {
        return false;
    };
    let slot = match binding.shape {
        solid_facts::ast::BindingShape::Identifier => None,
        solid_facts::ast::BindingShape::Array => {
            match binding
                .array_slots
                .iter()
                .position(|slot| slot.as_ref().is_some_and(|slot| slot.span == declaration))
            {
                Some(index) if lookup.array_iteration_intact() => Some(index),
                _ => return false,
            }
        }
        _ => return false,
    };
    !call.construct
        && lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                lookup
                    .dialect
                    .returned_callable_ignores_receiver(primitive, slot)
            })
        && !lookup.member_may_be_reassigned_on_builtin("bind")
        && !lookup.property_may_be_defined_reflectively_as("bind")
        // ADR 0338's wider tripwire (Object.assign / Reflect.set through a
        // prototype) guards every other `read.bind` proof; this one too.
        && !lookup.function_bind_may_be_patched()
}

/// Whether the value at `value` (in an element written in a component whose
/// props `enclosing` describes) is one that cannot use a `this` it is called
/// with; see [`origin_member_ignores_receiver`].
pub(super) fn value_ignores_receiver(
    file: &solid_facts::FileFacts,
    value: Span,
    enclosing: &FlowOrigin<'_>,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    let value = jsx_value_expression(file, value);
    if let Some(function) = file.ast.function_at(value) {
        return function.kind == solid_facts::ast::FunctionKind::Arrow
            || !function_uses_receiver(file, function);
    }
    if let Some(declaration) = file.ast.reference_declaration(value) {
        if let Some(function) = file.ast.functions.iter().find(|function| {
            function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }) {
            return !function_uses_receiver(file, function);
        }
        if let Some(binding) = file.ast.bindings.iter().find(|binding| {
            binding.immutable && binding.names.iter().any(|name| name.span == declaration)
        }) {
            return primitive_result_ignores_receiver(file, binding, declaration, lookup)
                || binding.initializer.is_some_and(|initializer| {
                    let initializer = file.ast.peel_ts_sugar_span(initializer);
                    file.ast.functions.iter().any(|function| {
                        function.span == initializer
                            && (function.kind == solid_facts::ast::FunctionKind::Arrow
                                || !function_uses_receiver(file, function))
                    })
                });
        }
        // A component's props parameter read as `props.x`: the enclosing
        // element's attribute `x`.
    }
    // `props.x` of the component the enclosing element renders, read in
    // that component: the enclosing element's member `x`.
    if let Some(member) = file.ast.member_at(value)
        && file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_err()
        && let FlowOrigin::Element(element_file, element, _) = enclosing
        && let Some(element) = element_file.ast.jsx_element_at(*element)
        && let Some((component_file, component)) =
            lookup.function_called_at(element_file.path.as_str(), element.name.span)
        && component_file.path == file.path
        && component.span.contains(member.span)
        && let Some(key) = member_static_key(file, member)
        && let key = key.as_str()
        && let Some(parameter) = component.parameters.first()
        && let [name] = parameter.names.as_slice()
        && parameter.shape == solid_facts::ast::BindingShape::Identifier
        && !crate::indexes::binding_written(file, name.span)
        && file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
            == Some(name.span)
    {
        return origin_member_ignores_receiver(enclosing, key, lookup, depth - 1);
    }
    file.ast.array_literals.binary_search(&value).is_ok()
        || file
            .source_text(value)
            .is_some_and(|text| text.starts_with(['"', '\'']) || text.parse::<f64>().is_ok())
}

/// Whether the binding declared at `declaration` is exported.
pub(crate) fn flow_binding_exported(file: &solid_facts::FileFacts, declaration: Span) -> bool {
    let module_level = containing_ast_function(&file.ast, declaration).is_none();
    // In a script every top-level binding is global.
    if module_level && !file_is_module(file) {
        return true;
    }
    file.ast.exports.iter().any(|export| {
        (module_level && export.span.contains(declaration))
            || export.specifiers.iter().any(|specifier| {
                file.ast.reference_declaration(specifier.local.span) == Some(declaration)
            })
    })
}
