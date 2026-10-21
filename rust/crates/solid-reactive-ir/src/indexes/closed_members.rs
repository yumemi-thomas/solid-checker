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
        let value = *self
            .closed_members
            .get_or_init(|| closed_members(self))
            .get(&(file.path.as_str(), member.span))?;
        let function = file.ast.function_at(value)?;
        Some((file, function))
    }
}

/// Every member read of a closed literal's function value, keyed by the member
/// expression and valued by the function's span.
fn closed_members<'a>(lookup: &SemanticLookup<'a>) -> HashMap<(&'a str, Span), Span> {
    let mut found = HashMap::default();
    for file in lookup.files() {
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
    if !binding.immutable
        || binding.shape != BindingShape::Identifier
        || binding.names.len() != 1
        || binding.initializer_value_kind != RuntimeValueKind::Object
    {
        return Vec::new();
    }
    let Some(initializer) = binding.initializer else {
        return Vec::new();
    };
    let literal = file.ast.peel_ts_sugar_span(initializer);
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
        let value = file.ast.peel_ts_sugar_span(property.value);
        if file.ast.function_at(value).is_some() {
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
