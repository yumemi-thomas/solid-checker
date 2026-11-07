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
    // ADR 0876: so is a withheld call's: this only vetoes.
    let declaration = lookup.placing_standard_declaration(file, call)?;
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
    ["===", "!==", "==", "!=", "instanceof", "typeof"]
        .iter()
        .any(|operator| before.ends_with(operator))
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
/// the base layer's [`crate::base::Hazard::ArrayIntrinsicObtained`].
pub(crate) fn array_intrinsic_obtained(file: &FileFacts) -> bool {
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
    let handed = |span: Span, kind: Obtained| handed_on(file, span, kind, HANDED_BUDGET);
    file.ast.members.iter().any(|member| {
        // A write is the visible side's (`visible_iteration_write`).
        if writes.contains(&member.span) {
            return false;
        }
        let key = member_key(file, member);
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
        match global_name(file, file.ast.peel_ts_sugar_span(member.object)) {
            Some(name) if GLOBAL_OBJECTS.contains(&name) => match key {
                Some("Array") => handed(member.span, Obtained::Intrinsic),
                // `globalThis.globalThis.Array`: the global object again.
                Some("Function" | "eval" | "Reflect" | "Object") | None => {
                    handed(member.span, Obtained::Function)
                }
                Some(key) if GLOBAL_OBJECTS.contains(&key) => handed(member.span, Obtained::Global),
                Some(_) => false,
            },
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
        global_name(file, identifier.span).is_some_and(|name| GLOBAL_OBJECTS.contains(&name))
            && identifier.role == ast::IdentifierRole::Reference
            && !file.ast.type_names.contains(&identifier.span)
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
                    .is_some_and(|name| GLOBAL_OBJECTS.contains(&name))
            }))
    }) || file.ast.identifiers.iter().any(|identifier| {
        let name = global_name(file, identifier.span);
        matches!(name, Some("Array" | "Function"))
            && !file.ast.type_names.contains(&identifier.span)
            && !objects.contains(&identifier.span)
            && !callees.contains_key(&identifier.span)
            && handed(
                identifier.span,
                if name == Some("Function") {
                    Obtained::Function
                } else {
                    Obtained::Intrinsic
                },
            )
    }) || file
        .ast
        .identifiers
        .iter()
        .any(|identifier| global_name(file, identifier.span) == Some("eval"))
        || file.ast.calls.iter().any(|call| {
            match global_name(file, file.ast.peel_ts_sugar_span(call.callee)) {
                Some("Function") => true,
                // A timer runs a string as code.
                Some("setTimeout" | "setInterval") => {
                    call.arguments.first().is_some_and(|handler| {
                        handler.spread
                            || handler.runtime_value_kind == ast::RuntimeValueKind::Primitive
                    })
                }
                _ => false,
            }
        })
}

/// The global object's own names.
const GLOBAL_OBJECTS: &[&str] = &["globalThis", "window", "self", "global"];

/// What an obtained value may be, for [`handed_on`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Obtained {
    /// `Array`, `Array.prototype`, a prototype: no code of its own.
    Intrinsic,
    /// Possibly `Function` or `eval`: called, it runs a string.
    Function,
    /// The global object: every global, `Array` and `Function` among them.
    Global,
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
fn handed_on(file: &FileFacts, span: Span, kind: Obtained, budget: usize) -> bool {
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
        return handed_on(file, logical.span, kind, budget - 1);
    }
    if let Some(conditional) = file.ast.conditional_expressions.iter().find(|conditional| {
        conditional.test == span || conditional.consequent == span || conditional.alternate == span
    }) {
        return conditional.test != span && handed_on(file, conditional.span, kind, budget - 1);
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
            // array literals iterate with.
            return kind != Obtained::Global
                && key.is_none_or(|key| matches!(key, "next" | "return" | "__proto__"));
        }
        let next = |kind: Obtained| handed_on(file, member.span, kind, budget - 1);
        return match (kind, key) {
            // The global object again, or one of its intrinsics by name.
            (Obtained::Global, Some("Array")) => next(Obtained::Intrinsic),
            (Obtained::Global, Some("Function" | "eval" | "Reflect" | "Object")) => {
                next(Obtained::Function)
            }
            (Obtained::Global, Some(key)) if GLOBAL_OBJECTS.contains(&key) => {
                next(Obtained::Global)
            }
            (Obtained::Global, None) => next(Obtained::Global),
            // A value that may be `Function` or `eval`, invoked through
            // `call`, `apply` or `bind`: code from a string.
            (Obtained::Function, Some("call" | "apply" | "bind")) => true,
            (_, Some("prototype" | "__proto__")) => next(Obtained::Intrinsic),
            (_, Some("constructor") | None) => next(Obtained::Function),
            (_, Some(_)) => false,
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
        // The global object handed to a call: whether the callee is code the
        // checker sees is a cross-file question this per-file census cannot
        // read (ADR 0803's premise: an intrinsic reached through a parameter
        // in seen code is outside the model). Not counted here.
        if kind == Obtained::Global {
            return false;
        }
        let first = call.arguments.first().is_some_and(|argument| {
            !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == span
        });
        return !(read_only_builtin(file, call) || (first && reflective_target(file, span)));
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
        // Stored, returned, spread: handed on. The global object handed on
        // like this, as to a call, is followed only where the receiving code
        // is known not to be seen (a project-level question, not this
        // per-file census's).
        return kind != Obtained::Global;
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
                && handed_on(file, identifier.span, kind, budget - 1)
        })
}

/// `!value`: the value is only tested.
fn tested(file: &FileFacts, span: Span) -> bool {
    let before = file.source.get(..span.start as usize).unwrap_or_default();
    before
        .trim_end_matches(|c: char| c.is_whitespace() || c == '(')
        .ends_with('!')
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

/// A built-in that only reads its arguments and keeps none of them.
fn read_only_builtin(file: &FileFacts, call: &ast::CallFact) -> bool {
    let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    if file.ast.computed_members.contains(&member.span) {
        return false;
    }
    let property = file.source_text(member.property).unwrap_or_default();
    match global_name(file, file.ast.peel_ts_sugar_span(member.object)) {
        Some("Object") => matches!(
            property,
            // Names and tests only: `values`, `entries` and the descriptor
            // getters return what the object holds.
            "getPrototypeOf"
                | "keys"
                | "getOwnPropertyNames"
                | "getOwnPropertySymbols"
                | "isFrozen"
                | "isSealed"
                | "isExtensible"
                | "is"
                | "hasOwn"
        ),
        Some("Reflect") => matches!(
            property,
            "getPrototypeOf" | "ownKeys" | "has" | "isExtensible"
        ),
        Some("Array") => property == "isArray",
        _ => false,
    }
}

/// ADR 0801: whether a hook's accessor slot may not be what the literal holds.
/// A local `const [a] = createSignal(0)` rests on the same protocol with no
/// census at all. What this asks is whether anything can reach that protocol:
/// a program that never obtains an intrinsic cannot change it, whatever it
/// writes. [`tuple_iteration_write`] refuses every call it cannot name, which
/// any callback prop is, and every computed write. Both answers are hazard
/// probes, so a reuse records which files' hazards it read.
pub(crate) fn tuple_slot_iteration_write(lookup: &SemanticLookup<'_>) -> bool {
    lookup.program_has_eval() || lookup.hazard_present(crate::base::Hazard::ArrayIntrinsicObtained)
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
    // ADR 0876: this only vetoes, so a withheld call stays a possible mutation.
    let declaration = lookup.placing_standard_declaration(file, call)?;
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
