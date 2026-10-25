//! Structured returns: the shorthand properties an exported function returns
//! that no exact project declaration answers (`StructuredReturnUnresolved`).
//!
//! The per-function return shapes (objects, tuples, nested results) that
//! package contracts published were computed here by a Jacobi loop over
//! every summary node. Contract emission is gone (D2), and the shorthand
//! question reads none of them: it asks only the property's own file, the
//! import's TypeScript symbol chain and the declaring files' syntax.

use super::*;

/// Why a shorthand return value cannot be resolved strongly enough to prove
/// either a reactive leaf or an exact non-reactive project value. `None`
/// means the binder/import graph closed the question.
pub(super) fn unresolved_shorthand_reason(
    file: &solid_facts::FileFacts,
    span: Span,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
) -> Option<String> {
    let property = file
        .ast
        .object_properties
        .iter()
        .find(|property| property.value == span && property.key == property.value)?;
    let Some(declaration) = property.shorthand_binding else {
        return Some("the shorthand resolves only through a global or unavailable binding".into());
    };
    let Some((module, binding)) = file.ast.imports.iter().find_map(|import| {
        import
            .bindings
            .iter()
            .find(|binding| binding.local.span == declaration)
            .map(|binding| (import.module.as_str(), binding))
    }) else {
        // A local declaration is exact. Whether it is reactive is answered
        // by the local accessor/source indexes, so absence there is a
        // proven non-reactive result rather than uncertainty.
        return None;
    };
    if binding.type_only || matches!(binding.kind, solid_facts::ast::ImportKind::Namespace) {
        return None;
    }
    if !matches!(
        binding.kind,
        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
    ) {
        return None;
    }
    // TypeScript already resolved the import, including tsconfig paths,
    // extension priority, and re-export cycles. Follow its exact symbol
    // chain and require a declaration in an analyzed project source file;
    // merely seeing the identity at a project re-export is insufficient,
    // because that export may forward an external package value. Local
    // accessor/source maps decide reactivity from there. External package
    // declarations therefore remain fail-closed even behind a relative
    // project re-export.
    if imported_binding_has_project_source_declaration(file, declaration, facts, lookup) {
        return None;
    }
    Some(format!(
        "{module:?} resolves to no runtime declaration in the analyzed project"
    ))
}

fn imported_binding_has_project_source_declaration(
    file: &solid_facts::FileFacts,
    declaration: Span,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(entity) = lookup.entity_at(file.path.as_str(), declaration) else {
        return false;
    };
    if entity.runtime_identity.is_empty() || entity.symbol.is_empty() {
        return false;
    }
    let mut symbol_id = entity.symbol.as_ref();
    let mut seen = HashSet::default();
    while seen.insert(symbol_id) {
        let Some(symbol) = facts.typescript.symbol(symbol_id) else {
            return false;
        };
        if symbol.declarations().iter().any(|declaration| {
            !declaration.location.path.ends_with(".d.ts")
                && lookup
                    .file_by_path(declaration.location.path.as_ref())
                    .is_some_and(|target_file| {
                        let is_binding = target_file.ast.identifiers.iter().any(|identifier| {
                            identifier.role == solid_facts::ast::IdentifierRole::Binding
                                && u64::from(identifier.span.start)
                                    == declaration.location.start_byte
                                && u64::from(identifier.span.end) == declaration.location.end_byte
                        });
                        let is_import = target_file.ast.imports.iter().any(|import| {
                            import.bindings.iter().any(|binding| {
                                u64::from(binding.local.span.start)
                                    == declaration.location.start_byte
                                    && u64::from(binding.local.span.end)
                                        == declaration.location.end_byte
                            })
                        });
                        is_binding && !is_import
                    })
        }) {
            return true;
        }
        let alias = symbol.alias_target();
        if alias.is_empty() {
            return false;
        }
        symbol_id = alias;
    }
    false
}
