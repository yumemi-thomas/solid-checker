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
//! Every other carrier (`Set`/`Map`/`NodeList` `forEach`, a variable or
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
