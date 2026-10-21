//! ADR 0280: what a call's arguments prove about the called function's
//! parameters, for [`solid_facts::ast::argument_site_standings`].
//!
//! Only an exact mapping proves anything: every argument lands in the
//! parameter of its index (no spread or `f.call`) and the call site has one
//! exact implementation, from [`crate::execution_role::project_callee_at_parameter`]
//! or a call-specific literal/transport witness. A value is read off the
//! argument's syntax (a literal, a function or object literal), or off its
//! type when the compiler proves every constituent callable. Anything else
//! proves nothing about that parameter.

use solid_facts::FileFacts;
use solid_facts::ast::{BindingShape, CallFact, FunctionFact, ParameterValue, RuntimeValueKind};
use solid_facts::core::Span;

use crate::SemanticLookup;

/// Exact positional correspondence between one call and one function body.
/// It is constructed either from `project_callee_at_parameter` or from an
/// independently proven literal/transported target. Keeping the call site and
/// target identity together prevents a signature or a function name from
/// standing in for exact dispatch.
#[derive(Clone, Copy)]
struct ExactArgumentMapping<'a> {
    call_path: &'a str,
    call_callee: solid_facts::core::Span,
    target_path: &'a str,
    target_span: solid_facts::core::Span,
}

impl<'a> ExactArgumentMapping<'a> {
    fn projected(
        file: &'a FileFacts,
        call: &CallFact,
        callee_file: &'a FileFacts,
        callee: &FunctionFact,
    ) -> Self {
        Self {
            call_path: file.path.as_str(),
            call_callee: call.callee,
            target_path: callee_file.path.as_str(),
            target_span: callee.span,
        }
    }

    fn literal(
        file: &'a FileFacts,
        call: &CallFact,
        callee_file: &'a FileFacts,
        callee: &FunctionFact,
    ) -> Option<Self> {
        if file.path != callee_file.path
            || call.construct
            || call.arguments.iter().any(|argument| argument.spread)
        {
            return None;
        }
        let literal = crate::cleanup::callback_argument_literal(
            file,
            file.ast.peel_ts_sugar_span(call.callee),
        )?;
        (literal.span == callee.span).then(|| Self::projected(file, call, callee_file, callee))
    }

    fn transported(
        file: &'a FileFacts,
        call: &CallFact,
        callee_file: &'a FileFacts,
        callee: &FunctionFact,
        exact_target: &crate::interproc::accessor_provenance::Callable,
        lookup: &SemanticLookup<'_>,
    ) -> Option<Self> {
        let crate::interproc::accessor_provenance::Callable::Function { path, span } = exact_target
        else {
            return None;
        };
        if path != callee_file.path.as_str()
            || *span != callee.span
            || call.construct
            || call.arguments.iter().any(|argument| argument.spread)
        {
            return None;
        }
        // A transported member can have receiver-sensitive behavior, and
        // `.call`/`.apply` shift the positional arguments. This witness is
        // limited to receiver-free calls; the existing member path remains
        // responsible for method execution.
        let callee_span = file.ast.peel_ts_sugar_span(call.callee);
        if lookup.is_member_span(file, callee_span)
            || file
                .ast
                .computed_members
                .binary_search(&callee_span)
                .is_ok()
        {
            return None;
        }
        Some(Self::projected(file, call, callee_file, callee))
    }

    fn matches(
        self,
        file: &FileFacts,
        call: &CallFact,
        callee_file: &FileFacts,
        callee: &FunctionFact,
    ) -> bool {
        self.call_path == file.path.as_str()
            && self.call_callee == call.callee
            && self.target_path == callee_file.path.as_str()
            && self.target_span == callee.span
    }
}

/// The values `call` proves for `callee`'s parameters, or `None` when the
/// argument mapping is not exact.
pub(crate) fn parameter_values(
    file: &FileFacts,
    call: &CallFact,
    callee_file: &FileFacts,
    callee: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Vec<ParameterValue>> {
    // A function that tests none of its parameters has nothing to decide.
    if !lookup.tests_its_parameters(callee_file, callee) {
        return None;
    }
    let (resolved_file, resolved, exact) = crate::execution_role::project_callee_at_parameter(
        file,
        call,
        call.arguments.len(),
        lookup,
    )?;
    if !exact || resolved_file.path != callee_file.path || resolved.span != callee.span {
        return None;
    }
    let mapping = ExactArgumentMapping::projected(file, call, resolved_file, resolved);
    mapped_parameter_values(file, call, callee_file, callee, mapping, lookup)
}

/// Values proved by a call whose exact target is an immediate function
/// literal. The literal callee itself supplies target identity, while this
/// mapping witness still requires ordinary positional arguments without
/// construction or spreads.
pub(crate) fn parameter_values_for_literal_call(
    file: &FileFacts,
    call: &CallFact,
    callee_file: &FileFacts,
    callee: &FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Vec<ParameterValue>> {
    if !lookup.tests_its_parameters(callee_file, callee) {
        return None;
    }
    let mapping = ExactArgumentMapping::literal(file, call, callee_file, callee)?;
    mapped_parameter_values(file, call, callee_file, callee, mapping, lookup)
}

/// Values proved by a stable transported callable target. `exact_target`
/// must be the target already selected for this exact call site by the source
/// callable transport map.
pub(crate) fn parameter_values_for_transported_call(
    file: &FileFacts,
    call: &CallFact,
    callee_file: &FileFacts,
    callee: &FunctionFact,
    exact_target: &crate::interproc::accessor_provenance::Callable,
    lookup: &SemanticLookup<'_>,
) -> Option<Vec<ParameterValue>> {
    if !lookup.tests_its_parameters(callee_file, callee) {
        return None;
    }
    let mapping =
        ExactArgumentMapping::transported(file, call, callee_file, callee, exact_target, lookup)?;
    mapped_parameter_values(file, call, callee_file, callee, mapping, lookup)
}

fn mapped_parameter_values(
    file: &FileFacts,
    call: &CallFact,
    callee_file: &FileFacts,
    callee: &FunctionFact,
    mapping: ExactArgumentMapping<'_>,
    lookup: &SemanticLookup<'_>,
) -> Option<Vec<ParameterValue>> {
    if !mapping.matches(file, call, callee_file, callee)
        || call.arguments.iter().any(|argument| argument.spread)
    {
        return None;
    }
    let mut values = Vec::new();
    for (index, parameter) in callee.parameters.iter().enumerate() {
        // A rest parameter is outside the evaluator's parameter items, so a
        // value at its index is ignored there.
        if parameter.shape != BindingShape::Identifier {
            break;
        }
        // A written or redeclared parameter (`var p = …`) may hold another
        // value by the time it is tested (review round 4).
        if parameter
            .names
            .first()
            .is_none_or(|name| crate::indexes::binding_written(callee_file, name.span))
        {
            continue;
        }
        let value = match call.arguments.get(index) {
            Some(argument) => argument_value(
                file,
                index,
                argument,
                lookup,
                (mapping.target_path, mapping.target_span),
            ),
            None => Some(ParameterValue::omitted(index)),
        };
        // A default replaces an `undefined` argument, explicit or omitted.
        if let Some(value) = value
            && (parameter.initializer.is_none() || value.undefined == Some(false))
        {
            values.push(value);
        }
    }
    Some(values)
}

fn argument_value(
    file: &FileFacts,
    index: usize,
    argument: &solid_facts::ast::ArgumentFact,
    lookup: &SemanticLookup<'_>,
    target: (&str, Span),
) -> Option<ParameterValue> {
    let span = file.ast.peel_ts_sugar_span(argument.span);
    let text = file.source_text(span)?.trim();
    let known = |type_of: &'static str, truthy: bool| ParameterValue {
        index,
        type_of: Some(type_of),
        truthy: Some(truthy),
        nullish: Some(false),
        undefined: Some(false),
        length: None,
    };
    match text {
        "true" => return Some(known("boolean", true)),
        "false" => return Some(known("boolean", false)),
        "null" => {
            return Some(ParameterValue {
                index,
                type_of: Some("object"),
                truthy: Some(false),
                nullish: Some(true),
                undefined: Some(false),
                length: None,
            });
        }
        "undefined" | "void 0" if argument.runtime_value_kind == RuntimeValueKind::Nullish => {
            return Some(ParameterValue::omitted(index));
        }
        _ => {}
    }
    if let Some(truthy) = numeric_literal_truthiness(text) {
        return Some(known("number", truthy));
    }
    if let Some(truthy) = string_literal_truthiness(text) {
        return Some(known("string", truthy));
    }
    if let Some(length) = solid_getter_length(file, span, lookup, target) {
        return Some(ParameterValue {
            length: Some(length),
            ..known("function", true)
        });
    }
    if crate::cleanup::callback_argument_literal(file, argument.span).is_some()
        || argument.runtime_value_kind == RuntimeValueKind::Function
    {
        return Some(known("function", true));
    }
    // A type says nothing about runtime nullishness without
    // `strictNullChecks`, which these facts do not record (review round 4),
    // so only syntax proves a value.
    matches!(
        argument.runtime_value_kind,
        RuntimeValueKind::Object | RuntimeValueKind::Array
    )
    .then(|| known("object", true))
}

/// ADR 0338: the exact `length` of the getter `reference` names, when it is
/// the accessor slot of a Solid primitive's result that the selected dialect
/// has audited ([`Dialect::accessor_getter_length`]) and nothing in the file
/// rewrites the binding. The primitive is the exact one the initializer's own
/// callee resolves to (a direct or namespace reference to an import of the
/// dialect's package, never a name and never a member call on a call's
/// result: `createSignal(1).slice(1)` is the array's method), the pattern's
/// slot is the accessor's (`const [value] = createSignal(0)`, or the whole of
/// a `createMemo`), and a store, a setter, a tuple left unpacked, a parameter
/// or an import prove nothing.
///
/// A function's `length` is configurable (`Object.defineProperty(getter,
/// "length", …)`), so the fact also needs a reference census
/// ([`getter_is_only_called_or_decided`]): the getter may be called, and
/// handed to the very function `target` this call decides, whose own uses of
/// the parameter cannot run for these arguments; any other use refuses.
fn solid_getter_length(
    file: &FileFacts,
    reference: Span,
    lookup: &SemanticLookup<'_>,
    target: (&str, Span),
) -> Option<u32> {
    use solid_dialect::ResultSlot;

    let declaration = file.ast.reference_declaration(reference)?;
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.call_initializer.is_some()
            && binding.names.iter().any(|name| name.span == declaration)
    })?;
    let call = file.ast.call_at(binding.call_initializer?)?;
    if call.construct || crate::indexes::binding_written(file, declaration) {
        return None;
    }
    // The callee is the primitive itself: an identifier, or a member of a
    // namespace import. A member of anything else (a call's result among
    // them) is some other value's method.
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let direct = file.ast.reference_declaration(callee).is_some()
        || file.ast.member_at(callee).is_some_and(|member| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
                .is_some()
        });
    if !direct {
        return None;
    }
    let entities = lookup.entities();
    let symbol = entities.at(file.path.as_str(), declaration)?;
    if crate::value_identity::binding_has_write(file, entities, symbol) {
        return None;
    }
    let primitive = crate::call_primitive_name_cached(
        lookup,
        file,
        call,
        entities,
        lookup.symbol_names(),
        lookup.dialect,
    )?
    .primitive()?;
    let slot = match binding.shape {
        BindingShape::Array => {
            let first = binding.array_slots.first()?.as_ref()?;
            (first.span == declaration).then_some(ResultSlot::TupleItem(0))?
        }
        BindingShape::Identifier => ResultSlot::Whole,
        _ => return None,
    };
    let length = lookup.dialect.accessor_getter_length(primitive, slot)?;
    // Every getter is `read.bind(null, node)`: a project that replaces
    // `Function.prototype.bind` can give it any `length`.
    if lookup.function_bind_may_be_patched() {
        return None;
    }
    // An exported getter is reachable with no static reference: through the
    // module namespace (`import * as m`, `Object.values(m)`, `m[key]`,
    // `import()`), where other code can redefine its `length` first.
    if binding_is_exported(file, binding, declaration) {
        return None;
    }
    getter_is_only_called_or_decided(file, declaration, symbol, length, lookup, target)
        .then_some(length)
}

/// Whether the module publishes the binding declared at `declaration`: an
/// export declaration that declares it at module level (`export const [v] =
/// …`), or an export specifier naming it (`export { v }`,
/// `export { v as default }`). A declaration nested in a function or class
/// the export contains belongs to that body, not to the module.
fn binding_is_exported(
    file: &FileFacts,
    binding: &solid_facts::ast::BindingFact,
    declaration: Span,
) -> bool {
    file.ast.module_level_exports().any(|export| {
        (export.span.contains(binding.declaration)
            && !file.ast.classes.iter().any(|class| {
                class.span.contains(binding.declaration) && class.span != binding.declaration
            })
            && !file.ast.functions.iter().any(|function| {
                export.span.contains(function.span) && function.body.contains(binding.declaration)
            }))
            || export.specifiers.iter().any(|specifier| {
                specifier.local.span == declaration
                    || file.ast.reference_declaration(specifier.local.span) == Some(declaration)
            })
    })
}

/// Whether every reference to the getter declared at `declaration` is a call
/// of it, or an argument of a call of `target` (exactly resolved, no spread)
/// whose parameter's other uses cannot run for a getter of `length`. No
/// reference may sit in another file, and none may be a type-facts reference
/// this file's own table lacks.
fn getter_is_only_called_or_decided(
    file: &FileFacts,
    declaration: Span,
    symbol: &crate::identity::SymbolId,
    length: u32,
    lookup: &SemanticLookup<'_>,
    target: (&str, Span),
) -> bool {
    // A direct `eval` in the declaring file reaches the binding with no static
    // reference (`eval("Object.defineProperty(value, 'length', …)")`).
    if crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX)) {
        return false;
    }
    let mut references = file
        .ast
        .reference_declarations
        .iter()
        .filter(|(reference, owner)| *owner == declaration && *reference != declaration)
        .map(|(reference, _)| *reference)
        .collect::<crate::HashSet<_>>();
    for other in lookup.symbol_references(symbol.as_str()) {
        if other.path.as_ref() != file.path.as_str() {
            return false;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(other.start_byte),
            u32::try_from(other.end_byte),
        ) else {
            return false;
        };
        let other = Span::new(start, end);
        if other != declaration {
            if file.ast.reference_declaration(other) != Some(declaration) {
                return false;
            }
            references.insert(other);
        }
    }
    references.into_iter().all(|reference| {
        if file
            .ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == reference)
        {
            return true;
        }
        let Some((call, index)) = file.ast.calls.iter().find_map(|call| {
            call.arguments
                .iter()
                .position(|argument| file.ast.peel_ts_sugar_span(argument.span) == reference)
                .map(|index| (call, index))
        }) else {
            return false;
        };
        if call.construct || call.arguments.iter().any(|argument| argument.spread) {
            return false;
        }
        let Some((callee_file, callee, true)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, lookup)
        else {
            return false;
        };
        if (callee_file.path.as_str(), callee.span) != target {
            return false;
        }
        let path = std::path::Path::new(callee_file.path.as_str());
        let Some(uses) =
            solid_facts::ast::parameter_value_uses(path, &callee_file.source, callee.span, index)
        else {
            return false;
        };
        if uses.is_empty() {
            return true;
        }
        let value = ParameterValue {
            index,
            type_of: Some("function"),
            truthy: Some(true),
            nullish: Some(false),
            undefined: Some(false),
            length: Some(length),
        };
        solid_facts::ast::argument_site_standings(
            path,
            &callee_file.source,
            callee.span,
            &[value],
            &lookup.never_returning_calls_within(callee_file, callee),
            &uses,
        )
        .is_some_and(|standings| {
            standings
                .iter()
                .all(|standing| *standing == solid_facts::ast::SiteStanding::Unreachable)
        })
    })
}

/// The truthiness of a decimal, hex, octal or binary numeric literal.
fn numeric_literal_truthiness(text: &str) -> Option<bool> {
    let digits = text.replace('_', "");
    let (body, radix) = match digits.get(..2) {
        Some("0x" | "0X") => (&digits[2..], 16),
        Some("0o" | "0O") => (&digits[2..], 8),
        Some("0b" | "0B") => (&digits[2..], 2),
        _ => {
            if !digits.bytes().all(|byte| {
                byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-')
            }) || !digits
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit() || byte == b'.')
            {
                return None;
            }
            let value = digits.parse::<f64>().ok()?;
            return Some(value != 0.0);
        }
    };
    if body.is_empty() || !body.chars().all(|digit| digit.is_digit(radix)) {
        return None;
    }
    Some(body.chars().any(|digit| digit != '0'))
}

/// The truthiness of one plain quoted string or substitution-free template.
/// Any escape, a quote of the same kind inside, or a line break refuses: an
/// escape can decode to nothing (`"\⏎"`), and a second quote means the text
/// is more than one literal (review round 4).
fn string_literal_truthiness(text: &str) -> Option<bool> {
    let first = text.chars().next()?;
    if !matches!(first, '"' | '\'' | '`') || text.len() < 2 || !text.ends_with(first) {
        return None;
    }
    let inner = &text[1..text.len() - 1];
    if inner.contains(first) || inner.contains('\\') || inner.contains('\n') || inner.contains("${")
    {
        return None;
    }
    Some(!inner.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{numeric_literal_truthiness, string_literal_truthiness};

    #[test]
    fn literal_truthiness() {
        assert_eq!(numeric_literal_truthiness("30_000"), Some(true));
        assert_eq!(numeric_literal_truthiness("0"), Some(false));
        assert_eq!(numeric_literal_truthiness("0.0"), Some(false));
        assert_eq!(numeric_literal_truthiness("0x0"), Some(false));
        assert_eq!(numeric_literal_truthiness("0x1f"), Some(true));
        assert_eq!(numeric_literal_truthiness("1e3"), Some(true));
        assert_eq!(numeric_literal_truthiness("count"), None);
        assert_eq!(numeric_literal_truthiness("-1"), None);
        assert_eq!(string_literal_truthiness("\"\""), Some(false));
        assert_eq!(string_literal_truthiness("'a'"), Some(true));
        assert_eq!(string_literal_truthiness("`a${b}`"), None);
        assert_eq!(string_literal_truthiness("name"), None);
        assert_eq!(
            string_literal_truthiness("\"\\\\\" !== sep && sep !== \"\""),
            None
        );
        assert_eq!(string_literal_truthiness("\"\\\n\""), None);
    }
}
