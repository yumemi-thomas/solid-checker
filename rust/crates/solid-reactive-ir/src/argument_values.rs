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
    mapped_parameter_values(file, call, callee_file, callee, mapping)
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
    mapped_parameter_values(file, call, callee_file, callee, mapping)
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
    mapped_parameter_values(file, call, callee_file, callee, mapping)
}

fn mapped_parameter_values(
    file: &FileFacts,
    call: &CallFact,
    callee_file: &FileFacts,
    callee: &FunctionFact,
    mapping: ExactArgumentMapping<'_>,
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
            Some(argument) => argument_value(file, index, argument),
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
) -> Option<ParameterValue> {
    let span = file.ast.peel_ts_sugar_span(argument.span);
    let text = file.source_text(span)?.trim();
    let known = |type_of: &'static str, truthy: bool| ParameterValue {
        index,
        type_of: Some(type_of),
        truthy: Some(truthy),
        nullish: Some(false),
        undefined: Some(false),
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
