//! Mutation and invocation sites: tuple and iteration writes, string-code slots, gateways, proxy and store setters, reflective prototype writes.

use super::*;

/// Whether a reflective write's target may be spelled through a `prototype`.
/// A spread can supply any target, a prototype included.
pub(super) fn targets_prototype(file: &FileFacts, call: &ast::CallFact) -> bool {
    call.arguments.first().is_some_and(|target| {
        target.spread
            || file
                .source_text(target.span)
                .is_some_and(|text| text.contains("prototype"))
    })
}

/// The engine's dense returned literal owns its indices and length, and the
/// iterator result owns value/done. Iteration still Gets @@iterator, next and
/// (on early close) return through mutable prototype chains. A __proto__ write
/// or setPrototypeOf can change any of those lookups. Unknown keys may name
/// these properties; a known different string cannot.
pub(super) fn iteration_key(key: Option<&str>) -> bool {
    key.is_none_or(|key| matches!(key, "next" | "return" | "__proto__"))
}

pub(super) fn member_key<'a>(file: &'a FileFacts, member: &ast::MemberFact) -> Option<&'a str> {
    if file.ast.computed_members.contains(&member.span) {
        file.ast
            .literal_computed_members
            .iter()
            .find(|key| key.span == member.span)
            .map(|key| key.key.as_str())
    } else {
        file.source_text(member.property)
    }
}

/// A negative proof only: a separate source allocation cannot be one of the
/// intrinsic prototypes, nor the fresh literal consumed directly at its return.
/// Follow exact immutable local aliases. Parameters, calls (getPrototypeOf
/// included), imports, reassigned bindings and cycles remain unknown.
pub(super) fn separate_iteration_target(file: &FileFacts, span: Span, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if file.ast.array_literals.contains(&span)
        || file
            .ast
            .functions
            .iter()
            .any(|function| function.span == span)
        || file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == span)
    {
        return true;
    }
    if let Some(member) = file.ast.member_at(span)
        && member_key(file, member) == Some("prototype")
        && let Some(declaration) = file
            .ast
            .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
        && file.ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        })
        && !iteration_binding_written(file, declaration)
    {
        return true;
    }
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.immutable
                && binding.shape == ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declaration
                && !iteration_binding_written(file, declaration)
        })
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| separate_iteration_target(file, initializer, depth - 1))
}

/// ADR 0803: whether the value at `span` is exactly `Array.prototype` or the
/// array iterator's prototype (`Object.getPrototypeOf([][Symbol.iterator]())`),
/// through the alias walk [`separate_iteration_target`] takes: an immutable
/// identifier binding no write reaches, to its initializer.
pub(crate) fn iteration_intrinsic_target(file: &FileFacts, span: Span, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    let global_member = |span: Span, keys: &[&str], globals: &[&str]| {
        file.ast
            .member_at(file.ast.peel_ts_sugar_span(span))
            .is_some_and(|member| {
                member_key(file, member).is_some_and(|key| keys.contains(&key))
                    && global_name(file, file.ast.peel_ts_sugar_span(member.object))
                        .is_some_and(|name| globals.contains(&name))
            })
    };
    if global_member(span, &["prototype"], &["Array"]) {
        return true;
    }
    // Review a9fc #2: each of these is exactly `Array.prototype`.
    let array_literal = |span: Span| {
        file.ast
            .array_literals
            .contains(&file.ast.peel_ts_sugar_span(span))
    };
    if let Some(member) = file.ast.member_at(span) {
        let key = member_key(file, member);
        // `[].__proto__`
        if key == Some("__proto__") && array_literal(member.object) {
            return true;
        }
        // `[].constructor.prototype`
        if key == Some("prototype")
            && file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(member.object))
                .is_some_and(|inner| {
                    member_key(file, inner) == Some("constructor") && array_literal(inner.object)
                })
        {
            return true;
        }
    }
    // `Object.getPrototypeOf([])`
    if let Some(call) = file.ast.call_at(span)
        && global_member(call.callee, &["getPrototypeOf"], &["Object", "Reflect"])
        && let [argument] = call.arguments.as_slice()
        && !argument.spread
        && array_literal(argument.span)
    {
        return true;
    }
    let array_iterator = |span: Span| {
        file.ast
            .call_at(file.ast.peel_ts_sugar_span(span))
            .is_some_and(|call| {
                call.arguments.is_empty()
                    && file
                        .ast
                        .member_at(file.ast.peel_ts_sugar_span(call.callee))
                        .is_some_and(|member| {
                            file.ast
                                .array_literals
                                .contains(&file.ast.peel_ts_sugar_span(member.object))
                                && (matches!(
                                    member_key(file, member),
                                    Some("values" | "keys" | "entries")
                                ) || file
                                    .source_text(member.property)
                                    .is_some_and(|text| text.trim() == "Symbol.iterator"))
                        })
            })
    };
    if let Some(call) = file.ast.call_at(span)
        && global_member(call.callee, &["getPrototypeOf"], &["Object", "Reflect"])
        && let [argument] = call.arguments.as_slice()
        && !argument.spread
        && array_iterator(argument.span)
    {
        return true;
    }
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.immutable
                && binding.shape == ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding.names[0].span == declaration
                && !iteration_binding_written(file, declaration)
        })
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| iteration_intrinsic_target(file, initializer, depth - 1))
}

/// ADR 0803: a visible write in `file` that can reach the array iteration
/// protocol: an iterator key (`Symbol.iterator`,
/// `next`, `return`) on a target not proven separate, an unknown key or
/// `__proto__` on `Array.prototype` or the iterator prototype, and the same
/// through the reflective mutators and `setPrototypeOf`.
pub(crate) fn visible_iteration_write(file: &FileFacts, lookup: &SemanticLookup<'_>) -> bool {
    let iterator_key = |text: &str| {
        matches!(
            text.trim().trim_matches(['"', '\'']),
            "next" | "return" | "Symbol.iterator"
        )
    };
    let writes = file
        .ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .any(|target| {
            let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(*target)) else {
                return false;
            };
            let property = file.source_text(member.property).unwrap_or_default();
            if iterator_key(property) {
                return !separate_iteration_target(file, member.object, 24);
            }
            let unknown = file.ast.computed_members.contains(&member.span)
                && member_key(file, member).is_none();
            (unknown || member_key(file, member) == Some("__proto__"))
                && iteration_intrinsic_target(file, member.object, 24)
        });
    writes
        || file.ast.calls.iter().any(|call| {
            let name = file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(call.callee))
                .and_then(|member| file.source_text(member.property));
            let Some(target) = call.arguments.first() else {
                return false;
            };
            if name == Some("setPrototypeOf") {
                return target.spread || iteration_intrinsic_target(file, target.span, 24);
            }
            let Some(method) = mutation_builtin(file, call, lookup) else {
                return false;
            };
            if target.spread || separate_iteration_target(file, target.span, 24) {
                return false;
            }
            let intrinsic = iteration_intrinsic_target(file, target.span, 24);
            match method {
                "assign" | "defineProperties" => call.arguments.iter().skip(1).any(|source| {
                    let span = file.ast.peel_ts_sugar_span(source.span);
                    match file.ast.object_get_shapes.iter().find(|shape| shape.span == span) {
                        Some(shape) => {
                            shape.properties.iter().any(|property| matches!(property.key.as_str(), "next" | "return"))
                                || (!shape.closed
                                    && (intrinsic
                                        || file.source_text(span).is_some_and(|text| text.contains("Symbol.iterator"))))
                        }
                        None => intrinsic || source.spread,
                    }
                }),
                _ => call.arguments.get(1).is_none_or(|key| {
                    key.spread
                        || matches!(&key.literal_value, ast::ArgumentLiteralFact::String(text) if iterator_key(text))
                        || file.source_text(key.span).is_some_and(iterator_key)
                        || (intrinsic && !matches!(key.literal_value, ast::ArgumentLiteralFact::String(_)))
                }),
            }
        })
}

pub(super) fn iteration_binding_written(file: &FileFacts, declaration: Span) -> bool {
    file.ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .any(|target| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(*target))
                == Some(declaration)
        })
}

pub(super) fn iteration_map(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    let Some(shape) = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span && shape.closed)
    else {
        return true;
    };
    // closed excludes spreads and unnamed computed keys. Do not accidentally
    // treat a symbol descriptor (which has no string key) as an empty map.
    shape
        .properties
        .iter()
        .any(|property| iteration_key(Some(property.key.as_str())))
}

pub(super) fn tuple_mutation_method<'a>(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    // A library signature on an alias is insufficient to certify the runtime
    // callee, but is sufficient to retain a possible mutation as a veto.
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let (method, library) = match declaration.qualified_name.as_ref() {
        "ObjectConstructor.assign" => ("assign", "lib.es2015.core.d.ts"),
        "ObjectConstructor.defineProperty" => ("defineProperty", "lib.es5.d.ts"),
        "ObjectConstructor.defineProperties" => ("defineProperties", "lib.es5.d.ts"),
        "ObjectConstructor.setPrototypeOf" => ("setPrototypeOf", "lib.es2015.core.d.ts"),
        "Reflect.set" => ("set", "lib.es2015.reflect.d.ts"),
        "Reflect.defineProperty" => ("defineProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.deleteProperty" => ("deleteProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.setPrototypeOf" => ("setPrototypeOf", "lib.es2015.reflect.d.ts"),
        "Function.call"
        | "Function.apply"
        | "Function.bind"
        | "CallableFunction.call"
        | "CallableFunction.apply"
        | "CallableFunction.bind"
        | "NewableFunction.call"
        | "NewableFunction.apply"
        | "NewableFunction.bind" => ("invoke", "lib.es5.d.ts"),
        "Reflect.apply" => ("reflectInvoke", "lib.es2015.reflect.d.ts"),
        "FunctionConstructor" | "FunctionConstructor.new" => ("stringCode", "lib.es5.d.ts"),
        "eval" => ("stringCode", "lib.es5.d.ts"),
        _ => return None,
    };
    (std::path::Path::new(declaration.source_file.as_ref())
        .file_name()?
        .to_str()?
        == library)
        .then_some(method)
}

/// Whether a call's TypeScript-resolved declaration is a function this checker
/// analyzes: a project member, such as `Object.defineProperty` of a namespace
/// that shadows the global `Object`. A library declaration is not one. Read from
/// the resolved call alone: reading the object-member index from inside this
/// veto hung the checker on the `t9` shape.
fn resolves_to_project_function(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
) -> bool {
    let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
        return false;
    };
    if resolved.validity != typefacts::ResolvedCallValidity::Valid {
        return false;
    }
    resolved
        .declaration
        .as_ref()
        .filter(|declaration| !declaration.symbol.is_empty())
        .is_some_and(|declaration| {
            lookup
                .function_for_symbol(declaration.symbol.as_ref())
                .is_some()
        })
}

/// The global `span` names: an identifier no declaration binds.
fn global_name(file: &FileFacts, span: Span) -> Option<&str> {
    let reference = file.ast.identifiers.iter().any(|identifier| {
        identifier.span == span && identifier.role == ast::IdentifierRole::Reference
    });
    (reference && file.ast.reference_declaration(span).is_none())
        .then(|| file.source_text(span))
        .flatten()
}

/// Whether the expression at `span` is only compared, so its value goes nowhere
/// (`x instanceof Array`, `v.constructor === Array`, `typeof Function`).
fn only_compared(file: &FileFacts, span: Span) -> bool {
    let (Some(before), Some(after)) = (
        file.source.get(..span.start as usize),
        file.source.get(span.end as usize..),
    ) else {
        return false;
    };
    let (before, after) = (before.trim_end(), after.trim_start());
    // `x === window.Function(code)()`: the operand goes on past `span`.
    (!after.starts_with(['.', '[', '(', '?'])
        && ["===", "!==", "==", "!=", "instanceof", "typeof"]
            .iter()
            .any(|operator| before.ends_with(operator)))
        || ["===", "!==", "==", "!=", "instanceof"]
            .iter()
            .any(|operator| after.starts_with(operator))
}

/// ADR 0803: whether `span` is exactly the target of a global reflective
/// mutator whose whole effect [`visible_iteration_write`] decides: the first
/// argument of `Reflect.set`, `defineProperty` or `deleteProperty` (a boolean
/// result), or of `Object.assign`, `defineProperty` or `defineProperties`
/// in an expression statement (they return the target, so a result used as
/// a value hands the intrinsic on).
fn reflective_target(file: &FileFacts, span: Span) -> bool {
    file.ast.calls.iter().any(|call| {
        let Some(first) = call.arguments.first() else {
            return false;
        };
        if first.spread || file.ast.peel_ts_sugar_span(first.span) != span {
            return false;
        }
        let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
            return false;
        };
        if file.ast.computed_members.contains(&member.span) {
            return false;
        }
        let property = file.source_text(member.property).unwrap_or_default();
        match global_name(file, file.ast.peel_ts_sugar_span(member.object)) {
            Some("Reflect") => matches!(property, "set" | "defineProperty" | "deleteProperty"),
            Some("Object") => {
                let before = file
                    .source
                    .get(..call.span.start as usize)
                    .unwrap_or_default()
                    .trim_end();
                let after = file
                    .source
                    .get(call.span.end as usize..)
                    .unwrap_or_default()
                    .trim_start();
                matches!(property, "assign" | "defineProperty" | "defineProperties")
                    && (before.is_empty() || before.ends_with([';', '{', '}']))
                    && (after.is_empty() || after.starts_with([';', '}']))
            }
            _ => false,
        }
    })
}

/// ADR 0801, narrowed by ADR 0803 (item 3): whether `file` runs code from a
/// string (`eval`, `Function(...)`, a timer's string), or hands `Function` or
/// what the array iteration protocol lives on to code the checker does not
/// see ([`handed_on`]). Reached by: the `Array` and `Function` globals as
/// values, `Array.prototype`, a global object's `Array`, `Function`, `eval` or
/// unknown key, `__proto__`, `getPrototypeOf`, `.constructor`. A script
/// element or a frame is outside the model. Per file, reading only the file:
/// the base layer's [`crate::base::Hazard::ArrayIntrinsicObtained`] (the
/// first answer). The second: each call that receives an obtained value,
/// encoded for [`crate::base::Hazard::IntrinsicHandedAtCall`]
/// ([`handed_call_entry`]); whether that call hands it to unseen code is the
/// project's question ([`handed_to_unseen_code`]).
pub(crate) fn intrinsic_census(file: &FileFacts) -> (bool, Vec<u32>) {
    let at_calls = std::cell::RefCell::new(Vec::new());
    let obtained = intrinsic_obtained_collecting(file, &at_calls);
    let calls = at_calls
        .into_inner()
        .into_iter()
        .filter_map(|(call, slot, kind)| {
            let ordinal = file.ast.calls.iter().position(|fact| fact.span == call)?;
            Some(handed_call_entry(u32::try_from(ordinal).ok()?, slot, kind))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    (obtained, calls)
}

/// A call that receives an obtained value: `(call span, where, kind)`.
type AtCalls = std::cell::RefCell<Vec<(Span, Slot, Obtained)>>;

/// Where a call receives an obtained value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    /// An argument, by index.
    Argument(usize),
    /// The receiver of a method call (`window.k()`): the method's `this`.
    This,
    /// A timer's callback, which the platform calls with the window as
    /// `this` (review D3).
    TimerCallback,
    /// An argument [`read_only_builtin`] names by its spelling: the project
    /// decides whether the member is still the standard one (review R19b).
    ReadOnlyBuiltin,
    /// `path.find((el) => el instanceof Node && ...)`: no window comes back
    /// while `Node` and `instanceof` are the standard ones.
    PathFind,
}

fn intrinsic_obtained_collecting(file: &FileFacts, at_calls: &AtCalls) -> bool {
    let aliases = this_aliases(file);
    let callees = file
        .ast
        .calls
        .iter()
        .map(|call| (file.ast.peel_ts_sugar_span(call.callee), call.span))
        .collect::<HashMap<_, _>>();
    let objects = file
        .ast
        .members
        .iter()
        .map(|member| file.ast.peel_ts_sugar_span(member.object))
        .collect::<HashSet<_>>();
    let writes = file
        .ast
        .write_targets
        .iter()
        .chain(&file.ast.deleted_targets)
        .map(|target| file.ast.peel_ts_sugar_span(*target))
        .collect::<HashSet<_>>();
    let handed = |span: Span, kind: Obtained| handed_on(file, span, kind, HANDED_BUDGET, at_calls);
    let in_type_query = |span: Span| {
        file.ast
            .type_queries
            .iter()
            .any(|query| query.contains(span))
    };
    file.ast.members.iter().any(|member| {
        // A member of the global object goes as the global object does
        // ([`handed_on`]): an intrinsic by name, an event handler written, a
        // method called with it as `this`.
        let object = file.ast.peel_ts_sugar_span(member.object);
        // `typeof globalThis.setTimeout` in a type is erased: no value.
        if in_type_query(member.span) {
            return false;
        }
        if global_name(file, object).is_some_and(|name| WINDOW_NAMES.contains(&name)) {
            return handed(object, Obtained::Global);
        }
        // A write is the visible side's (`visible_iteration_write`).
        if writes.contains(&member.span) {
            return false;
        }
        let key = member_key(file, member);
        // Review D4: `this` read through to a global's name, in any file
        // (the window's names for itself are typed candidates, R12); under
        // a key no literal names, in sloppy code, where `this` may be the
        // global object.
        // In sloppy code a never-reassigned alias (`var self = this`) is
        // `this` too.
        let alias = file
            .ast
            .reference_declaration(object)
            .is_some_and(|declaration| aliases.contains(&declaration));
        if alias || file.source_text(object) == Some("this") {
            match key {
                Some("Array") => return handed(member.span, Obtained::Intrinsic),
                Some("Function" | "eval" | "Reflect" | "Object") => {
                    return handed(member.span, Obtained::Function);
                }
                Some("global") => return handed(member.span, Obtained::Global),
                None => return alias || sloppy_this(file, object),
                Some(_) => {}
            }
        }
        match key {
            // What these reach is a prototype or a constructor: an intrinsic
            // when the receiver is an array, `Function` through `.constructor`.
            Some("__proto__") => return handed(member.span, Obtained::Intrinsic),
            Some("getPrototypeOf") => {
                // Called, its result; held as a value, the function itself.
                return callees.get(&member.span).map_or_else(
                    || handed(member.span, Obtained::Intrinsic),
                    |call| handed(*call, Obtained::Intrinsic),
                );
            }
            Some("constructor") => return handed(member.span, Obtained::Function),
            _ => {}
        }
        match global_name(file, object) {
            Some("Array") => match key {
                // `Array[k]` may be `Array.constructor`, which is `Function`.
                None => handed(member.span, Obtained::Function),
                Some("prototype") => {
                    !reflective_target(file, member.span)
                        && handed(member.span, Obtained::Intrinsic)
                }
                Some(_) => false,
            },
            _ => false,
        }
    }) || file.ast.identifiers.iter().any(|identifier| {
        // Review a9fc #1: the global object as a value (`const g = globalThis;
        // g.Array`, `Reflect.get(globalThis, "Array")`) reaches every global.
        global_name(file, identifier.span).is_some_and(|name| WINDOW_NAMES.contains(&name))
            && identifier.role == ast::IdentifierRole::Reference
            && !file.ast.type_names.contains(&identifier.span)
            && !in_type_query(identifier.span)
            && !objects.contains(&identifier.span)
            && handed(identifier.span, Obtained::Global)
    }) || file.ast.bindings.iter().any(|binding| {
        // A pattern that takes an intrinsic or a prototype getter out by key
        // (`const { getPrototypeOf } = Object`, `const { constructor: F } =
        // f`, `const { Array: A } = globalThis`), or destructures the global
        // object at all.
        binding.shape == ast::BindingShape::Object
            && (binding.object_slots.iter().any(|slot| {
                matches!(
                    slot.property.as_str(),
                    "getPrototypeOf" | "constructor" | "__proto__" | "Array" | "Function" | "eval"
                ) || GLOBAL_OBJECTS.contains(&slot.property.as_str())
            }) || binding.initializer.is_some_and(|initializer| {
                global_name(file, file.ast.peel_ts_sugar_span(initializer))
                    .is_some_and(|name| WINDOW_NAMES.contains(&name))
            }))
    }) || file.ast.identifiers.iter().any(|identifier| {
        let name = global_name(file, identifier.span);
        matches!(name, Some("Array" | "Function" | "Object" | "Reflect"))
            && !file.ast.type_names.contains(&identifier.span)
            && !objects.contains(&identifier.span)
            && !callees.contains_key(&identifier.span)
            && handed(
                identifier.span,
                if name == Some("Array") {
                    Obtained::Intrinsic
                } else {
                    Obtained::Function
                },
            )
    }) || file.ast.identifiers.iter().any(|identifier| {
        // Review R19d: a caller of platform callbacks as a value (`const st =
        // setTimeout`, `[f].forEach(setTimeout)`, `setTimeout.bind(null)`).
        global_name(file, identifier.span).is_some_and(|name| PLATFORM_CALLERS.contains(&name))
            && !file.ast.type_names.contains(&identifier.span)
            && !in_type_query(identifier.span)
            && !callees.contains_key(&identifier.span)
            && match logical_timer_call(file, identifier.span) {
                Some(call) => {
                    at_calls
                        .borrow_mut()
                        .push((call, Slot::TimerCallback, Obtained::Global));
                    false
                }
                None => handed(identifier.span, Obtained::Function),
            }
    }) || file.ast.calls.iter().any(|call| {
        // Review R19e: a bubbling event's path ends at the window.
        file.ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .is_some_and(|member| member_key(file, member) == Some("composedPath"))
            && handed(call.span, Obtained::Path)
    }) || file
        .ast
        .identifiers
        .iter()
        .any(|identifier| global_name(file, identifier.span) == Some("eval"))
        || file.ast.calls.iter().any(|call| {
            match global_name(file, file.ast.peel_ts_sugar_span(call.callee)) {
                Some("Function") => true,
                // A timer runs a string as code, and calls a function with
                // the window as `this` (review D3).
                Some("setTimeout" | "setInterval") => match call.arguments.first() {
                    None => false,
                    Some(handler)
                        if handler.spread
                            || handler.runtime_value_kind == ast::RuntimeValueKind::Primitive =>
                    {
                        true
                    }
                    Some(handler) => match function_literal(file, handler.span) {
                        Some(function) => {
                            function.kind != ast::FunctionKind::Arrow
                                && file
                                    .source_text(function.span)
                                    .is_none_or(crate::execution_role::mentions_receiver)
                        }
                        None => {
                            at_calls.borrow_mut().push((
                                call.span,
                                Slot::TimerCallback,
                                Obtained::Global,
                            ));
                            false
                        }
                    },
                },
                // The window's own method, called bare: its listener runs
                // with the window as `this` and an event that reaches it.
                Some("addEventListener") => {
                    at_calls
                        .borrow_mut()
                        .push((call.span, Slot::This, Obtained::Global));
                    false
                }
                _ => false,
            }
        })
        || sloppy_this_handed_on(file, &aliases, at_calls)
        || file.ast.assignments.iter().any(|assignment| {
            // A bare event handler (`onmessage = f`) is the window's.
            let target = file.ast.peel_ts_sugar_span(assignment.target);
            file.ast.reference_declaration(target).is_none()
                && file
                    .source_text(target)
                    .is_some_and(|name| name.starts_with("on") && !name.contains(['.', '[']))
                && !inert_handler(file, target)
        })
}

/// Review D4: whether `this` at `span` is in sloppy code, where a plain call
/// makes it the global object: a file that is no ECMAScript module with no
/// `"use strict"` prologue, outside every class and every function whose
/// prologue says `"use strict"`.
fn sloppy_this(file: &FileFacts, span: Span) -> bool {
    let strict = |directive: &str| directive == "use strict";
    !crate::execution_role::file_is_module(file)
        && !file
            .ast
            .module_directives
            .iter()
            .any(|directive| strict(&directive.value))
        && !file
            .ast
            .classes
            .iter()
            .any(|class| class.span.contains(span))
        && !file.ast.functions.iter().any(|function| {
            function.span.contains(span)
                && function
                    .directives
                    .iter()
                    .any(|directive| strict(directive))
        })
}

/// Review D4: the bindings that alias sloppy code's `this` and are never
/// written again (`var self = this`), by declaration name.
fn this_aliases(file: &FileFacts) -> HashSet<Span> {
    if crate::execution_role::file_is_module(file) {
        return HashSet::default();
    }
    let written = file
        .ast
        .write_targets
        .iter()
        .filter_map(|target| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(*target))
        })
        .collect::<HashSet<_>>();
    file.ast
        .bindings
        .iter()
        .filter(|binding| binding.shape == ast::BindingShape::Identifier)
        .filter_map(|binding| {
            let [name] = binding.names.as_slice() else {
                return None;
            };
            let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
            (file.source_text(initializer) == Some("this")
                && sloppy_this(file, initializer)
                && !written.contains(&name.span))
            .then_some(name.span)
        })
        .collect()
}

/// Review D4: sloppy code that hands `this` on: as the receiver of
/// `call`, `apply` or `bind` (the target runs with it), or as an argument,
/// recorded for the project's callee check.
fn sloppy_this_handed_on(file: &FileFacts, aliases: &HashSet<Span>, at_calls: &AtCalls) -> bool {
    file.ast.calls.iter().any(|call| {
        let Some(index) = call.arguments.iter().position(|argument| {
            let span = file.ast.peel_ts_sugar_span(argument.span);
            (file.source_text(span) == Some("this") && sloppy_this(file, span))
                || file
                    .ast
                    .reference_declaration(span)
                    .is_some_and(|declaration| aliases.contains(&declaration))
        }) else {
            return false;
        };
        let rebinds = file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .is_some_and(|member| {
                matches!(member_key(file, member), Some("call" | "apply" | "bind"))
            });
        if rebinds {
            return true;
        }
        at_calls.borrow_mut().push((
            call.span,
            argument_slot(file, call, index),
            Obtained::Global,
        ));
        false
    })
}

/// The function literal at `span`, through TypeScript sugar.
fn function_literal(file: &FileFacts, span: Span) -> Option<&ast::FunctionFact> {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .functions
        .iter()
        .find(|function| function.span == span)
}

/// Whether the value written to the event handler at `target` reaches nothing
/// the platform hands it: `null`, `undefined`, or a function literal that
/// meets [`listener_reaches_nothing`]. Anything else (a compound write, a
/// `delete`, a value from elsewhere) may.
fn inert_handler(file: &FileFacts, target: Span) -> bool {
    file.ast
        .assignments
        .iter()
        .find(|assignment| {
            file.ast.peel_ts_sugar_span(assignment.target) == target
                && !assignment.reads_target
                && assignment.logical.is_none()
        })
        .is_some_and(|assignment| {
            let value = file.ast.peel_ts_sugar_span(assignment.value_span);
            (matches!(file.source_text(value), Some("null" | "undefined"))
                && file.ast.reference_declaration(value).is_none())
                || function_literal(file, value)
                    .is_some_and(|function| listener_reaches_nothing(file, function))
        })
}

/// Review D3: whether a function the platform calls with the window as `this`
/// and an event (which reaches the window through `currentTarget`, `view`,
/// `source`) can reach neither. Every parameter is a plain name nothing
/// references (a rest one too), nothing in it runs code from a string, and,
/// unless it is an arrow (whose `this` and `arguments` are its context's),
/// its text mentions neither `this` nor `super` and it reads no `arguments`.
fn listener_reaches_nothing(file: &FileFacts, function: &ast::FunctionFact) -> bool {
    let referenced = |name: Span| {
        file.ast.identifiers.iter().any(|identifier| {
            identifier.role == ast::IdentifierRole::Reference
                && identifier.span != name
                && file.ast.reference_declaration(identifier.span) == Some(name)
        })
    };
    let within = |names: &[&str]| {
        file.ast.identifiers.iter().any(|identifier| {
            function.span.contains(identifier.span)
                && identifier.role == ast::IdentifierRole::Reference
                && file
                    .source_text(identifier.span)
                    .is_some_and(|text| names.contains(&text))
        })
    };
    function
        .parameters
        .iter()
        .enumerate()
        .all(|(index, parameter)| {
            parameter.shape == ast::BindingShape::Identifier
                && parameter.names.iter().all(|name| {
                    !referenced(name.span) || (index == 0 && event_only_read(file, name.span))
                })
        })
        && function
            .rest_parameter_names
            .iter()
            .all(|name| !referenced(name.span))
        && !within(&["eval", "Function"])
        && (function.kind == ast::FunctionKind::Arrow
            || (file
                .source_text(function.span)
                .is_some_and(|text| !crate::execution_role::mentions_receiver(text))
                && !within(&["arguments"])))
}

/// What a listener may read of its event: facts about the input, and the
/// methods that only flag the event. None of them is the window by itself:
/// `detail` and `data` hold what the program put there, and `handed_on` counts
/// the window stored in any object literal, an event's init among them. Closed: `target`, `currentTarget`, `view`,
/// `source`, `composedPath`, `srcElement`, `relatedTarget` and any other key
/// may reach it.
const EVENT_READS: &[&str] = &[
    "altKey",
    "button",
    "buttons",
    "clientX",
    "clientY",
    "code",
    "ctrlKey",
    "data",
    "defaultPrevented",
    "deltaMode",
    "deltaX",
    "deltaY",
    "deltaZ",
    "detail",
    "isComposing",
    "isTrusted",
    "key",
    "metaKey",
    "movementX",
    "movementY",
    "newValue",
    "offsetX",
    "offsetY",
    "oldValue",
    "pageX",
    "pageY",
    "persisted",
    "pointerId",
    "pointerType",
    "preventDefault",
    "repeat",
    "screenX",
    "screenY",
    "shiftKey",
    "stopImmediatePropagation",
    "stopPropagation",
    "timeStamp",
    "type",
];

/// Whether the member at `span` is a tagged template's tag: called, with its
/// object as `this`.
fn tagged(file: &FileFacts, span: Span) -> bool {
    file.ast
        .tagged_templates
        .iter()
        .any(|template| file.ast.peel_ts_sugar_span(template.tag) == span)
}

/// The members of [`EVENT_READS`] a listener may call.
const EVENT_FLAGS: [&str; 3] = [
    "preventDefault",
    "stopPropagation",
    "stopImmediatePropagation",
];

/// Whether every reference to the event parameter named at `name` only tests
/// it, compares it, or reads one of [`EVENT_READS`] by a literal key (review
/// D21, T3). A write through it or a hand-on is none of these.
fn event_only_read(file: &FileFacts, name: Span) -> bool {
    file.ast.identifiers.iter().all(|identifier| {
        identifier.role != ast::IdentifierRole::Reference
            || identifier.span == name
            || file.ast.reference_declaration(identifier.span) != Some(name)
            || only_compared(file, identifier.span)
            || tested(file, identifier.span)
            || file
                .ast
                .members
                .iter()
                .find(|member| file.ast.peel_ts_sugar_span(member.object) == identifier.span)
                .is_some_and(|member| {
                    // A listed value may hold a function, which a call would
                    // run with the event as `this` (review R53).
                    let called = file
                        .ast
                        .calls
                        .iter()
                        .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span)
                        || tagged(file, member.span);
                    member_key(file, member).is_some_and(|key| {
                        EVENT_READS.contains(&key) && (!called || EVENT_FLAGS.contains(&key))
                    }) && !file
                        .ast
                        .write_targets
                        .iter()
                        .any(|target| file.ast.peel_ts_sugar_span(*target) == member.span)
                })
    })
}

/// The timers and frame requests the platform calls one callback from.
const TIMER_CALLERS: [&str; 4] = [
    "setTimeout",
    "setInterval",
    "requestAnimationFrame",
    "requestIdleCallback",
];

/// The platform's name for the operand at `span`: a global timer, or one read
/// off the global object.
fn timer_operand(file: &FileFacts, span: Span) -> Option<&str> {
    let span = file.ast.peel_ts_sugar_span(span);
    let name = global_name(file, span).or_else(|| {
        let member = file.ast.member_at(span)?;
        global_name(file, file.ast.peel_ts_sugar_span(member.object))
            .filter(|name| WINDOW_NAMES.contains(name))?;
        member_key(file, member)
    })?;
    TIMER_CALLERS.contains(&name).then_some(name)
}

/// `(window.requestIdleCallback || setTimeout)(f)`: the call whose callee is a
/// logical of two platform timers, one of them at `span` (review D21, T3: each
/// arm passes by itself).
fn logical_timer_call(file: &FileFacts, span: Span) -> Option<Span> {
    let logical = file
        .ast
        .logical_expressions
        .iter()
        .find(|logical| logical.left == span || logical.right == span)?;
    timer_operand(file, logical.left)?;
    timer_operand(file, logical.right)?;
    file.ast
        .calls
        .iter()
        .find(|call| file.ast.peel_ts_sugar_span(call.callee) == logical.span)
        .map(|call| call.span)
}

/// The two names of [`logical_timer_call`]'s callee, for the project's check
/// that neither was written.
fn logical_timer_keys<'a>(file: &'a FileFacts, call: &ast::CallFact) -> Vec<&'a str> {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    file.ast
        .logical_expressions
        .iter()
        .find(|logical| logical.span == callee)
        .map(|logical| {
            [logical.left, logical.right]
                .into_iter()
                .filter_map(|span| timer_operand(file, span))
                .collect()
        })
        .unwrap_or_default()
}

/// `(el) => el instanceof Node && ...`, the test first and nothing that can
/// answer without it: what `find` returns is a `Node` or nothing, and the
/// window is no `Node` (review D21, T3).
fn callback_guards_node(file: &FileFacts, call: &ast::CallFact) -> bool {
    let Some(function) = call
        .arguments
        .first()
        .filter(|argument| !argument.spread)
        .and_then(|argument| function_literal(file, argument.span))
    else {
        return false;
    };
    let ([parameter], true) = (
        function.parameters.as_slice(),
        function.kind == ast::FunctionKind::Arrow && function.rest_parameter_names.is_empty(),
    ) else {
        return false;
    };
    let ([name], ast::BindingShape::Identifier) = (parameter.names.as_slice(), parameter.shape)
    else {
        return false;
    };
    let (Some(parameter), Some(body)) =
        (file.source_text(name.span), file.source_text(function.body))
    else {
        return false;
    };
    let guard = format!("{parameter} instanceof Node &&");
    let node_is_global = file.ast.identifiers.iter().any(|identifier| {
        function.body.contains(identifier.span)
            && global_name(file, identifier.span) == Some("Node")
    });
    body.starts_with(&guard) && !body.contains(['|', '?']) && node_is_global
}

/// The global object's own names.
const GLOBAL_OBJECTS: &[&str] = &["globalThis", "window", "self", "global"];

/// The global object's names, and the window's for itself and its relatives
/// (`top`, `parent`, `frames`, `opener`): as a global or a member of the
/// global object, each is a window. A destructured key is the type's question
/// (review R12): `const { top } = rect` is no window.
const WINDOW_NAMES: &[&str] = &[
    "globalThis",
    "window",
    "self",
    "global",
    "top",
    "parent",
    "frames",
    "opener",
];

/// The globals that call what they are given with the window as `this`.
const PLATFORM_CALLERS: [&str; 3] = ["setTimeout", "setInterval", "addEventListener"];

/// What an obtained value may be, for [`handed_on`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Obtained {
    /// `Array`, `Array.prototype`, a prototype: no code of its own.
    Intrinsic,
    /// Possibly `Function` or `eval`: called, it runs a string.
    Function,
    /// The global object: every global, `Array` and `Function` among them.
    Global,
    /// An array that may hold the global object (`composedPath()`).
    Path,
}

/// [`crate::base::Hazard::IntrinsicHandedAtCall`]'s value: the call's ordinal,
/// what it receives, and where: an argument (59 and up as 59: past any
/// parameter a function names, so a rest parameter's or none's), 60 a path's
/// `find`, 61 a read-only builtin's, 62 a timer's callback, 63 the receiver.
fn handed_call_entry(ordinal: u32, slot: Slot, kind: Obtained) -> u32 {
    let kind = match kind {
        Obtained::Intrinsic => 0,
        Obtained::Function => 1,
        Obtained::Global => 2,
        Obtained::Path => 3,
    };
    let slot = match slot {
        Slot::Argument(index) => u32::try_from(index.min(ARGUMENT_CAP)).unwrap_or(59),
        Slot::PathFind => 60,
        Slot::ReadOnlyBuiltin => 61,
        Slot::TimerCallback => 62,
        Slot::This => 63,
    };
    (ordinal << 8) | (kind << 6) | slot
}

/// The last argument index [`handed_call_entry`] tells apart.
const ARGUMENT_CAP: usize = 59;

fn handed_call_of(entry: u32) -> (usize, Slot, Obtained) {
    let kind = match (entry >> 6) & 3 {
        0 => Obtained::Intrinsic,
        1 => Obtained::Function,
        2 => Obtained::Global,
        _ => Obtained::Path,
    };
    let slot = match entry & 63 {
        60 => Slot::PathFind,
        61 => Slot::ReadOnlyBuiltin,
        62 => Slot::TimerCallback,
        63 => Slot::This,
        index => Slot::Argument(index as usize),
    };
    ((entry >> 8) as usize, slot, kind)
}

/// ADR 0803, review a9fc #1 step 2: whether some call hands an obtained value
/// (the global object, `Function`, an array intrinsic) to code the checker
/// does not see. Seen: a member of a closed standard list that nothing wrote,
/// or the exact project or analyzed-dependency function the callee's value is
/// (review R19a), whose parameter, followed with the same rules, hands nothing
/// on. Anything else (unresolved, a declaration only, a value a call made, a
/// written binding or member, a rest parameter, `arguments`) is unseen.
pub(crate) fn handed_to_unseen_code(lookup: &SemanticLookup<'_>) -> bool {
    lookup
        .slot_entries(lookup.base_hazards())
        .any(|(file, entries)| {
            let mut candidates = None;
            entries.iter().any(|(hazard, entry)| match hazard {
                crate::base::Hazard::IntrinsicHandedAtCall => {
                    let (ordinal, slot, kind) = handed_call_of(*entry);
                    file.ast.calls.get(ordinal).is_none_or(|call| {
                        !received_by_seen_code(lookup, file, call, slot, kind, SEEN_DEPTH, None)
                    })
                }
                crate::base::Hazard::WindowValuedCandidate => candidates
                    .get_or_insert_with(|| file.window_valued_candidates())
                    .get(*entry as usize)
                    .is_none_or(|span| window_handed_to_unseen_code(lookup, file, *span)),
                _ => false,
            })
        })
}

/// Review R12: whether the value at `span`, if its type says it may be a
/// window (`Window`, `any`, `unknown`, or no type fact at all), reaches code
/// the checker does not see, followed as the global object is.
fn window_handed_to_unseen_code(lookup: &SemanticLookup<'_>, file: &FileFacts, span: Span) -> bool {
    // Review D4: `any` or `unknown` (a cast away, `(document as any)
    // .defaultView`) may be one; a concrete type that is no window
    // (`rect.top: number`, a project's `node.parent`) is not.
    let window = lookup
        .entity_at(file.path.as_str(), span)
        .is_none_or(|entity| {
            entity.library_types.as_deref().is_some_and(|types| {
                types
                    .iter()
                    .any(|name| matches!(name.as_ref(), "Window" | "globalThis"))
            }) || entity
                .runtime_value_domain
                .is_none_or(typefacts::RuntimeValueDomain::unknown)
        });
    if !window {
        return false;
    }
    let nested: AtCalls = std::cell::RefCell::new(Vec::new());
    let destructured = file.ast.bindings.iter().any(|binding| {
        binding
            .object_slots
            .iter()
            .any(|slot| slot.local.span == span)
    });
    let handed = if destructured {
        references_handed_on(file, span, Obtained::Global, &nested)
    } else {
        handed_on(file, span, Obtained::Global, HANDED_BUDGET, &nested)
    };
    handed || !nested_calls_seen(lookup, file, &nested, SEEN_DEPTH, None)
}

/// Whether some reference to the binding named at `name` hands a value of
/// `kind` on ([`handed_on`]); calls it reaches go to `nested`.
fn references_handed_on(file: &FileFacts, name: Span, kind: Obtained, nested: &AtCalls) -> bool {
    file.ast.identifiers.iter().any(|identifier| {
        identifier.role == ast::IdentifierRole::Reference
            && identifier.span != name
            && file.ast.reference_declaration(identifier.span) == Some(name)
            && !file
                .ast
                .write_targets
                .iter()
                .any(|target| file.ast.peel_ts_sugar_span(*target) == identifier.span)
            && handed_on(file, identifier.span, kind, HANDED_BUDGET, nested)
    })
}

/// Whether every call in `nested` receives its value as seen code.
fn nested_calls_seen(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    nested: &AtCalls,
    depth: usize,
    frame: Option<&Frame<'_>>,
) -> bool {
    nested.borrow().iter().all(|(span, slot, kind)| {
        file.ast
            .calls
            .iter()
            .find(|call| call.span == *span)
            .is_some_and(|call| {
                received_by_seen_code(lookup, file, call, *slot, *kind, depth, frame)
            })
    })
}

/// How many seen functions [`received_by_seen_code`] follows a value into.
const SEEN_DEPTH: usize = 3;

fn received_by_seen_code(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
    slot: Slot,
    kind: Obtained,
    depth: usize,
    frame: Option<&Frame<'_>>,
) -> bool {
    match slot {
        // A timer calls a function callback with the window as `this` (review
        // D3): seen only when the callback is an arrow or never mentions
        // `this`, and for `window.setTimeout` when that is still the timer.
        Slot::TimerCallback => {
            let member = file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(call.callee))
                .map(|member| member_key(file, member));
            let logical = logical_timer_keys(file, call);
            return member
                .is_none_or(|name| name.is_some_and(|name| standard_member_intact(lookup, name)))
                && logical
                    .iter()
                    .all(|name| standard_member_intact(lookup, name))
                && (argument_seen(
                    lookup,
                    file,
                    call,
                    0,
                    frame,
                    depth,
                    &|callee_file, function| {
                        function.kind == ast::FunctionKind::Arrow
                            || callee_file
                                .source_text(function.span)
                                .is_some_and(|text| !crate::execution_role::mentions_receiver(text))
                    },
                ) || timer_runs_promise_settler(lookup, file, call));
        }
        Slot::ReadOnlyBuiltin => {
            return read_only_builtin(file, call)
                .is_some_and(|name| standard_member_intact(lookup, &name));
        }
        Slot::PathFind => {
            return [
                "Array.find",
                "Node",
                "NodeConstructor.hasInstance",
                "Function.hasInstance",
            ]
            .iter()
            .all(|name| standard_member_intact(lookup, name));
        }
        Slot::This | Slot::Argument(_) => {}
    }
    // No exemption for the dialect's runtime: a primitive keeps what it is
    // given and hands it back to project code (`createSignal(x)`, a store,
    // a context), where this census cannot follow it, and calls a function
    // it is given (review D1).
    let standard = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call);
    let Slot::Argument(argument) = slot else {
        if kind == Obtained::Global && absent_window_member(lookup, file, call) {
            return true;
        }
        // A method run with the value as `this`: seen only for a closed list
        // of the window's methods that call nothing with it, and a listener
        // that can reach neither its `this` nor its event (review D3).
        // The value is the window, so whatever the types say (a dependency's
        // untyped parameter, `@types/node`'s `clearTimeout`, an invalid call)
        // its method is the window's by key, while nothing wrote it.
        let name = standard
            .map(|declaration| declaration.qualified_name.as_ref())
            .or_else(|| {
                let member = file
                    .ast
                    .member_at(file.ast.peel_ts_sugar_span(call.callee))?;
                (kind == Obtained::Global)
                    .then(|| member_key(file, member))
                    .flatten()
            });
        return name.is_some_and(|name| {
            standard_member_intact(lookup, name)
                && (!name.ends_with("dispatchEvent")
                    || dispatched_event_stays_inline(lookup, file, call))
                && (THIS_SEEN.iter().any(|(seen, _)| *seen == name)
                    || (LISTENER_ADDERS.contains(&name)
                        && argument_seen(
                            lookup,
                            file,
                            call,
                            1,
                            frame,
                            depth,
                            &listener_reaches_nothing,
                        )))
        });
    };
    if let Some(declaration) = standard {
        // A closed list of standard functions that neither call nor keep
        // what they are given ([`SEEN_STANDARD`]); `call`/`bind` run their
        // target with the arguments shifted by one; any other standard
        // function may call, store or schedule it (map, timers, `Map`,
        // `apply`, `Reflect.apply`): handed on.
        let name = declaration.qualified_name.as_ref();
        if seen_standard(name, argument) && standard_member_intact(lookup, name) {
            return true;
        }
        if !matches!(
            name,
            "CallableFunction.call" | "CallableFunction.bind" | "Function.call" | "Function.bind"
        ) || argument == 0
            || depth == 0
        {
            return false;
        }
        let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
            return false;
        };
        if file.ast.computed_members.contains(&member.span)
            || call
                .arguments
                .iter()
                .take(argument + 1)
                .any(|argument| argument.spread)
        {
            return false;
        }
        let Some((callee_file, function)) = function_value_at(lookup, file, member.object) else {
            return false;
        };
        let called = Called {
            file,
            call,
            shift: 1,
            parent: frame,
        };
        return parameter_hands_nothing(
            lookup,
            callee_file,
            function,
            argument - 1,
            kind,
            depth,
            called,
        );
    }
    if depth == 0 {
        return false;
    }
    // The exact function the callee's runtime value is, or nothing (review
    // R19a): the declaration a resolution names is not the value called (a
    // conditional whose arms reduce to one signature, an override).
    let Some((callee_file, function, true)) =
        crate::execution_role::declared_callee_at_parameter(file, call, argument, lookup)
    else {
        return false;
    };
    let called = Called {
        file,
        call,
        shift: 0,
        parent: frame,
    };
    parameter_hands_nothing(lookup, callee_file, function, argument, kind, depth, called)
}

/// The call a followed function was entered by: what its parameters hold at
/// the one call that hands the window on (review D21's T2, by call rather than
/// by every caller: another call with another target hands no window).
#[derive(Clone, Copy)]
struct Called<'f> {
    file: &'f FileFacts,
    call: &'f ast::CallFact,
    /// How many leading arguments no parameter takes (`call`'s `this`).
    shift: usize,
    parent: Option<&'f Frame<'f>>,
}

/// A followed function and the call that entered it.
struct Frame<'f> {
    function: &'f ast::FunctionFact,
    called: Called<'f>,
}

/// Decides one function a platform caller may call.
type CallbackTest<'t> = &'t dyn Fn(&FileFacts, &ast::FunctionFact) -> bool;

/// Whether the call's argument at `index` is a callback `test` passes.
fn argument_seen(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
    index: usize,
    frame: Option<&Frame<'_>>,
    depth: usize,
    test: CallbackTest<'_>,
) -> bool {
    !call
        .arguments
        .iter()
        .take(index + 1)
        .any(|argument| argument.spread)
        && call
            .arguments
            .get(index)
            .is_some_and(|argument| callback_seen(lookup, file, argument.span, frame, depth, test))
}

/// Whether every function the value at `span` may be passes `test`: a literal,
/// a name that holds one now, either branch of a conditional, `null` or
/// `undefined`, what a function returns on each of its paths, or a parameter
/// by what the entering call gave it. Anything else may be any function.
fn callback_seen(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    span: Span,
    frame: Option<&Frame<'_>>,
    depth: usize,
    test: CallbackTest<'_>,
) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(function) = function_literal(file, span) {
        return test(file, function);
    }
    if matches!(file.source_text(span), Some("null" | "undefined"))
        && file.ast.reference_declaration(span).is_none()
    {
        return true;
    }
    if depth == 0 {
        return false;
    }
    if let Some(conditional) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|conditional| conditional.span == span)
    {
        return callback_seen(lookup, file, conditional.consequent, frame, depth - 1, test)
            && callback_seen(lookup, file, conditional.alternate, frame, depth - 1, test);
    }
    if let Some(inner) = file.ast.call_at(span).filter(|inner| !inner.construct) {
        return call_result_seen(lookup, file, inner, frame, depth, test);
    }
    if let Some(frame) = frame
        && let Some(declaration) = file.ast.reference_declaration(span)
        && frame.function.span.contains(declaration)
        && let Some(argument) = parameter_argument(file, frame, declaration)
    {
        return callback_seen(
            lookup,
            frame.called.file,
            argument,
            frame.called.parent,
            depth - 1,
            test,
        );
    }
    if let Some(declaration) = file.ast.reference_declaration(span) {
        if let Some(functions) = assigned_literals(file, declaration) {
            return functions.into_iter().all(|function| test(file, function));
        }
        if let Some(seen) = closed_parameter_seen(lookup, file, declaration, depth, test) {
            return seen;
        }
        // A `const` holds what its initializing call returned.
        let held = file.ast.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == ast::BindingShape::Identifier
                && matches!(binding.names.as_slice(), [name] if name.span == declaration)
        });
        if let Some(inner) = held
            .and_then(|binding| binding.initializer)
            .and_then(|initializer| file.ast.call_at(file.ast.peel_ts_sugar_span(initializer)))
            .filter(|inner| !inner.construct)
        {
            return call_result_seen(lookup, file, inner, frame, depth, test);
        }
    }
    function_value_at(lookup, file, span)
        .is_some_and(|(callee_file, function)| test(callee_file, function))
}

/// The functions a binding declared with no value can hold, when every write
/// to it is a plain assignment of a literal (`let wake; wake = () => ...`): a
/// destructuring, a loop or `??=` is a write no assignment here names.
fn assigned_literals(file: &FileFacts, declaration: Span) -> Option<Vec<&ast::FunctionFact>> {
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.shape == ast::BindingShape::Identifier
            && matches!(binding.names.as_slice(), [name] if name.span == declaration)
    })?;
    let exported = file.ast.exports.iter().any(|export| {
        export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .any(|specifier| {
                specifier.local.span == declaration
                    || file.ast.reference_declaration(specifier.local.span) == Some(declaration)
            })
    });
    if binding.initializer.is_some() || exported {
        return None;
    }
    let writes = file
        .ast
        .write_targets
        .iter()
        .filter(|target| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(**target))
                == Some(declaration)
        })
        .count();
    let functions = file
        .ast
        .assignments
        .iter()
        .filter(|assignment| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(assignment.target))
                == Some(declaration)
        })
        .map(|assignment| {
            (!assignment.reads_target && assignment.logical.is_none())
                .then(|| function_literal(file, assignment.value_span))
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    (writes == functions.len() && !functions.is_empty()).then_some(functions)
}

/// A parameter whose function is entered only by direct calls in this file
/// (never exported, never a value): what each call gives it. `None` when
/// `declaration` is no such parameter (review D21, T2).
fn closed_parameter_seen(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    declaration: Span,
    depth: usize,
    test: CallbackTest<'_>,
) -> Option<bool> {
    let function = file.ast.functions.iter().find(|function| {
        function
            .parameters
            .iter()
            .any(|parameter| parameter.names.iter().any(|name| name.span == declaration))
    })?;
    let name = match function.kind {
        ast::FunctionKind::Declaration => function.name.as_ref()?.span,
        // A function expression's own name is an entry no binding shows
        // (review R52).
        _ if function.name.is_some() => return Some(false),
        _ => {
            file.ast
                .bindings
                .iter()
                .find(|binding| {
                    binding.immutable
                        && binding.shape == ast::BindingShape::Identifier
                        && binding.initializer.is_some_and(|initializer| {
                            let initializer = file.ast.peel_ts_sugar_span(initializer);
                            // `options.run ?? ((f) => ...)`: this function is
                            // reachable through the binding alone.
                            initializer == function.span
                                || file.ast.logical_expressions.iter().any(|logical| {
                                    logical.span == initializer
                                        && file.ast.peel_ts_sugar_span(logical.right)
                                            == function.span
                                })
                        })
                })?
                .names
                .first()?
                .span
        }
    };
    // Only a module-level binding can be exported: an exported function's
    // span contains every local of its body.
    let module_level = !file
        .ast
        .functions
        .iter()
        .any(|outer| outer.body.contains(name));
    let exported = file.ast.exports.iter().any(|export| {
        (module_level && export.span.contains(name))
            || export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .any(|specifier| {
                    specifier.local.span == name
                        || file.ast.reference_declaration(specifier.local.span) == Some(name)
                })
    });
    if exported {
        return Some(false);
    }
    let mut calls = Vec::new();
    for identifier in &file.ast.identifiers {
        if identifier.role != ast::IdentifierRole::Reference
            || identifier.span == name
            || file.ast.reference_declaration(identifier.span) != Some(name)
        {
            continue;
        }
        let Some(call) = file
            .ast
            .calls
            .iter()
            .find(|call| file.ast.peel_ts_sugar_span(call.callee) == identifier.span)
        else {
            return Some(false);
        };
        calls.push(call);
    }
    Some(calls.into_iter().all(|call| {
        let frame = Frame {
            function,
            called: Called {
                file,
                call,
                shift: 0,
                parent: None,
            },
        };
        parameter_argument(file, &frame, declaration)
            .is_some_and(|argument| callback_seen(lookup, file, argument, None, depth - 1, test))
    }))
}

/// Whether every value the exact function `inner` calls can return passes
/// `test`. Falling off the end returns `undefined`, which no platform calls.
fn call_result_seen(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    inner: &ast::CallFact,
    frame: Option<&Frame<'_>>,
    depth: usize,
    test: CallbackTest<'_>,
) -> bool {
    let Some((callee_file, function, true)) =
        crate::execution_role::declared_callee_at_parameter(file, inner, 0, lookup)
    else {
        return false;
    };
    if function.r#async || function.generator {
        return false;
    }
    let entered = Frame {
        function,
        called: Called {
            file,
            call: inner,
            shift: 0,
            parent: frame,
        },
    };
    function
        .expression_return
        .iter()
        .chain(callee_file.ast.own_returns(function))
        .filter_map(|returned| returned.argument)
        .all(|argument| {
            callback_seen(
                lookup,
                callee_file,
                argument,
                Some(&entered),
                depth - 1,
                test,
            )
        })
}

/// The argument the entering call gives the parameter declared at
/// `declaration`: a plain name, never written, with no default and no
/// `arguments` beside it. `None` when none can be named: the value is open.
fn parameter_argument(file: &FileFacts, frame: &Frame<'_>, declaration: Span) -> Option<Span> {
    let function = frame.function;
    let index = function.parameters.iter().position(|parameter| {
        parameter.shape == ast::BindingShape::Identifier
            && parameter.initializer.is_none()
            && matches!(parameter.names.as_slice(), [name] if name.span == declaration)
    })?;
    let written = file.ast.write_targets.iter().any(|target| {
        file.ast
            .reference_declaration(file.ast.peel_ts_sugar_span(*target))
            == Some(declaration)
    });
    let reads_arguments = function.kind != ast::FunctionKind::Arrow
        && file.ast.identifiers.iter().any(|identifier| {
            function.span.contains(identifier.span)
                && identifier.role == ast::IdentifierRole::Reference
                && file.source_text(identifier.span) == Some("arguments")
        });
    if written || reads_arguments {
        return None;
    }
    let position = index + frame.called.shift;
    let arguments = &frame.called.call.arguments;
    if arguments
        .iter()
        .take(position + 1)
        .any(|argument| argument.spread)
    {
        return None;
    }
    arguments.get(position).map(|argument| argument.span)
}

/// The sleep idiom (`new Promise((done) => setTimeout(done, ms))`, review D11):
/// whether the callback of the timer `call` is a settling function of a
/// `new Promise` the checker can name. The engine's `resolve` and `reject`
/// ignore `this` and run nothing with it. Exactly the callback and a delay (a
/// further argument is handed to them); the reference is the parameter itself,
/// never written; a non-arrow executor reads no `arguments`, which aliases its
/// parameters in sloppy code, and has no name it could call itself by with any
/// value (review R35); and `Promise` is the global, unwritten, standard.
fn timer_runs_promise_settler(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
) -> bool {
    let [callback, ..] = call.arguments.as_slice() else {
        return false;
    };
    if call.arguments.len() > 2 || call.arguments.iter().any(|argument| argument.spread) {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(callback.span);
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    let written = file.ast.write_targets.iter().any(|target| {
        file.ast
            .reference_declaration(file.ast.peel_ts_sugar_span(*target))
            == Some(declaration)
    });
    let Some(executor) = file.ast.functions.iter().find(|function| {
        function.parameters.iter().take(2).any(|parameter| {
            parameter.shape == ast::BindingShape::Identifier
                && matches!(parameter.names.as_slice(), [name] if name.span == declaration)
        })
    }) else {
        return false;
    };
    let reads_arguments = file.ast.identifiers.iter().any(|identifier| {
        executor.span.contains(identifier.span)
            && identifier.role == ast::IdentifierRole::Reference
            && file.source_text(identifier.span) == Some("arguments")
    });
    let promise = file.ast.calls.iter().find(|construct| {
        construct.construct
            && construct
                .arguments
                .first()
                .is_some_and(|argument| file.ast.peel_ts_sugar_span(argument.span) == executor.span)
    });
    !written
        && (executor.kind == ast::FunctionKind::Arrow
            || (!reads_arguments && executor.name.is_none()))
        && promise.is_some_and(|promise| {
            global_name(file, file.ast.peel_ts_sugar_span(promise.callee)) == Some("Promise")
                && lookup
                    .standard_library_declaration(file, promise, typefacts::CallKind::Construct)
                    .is_some_and(|declaration| {
                        declaration.qualified_name.as_ref() == "PromiseConstructor.construct"
                    })
                && standard_member_intact(lookup, "Promise")
        })
}

/// The only `__` members a browser defines: `Object.prototype`'s (Chromium 148,
/// walked over the window's prototype chain).
const NATIVE_DUNDER: [&str; 5] = [
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "__proto__",
];

/// `(window as any).__log?.(x)`: a member of the global object no declaration
/// resolves, under a `__` literal key no platform defines and nothing analyzed
/// can install: no write of the name on any receiver, a window prototype's, or
/// `Object.prototype`'s, no reflective definition of it, no declaration of the
/// name in a classic script. It is absent, so the window reaches no code. A
/// member installed from outside the analyzed program is outside ADR 0803's
/// premise (review D24).
fn absent_window_member(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
) -> bool {
    let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    let Some(key) = member_key(file, member) else {
        return false;
    };
    key.starts_with("__")
        && !NATIVE_DUNDER.contains(&key)
        && lookup
            .resolved_callee_call(file, call.callee)
            .is_none_or(|resolved| resolved.declaration.is_none())
        && standard_member_intact(lookup, key)
        && ["Object", "Window", "EventTarget"]
            .iter()
            .all(|owner| standard_member_intact(lookup, &format!("{owner}.{key}")))
        && !lookup.property_may_be_defined_reflectively_as(key)
        && !lookup.hazard_present(crate::base::contributions::classic_script_dunder(key))
}

/// The global event classes a program dispatches, whose constructors keep no
/// reference to the event.
const EVENT_CLASSES: [&str; 10] = [
    "Event",
    "CustomEvent",
    "StorageEvent",
    "MessageEvent",
    "KeyboardEvent",
    "MouseEvent",
    "PointerEvent",
    "FocusEvent",
    "PopStateEvent",
    "HashChangeEvent",
];

/// Review D21: the event `dispatchEvent` is given keeps `target`, the window,
/// once dispatched. Seen only when nothing else can read it: `new Event(...)`
/// in place, or a binding the call is the only use of.
fn dispatched_event_stays_inline(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    call: &ast::CallFact,
) -> bool {
    let [event] = call.arguments.as_slice() else {
        return false;
    };
    if event.spread {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(event.span);
    // The global `Event` or `CustomEvent` itself: a subclass's constructor can
    // keep the event, a shadowing class is another class (review of 37ce9020a).
    let constructed = |span: Span| {
        file.ast.call_at(span).is_some_and(|construct| {
            construct.construct
                && global_name(file, file.ast.peel_ts_sugar_span(construct.callee))
                    .is_some_and(|name| EVENT_CLASSES.contains(&name))
                && lookup
                    .standard_library_declaration(file, construct, typefacts::CallKind::Construct)
                    .is_some()
        })
    };
    if constructed(span) {
        return true;
    }
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    let single = file
        .ast
        .identifiers
        .iter()
        .filter(|identifier| {
            identifier.role == ast::IdentifierRole::Reference
                && file.ast.reference_declaration(identifier.span) == Some(declaration)
        })
        .all(|identifier| identifier.span == span);
    single
        && file.ast.bindings.iter().any(|binding| {
            binding.immutable
                && binding.shape == ast::BindingShape::Identifier
                && matches!(binding.names.as_slice(), [name] if name.span == declaration)
                && binding.initializer.is_some_and(|initializer| {
                    constructed(file.ast.peel_ts_sugar_span(initializer))
                })
        })
}

/// Whether nothing replaced the standard member `qualified_name`: no write of
/// a member of its name on any receiver (`window.scrollTo = f`), and none of
/// the writes [`crate::base::contributions::patch_veto`] names: a standard
/// root's own member by any key, a definer's, a global of its name or its
/// owner's (review R19b).
fn standard_member_intact(lookup: &SemanticLookup<'_>, qualified_name: &str) -> bool {
    let member = qualified_name
        .rsplit_once('.')
        .map_or(qualified_name, |(_, member)| member);
    // The base index, not the assigned-member memo: this census runs inside
    // the member-stability memo probe, which may read no memo.
    !lookup.member_name_assigned_in_base(member)
        && !(lookup.hazard_present(crate::base::Hazard::AnyStandardPatch)
            && crate::base::contributions::patch_veto(qualified_name)
                .into_iter()
                .any(|hazard| lookup.hazard_present(hazard)))
}

/// The function the value at `span` is: a name whose binding holds it now
/// (review R19a). A member is no such name: its method may be overridden or
/// written. A value a call made (`const h = make(() => {})`) holds no function
/// of the initializer.
fn function_value_at<'a>(
    lookup: &SemanticLookup<'a>,
    file: &FileFacts,
    span: Span,
) -> Option<(&'a FileFacts, &'a ast::FunctionFact)> {
    let span = file.ast.peel_ts_sugar_span(span);
    if file.ast.member_at(span).is_some() {
        return None;
    }
    lookup
        .function_called_at(file.path.as_str(), span)
        .filter(|(callee_file, function)| lookup.function_value_is_current(callee_file, function))
}

/// The window's methods that, run with the window as `this`, call no code
/// with it, by declaration (as a `Window` member and as the global function),
/// and why. Closed (review D3).
const THIS_SEEN: &[(&str, &str)] = &[
    (
        "Window.getComputedStyle",
        "reads an element's computed style",
    ),
    ("getComputedStyle", "reads an element's computed style"),
    ("Window.matchMedia", "returns a query list"),
    ("matchMedia", "returns a query list"),
    ("Window.scrollTo", "scrolls"),
    ("scrollTo", "scrolls"),
    ("Window.scrollBy", "scrolls"),
    ("scrollBy", "scrolls"),
    ("Window.scroll", "scrolls"),
    ("scroll", "scrolls"),
    (
        "Window.postMessage",
        "queues a message event, whose listeners are each decided where added",
    ),
    (
        "postMessage",
        "queues a message event, whose listeners are each decided where added",
    ),
    ("Window.clearTimeout", "cancels a timer"),
    ("WindowOrWorkerGlobalScope.clearTimeout", "cancels a timer"),
    ("clearTimeout", "cancels a timer"),
    ("Window.clearInterval", "cancels a timer"),
    ("WindowOrWorkerGlobalScope.clearInterval", "cancels a timer"),
    ("clearInterval", "cancels a timer"),
    ("Window.cancelAnimationFrame", "cancels a frame callback"),
    (
        "AnimationFrameProvider.cancelAnimationFrame",
        "cancels a frame callback",
    ),
    ("cancelAnimationFrame", "cancels a frame callback"),
    ("Window.cancelIdleCallback", "cancels an idle callback"),
    ("cancelIdleCallback", "cancels an idle callback"),
    ("Window.alert", "shows a dialog"),
    ("alert", "shows a dialog"),
    ("Window.confirm", "shows a dialog"),
    ("confirm", "shows a dialog"),
    ("Window.prompt", "shows a dialog"),
    ("prompt", "shows a dialog"),
    (
        "EventTarget.dispatchEvent",
        "runs listeners, each decided where it is added",
    ),
    (
        "Window.dispatchEvent",
        "runs listeners, each decided where it is added",
    ),
    (
        "dispatchEvent",
        "runs listeners, each decided where it is added",
    ),
    ("Window.removeEventListener", "removes a listener"),
    ("EventTarget.removeEventListener", "removes a listener"),
    ("removeEventListener", "removes a listener"),
];

/// The methods that add a listener the platform calls with the receiver as
/// `this` and an event that reaches it ([`listener_reaches_nothing`]).
const LISTENER_ADDERS: [&str; 3] = [
    "Window.addEventListener",
    "EventTarget.addEventListener",
    "addEventListener",
];

/// Standard functions that neither call nor keep what they are given, by
/// declaration, whether the second argument is exempt too, and why. Closed: a
/// function missing here is handed the value; only index 0 counts unless the
/// entry says both (review D1: a key argument is coerced through code the
/// program may own). `getPrototypeOf`'s result is the obtained arm's own question.
const SEEN_STANDARD: &[(&str, bool, &str)] = &[
    (
        "ObjectConstructor.keys",
        false,
        "returns the own enumerable key names",
    ),
    (
        "ObjectConstructor.getOwnPropertyNames",
        false,
        "returns the own key names",
    ),
    (
        "ObjectConstructor.getOwnPropertySymbols",
        false,
        "returns the own symbol keys",
    ),
    (
        "ObjectConstructor.getPrototypeOf",
        false,
        "returns the prototype, which the getPrototypeOf arm follows",
    ),
    ("ObjectConstructor.isFrozen", false, "tests"),
    ("ObjectConstructor.isSealed", false, "tests"),
    ("ObjectConstructor.isExtensible", false, "tests"),
    (
        "ObjectConstructor.is",
        true,
        "compares its two arguments, coercing neither",
    ),
    (
        "ObjectConstructor.hasOwn",
        false,
        "tests one key (the key argument is coerced: index 0 only)",
    ),
    ("ArrayConstructor.isArray", false, "tests"),
    ("Reflect.ownKeys", false, "returns the own keys"),
    (
        "Reflect.has",
        false,
        "tests one key (the key argument is coerced: index 0 only)",
    ),
    ("Reflect.isExtensible", false, "tests"),
    (
        "Reflect.getPrototypeOf",
        false,
        "returns the prototype, which the getPrototypeOf arm follows",
    ),
];

/// Whether [`SEEN_STANDARD`] exempts argument `index` of `name`.
fn seen_standard(name: &str, index: usize) -> bool {
    SEEN_STANDARD
        .iter()
        .any(|(seen, both, _)| *seen == name && (index == 0 || (index == 1 && *both)))
}

/// Whether `function`'s parameter at `index`, given an obtained value of
/// `kind`, hands it on to nothing the checker cannot see: followed with
/// [`handed_on`]'s rules, and each call it reaches through seen code.
fn parameter_hands_nothing(
    lookup: &SemanticLookup<'_>,
    callee_file: &FileFacts,
    function: &ast::FunctionFact,
    index: usize,
    kind: Obtained,
    depth: usize,
    called: Called<'_>,
) -> bool {
    let arguments_read = callee_file.ast.identifiers.iter().any(|identifier| {
        function.span.contains(identifier.span)
            && identifier.role == ast::IdentifierRole::Reference
            && callee_file.source_text(identifier.span) == Some("arguments")
    });
    if arguments_read {
        return false;
    }
    // An argument at the cap stands for every later one.
    if index >= ARGUMENT_CAP && function.parameters.len() > ARGUMENT_CAP {
        return false;
    }
    let Some(parameter) = function.parameters.get(index) else {
        // Past the named parameters: a rest parameter takes it, else nothing.
        return !function.rest_parameter;
    };
    let [name] = parameter.names.as_slice() else {
        return false;
    };
    if parameter.shape != ast::BindingShape::Identifier {
        return false;
    }
    let nested: AtCalls = std::cell::RefCell::new(Vec::new());
    let frame = Frame { function, called };
    !references_handed_on(callee_file, name.span, kind, &nested)
        && nested_calls_seen(lookup, callee_file, &nested, depth - 1, Some(&frame))
}

/// How many bindings [`handed_on`] follows before it answers yes.
const HANDED_BUDGET: usize = 8;

/// ADR 0803, item 3: whether the value at `span`, an array intrinsic, possibly
/// `Function`, or the global object (`kind`), may reach code the checker does
/// not see.
/// Not handed on: compared or tested, a discarded statement, read through
/// (`Array.prototype.slice`), a class heritage, an argument of a read-only
/// built-in (`Object.keys`), and a `const`/`let` or assigned binding whose
/// uses are each one of these and which is not exported. Called: an array
/// intrinsic is no code; `Function` called runs a string. Anything else,
/// passed to another call, stored, returned or spread, is handed on.
fn handed_on(
    file: &FileFacts,
    span: Span,
    kind: Obtained,
    budget: usize,
    at_calls: &AtCalls,
) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    if budget == 0 {
        return true;
    }
    if only_compared(file, span)
        || tested(file, span)
        || discarded(file, span)
        || file.ast.key_probed_operands.contains(&span)
        || file.ast.inert_operands.contains(&span)
    {
        return false;
    }
    // An operand of `||`, `??`, `&&`, or a branch of `?:`: the value goes on
    // as the whole expression's. A conditional's test goes nowhere.
    if let Some(logical) = file
        .ast
        .logical_expressions
        .iter()
        .find(|logical| logical.left == span || logical.right == span)
    {
        return handed_on(file, logical.span, kind, budget - 1, at_calls);
    }
    if let Some(conditional) = file.ast.conditional_expressions.iter().find(|conditional| {
        conditional.test == span || conditional.consequent == span || conditional.alternate == span
    }) {
        return conditional.test != span
            && handed_on(file, conditional.span, kind, budget - 1, at_calls);
    }
    if let Some(member) = file
        .ast
        .members
        .iter()
        .find(|member| file.ast.peel_ts_sugar_span(member.object) == span)
    {
        // Written through: the value may be the intrinsic itself, which the
        // visible side resolves only when exact, so a key that may reach
        // iteration counts here (a literal other key cannot). Read through a
        // member that reaches another intrinsic (`.prototype`,
        // `.constructor`, `.__proto__`, a key no literal names), the chain
        // goes on.
        let key = member_key(file, member);
        let written = file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .any(|target| file.ast.peel_ts_sugar_span(*target) == member.span);
        if written {
            // A write to the global object replaces a global, never what
            // array literals iterate with; but an event handler (`on*`, or a
            // key no literal names) is called with the window as `this` and
            // an event that reaches it (review D3).
            return if kind == Obtained::Global {
                key.is_none_or(|key| key.starts_with("on")) && !inert_handler(file, member.span)
            } else {
                key.is_none_or(|key| matches!(key, "next" | "return" | "__proto__"))
            };
        }
        let next = |kind: Obtained| handed_on(file, member.span, kind, budget - 1, at_calls);
        return match (kind, key) {
            // A path's own answers about what it holds hand nothing on; an
            // element may be the global object, and every other use reaches
            // the elements.
            (Obtained::Path, Some("includes" | "indexOf" | "lastIndexOf" | "length")) => false,
            (Obtained::Path, Some("some")) => !file.ast.calls.iter().any(|call| {
                file.ast.peel_ts_sugar_span(call.callee) == member.span
                    && callback_only_compares(file, call)
            }),
            (Obtained::Path, Some("find")) => {
                let guarded = file.ast.calls.iter().find(|call| {
                    file.ast.peel_ts_sugar_span(call.callee) == member.span
                        && callback_guards_node(file, call)
                });
                if let Some(call) = guarded {
                    at_calls
                        .borrow_mut()
                        .push((call.span, Slot::PathFind, kind));
                }
                guarded.is_none()
            }
            (Obtained::Path, Some(_)) => true,
            (Obtained::Path, None) => next(Obtained::Global),
            // The global object again, or one of its intrinsics by name.
            (Obtained::Global, Some("Array")) => next(Obtained::Intrinsic),
            (Obtained::Global, Some("Function" | "eval" | "Reflect" | "Object")) => {
                next(Obtained::Function)
            }
            (Obtained::Global, Some(key)) if WINDOW_NAMES.contains(&key) => next(Obtained::Global),
            (Obtained::Global, None) => next(Obtained::Global),
            // A value that may be `Function` or `eval` (a global under a key
            // no literal names among them), invoked through `call`, `apply` or
            // `bind`: code from a string.
            (Obtained::Function | Obtained::Global, Some("call" | "apply" | "bind")) => true,
            (_, Some("prototype" | "__proto__")) => next(Obtained::Intrinsic),
            (_, Some("constructor") | None) => next(Obtained::Function),
            // Any other method called on it runs with it as `this` (review
            // D3): whether that method is seen code is the project's question.
            // A timer of the window's calls its callback with it instead
            // (review R19 D8), and either, as a value, goes on as `Function`.
            (_, Some(key)) => {
                let call = file
                    .ast
                    .calls
                    .iter()
                    .find(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span);
                match call {
                    // A tag runs with the value as `this`, and no call fact
                    // records it.
                    None if tagged(file, member.span) => true,
                    None => {
                        kind == Obtained::Global
                            && PLATFORM_CALLERS.contains(&key)
                            && next(Obtained::Function)
                    }
                    Some(call) => {
                        let timer = kind == Obtained::Global
                            && matches!(
                                key,
                                "setTimeout"
                                    | "setInterval"
                                    | "requestAnimationFrame"
                                    | "requestIdleCallback"
                            );
                        let slot = if timer {
                            Slot::TimerCallback
                        } else {
                            Slot::This
                        };
                        at_calls.borrow_mut().push((call.span, slot, kind));
                        false
                    }
                }
            }
        };
    }
    if file
        .ast
        .calls
        .iter()
        .any(|call| file.ast.peel_ts_sugar_span(call.callee) == span)
    {
        return kind != Obtained::Intrinsic;
    }
    if file.ast.classes.iter().any(|class| {
        class
            .heritage
            .is_some_and(|heritage| file.ast.peel_ts_sugar_span(heritage) == span)
    }) {
        return false;
    }
    if let Some(call) = file.ast.calls.iter().find(|call| {
        call.arguments
            .iter()
            .any(|argument| file.ast.peel_ts_sugar_span(argument.span) == span)
    }) {
        let index = call
            .arguments
            .iter()
            .position(|argument| file.ast.peel_ts_sugar_span(argument.span) == span)
            .unwrap_or_default();
        let first = index == 0 && !call.arguments[0].spread;
        // The reflective mutators on the global object may install a handler
        // (`Object.assign(window, { onclick })`): handed on.
        if !(first && kind != Obtained::Global && reflective_target(file, span)) {
            // Whether the callee is code the checker sees is decided over the
            // project ([`handed_to_unseen_code`]); recorded, not counted here.
            at_calls
                .borrow_mut()
                .push((call.span, argument_slot(file, call, index), kind));
        }
        return false;
    }
    let declaration = file
        .ast
        .bindings
        .iter()
        .find(|binding| {
            binding.shape == ast::BindingShape::Identifier
                && binding.names.len() == 1
                && binding
                    .initializer
                    .is_some_and(|initializer| file.ast.peel_ts_sugar_span(initializer) == span)
        })
        .map(|binding| binding.names[0].span)
        .or_else(|| {
            file.ast
                .assignments
                .iter()
                .find(|assignment| {
                    file.ast.peel_ts_sugar_span(assignment.value_span) == span
                        && !assignment.reads_target
                        && assignment.logical.is_none()
                        && file.ast.member_at(assignment.target).is_none()
                })
                .and_then(|assignment| {
                    // The assignment's own value goes on too, unless tested,
                    // compared, discarded, or the first of a sequence.
                    let whole = Span::new(assignment.target.start, span.end);
                    let before = file
                        .source
                        .get(..whole.start as usize)
                        .unwrap_or_default()
                        .trim_end();
                    let after = file
                        .source
                        .get(whole.end as usize..)
                        .unwrap_or_default()
                        .trim_start();
                    let sequence = before.ends_with('(') && after.starts_with(',');
                    (tested(file, whole)
                        || only_compared(file, whole)
                        || discarded(file, whole)
                        || sequence)
                        .then(|| file.ast.reference_declaration(assignment.target))
                        .flatten()
                })
        });
    let Some(declaration) = declaration else {
        // Stored (an object, an array, a collection), returned, spread:
        // handed on, whatever it is (review R2, R7).
        return true;
    };
    let exported = file.ast.exports.iter().any(|export| {
        export
            .specifiers
            .iter()
            .chain(export.declarations.iter())
            .any(|specifier| specifier.local.span == declaration)
    });
    exported
        || file.ast.identifiers.iter().any(|identifier| {
            identifier.role == ast::IdentifierRole::Reference
                && identifier.span != declaration
                && file.ast.reference_declaration(identifier.span) == Some(declaration)
                && !file
                    .ast
                    .write_targets
                    .iter()
                    .any(|target| file.ast.peel_ts_sugar_span(*target) == identifier.span)
                && handed_on(file, identifier.span, kind, budget - 1, at_calls)
        })
}

/// `!value` or `if (value)`: the value is only tested.
fn tested(file: &FileFacts, span: Span) -> bool {
    let before = file.source.get(..span.start as usize).unwrap_or_default();
    let after = file
        .source
        .get(span.end as usize..)
        .unwrap_or_default()
        .trim_start();
    let head = before.trim_end_matches(|c: char| c.is_whitespace() || c == '(');
    head.ends_with('!')
        || (after.starts_with(')')
            && before.trim_end().ends_with('(')
            && head.strip_suffix("if").is_some_and(|rest| {
                !rest
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.'))
            }))
}

/// An expression statement of its own: the value goes nowhere.
fn discarded(file: &FileFacts, span: Span) -> bool {
    let before = file
        .source
        .get(..span.start as usize)
        .unwrap_or_default()
        .trim_end();
    let after = file
        .source
        .get(span.end as usize..)
        .unwrap_or_default()
        .trim_start();
    (before.is_empty() || before.ends_with([';', '{', '}']))
        && (after.is_empty() || after.starts_with([';', '}']))
}

/// The declaration `Object.keys(x)` and its like would have: a global's
/// member by its spelling, which [`SEEN_STANDARD`] lists. Whether nothing wrote
/// the member is the project's question ([`standard_member_intact`]).
fn read_only_builtin(file: &FileFacts, call: &ast::CallFact) -> Option<String> {
    let member = file
        .ast
        .member_at(file.ast.peel_ts_sugar_span(call.callee))?;
    if file.ast.computed_members.contains(&member.span) {
        return None;
    }
    let property = file.source_text(member.property)?;
    let owner = match global_name(file, file.ast.peel_ts_sugar_span(member.object))? {
        "Object" => "ObjectConstructor",
        "Array" => "ArrayConstructor",
        "Reflect" => "Reflect",
        _ => return None,
    };
    Some(format!("{owner}.{property}"))
}

/// Where a call receives the value at argument `index`: a listed builtin's
/// exempt position (no spread can move it), or an ordinary argument. A key
/// argument is coerced through code the program may own (review D8).
fn argument_slot(file: &FileFacts, call: &ast::CallFact, index: usize) -> Slot {
    let exempt = !call
        .arguments
        .iter()
        .take(index + 1)
        .any(|argument| argument.spread)
        && read_only_builtin(file, call).is_some_and(|name| seen_standard(&name, index));
    if exempt {
        Slot::ReadOnlyBuiltin
    } else {
        Slot::Argument(index)
    }
}

/// Whether `call`'s callback is an arrow whose every parameter is a plain name
/// only compared or tested: `path.some((t) => t === el)`. An element may be the
/// global object, so anything else reaches it.
fn callback_only_compares(file: &FileFacts, call: &ast::CallFact) -> bool {
    let Some(function) = call
        .arguments
        .first()
        .filter(|argument| !argument.spread)
        .and_then(|argument| function_literal(file, argument.span))
    else {
        return false;
    };
    function.kind == ast::FunctionKind::Arrow
        && function.rest_parameter_names.is_empty()
        && function.parameters.iter().all(|parameter| {
            parameter.shape == ast::BindingShape::Identifier
                && parameter.names.iter().all(|name| {
                    file.ast.identifiers.iter().all(|identifier| {
                        identifier.role != ast::IdentifierRole::Reference
                            || identifier.span == name.span
                            || file.ast.reference_declaration(identifier.span) != Some(name.span)
                            || only_compared(file, identifier.span)
                            || tested(file, identifier.span)
                    })
                })
        })
}

/// ADR 0801: whether a hook's accessor slot may not be what the literal holds.
/// A local `const [a] = createSignal(0)` rests on the same protocol with no
/// census at all. What this asks is whether anything can reach that protocol:
/// a program that never obtains an intrinsic cannot change it, whatever it
/// writes. [`tuple_iteration_write`] refuses every call it cannot name, which
/// any callback prop is, and every computed write. Both answers are hazard
/// probes, so a reuse records which files' hazards it read.
pub(crate) fn tuple_slot_iteration_write(lookup: &SemanticLookup<'_>) -> bool {
    lookup.program_has_eval()
        || lookup.hazard_present(crate::base::Hazard::ArrayIntrinsicObtained)
        || lookup.intrinsic_handed_to_unseen_code()
}

pub(crate) fn tuple_iteration_write(lookup: &SemanticLookup<'_>) -> bool {
    if lookup.visible_string_code() {
        return true;
    }
    lookup.files().iter().any(|file| {
        let targets = file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .map(|span| file.ast.peel_ts_sugar_span(*span))
            .collect::<HashSet<_>>();
        file.ast.members.iter().any(|member| {
            targets.contains(&member.span)
                && iteration_key(member_key(file, member))
                && !separate_iteration_target(file, member.object, 24)
        }) || file.ast.calls.iter().any(|call| {
            let Some(method) = tuple_mutation_method(file, call, lookup) else {
                // An erased reflective signature cannot use its receiver or
                // first argument as the mutator's target. These spellings are
                // possible invocation only, never positive callee authority.
                if lookup
                    .standard_library_declaration(file, call, typefacts::CallKind::Call)
                    .is_none()
                    && file
                        .ast
                        .members_at(file.ast.peel_ts_sugar_span(call.callee))
                        .any(|member| {
                            matches!(
                                member_key(file, member),
                                None | Some("call" | "apply" | "bind")
                            )
                        })
                    && !iteration_source_function(file, call.callee, lookup)
                {
                    return true;
                }
                // Missing callee evidence must not turn an opaque mutation
                // helper into a negative census. A supplied receiver that may
                // be an intrinsic retains the veto. Source implementations
                // are covered by the same project-wide write census; exact
                // non-mutating library signatures are outside this catalog.
                return lookup.primitive_at_call(file, call.span).is_none()
                    && !intrinsic_disjoint_setter(file, call, lookup)
                    && !intrinsic_disjoint_proxy_creation(file, call, lookup)
                    && !source_bound_invocation(file, call.callee, lookup)
                    && lookup
                        .standard_library_declaration(file, call, typefacts::CallKind::Call)
                        .is_none()
                    // A factory's declaration does not describe the callable
                    // it returns. Its object handoff needs its own census.
                    && (file.ast.call_at(file.ast.peel_ts_sugar_span(call.callee)).is_some()
                        || (lookup
                            .function_called_at(
                                file.path.as_str(),
                                file.ast.peel_ts_sugar_span(call.callee),
                            )
                            .is_none()
                            // A project member (`Object.defineProperty` of a
                            // shadowing namespace) is a known function by its own
                            // resolution, not by its object's leading token. The
                            // resolved declaration is read here, not the member
                            // index, which hung when read from inside this veto.
                            && !resolves_to_project_function(lookup, file, call)))
                    && call.arguments.first().is_some_and(|target| {
                        target.spread
                            || (!matches!(
                                target.runtime_value_kind,
                                ast::RuntimeValueKind::Primitive
                                    | ast::RuntimeValueKind::Nullish
                                    | ast::RuntimeValueKind::Function
                            ) && !separate_iteration_target(file, target.span, 24))
                    });
            };
            if method == "stringCode" {
                return true;
            }
            if matches!(method, "invoke" | "reflectInvoke") {
                if source_bound_invocation(file, call.callee, lookup) {
                    return false;
                }
                // Receiver/argument slots are different for reflective calls.
                // Only a source-resolved implementation discharges the target;
                // its writes are covered by this same whole-program census.
                let target = if method == "reflectInvoke" {
                    call.arguments
                        .first()
                        .filter(|argument| !argument.spread)
                        .map(|argument| argument.span)
                } else {
                    file.ast
                        .member_at(file.ast.peel_ts_sugar_span(call.callee))
                        .map(|member| member.object)
                };
                return !invocation_operator_is_exact(file, call, method, lookup)
                    || target
                        .is_none_or(|target| !iteration_source_function(file, target, lookup));
            }
            if call.arguments.iter().any(|argument| argument.spread) {
                return true;
            }
            let Some(target) = call.arguments.first() else {
                return true;
            };
            if separate_iteration_target(file, target.span, 24) {
                return false;
            }
            match method {
                "setPrototypeOf" => true,
                "assign" => call
                    .arguments
                    .iter()
                    .skip(1)
                    .any(|argument| iteration_map(file, argument.span)),
                "defineProperties" => call
                    .arguments
                    .get(1)
                    .is_none_or(|argument| iteration_map(file, argument.span)),
                _ => iteration_key(call.arguments.get(1).and_then(|argument| {
                    match &argument.literal_value {
                        ast::ArgumentLiteralFact::String(key) => Some(key.as_ref()),
                        ast::ArgumentLiteralFact::Integer(_) => Some("0"),
                        _ => None,
                    }
                })),
            }
        })
    })
}

/// This census withholds authority: it never proves that a gateway ran or
/// mutated a particular property. Erased signatures cannot hide visible code
/// construction, including source slots and returned gateway values.
pub(crate) fn visible_string_code(lookup: &SemanticLookup<'_>) -> bool {
    lookup.program_has_eval()
        || lookup.files().iter().any(|file| {
            file.ast.calls.iter().any(|call| {
                string_code_reference(file, call.callee, lookup, 24, false)
                    || tuple_mutation_method(file, call, lookup) == Some("stringCode")
                    || (call.construct
                        && lookup
                            .standard_library_declaration(
                                file,
                                call,
                                typefacts::CallKind::Construct,
                            )
                            .is_some_and(|declaration| {
                                declaration
                                    .qualified_name
                                    .starts_with("FunctionConstructor")
                            }))
            })
        })
}

pub(super) fn string_code_reference(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    returned_value: bool,
) -> bool {
    if depth == 0 {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(branch) = file.ast.conditional_at(span) {
        return string_code_reference(file, branch.consequent, lookup, depth - 1, returned_value)
            || string_code_reference(file, branch.alternate, lookup, depth - 1, returned_value);
    }
    if let Some(branch) = file.ast.logical_at(span) {
        return string_code_reference(file, branch.left, lookup, depth - 1, returned_value)
            || string_code_reference(file, branch.right, lookup, depth - 1, returned_value);
    }
    if let Some(member) = file.ast.member_at(span) {
        return string_code_slot(
            file,
            member.object,
            member_key(file, member),
            lookup,
            depth - 1,
            false,
        );
    }
    if let Some(call) = file.ast.call_at(span) {
        if string_code_reference(file, call.callee, lookup, depth - 1, false) {
            return true;
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if gateway_binding(file, callee, lookup).is_some_and(|(origin, binding, declaration)| {
            !binding.immutable || iteration_binding_written(origin, declaration)
        }) || file
            .ast
            .reference_declaration(callee)
            .is_some_and(|declaration| iteration_binding_written(file, declaration))
        {
            return true;
        }
        // Source implementation, not an identifier's nominal signature, owns
        // the value returned by a visible gateway factory.
        if let Some((origin, function)) =
            lookup.function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
        {
            return function
                .expression_return
                .as_ref()
                .filter(|returned| {
                    !matches!(
                        returned.runtime_value_kind,
                        ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                    )
                })
                .and_then(|returned| returned.argument)
                .into_iter()
                .chain(
                    origin
                        .ast
                        .returns
                        .iter()
                        .filter(|returned| {
                            containing_ast_function(&origin.ast, returned.span)
                                .is_some_and(|owner| owner.span == function.span)
                        })
                        .filter(|returned| {
                            !matches!(
                                returned.runtime_value_kind,
                                ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                            )
                        })
                        .filter_map(|returned| returned.argument),
                )
                .any(|returned| string_code_reference(origin, returned, lookup, depth - 1, true));
        }
        return string_code_reference(file, call.callee, lookup, depth - 1, returned_value);
    }
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == span)
    {
        // Unsupported expression transports (a sequence, for example) do
        // not prove absence. Exact fresh functions/allocations are not code
        // constructors; their bodies still participate in the call census.
        return !file
            .ast
            .functions
            .iter()
            .any(|function| function.span == span)
            && !file.ast.array_literals.contains(&span)
            && !file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == span);
    }
    let Some((file, binding, declaration)) = gateway_binding(file, span, lookup) else {
        if file
            .ast
            .reference_declaration(span)
            .is_some_and(|declaration| gateway_assignment(file, declaration, lookup, depth))
        {
            return true;
        }
        return matches!(file.source_text(span), Some("Function" | "eval"))
            && file.ast.reference_declaration(span).is_none()
            || (returned_value
                && !file.ast.functions.iter().any(|function| {
                    function
                        .name
                        .as_ref()
                        .is_some_and(|name| Some(name.span) == file.ast.reference_declaration(span))
                        && lookup.function_value_is_current(file, function)
                }));
    };
    if gateway_assignment(file, declaration, lookup, depth) {
        return true;
    }
    let Some(initializer) = binding.initializer else {
        return returned_value;
    };
    if binding.shape == ast::BindingShape::Object {
        return binding
            .object_slots
            .iter()
            .find(|slot| slot.local.span == declaration)
            .is_none_or(|slot| {
                string_code_slot(
                    file,
                    initializer,
                    Some(slot.property.as_str()),
                    lookup,
                    depth - 1,
                    false,
                )
            });
    }
    // ADR 0511: an array pattern over a binding holding an array literal
    // (`const pair = [0, eval]; const [, f] = pair;`) takes its slot from that
    // literal as from one written in place, where the holder is an immutable,
    // never-written `const` this pattern is the one reference to (nothing
    // else can replace the array or a slot). Any other holder of a literal
    // (a `let`, a written or shared one) may hold anything: a possible
    // gateway. Before this, the holder's initializer counted as a fresh
    // allocation and no element was read.
    let literal = {
        let initializer = file.ast.peel_ts_sugar_span(initializer);
        if file.ast.array_literals.contains(&initializer) {
            Some(initializer)
        } else if binding.shape == ast::BindingShape::Array {
            let held = file
                .ast
                .reference_declaration(initializer)
                .and_then(|holder| {
                    file.ast
                        .bindings
                        .iter()
                        .find(|candidate| {
                            candidate.shape == ast::BindingShape::Identifier
                                && candidate.names.iter().any(|name| name.span == holder)
                        })
                        .map(|candidate| (holder, candidate))
                });
            match held {
                Some((holder, candidate))
                    if candidate.initializer.is_some_and(|value| {
                        file.ast
                            .array_literals
                            .contains(&file.ast.peel_ts_sugar_span(value))
                    }) =>
                {
                    let references = file
                        .ast
                        .identifiers
                        .iter()
                        .filter(|id| {
                            id.role == ast::IdentifierRole::Reference
                                && file.ast.reference_declaration(id.span) == Some(holder)
                        })
                        .count();
                    if !candidate.immutable
                        || references != 1
                        || crate::indexes::binding_written(file, holder)
                    {
                        return true;
                    }
                    candidate
                        .initializer
                        .map(|value| file.ast.peel_ts_sugar_span(value))
                }
                _ => None,
            }
        } else {
            None
        }
    };
    if binding.shape == ast::BindingShape::Array
        && let Some(initializer) = literal
    {
        // No element identity is guessed. Any possible gateway in a literal
        // withholds every selected callable slot until exact value flow exists.
        return file
            .ast
            .identifiers
            .iter()
            .filter(|id| id.role == ast::IdentifierRole::Reference && initializer.contains(id.span))
            .any(|id| string_code_reference(file, id.span, lookup, depth - 1, true))
            || file
                .ast
                .members
                .iter()
                .filter(|member| initializer.contains(member.span))
                .any(|member| string_code_reference(file, member.span, lookup, depth - 1, true));
    }
    if matches!(
        binding.initializer_value_kind,
        ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
    ) {
        return false;
    }
    string_code_reference(file, initializer, lookup, depth - 1, returned_value)
}

pub(super) fn gateway_assignment(
    file: &FileFacts,
    declaration: Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    iteration_binding_written(file, declaration)
        && file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .filter(|target| {
                file.ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(**target))
                    == Some(declaration)
            })
            .any(|target| {
                file.ast
                    .assignments
                    .iter()
                    .find(|assignment| assignment.target == *target && !assignment.reads_target)
                    .is_none_or(|assignment| {
                        assignment.value != ast::AssignmentValueKind::Function
                            || string_code_reference(
                                file,
                                assignment.value_span,
                                lookup,
                                depth - 1,
                                true,
                            )
                    })
            })
}

pub(super) fn gateway_binding<'a>(
    file: &'a FileFacts,
    span: Span,
    lookup: &'a SemanticLookup<'_>,
) -> Option<(&'a FileFacts, &'a ast::BindingFact, Span)> {
    file.ast
        .reference_declaration(span)
        .and_then(|declaration| {
            file.ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
                .map(|binding| (file, binding, declaration))
        })
        .or_else(|| {
            let (origin, binding, symbol) =
                lookup.binding_at_reference(file.path.as_str(), span)?;
            let declaration = binding
                .names
                .iter()
                .find(|name| lookup.entity_symbol(origin, name.span) == Some(symbol.as_str()))?
                .span;
            Some((origin, binding, declaration))
        })
}

pub(super) fn string_code_slot(
    file: &FileFacts,
    object: Span,
    key: Option<&str>,
    lookup: &SemanticLookup<'_>,
    depth: usize,
    returned_receiver: bool,
) -> bool {
    let possible_gateway_key = matches!(key, None | Some("Function" | "eval" | "constructor"));
    // Only exact, current source implementations discharge these potentially
    // executable slots. A Function/Proxy type is no runtime identity proof.
    // A selected property declaration on a structural receiver is not enough.
    // The receiver below must be a closed, non-escaping source allocation.
    if depth == 0 {
        return true;
    }
    let object = file.ast.peel_ts_sugar_span(object);
    if let Some(branch) = file.ast.conditional_at(object) {
        return string_code_slot(
            file,
            branch.consequent,
            key,
            lookup,
            depth - 1,
            returned_receiver,
        ) || string_code_slot(
            file,
            branch.alternate,
            key,
            lookup,
            depth - 1,
            returned_receiver,
        );
    }
    if let Some(branch) = file.ast.logical_at(object) {
        return string_code_slot(file, branch.left, key, lookup, depth - 1, returned_receiver)
            || string_code_slot(
                file,
                branch.right,
                key,
                lookup,
                depth - 1,
                returned_receiver,
            );
    }
    if let Some(call) = file.ast.call_at(object)
        && let Some((origin, function)) =
            lookup.function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
    {
        return function
            .expression_return
            .as_ref()
            .filter(|returned| {
                !matches!(
                    returned.runtime_value_kind,
                    ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                )
            })
            .and_then(|returned| returned.argument)
            .into_iter()
            .chain(
                origin
                    .ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        containing_ast_function(&origin.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    })
                    .filter(|returned| {
                        !matches!(
                            returned.runtime_value_kind,
                            ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                        )
                    })
                    .filter_map(|returned| returned.argument),
            )
            .any(|returned| string_code_slot(origin, returned, key, lookup, depth - 1, true));
    }
    if let Some((file, binding, declaration)) = gateway_binding(file, object, lookup)
        && binding.shape == ast::BindingShape::Identifier
        && binding.immutable
        && !iteration_binding_written(file, declaration)
        && let Some(initializer) = binding.initializer
    {
        let possible =
            string_code_slot(file, initializer, key, lookup, depth - 1, returned_receiver);
        if possible || !possible_gateway_key {
            return possible;
        }
        if file.ast.exports.iter().any(|export| {
            export
                .declarations
                .iter()
                .any(|value| value.local.span == declaration)
                || export.specifiers.iter().any(|value| {
                    file.ast.reference_declaration(value.local.span) == Some(declaration)
                })
        }) {
            return true;
        }
        if !file
            .ast
            .reference_declarations
            .iter()
            .filter(|(_, bound)| *bound == declaration)
            .all(|(reference, _)| {
                file.ast.members.iter().any(|member| {
                    file.ast.peel_ts_sugar_span(member.object) == *reference
                        && !file.ast.write_targets.contains(&member.span)
                        && !file.ast.deleted_targets.contains(&member.span)
                }) || file.ast.bindings.iter().any(|binding| {
                    binding.immutable
                        && binding.shape == ast::BindingShape::Object
                        && binding
                            .initializer
                            .map(|span| file.ast.peel_ts_sugar_span(span))
                            == Some(*reference)
                })
            })
        {
            return true;
        }
        return false;
    }
    if let Some(key) = key
        && let Some(shape) = file.ast.object_get_shapes.iter().find(|shape| {
            shape.span == object
                && shape.closed
                && shape
                    .properties
                    .iter()
                    .all(|property| property.getter.is_none())
        })
        && let Some(property) = file.ast.object_properties.iter().find(|property| {
            object.contains(property.span)
                && !property.computed
                && file.source_text(property.key) == Some(key)
                && !file.ast.object_get_shapes.iter().any(|nested| {
                    nested.span != object
                        && object.contains(nested.span)
                        && nested.span.contains(property.span)
                })
        })
        && shape.properties.iter().any(|property| property.key == key)
    {
        // A method body can expose/replace its slot through `this`, without a
        // reference to the object binding. Refuse possible slot replacement
        // before using an own source method as negative gateway evidence.
        if lookup.files().iter().any(|candidate| {
            candidate.ast.write_targets.iter().chain(&candidate.ast.deleted_targets).any(|target| {
                candidate.ast.members.iter().any(|member| member.span == candidate.ast.peel_ts_sugar_span(*target)
                    && member_key(candidate, member).is_none_or(|written| written == key || written == "__proto__"))
            }) || candidate.ast.calls.iter().any(|call| {
                tuple_mutation_method(candidate, call, lookup).is_some_and(|method| match method {
                    "setPrototypeOf" => true,
                    "assign" | "defineProperties" => call.arguments.iter().skip(1).any(|argument| {
                        candidate.ast.object_get_shapes.iter().find(|shape| shape.span == candidate.ast.peel_ts_sugar_span(argument.span) && shape.closed)
                            .is_none_or(|shape| shape.properties.iter().any(|property| property.key == key || property.key == "__proto__"))
                    }),
                    "set" | "defineProperty" | "deleteProperty" => call.arguments.get(1).is_none_or(|argument| {
                        !matches!(&argument.literal_value, ast::ArgumentLiteralFact::String(written) if written.as_str() != key && written.as_str() != "__proto__")
                    }),
                    _ => false,
                })
            })
        }) { return true; }
        let Some(function) = file
            .ast
            .function_at(file.ast.peel_ts_sugar_span(property.value))
        else {
            if matches!(
                property.value_kind,
                ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
            ) {
                return false;
            }
            return string_code_reference(file, property.value, lookup, depth - 1, true);
        };
        return function
            .expression_return
            .as_ref()
            .and_then(|returned| returned.argument)
            .into_iter()
            .chain(
                file.ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|owner| owner.span == function.span)
                    })
                    .filter(|returned| {
                        !matches!(
                            returned.runtime_value_kind,
                            ast::RuntimeValueKind::Primitive | ast::RuntimeValueKind::Nullish
                        )
                    })
                    .filter_map(|returned| returned.argument),
            )
            .any(|returned| string_code_reference(file, returned, lookup, depth - 1, true));
    }
    // Unknown member authority remains a possibility, including a function's
    // inherited constructor and erased/aliased realm objects. No name grants
    // positive authority and no unresolved value proves a mutation.
    possible_gateway_key || returned_receiver
}

pub(super) fn iteration_source_function(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if invocation_property_write(lookup) {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if lookup
        .function_called_at(file.path.as_str(), span)
        .is_some()
    {
        return true;
    }
    let declaration = file.ast.reference_declaration(span);
    file.ast.functions.iter().any(|function| {
        (function.span == span
            || function
                .name
                .as_ref()
                .is_some_and(|name| Some(name.span) == declaration))
            && lookup.function_value_is_current(file, function)
    })
}

pub(super) fn invocation_key(key: Option<&str>) -> bool {
    key.is_none_or(|key| matches!(key, "call" | "apply" | "bind" | "__proto__" | "Reflect"))
}

pub(super) fn invocation_operator_is_exact(
    file: &FileFacts,
    call: &ast::CallFact,
    method: &str,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    if method == "invoke" {
        return matches!(member_key(file, member), Some("call" | "apply" | "bind"));
    }
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if member_key(file, member) != Some("apply")
        || file.source_text(receiver) != Some("Reflect")
        || file.ast.reference_declaration(receiver).is_some()
    {
        return false;
    }
    let Some(symbol) = lookup.symbol_at(file.path.as_str(), receiver) else {
        return false;
    };
    let Some(declaration) =
        lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)
    else {
        return false;
    };
    symbol.alias_target().is_empty()
        && !symbol.declarations().is_empty()
        && symbol.declarations().iter().all(|binding| {
            binding.name.as_ref() == "Reflect" && binding.location.path == declaration.source_file
        })
}

/// Knowing f's implementation does not prove that f.call/f.apply/f.bind, or
/// Reflect.apply, still selects the intrinsic invocation operator. Retain
/// visible possible replacement, including writes onto a fresh function.
pub(super) fn invocation_property_write(lookup: &SemanticLookup<'_>) -> bool {
    if lookup.visible_string_code() {
        return true;
    }
    let map = |file: &FileFacts, span: Span| {
        file.ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == file.ast.peel_ts_sugar_span(span) && shape.closed)
            .is_none_or(|shape| {
                shape
                    .properties
                    .iter()
                    .any(|property| invocation_key(Some(property.key.as_str())))
            })
    };
    lookup.files().iter().any(|file| {
        file.ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
            .any(|target| {
                let target = file.ast.peel_ts_sugar_span(*target);
                (file.source_text(target) == Some("Reflect")
                    && file.ast.reference_declaration(target).is_none())
                    || file.ast.members.iter().any(|member| {
                        member.span == target && invocation_key(member_key(file, member))
                    })
            })
            || file.ast.calls.iter().any(|call| {
                let Some(method) = tuple_mutation_method(file, call, lookup) else {
                    return false;
                };
                if call.arguments.iter().any(|argument| argument.spread) {
                    return true;
                }
                match method {
                    "invoke" | "reflectInvoke" | "stringCode" => false,
                    "setPrototypeOf" => true,
                    "assign" => call
                        .arguments
                        .iter()
                        .skip(1)
                        .any(|argument| map(file, argument.span)),
                    "defineProperties" => call
                        .arguments
                        .get(1)
                        .is_none_or(|argument| map(file, argument.span)),
                    _ => invocation_key(call.arguments.get(1).and_then(|argument| {
                        match &argument.literal_value {
                            ast::ArgumentLiteralFact::String(key) => Some(key.as_ref()),
                            ast::ArgumentLiteralFact::Integer(_) => Some("0"),
                            _ => None,
                        }
                    })),
                }
            })
    })
}

/// ProxyCreate validates two object values and installs internal slots; it
/// invokes neither value and writes no intrinsic. A ProxyConstructor type is
/// not this runtime identity: require the unaliased global's library variable,
/// and refuse visible replacement (including unknown reflective targets).
pub(super) fn intrinsic_disjoint_proxy_creation(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    if lookup.visible_string_code()
        || !call.construct
        || call.arguments.iter().any(|argument| argument.spread)
    {
        return false;
    }
    let Some(declaration) =
        lookup.standard_library_declaration(file, call, typefacts::CallKind::Construct)
    else {
        return false;
    };
    if declaration.qualified_name.as_ref() != "ProxyConstructor.construct"
        || std::path::Path::new(declaration.source_file.as_ref())
            .file_name()
            .and_then(|name| name.to_str())
            != Some("lib.es2015.proxy.d.ts")
    {
        return false;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
        || file.ast.reference_declaration(callee).is_some()
    {
        return false;
    }
    let Some(symbol) = lookup.symbol_at(file.path.as_str(), callee) else {
        return false;
    };
    if !symbol.alias_target().is_empty()
        || symbol.declarations().len() != 1
        || !symbol.declarations().iter().all(|binding| {
            binding.name.as_ref() == "Proxy"
                && binding.kind.as_ref() == "variable"
                && binding.location.path == declaration.source_file
        })
    {
        return false;
    }
    !lookup.files().iter().any(|candidate| {
        candidate
            .ast
            .write_targets
            .iter()
            .chain(&candidate.ast.deleted_targets)
            .any(|target| {
                let target = candidate.ast.peel_ts_sugar_span(*target);
                (candidate.source_text(target) == Some("Proxy")
                    && candidate.ast.reference_declaration(target).is_none())
                    || candidate.ast.members.iter().any(|member| {
                        member.span == target
                            && matches!(member_key(candidate, member), None | Some("Proxy"))
                    })
            })
            || candidate.ast.calls.iter().any(|mutation| {
                tuple_mutation_method(candidate, mutation, lookup).is_some_and(|method| {
                    !matches!(method, "invoke" | "reflectInvoke" | "stringCode")
                }) && mutation.arguments.first().is_none_or(|target| {
                    target.spread || !separate_iteration_target(candidate, target.span, 24)
                })
            })
    })
}

pub(super) fn source_bound_invocation(
    file: &FileFacts,
    callee: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(bound) = file.ast.call_at(file.ast.peel_ts_sugar_span(callee)) else {
        return false;
    };
    let Some(declaration) =
        lookup.standard_library_declaration(file, bound, typefacts::CallKind::Call)
    else {
        return false;
    };
    declaration.qualified_name.ends_with(".bind")
        && tuple_mutation_method(file, bound, lookup) == Some("invoke")
        && file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(bound.callee))
            .is_some_and(|member| {
                member_key(file, member) == Some("bind")
                    && iteration_source_function(file, member.object, lookup)
            })
}

/// ADR 0801: a setter stores its argument, or calls it with the previous value,
/// and a function it calls is in the same census. It mutates no intrinsic, so an
/// object handed to it (`setNow(new Date())`) leaves array iteration alone. This
/// needs the exact unwritten setter slot a dialect primitive's tuple produced: a
/// type alone does not prove runtime identity, so a cast or a declared-only
/// setter cannot discharge a call.
pub(super) fn intrinsic_disjoint_setter(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if !file
        .ast
        .identifiers
        .iter()
        .any(|identifier| identifier.span == callee)
    {
        return false;
    }
    let Some((origin, binding, symbol)) = lookup.binding_at_reference(file.path.as_str(), callee)
    else {
        return false;
    };
    let Some(slot) = binding.array_slots.get(1).and_then(Option::as_ref) else {
        return false;
    };
    simple_array_binding(binding)
        && lookup.entity_symbol(origin, slot.span) == Some(symbol.as_str())
        && !crate::indexes::binding_written(origin, slot.span)
        && binding
            .initializer
            .and_then(|span| origin.ast.call_at(origin.ast.peel_ts_sugar_span(span)))
            .and_then(|created| lookup.primitive_at_call(origin, created.span))
            .is_some_and(|primitive| lookup.dialect.returns_reactive_tuple(primitive))
}

/// ADR 0292: whether any analyzed file holds a reflective write
/// (`Object.assign`, `defineProperty`, `Reflect.set`, ...) whose target is
/// spelled through a `prototype`, the one write [`MemberStability`] lets reach
/// a fresh literal nothing else holds (ADR 0282, narrowed by ADR 0288's
/// review). The same visible veto, for a fresh module object.
pub(crate) fn reflective_prototype_write(lookup: &SemanticLookup<'_>) -> bool {
    lookup.reflective_prototype_write_present()
}

/// The census behind [`reflective_prototype_write`], which the lookup
/// memoizes: the calls the base index lists as mutation candidates
/// ([`crate::base::Hazard::MutationBuiltinCall`]), the only calls
/// [`mutation_builtin`] can accept.
pub(crate) fn scan_reflective_prototype_write(lookup: &SemanticLookup<'_>) -> bool {
    mutation_builtin_calls(lookup).any(|(file, call)| {
        targets_prototype(file, call) && mutation_builtin(file, call, lookup).is_some()
    })
}

/// Every call the base index lists as a mutation candidate, in file and call
/// order.
pub(crate) fn mutation_builtin_calls<'s, 'a: 's>(
    lookup: &'s SemanticLookup<'a>,
) -> impl Iterator<Item = (&'a FileFacts, &'a ast::CallFact)> + 's {
    lookup
        .slot_entries(lookup.base_hazards())
        .flat_map(|(file, entries)| {
            entries.iter().filter_map(move |(hazard, call)| {
                (*hazard == crate::base::Hazard::MutationBuiltinCall)
                    .then(|| &file.ast.calls[*call as usize])
                    .map(|call| (file, call))
            })
        })
}

/// The global, method and library of a standard-library reflective mutator
/// [`mutation_builtin`] knows, by the qualified name of its declaration.
pub(crate) fn mutation_builtin_spelling(
    qualified_name: &str,
) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match qualified_name {
        "ObjectConstructor.defineProperty" => ("Object", "defineProperty", "lib.es5.d.ts"),
        "ObjectConstructor.defineProperties" => ("Object", "defineProperties", "lib.es5.d.ts"),
        "ObjectConstructor.assign" => ("Object", "assign", "lib.es2015.core.d.ts"),
        "Reflect.set" => ("Reflect", "set", "lib.es2015.reflect.d.ts"),
        "Reflect.defineProperty" => ("Reflect", "defineProperty", "lib.es2015.reflect.d.ts"),
        "Reflect.deleteProperty" => ("Reflect", "deleteProperty", "lib.es2015.reflect.d.ts"),
        _ => return None,
    })
}

/// Signature and runtime receiver identities are independent, as in ADR 0272.
/// A local ObjectConstructor-typed value or a homonymous helper is not Object.
pub(crate) fn mutation_builtin<'a>(
    file: &FileFacts,
    call: &ast::CallFact,
    lookup: &'a SemanticLookup<'_>,
) -> Option<&'a str> {
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    let (global, property, library) =
        mutation_builtin_spelling(declaration.qualified_name.as_ref())?;
    let member = file.ast.members.iter().find(|member| {
        member.span == file.ast.peel_ts_sugar_span(call.callee)
            && !file.ast.computed_members.contains(&member.span)
            && file.source_text(member.property) == Some(property)
    })?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if file.ast.reference_declaration(receiver).is_some() {
        return None;
    }
    let entity = lookup.entity_at(file.path.as_str(), receiver)?;
    if entity.symbol_unresolved() || entity.symbol.is_empty() {
        return None;
    }
    let symbol = lookup.symbol_at(file.path.as_str(), receiver)?;
    if !symbol.alias_target().is_empty()
        || symbol.declarations().iter().any(|binding| {
            lookup
                .file_by_path(binding.location.path.as_ref())
                .is_some()
        })
    {
        return None;
    }
    let mut bindings = symbol.declarations().iter().filter(|binding| {
        binding.name.as_ref() == global
            && (global == "Reflect" || binding.kind.as_ref() == "variable")
    });
    let binding = bindings.next()?;
    // Object's runtime variable is in es5 even when assign's signature is in es2015.core.
    let expected = std::path::Path::new(declaration.source_file.as_ref())
        .parent()?
        .join(if global == "Object" {
            "lib.es5.d.ts"
        } else {
            library
        });
    (bindings.next().is_none()
        && std::path::Path::new(binding.location.path.as_ref()) == expected
        && declaration.location.path == declaration.source_file
        && std::path::Path::new(declaration.source_file.as_ref())
            .file_name()?
            .to_str()?
            == library)
        .then_some(property)
}
