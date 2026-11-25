//! ADR 0563: whether a standard-library carrier provably runs the callback it
//! is handed.
//!
//! [`crate::runtime_semantics`]'s `InlineCallback` rows (and ADR 0531's
//! `HandOff::Inline`) answer *when* a callback runs: before the carrier
//! returns, on the caller's stack. That is a timing claim, not a count: an
//! empty receiver never invokes it (`[].forEach(hf)` runs `hf` zero times), and
//! neither does an array of unknown length that turns out empty. A finding
//! about what the callback does (a pending read that throws, a context read
//! with no provider, an owner-requiring call) is a claim about code that runs,
//! so it is proven only where the carrier provably runs the callback at least
//! once:
//!
//! - `f.call(…)`, `f.apply(…)` (`Function.prototype`), `Reflect.apply(f, …)`,
//!   the `Promise` executor and a tagged template's tag always invoke it;
//! - an `Array`/`ReadonlyArray` iteration method (`forEach`, `map`, `filter`,
//!   `some`, `every`, `find`, ...) only on an array literal receiver with at
//!   least one element and no spread or hole (a hole is skipped);
//! - `reduce`/`reduceRight` with two such elements, or one and an initial
//!   value (one element and no initial value returns it uncalled);
//! - `sort` with two elements no expression of which may be `undefined`
//!   (`CompareArrayElements` orders `undefined` without calling the
//!   comparator);
//! - `Array.from` (and the typed-array `from`s) only with such an array literal
//!   as its source.
//!
//! - (ADR 1216 slice 3) `Set`/`Map` `forEach` on `new Set(…)`/`new Map(…)`
//!   written there from such an array literal (for a `Map`, of array
//!   literals).
//!
//! Every other carrier (`Set`/`Map`/`NodeList` `forEach` elsewhere, a variable or
//! property receiver, `String.replace`, `JSON.parse`'s reviver) may run it
//! zero times: the finding stays an obligation. The carrier is the
//! compiler-selected standard-library declaration, never a spelling; a local
//! or project `forEach`, or a local `Reflect`, resolves elsewhere and is no
//! carrier at all. ECMAScript semantics, not a Solid version's: no dialect
//! question.

use crate::indexes::SemanticLookup;
use solid_facts::ast::ArrayElementKind;
use solid_facts::core::Span;

/// Whether the carrier at `carrier` (a call's span, or a tagged template's)
/// provably runs the callback it is handed at least once before it returns.
/// The caller has already established that the callback sits at the
/// carrier's inline position (`HandOff::Inline`, an `InlineCallback` row).
pub(crate) fn carrier_provably_runs(
    file: &solid_facts::FileFacts,
    carrier: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    if ast
        .tagged_templates
        .iter()
        .any(|template| template.span == carrier)
    {
        return true;
    }
    let Some(call) = ast.calls.iter().find(|call| call.span == carrier) else {
        return false;
    };
    // ADR 0628: `untrack(f)` runs `f` before it returns.
    if lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::Untrack) {
        return true;
    }
    let kind = if call.construct {
        typefacts::CallKind::Construct
    } else {
        typefacts::CallKind::Call
    };
    let Some(declaration) = crate::indexes::standard_library_declaration_of(
        lookup.resolved_callee_call(file, call.callee),
        kind,
    ) else {
        return false;
    };
    let receiver = || {
        ast.member_at(ast.peel_ts_sugar_span(call.callee))
            .map(|member| member.object)
    };
    let argument = |index: usize| {
        call.arguments
            .get(index)
            .filter(|argument| !argument.spread)
            .map(|argument| argument.span)
    };
    match declaration.qualified_name.as_ref() {
        "Function.call"
        | "Function.apply"
        | "CallableFunction.call"
        | "CallableFunction.apply"
        | "NewableFunction.call"
        | "NewableFunction.apply"
        | "Reflect.apply"
        | "PromiseConstructor.construct" => true,
        "Array.forEach"
        | "ReadonlyArray.forEach"
        | "Array.map"
        | "ReadonlyArray.map"
        | "Array.flatMap"
        | "ReadonlyArray.flatMap"
        | "Array.filter"
        | "ReadonlyArray.filter"
        | "Array.some"
        | "ReadonlyArray.some"
        | "Array.every"
        | "ReadonlyArray.every"
        | "Array.find"
        | "ReadonlyArray.find"
        | "Array.findIndex"
        | "ReadonlyArray.findIndex"
        | "Array.findLast"
        | "ReadonlyArray.findLast"
        | "Array.findLastIndex"
        | "ReadonlyArray.findLastIndex" => {
            receiver().is_some_and(|receiver| literal_elements(file, receiver) >= 1)
        }
        // ADR 1216 slice 3: a collection built there from an array literal of at least one entry holds an entry when
        // `forEach` starts, and `forEach` visits it.
        "Set.forEach" => receiver().is_some_and(|receiver| {
            seeded_collection(file, receiver, lookup, "SetConstructor.construct")
        }),
        "Map.forEach" => receiver().is_some_and(|receiver| {
            seeded_collection(file, receiver, lookup, "MapConstructor.construct")
        }),
        "Array.reduce"
        | "ReadonlyArray.reduce"
        | "Array.reduceRight"
        | "ReadonlyArray.reduceRight" => receiver().is_some_and(|receiver| {
            let elements = literal_elements(file, receiver);
            elements >= 2 || (elements >= 1 && argument(1).is_some())
        }),
        "Array.sort" => {
            receiver().is_some_and(|receiver| defined_literal_elements(file, receiver) >= 2)
        }
        "ArrayConstructor.from"
        | "Int8ArrayConstructor.from"
        | "Uint8ArrayConstructor.from"
        | "Uint8ClampedArrayConstructor.from"
        | "Int16ArrayConstructor.from"
        | "Uint16ArrayConstructor.from"
        | "Int32ArrayConstructor.from"
        | "Uint32ArrayConstructor.from"
        | "Float32ArrayConstructor.from"
        | "Float64ArrayConstructor.from"
        | "BigInt64ArrayConstructor.from"
        | "BigUint64ArrayConstructor.from" => {
            argument(0).is_some_and(|source| literal_elements(file, source) >= 1)
        }
        _ => false,
    }
}

/// ADR 0563: whether code at `span` sits in a callback some standard-library
/// carrier around it may run zero times: walking out from `span` through the
/// function literals handed directly to an inline carrier (and the
/// immediately invoked ones between them), the first carrier that does not
/// [`carrier_provably_runs`]. `false` where the walk ends at anything else:
/// that position is other arms' business.
pub(crate) fn carrier_run_unproven(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let mut span = span;
    loop {
        let Some(function) = crate::owners::containing_ast_function(ast, span) else {
            return false;
        };
        // An immediately invoked literal runs where it is written.
        if let Some(call) = ast
            .calls_with_peeled_callee(function.span)
            .min_by_key(|call| call.span.end - call.span.start)
        {
            span = call.span;
            continue;
        }
        let Some((call, index)) = ast
            .arguments_containing(function.span)
            .find(|(call, index)| {
                let argument = &call.arguments[*index];
                !argument.spread && ast.peel_ts_sugar_span(argument.span) == function.span
            })
        else {
            return false;
        };
        if lookup.primitive_at_call(file, call.span).is_some() {
            return false;
        }
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            return false;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        if !crate::runtime_semantics::is_inline_standard_callback(resolved, callability, index) {
            return false;
        }
        if !carrier_provably_runs(file, call.span, lookup) {
            return true;
        }
        // The call contains the literal strictly, so the walk terminates.
        span = call.span;
    }
}

/// The elements of the array literal at `span` (through parentheses and type
/// assertions) when it has no spread or hole; zero otherwise, and for any
/// other expression.
fn literal_elements(file: &solid_facts::FileFacts, span: Span) -> usize {
    file.ast
        .array_literal_at(file.ast.peel_ts_sugar_span(span))
        .filter(|array| {
            array
                .elements
                .iter()
                .all(|element| element.kind == ArrayElementKind::Value)
        })
        .map_or(0, |array| array.elements.len())
}

/// [`literal_elements`], counting only elements that cannot evaluate to
/// `undefined`: number, string, boolean and `null` literals, and array and
/// function literals.
fn defined_literal_elements(file: &solid_facts::FileFacts, span: Span) -> usize {
    let ast = &file.ast;
    let Some(array) = ast.array_literal_at(ast.peel_ts_sugar_span(span)) else {
        return 0;
    };
    if array
        .elements
        .iter()
        .any(|element| element.kind != ArrayElementKind::Value)
    {
        return 0;
    }
    array
        .elements
        .iter()
        .filter(|element| {
            let value = ast.peel_ts_sugar_span(element.span);
            let text = file.source_text(value).unwrap_or_default();
            let first = text.chars().next();
            first.is_some_and(|c| c.is_ascii_digit() || c == '"' || c == '\'')
                || (first == Some('.') && text.chars().nth(1).is_some_and(|c| c.is_ascii_digit()))
                || matches!(text, "true" | "false" | "null")
                || ast.array_literal_at(value).is_some()
                || ast.function_at(value).is_some()
        })
        .count()
}

/// ADR 1019: whether the carrier at `carrier` (a call's span) provably never
/// runs the callback it is handed, the dual of [`carrier_provably_runs`]. The
/// caller has already established that the callback sits at the carrier's
/// inline position (`HandOff::Inline`, an `InlineCallback` row: the carrier
/// runs it before it returns or never, and keeps no reference to it). The
/// receiver or source must be written as a literal no execution can fill:
///
/// - an `Array`/`ReadonlyArray` iteration method (`forEach`, `map`, `filter`,
///   `some`, `every`, `find`, ..., `sort`) on an empty array literal `[]`,
///   or (ADR 1109) on a `const` name holding `[]` that nothing can fill
///   ([`empty_array_name`]);
/// - (ADR 1109) `sort`/`toSorted` on a one-element literal with no spread or
///   hole: one element is never compared;
/// - `reduce`/`reduceRight` on `[]`, or on a one-element literal with no
///   spread or hole and no initial value (it returns the element uncalled);
/// - `Array.from` (and the typed-array `from`s) with `[]` as its source;
/// - `Set.forEach`/`Map.forEach` on `new Set()`/`new Map()` written there
///   with no argument, the constructor the standard-library one;
/// - `String.replace`/`replaceAll` with a string literal receiver and a
///   non-empty string literal pattern (no escape) the receiver does not
///   contain.
///
/// - (ADR 1033) `NodeList.forEach`/`NodeListOf.forEach` on the `childNodes`
///   of, or a `querySelectorAll` called on, a node made there and never
///   handed on: `document.createElement` of a quoted name with no `-` and no
///   second argument (no custom element constructor or customized built-in
///   runs), `document` the library's global ([`fresh_empty_node`]).
///
/// Anything else (another name, a property, a literal with an element, a
/// `NodeList` of a node that is not fresh, `document.querySelectorAll`)
/// answers `false`: the callback may run.
pub(crate) fn carrier_provably_never_runs(
    file: &solid_facts::FileFacts,
    carrier: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(call) = ast.calls.iter().find(|call| call.span == carrier) else {
        return false;
    };
    if call.construct || call.arguments.iter().any(|argument| argument.spread) {
        return false;
    }
    let Some(declaration) = crate::indexes::standard_library_declaration_of(
        lookup.resolved_callee_call(file, call.callee),
        typefacts::CallKind::Call,
    ) else {
        return false;
    };
    let receiver = || {
        ast.member_at(ast.peel_ts_sugar_span(call.callee))
            .map(|member| member.object)
    };
    let empty = |span: Span| {
        ast.array_literal_at(ast.peel_ts_sugar_span(span))
            .is_some_and(|array| array.elements.is_empty())
            || empty_array_name(file, span, lookup)
    };
    match declaration.qualified_name.as_ref() {
        "Array.forEach"
        | "ReadonlyArray.forEach"
        | "Array.map"
        | "ReadonlyArray.map"
        | "Array.flatMap"
        | "ReadonlyArray.flatMap"
        | "Array.filter"
        | "ReadonlyArray.filter"
        | "Array.some"
        | "ReadonlyArray.some"
        | "Array.every"
        | "ReadonlyArray.every"
        | "Array.find"
        | "ReadonlyArray.find"
        | "Array.findIndex"
        | "ReadonlyArray.findIndex"
        | "Array.findLast"
        | "ReadonlyArray.findLast"
        | "Array.findLastIndex"
        | "ReadonlyArray.findLastIndex" => receiver().is_some_and(empty),
        // ADR 1109: a sort compares two elements; one or none is never compared.
        "Array.sort" | "Array.toSorted" | "ReadonlyArray.toSorted" => receiver()
            .is_some_and(|receiver| empty(receiver) || literal_elements(file, receiver) == 1),
        "Array.reduce"
        | "ReadonlyArray.reduce"
        | "Array.reduceRight"
        | "ReadonlyArray.reduceRight" => receiver().is_some_and(|receiver| {
            empty(receiver) || (literal_elements(file, receiver) == 1 && call.arguments.len() == 1)
        }),
        "ArrayConstructor.from"
        | "Int8ArrayConstructor.from"
        | "Uint8ArrayConstructor.from"
        | "Uint8ClampedArrayConstructor.from"
        | "Int16ArrayConstructor.from"
        | "Uint16ArrayConstructor.from"
        | "Int32ArrayConstructor.from"
        | "Uint32ArrayConstructor.from"
        | "Float32ArrayConstructor.from"
        | "Float64ArrayConstructor.from"
        | "BigInt64ArrayConstructor.from"
        | "BigUint64ArrayConstructor.from" => call
            .arguments
            .first()
            .is_some_and(|source| empty(source.span)),
        "Set.forEach" => receiver().is_some_and(|receiver| {
            fresh_collection(file, receiver, lookup, "SetConstructor.construct")
        }),
        "Map.forEach" => receiver().is_some_and(|receiver| {
            fresh_collection(file, receiver, lookup, "MapConstructor.construct")
        }),
        "NodeList.forEach" | "NodeListOf.forEach" => {
            receiver().is_some_and(|receiver| fresh_empty_node_list(file, receiver, lookup))
        }
        "String.replace" | "String.replaceAll" => {
            let text = |span: Span| string_literal_text(file, span);
            receiver()
                .and_then(text)
                .zip(
                    call.arguments
                        .first()
                        .and_then(|pattern| text(pattern.span)),
                )
                .is_some_and(|(receiver, pattern)| {
                    !pattern.is_empty() && !receiver.contains(pattern)
                })
        }
        _ => false,
    }
}

/// The standard-library methods that call nothing and add nothing when their receiver is empty, and never write it: an
/// array a name holds that only these read stays empty.
const KEEPS_EMPTY: &[&str] = &[
    "Array.forEach",
    "ReadonlyArray.forEach",
    "Array.map",
    "ReadonlyArray.map",
    "Array.flatMap",
    "ReadonlyArray.flatMap",
    "Array.filter",
    "ReadonlyArray.filter",
    "Array.some",
    "ReadonlyArray.some",
    "Array.every",
    "ReadonlyArray.every",
    "Array.find",
    "ReadonlyArray.find",
    "Array.findIndex",
    "ReadonlyArray.findIndex",
    "Array.findLast",
    "ReadonlyArray.findLast",
    "Array.findLastIndex",
    "ReadonlyArray.findLastIndex",
    "Array.reduce",
    "ReadonlyArray.reduce",
    "Array.reduceRight",
    "ReadonlyArray.reduceRight",
    "Array.sort",
    "Array.toSorted",
    "ReadonlyArray.toSorted",
];

/// ADR 1109: whether `span` (through parentheses and type assertions) is a reference to a `const` name (one identifier)
/// whose initializer is `[]`, that no module exports and no direct `eval` can reach, every reference to which is a type
/// query or the receiver of a call of a [`KEEPS_EMPTY`] method the compiler selected: nothing can fill the array, so it
/// holds no element wherever the name is read.
fn empty_array_name(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let Some(declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(span)) else {
        return false;
    };
    let holds_empty = ast.bindings.iter().any(|binding| {
        binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.immutable
            && binding.names.iter().any(|name| name.span == declaration)
            && binding.initializer.is_some_and(|initializer| {
                ast.array_literal_at(ast.peel_ts_sugar_span(initializer))
                    .is_some_and(|array| array.elements.is_empty())
            })
    });
    if !holds_empty
        || lookup.program_has_eval()
        || crate::execution_role::flow_binding_exported(file, declaration)
    {
        return false;
    }
    ast.reference_identifiers_to(declaration).all(|reference| {
        ast.type_queries
            .iter()
            .any(|query| query.contains(reference.span))
            || ast.members.iter().any(|member| {
                member.object == reference.span
                    && ast.calls.iter().any(|call| {
                        !call.construct
                            && ast.peel_ts_sugar_span(call.callee) == member.span
                            && crate::indexes::standard_library_declaration_of(
                                lookup.resolved_callee_call(file, call.callee),
                                typefacts::CallKind::Call,
                            )
                            .is_some_and(|declaration| {
                                KEEPS_EMPTY.contains(&declaration.qualified_name.as_ref())
                            })
                    })
            })
    })
}

/// ADR 1019: whether `span` (a reference to a function's name, or a function
/// literal) is handed, as it stands, to a standard-library inline carrier that
/// [`carrier_provably_never_runs`] it: the carrier neither calls it nor keeps
/// it, so the value goes nowhere from there.
pub(crate) fn handed_to_idle_carrier(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        if argument.spread || ast.peel_ts_sugar_span(argument.span) != span {
            return false;
        }
        if lookup.primitive_at_call(file, call.span).is_some() {
            return false;
        }
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            return false;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), argument.span)
            .and_then(|entity| entity.callability);
        crate::runtime_semantics::is_inline_standard_callback(resolved, callability, index)
            && carrier_provably_never_runs(file, call.span, lookup)
    })
}

/// Whether `span` (through parentheses and type assertions) is a `new`
/// expression with no argument whose constructor is the standard-library
/// `constructor` (`SetConstructor.construct`, `MapConstructor.construct`).
fn fresh_collection(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    constructor: &str,
) -> bool {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    let Some(construct) = ast
        .calls
        .iter()
        .find(|call| call.span == span && call.construct)
    else {
        return false;
    };
    construct.arguments.is_empty()
        && crate::indexes::standard_library_declaration_of(
            lookup.resolved_callee_call(file, construct.callee),
            typefacts::CallKind::Construct,
        )
        .is_some_and(|declaration| declaration.qualified_name.as_ref() == constructor)
}

/// ADR 1216 slice 3: whether `span` (through parentheses and type assertions) is a `new` expression of the
/// standard-library `constructor` (`SetConstructor.construct`, `MapConstructor.construct`) whose one argument is an
/// array literal with at least one element and no spread or hole; for a `Map`, each element is an array literal itself
/// (an entry object: `AddEntriesFromIterable` throws on a primitive before any entry is added).
fn seeded_collection(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
    constructor: &str,
) -> bool {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    let Some(construct) = ast
        .calls
        .iter()
        .find(|call| call.span == span && call.construct)
    else {
        return false;
    };
    let [source] = construct.arguments.as_slice() else {
        return false;
    };
    if source.spread || literal_elements(file, source.span) == 0 {
        return false;
    }
    let entries = constructor != "MapConstructor.construct"
        || ast
            .array_literal_at(ast.peel_ts_sugar_span(source.span))
            .is_some_and(|array| {
                array.elements.iter().all(|element| {
                    ast.array_literal_at(ast.peel_ts_sugar_span(element.span))
                        .is_some()
                })
            });
    entries
        && crate::indexes::standard_library_declaration_of(
            lookup.resolved_callee_call(file, construct.callee),
            typefacts::CallKind::Construct,
        )
        .is_some_and(|declaration| declaration.qualified_name.as_ref() == constructor)
}

/// The value of the string literal at `span` (through parentheses and type
/// assertions) when it is written in single or double quotes with no escape:
/// its text between the quotes is its value. `None` for anything else.
fn string_literal_text(file: &solid_facts::FileFacts, span: Span) -> Option<&str> {
    let text = file.source_text(file.ast.peel_ts_sugar_span(span))?;
    let quote = text.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let inner = text.strip_prefix(quote)?.strip_suffix(quote)?;
    (!inner.contains(quote) && !inner.contains('\\') && !inner.contains('\n')).then_some(inner)
}

/// ADR 1033: whether `span` (through parentheses and type assertions) is a
/// `NodeList` that holds no node: the `childNodes` of a [`fresh_empty_node`],
/// or the result of the standard `querySelectorAll` called on one (a node with
/// no descendants matches nothing).
fn fresh_empty_node_list(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    if let Some(member) = ast.member_at(span) {
        // The property is the DOM's own `childNodes` by its symbol's declarations, not by its spelling.
        return lookup.lib_dom_member(file, member.property, "childNodes")
            && fresh_empty_node(file, member.object, lookup);
    }
    let Some(call) = ast
        .calls
        .iter()
        .find(|call| call.span == span && !call.construct)
    else {
        return false;
    };
    let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    call.arguments.len() == 1
        && !call.arguments[0].spread
        && crate::indexes::standard_library_declaration_of(
            lookup.resolved_callee_call(file, call.callee),
            typefacts::CallKind::Call,
        )
        .is_some_and(|declaration| {
            declaration.qualified_name.as_ref() == "ParentNode.querySelectorAll"
        })
        && fresh_empty_node(file, member.object, lookup)
}

/// ADR 1033: whether `span` (through parentheses and type assertions) makes an
/// element with no children, written there: `document.createElement("i")`, a
/// quoted name with no `-` (no custom element's constructor runs) and no
/// options (no customized built-in's), the standard-library method of the
/// library's global `document`.
fn fresh_empty_node(
    file: &solid_facts::FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let ast = &file.ast;
    let span = ast.peel_ts_sugar_span(span);
    let Some(call) = ast
        .calls
        .iter()
        .find(|call| call.span == span && !call.construct)
    else {
        return false;
    };
    let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
        return false;
    };
    if lookup.global_dom_variable(file, ast.peel_ts_sugar_span(member.object)) != Some("document") {
        return false;
    }
    let Some(declaration) = crate::indexes::standard_library_declaration_of(
        lookup.resolved_callee_call(file, call.callee),
        typefacts::CallKind::Call,
    ) else {
        return false;
    };
    declaration.qualified_name.as_ref() == "Document.createElement"
        && call.arguments.len() == 1
        && !call.arguments[0].spread
        && string_literal_text(file, call.arguments[0].span)
            .is_some_and(|name| !name.is_empty() && !name.contains('-'))
}
