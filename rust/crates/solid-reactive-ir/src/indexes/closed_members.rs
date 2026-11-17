//! Exact object-literal members (ADR 0351): `api.load()` where `api` is a
//! `const` object literal that nothing else can reach, and `load` is one of its
//! function values. Its callee is that function, whatever the receiver's class:
//! a literal has no prototype channel the walk must consider, and no write can
//! replace the property, since the binding's every reference is a plain member
//! read.

use super::*;
use solid_facts::ast::{
    BindingFact, BindingShape, ExportKind, FunctionFact, IdentifierRole, MemberFact,
    RuntimeValueKind,
};

impl<'a> SemanticLookup<'a> {
    /// The function a member read names when `member` is `object.name` on a
    /// closed `const` object literal and `name` is one of its function values
    /// (ADR 0351). Nothing else is claimed: a reassigned, spread, computed,
    /// exported or escaping literal names no function.
    pub(crate) fn closed_member_function(
        &self,
        file: &FileFacts,
        member: &MemberFact,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        let file = self.file_by_path(file.path.as_str())?;
        let function = file
            .ast
            .function_at(self.closed_member_value(file, member)?)?;
        Some((file, function))
    }

    /// The value of a closed literal's member read: a function, or a name
    /// that may hold one.
    pub(crate) fn closed_member_value(
        &self,
        file: &FileFacts,
        member: &MemberFact,
    ) -> Option<Span> {
        self.closed_members
            .get_or_init(|| closed_members(self))
            .get(&(file.path.as_str(), member.span))
            .copied()
    }
}

/// Every member read of a closed literal's function value, keyed by the member
/// expression and valued by the function's span.
fn closed_members<'a>(lookup: &SemanticLookup<'a>) -> HashMap<(&'a str, Span), Span> {
    let mut found = HashMap::default();
    for file in lookup.file_slots() {
        // Built once per file: the member and reference indexes every binding
        // of the file needs.
        let members: HashMap<Span, &MemberFact> = file
            .ast
            .members
            .iter()
            .map(|member| (member.object, member))
            .collect();
        for binding in &file.ast.bindings {
            for (member, value) in closed_binding_members(lookup, file, &members, binding) {
                found.insert((file.path.as_str(), member), value);
            }
        }
    }
    found
}

/// The member reads of one binding's literal that name a function value, when
/// the binding is closed: a `const` of one name initialized by a plain object
/// literal (no spread, computed key, accessor, duplicate key, `this`, or
/// `__proto__`), not exported, whose every reference is a plain member read
/// that is neither optional nor written nor deleted.
fn closed_binding_members(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    members: &HashMap<Span, &MemberFact>,
    binding: &BindingFact,
) -> Vec<(Span, Span)> {
    if !binding.immutable || binding.shape != BindingShape::Identifier || binding.names.len() != 1 {
        return Vec::new();
    }
    let Some(initializer) = binding.initializer else {
        return Vec::new();
    };
    // ADR 1058: `const box = h(f)` where `h` returns a fresh literal holding its parameter (`return { cb }`) is that literal,
    // with the parameter's property naming the argument.
    let boxed = if binding.initializer_value_kind == RuntimeValueKind::Object {
        None
    } else {
        match boxed_literal(lookup, file, initializer) {
            Some(boxed) => Some(boxed),
            None => return Vec::new(),
        }
    };
    let literal = boxed.map_or_else(
        || file.ast.peel_ts_sugar_span(initializer),
        |boxed| boxed.literal,
    );
    let Some(text) = file.source_text(literal) else {
        return Vec::new();
    };
    // A text check is coarse, and it is conservative: any spread, `this` or
    // prototype spelling in the literal (or in a function it holds) closes nothing.
    if !text.starts_with('{')
        || text.contains("...")
        || text.contains("__proto__")
        || mentions_word(text, "this")
    {
        return Vec::new();
    }
    let mut keys: HashMap<&str, usize> = HashMap::default();
    let mut functions: HashMap<&str, Span> = HashMap::default();
    for property in &file.ast.object_properties {
        if !literal.contains(property.span) {
            continue;
        }
        // A computed key may name any property, and an accessor runs its body.
        if property.computed || !(property.data || property.method) {
            return Vec::new();
        }
        let key = file
            .source_text(property.key)
            .unwrap_or_default()
            .trim_matches(['"', '\'']);
        *keys.entry(key).or_default() += 1;
        let mut value = file.ast.peel_ts_sugar_span(property.value);
        if let Some(boxed) = boxed {
            // The helper's other values are its own: only the parameter's property is the caller's function.
            if value != boxed.parameter_use {
                continue;
            }
            value = boxed.argument;
        }
        if file.ast.function_at(value).is_some() || crate::owners::is_identifier_span(file, value) {
            functions.insert(key, value);
        }
    }
    if functions.is_empty() || keys.values().any(|count| *count > 1) {
        return Vec::new();
    }
    let name = binding.names[0].span;
    let Some(symbol) = lookup.entities.at(file.path.as_str(), name) else {
        return Vec::new();
    };
    // Exported (by name, or by its symbol through `export { … }`), or a default
    // export that names it in its text: other modules could reach it.
    let declared = file.source_text(name).unwrap_or_default();
    let exported = file.ast.exports.iter().any(|export| {
        export
            .declarations
            .iter()
            .chain(&export.specifiers)
            .any(|specifier| {
                specifier.local.span == name
                    || lookup.entities.at(file.path.as_str(), specifier.local.span) == Some(symbol)
            })
            || (export.kind == ExportKind::Default
                && mentions_word(file.source_text(export.span).unwrap_or_default(), declared))
    });
    if exported {
        return Vec::new();
    }
    let mut reads = Vec::new();
    for identifier in &file.ast.identifiers {
        if identifier.role != IdentifierRole::Reference
            || lookup.entities.at(file.path.as_str(), identifier.span) != Some(symbol)
        {
            continue;
        }
        // Any other use (an argument, an alias, a computed or optional member,
        // a write or delete of the member) may change or hand out the object.
        let Some(member) = members.get(&identifier.span) else {
            return Vec::new();
        };
        if file.ast.computed_members.contains(&member.span)
            || file
                .ast
                .optional_members
                .binary_search(&member.span)
                .is_ok()
            || file.ast.write_targets.contains(&member.span)
            || file.ast.deleted_targets.contains(&member.span)
        {
            return Vec::new();
        }
        let property = file
            .source_text(member.property)
            .unwrap_or_default()
            .trim_matches(['"', '\'']);
        if let Some(value) = functions.get(property) {
            reads.push((member.span, *value));
        }
    }
    reads
}

/// A call that evaluates to a fresh object literal holding one of its arguments (ADR 1058): `h(f)` where `h` is a synchronous,
/// non-generator function declared in this file under a name nothing writes, whose only completion is one straight-line `return`
/// (or an arrow's expression body) of an object literal, and whose parameter at `f`'s position is a plain identifier read exactly
/// once, as a property value of that literal (`return { cb }`, `return { cb: cb }`). Each call makes a new object nothing else
/// names, so the caller's `const` holds the only reference.
#[derive(Clone, Copy)]
struct BoxedLiteral {
    /// The object literal in the helper.
    literal: Span,
    /// The parameter's read in the literal: the property value that holds the argument.
    parameter_use: Span,
    /// The argument at the parameter's position, peeled.
    argument: Span,
}

fn boxed_literal(
    lookup: &SemanticLookup<'_>,
    file: &FileFacts,
    initializer: Span,
) -> Option<BoxedLiteral> {
    let call = file.ast.call_at(file.ast.peel_ts_sugar_span(initializer))?;
    if call.construct || !crate::owners::is_identifier_span(file, call.callee) {
        return None;
    }
    let path = file.path.as_str();
    let symbol = lookup.entities.value_symbol(file, call.callee)?;
    let helper = file.ast.functions.as_slice().iter().find(|function| {
        function.kind == solid_facts::ast::FunctionKind::Declaration
            && function.name.as_ref().is_some_and(|name| {
                lookup.entities.at(path, name.span) == Some(symbol)
                    && !super::binding_written_through_wrappers(file, name.span)
            })
    })?;
    boxing_helper(file, helper).and_then(|(literal, index, parameter_use)| {
        if call.arguments.len() <= index
            || call
                .arguments
                .iter()
                .take(index + 1)
                .any(|argument| argument.spread)
        {
            return None;
        }
        Some(BoxedLiteral {
            literal,
            parameter_use,
            argument: file.ast.peel_ts_sugar_span(call.arguments[index].span),
        })
    })
}

/// The literal `helper` returns, the index of the parameter it holds, and that parameter's one read (see [`BoxedLiteral`]).
fn boxing_helper(file: &FileFacts, helper: &FunctionFact) -> Option<(Span, usize, Span)> {
    if helper.r#async || helper.generator || helper.parameters.is_empty() {
        return None;
    }
    let returns = if helper.expression_body {
        helper.expression_return.iter().collect::<Vec<_>>()
    } else {
        file.ast
            .returns
            .iter()
            .filter(|returned| {
                crate::owners::containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == helper.span)
            })
            .collect()
    };
    let [returned] = returns.as_slice() else {
        return None;
    };
    if returned.runtime_value_kind != RuntimeValueKind::Object
        || !(returned.straight_line || helper.expression_body)
    {
        return None;
    }
    // `runtime_value_kind` is `Object` only for an object literal node.
    let literal = file.ast.peel_ts_sugar_span(returned.argument?);
    let mut held = None;
    for property in &file.ast.object_properties {
        if !literal.contains(property.span) || property.computed || !property.data {
            continue;
        }
        let value = file.ast.peel_ts_sugar_span(property.value);
        if !crate::owners::is_identifier_span(file, value) {
            continue;
        }
        let text = file.source_text(value)?;
        let Some(index) = helper.parameters.iter().position(|parameter| {
            parameter.shape == BindingShape::Identifier
                && parameter.names.len() == 1
                && file.source_text(parameter.names[0].span) == Some(text)
        }) else {
            continue;
        };
        if held.is_some() {
            return None;
        }
        held = Some((index, value, property.key, text));
    }
    let (index, value, key, text) = held?;
    let parameter = helper.parameters[index].names[0].span;
    // Any other occurrence of the name in the helper (a store, a call, a default that captures it, a shadowing binding), and
    // `arguments` or `eval`, may hand the parameter on or read another value under the name.
    for identifier in &file.ast.identifiers {
        if !helper.span.contains(identifier.span) {
            continue;
        }
        let spelled = file.source_text(identifier.span);
        if spelled == Some("arguments") || spelled == Some("eval") {
            return None;
        }
        if spelled == Some(text) && ![parameter, value, key].contains(&identifier.span) {
            return None;
        }
    }
    // The read must be the parameter's own binding, not a nested function's parameter of the same name.
    if !crate::owners::containing_ast_function(&file.ast, value)
        .is_some_and(|owner| owner.span == helper.span)
    {
        return None;
    }
    Some((literal, index, value))
}
