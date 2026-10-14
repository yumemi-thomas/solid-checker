//! ADR 0299: exact calls through immutable, source-owned object bindings.
//! Export/import is a channel into the same receiver census, not an escape
//! waiver. No structural signature or composite entity supplies a target.

use std::collections::{HashMap, HashSet};

use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use crate::{
    SymbolId,
    indexes::{SemanticLookup, binding_written},
    interproc::accessor_provenance::{MemberStability, Stability},
    location,
};

pub(crate) type Targets = HashMap<Location, (String, Span)>;

pub(crate) fn index(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Targets {
    if facts.files.iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == ast::ModuleHazardKind::Eval)
    }) {
        return HashMap::new();
    }
    let stability = MemberStability::index(facts, lookup);
    let mut targets = HashMap::new();
    let mut closed = HashMap::new();
    for file in &facts.files {
        for call in &file.ast.calls {
            if call.construct {
                continue;
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let Some(member) = file.ast.members.iter().find(|m| m.span == callee) else {
                continue;
            };
            if file.ast.computed_members.contains(&callee) {
                continue;
            }
            let receiver = file.ast.peel_ts_sugar_span(member.object);
            if !identifier(file, receiver) {
                continue;
            }
            let Some(symbol) = lookup.entities().at(file.path.as_str(), receiver) else {
                continue;
            };
            let Some((owner, literal, declaration, frozen)) =
                receiver_literal(file, receiver, symbol, lookup)
            else {
                continue;
            };
            let stable = *closed
                .entry((owner.path.to_string(), declaration, symbol.clone()))
                .or_insert_with(|| receiver_closed(owner, literal, declaration, symbol, lookup));
            if !stable {
                continue;
            }
            let Some(name) = file.source_text(member.property) else {
                continue;
            };
            if !matches!(
                stability.state_of(facts, owner.path.as_str(), name),
                Stability::Assumed
            ) {
                continue;
            }
            if frozen
                && !matches!(
                    stability.state_of(facts, owner.path.as_str(), "freeze"),
                    Stability::Assumed
                )
            {
                continue;
            }
            let Some(function) = literal_function(owner, literal, name, lookup) else {
                continue;
            };
            if !receiver_inert(function.0, function.1) {
                continue;
            }
            // Both receiver and own callable are now proven non-nullish;
            // optional links therefore select the same function.
            targets.insert(
                location(file.path.shared(), call.callee),
                (function.0.path.to_string(), function.1.span),
            );
        }
    }
    targets
}

fn identifier(file: &FileFacts, span: Span) -> bool {
    file.ast
        .identifiers
        .iter()
        .any(|id| id.span == span && id.role == ast::IdentifierRole::Reference)
}

fn object_literal(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(Span, bool)> {
    let span = file.ast.peel_ts_sugar_span(span);
    if file
        .ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == span)
    {
        return Some((span, false));
    }
    let call = file.ast.call_at(span)?;
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    if declaration.qualified_name.as_ref() != "ObjectConstructor.freeze"
        || call.arguments.len() != 1
        || call.arguments[0].spread
    {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let member = file
        .ast
        .members
        .iter()
        .find(|m| m.span == callee && !file.ast.computed_members.contains(&callee))?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if file.source_text(member.property) != Some("freeze")
        || file.source_text(receiver) != Some("Object")
        || file.ast.reference_declaration(receiver).is_some()
    {
        return None;
    }
    let symbol = lookup.symbol_at(file.path.as_str(), receiver)?;
    if !symbol.alias_target().is_empty() {
        return None;
    }
    let declarations = symbol
        .declarations()
        .iter()
        .filter(|declaration| {
            declaration.kind.as_ref() == "variable" && declaration.name.as_ref() == "Object"
        })
        .collect::<Vec<_>>();
    if declarations.len() != 1
        || declarations[0].location.path != declaration.source_file
        || !declaration.source_file.ends_with("/lib.es5.d.ts")
    {
        return None;
    }
    let literal = file.ast.peel_ts_sugar_span(call.arguments[0].span);
    file.ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == literal)
        .then_some((literal, true))
}

fn receiver_literal<'a>(
    file: &FileFacts,
    receiver: Span,
    symbol: &SymbolId,
    lookup: &SemanticLookup<'a>,
) -> Option<(&'a FileFacts, Span, Span, bool)> {
    if let Some((owner, binding, _)) = lookup.binding_at_reference(file.path.as_str(), receiver) {
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
        {
            return None;
        }
        let declaration = binding.names[0].span;
        if binding_written(owner, declaration) {
            return None;
        }
        let (literal, frozen) = object_literal(owner, binding.initializer?, lookup)?;
        return Some((owner, literal, declaration, frozen));
    }
    // Anonymous `export default { … }` has an exact export declaration,
    // not a variable binding. Require that very expression location.
    let declarations = lookup.symbol_declarations(symbol.as_str())?;
    if declarations.len() != 1 {
        return None;
    }
    let declaration = &declarations[0].location;
    let owner = lookup.file_by_path(declaration.path.as_ref())?;
    let export = owner.ast.exports.iter().find(|export| {
        export.kind == ast::ExportKind::Default
            && ((u64::from(export.span.start) == declaration.start_byte
                && u64::from(export.span.end) == declaration.end_byte)
                || export.declarations.iter().any(|value| {
                    u64::from(value.local.span.start) == declaration.start_byte
                        && u64::from(value.local.span.end) == declaration.end_byte
                }))
    })?;
    let value = export.declarations.first()?.local.span;
    let (literal, frozen) = object_literal(owner, value, lookup)?;
    Some((owner, literal, value, frozen))
}

fn receiver_closed<'a>(
    owner: &'a FileFacts,
    literal: Span,
    declaration: Span,
    symbol: &SymbolId,
    lookup: &SemanticLookup<'a>,
) -> bool {
    let mut references = HashSet::new();
    // Binder references remain essential when a dependency import's Type
    // Facts references still name its .d.ts symbol before runtime redirects.
    for file in lookup.files() {
        let mut bindings = HashSet::new();
        if file.path == owner.path {
            bindings.insert(declaration);
        }
        for import in &file.ast.imports {
            for binding in &import.bindings {
                if lookup.entities().at(file.path.as_str(), binding.local.span) == Some(symbol) {
                    if binding.imported.is_none() {
                        return false;
                    }
                    bindings.insert(binding.local.span);
                }
            }
        }
        for (reference, bound) in &file.ast.reference_declarations {
            if lookup.entities().at(file.path.as_str(), *reference) == Some(symbol)
                && file.ast.imports.iter().any(|import| {
                    import
                        .bindings
                        .iter()
                        .any(|binding| binding.local.span == *bound && binding.imported.is_some())
                })
            {
                bindings.insert(*bound);
            }
        }
        for (reference, bound) in &file.ast.reference_declarations {
            if bindings.contains(bound) {
                references.insert(location(file.path.shared(), *reference));
            }
        }
        for id in &file.ast.identifiers {
            if id.role == ast::IdentifierRole::Reference
                && lookup.entities().at(file.path.as_str(), id.span) == Some(symbol)
            {
                references.insert(location(file.path.shared(), id.span));
            }
        }
    }
    if let Some(semantic) = lookup.symbol_references_if_present(symbol.as_str()) {
        references.extend(semantic);
    }
    for reference in references {
        let Some(file) = lookup.file_by_path(reference.path.as_ref()) else {
            return false;
        };
        let Ok(start) = u32::try_from(reference.start_byte) else {
            return false;
        };
        let Ok(end) = u32::try_from(reference.end_byte) else {
            return false;
        };
        let span = Span::new(start, end);
        if file
            .ast
            .type_queries
            .iter()
            .any(|query| query.contains(span))
        {
            continue;
        }
        // Import/export name tokens are the exact module channel. Namespace
        // accesses and any ordinary value use are deliberately refused.
        if file.ast.imports.iter().any(|import| {
            !import.type_only
                && import
                    .bindings
                    .iter()
                    .any(|binding| binding.imported.is_some() && binding.local.span == span)
        }) || file.ast.exports.iter().any(|export| {
            export.namespace.is_none()
                && export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span == span)
        }) {
            continue;
        }
        if file.path == owner.path && span == declaration {
            continue;
        }
        let Some(member) = file.ast.members.iter().find(|member| {
            file.ast.peel_ts_sugar_span(member.object) == span
                && !file.ast.computed_members.contains(&member.span)
        }) else {
            return false;
        };
        // Merely extracting a member lets a method receive an opaque `this`.
        // Require immediate calls, rather than treating all member reads as
        // harmless receiver uses.
        if !file
            .ast
            .calls
            .iter()
            .any(|call| !call.construct && file.ast.peel_ts_sugar_span(call.callee) == member.span)
        {
            return false;
        }
        let Some((function_file, function)) = file
            .source_text(member.property)
            .and_then(|name| literal_function(owner, literal, name, lookup))
        else {
            return false;
        };
        if !receiver_inert(function_file, function) {
            return false;
        }
    }
    true
}

fn receiver_inert(file: &FileFacts, function: &ast::FunctionFact) -> bool {
    !function.generator
        && file.source_text(function.span).is_some_and(|text| {
            !["this", "arguments", "eval", "super", "\\"]
                .iter()
                .any(|spelling| text.contains(spelling))
        })
}

fn literal_function<'a>(
    file: &'a FileFacts,
    literal: Span,
    name: &str,
    lookup: &SemanticLookup<'a>,
) -> Option<(&'a FileFacts, &'a ast::FunctionFact)> {
    let shape = file.ast.object_get_shapes.iter().find(|shape| {
        shape.span == literal && shape.closed && shape.properties.iter().all(|p| p.getter.is_none())
    })?;
    let own = |property: &&ast::ObjectPropertyFact| {
        shape.span.contains(property.span)
            && !file.ast.object_get_shapes.iter().any(|nested| {
                nested.span != literal
                    && literal.contains(nested.span)
                    && nested.span.contains(property.span)
            })
    };
    let properties = file
        .ast
        .object_properties
        .iter()
        .filter(own)
        .collect::<Vec<_>>();
    // Methods can expose their receiver through `this` or `arguments` to
    // opaque code. This conservative spelling veto only withholds a target;
    // it never supplies identity (comments/strings can over-veto).
    if properties
        .iter()
        .any(|p| !p.data || p.value_kind == ast::RuntimeValueKind::Function)
        && file.source_text(literal).is_some_and(|text| {
            text.contains("this")
                || text.contains("arguments")
                || text.contains("eval")
                || text.contains("super")
        })
    {
        return None;
    }
    if properties.len() != shape.properties.len() {
        return None;
    }
    let property = properties
        .into_iter()
        .zip(&shape.properties)
        .find(|(property, key)| !property.computed && key.key == name)?
        .0;
    let value = file.ast.peel_ts_sugar_span(property.value);
    if let Some(function) = file.ast.functions.iter().find(|f| f.span == value) {
        return Some((file, function));
    }
    // Shorthand is one declared function, not a callable type or an alias.
    let declaration = property.shorthand_binding?;
    if let Some(function) = file.ast.functions.iter().find(|function| {
        function.kind == ast::FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) {
        return lookup
            .function_value_is_current(file, function)
            .then_some((file, function));
    }
    let symbol = lookup.entities().at(file.path.as_str(), declaration)?;
    let (owner, function) = lookup.function_for_symbol(symbol.as_str())?;
    (function.kind == ast::FunctionKind::Declaration
        && lookup.function_value_is_current(owner, function))
    .then_some((owner, function))
}
