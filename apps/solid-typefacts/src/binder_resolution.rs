//! Which identifiers the checker resolves without type information (ADR 0369).
//!
//! `get_symbol_at_location` answers most identifiers by scope lookup alone:
//! `resolve_entity_name` with `dontResolveAlias`, which walks the binder's
//! scopes, the file's module scope and the global table and touches no type.
//! Such an answer, and its id (a pure function of the program, ADR 0366),
//! depends on the file's text, the declarations it reaches, the global scope
//! and the options, and not on what the checker computed before. Every other
//! position goes through types (a property name is a property of the
//! object's type), through alias resolution (which can read an `export =`
//! value's type or a default export's expression), or through JSDoc and JSX
//! intrinsics, and its answer is history-dependent: it is never retained.
//!
//! The rules mirror `Checker::get_symbol_at_location` and
//! `get_symbol_of_name_or_property_access_expression` at tsrs 3bbcc754
//! (checker_15.rs; the meta-property and import-phase arms differ from 467182f
//! and are type-resolved here). A position this module cannot place is type-resolved
//! (fail closed): the cost of a wrong "type-resolved" is a recomputation, the
//! cost of a wrong "binder-resolved" a stale answer.

use tsrs_ast::{
    Kind, Node, NodeFlags, is_binding_element, is_declaration_name,
    is_declaration_name_or_import_property_name, is_identifier, is_import_or_export_specifier,
    is_jsx_tag_name, is_meta_property, is_name_of_heritage_clause_type_reference,
    is_object_binding_pattern, is_qualified_name,
};
use tsrs_core::P;
use tsrs_scanner::is_intrinsic_jsx_name;

/// Whether `get_symbol_at_location(node)` answers the reference scan's
/// identifier `node` (not a declaration name) by scope lookup alone.
#[must_use]
pub fn reference_resolves_without_types(node: P<Node>) -> bool {
    if !is_identifier(node) || node.flags().intersects(NodeFlags::JSDoc) {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    if is_declaration_name_or_import_property_name(node) {
        return false;
    }
    // `obj.name`, `A.B`: the right side is a property or export of the left
    // side's type or namespace.
    if (parent.kind() == Kind::PropertyAccessExpression || is_qualified_name(parent))
        && parent.name() == Some(node)
    {
        return false;
    }
    // `{ name: local } = value`: a property of the pattern's type.
    if is_binding_element(parent)
        && parent.parent().is_some_and(is_object_binding_pattern)
        && parent.property_name() == Some(node)
    {
        return false;
    }
    // `new.target`, `import.meta`.
    if is_meta_property(parent) {
        return false;
    }
    // `<div>`: a property of `JSX.IntrinsicElements`.
    if is_jsx_tag_name(node) && is_intrinsic_jsx_name(node.text()) {
        return false;
    }
    // `export default x`, `export = x`, `import x = A.B`: resolved with the
    // alias followed.
    if in_right_side_of_import_or_export_assignment(node) {
        return false;
    }
    // `f<T>`, `class C extends B`, `implements I`: resolved with the alias
    // followed (`dontResolveAlias` false).
    if in_name_of_expression_with_type_arguments_or_heritage_type_reference(node) {
        return false;
    }
    // `x is T`: resolved with the alias followed.
    if parent.kind() == Kind::TypePredicate {
        return false;
    }
    // `import("m").A`: the import type is resolved first.
    if in_import_type_qualifier(node) {
        return false;
    }
    true
}

/// Whether an entity demand's node is answered by scope lookup alone: a
/// reference the scan would place there, or a declaration's own name
/// (`get_symbol_of_declaration` of an identifier-named declaration reads only
/// the binder). The property name of `import { a as b }` and
/// `export { a as b }` resolves the alias one step and is excluded.
#[must_use]
pub fn entity_resolves_without_types(node: P<Node>) -> bool {
    if !is_identifier(node) || node.flags().intersects(NodeFlags::JSDoc) {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    if is_import_or_export_specifier(parent) {
        return parent.property_name() != Some(node) && parent.name() == Some(node);
    }
    if is_declaration_name(node) {
        return true;
    }
    reference_resolves_without_types(node)
}

/// `utilities.rs` `is_in_right_side_of_import_or_export_assignment`
/// (`pub(crate)` in tsrs).
fn in_right_side_of_import_or_export_assignment(node: P<Node>) -> bool {
    let mut node = node;
    while let Some(parent) = node.parent()
        && parent.kind() == Kind::QualifiedName
    {
        node = parent;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    (parent.kind() == Kind::ImportEqualsDeclaration
        && parent.as_import_equals_declaration().module_reference == node)
        || (parent.kind() == Kind::ExportAssignment && parent.expression() == Some(node))
}

/// `utilities.rs`
/// `is_in_name_of_expression_with_type_arguments_or_heritage_type_reference`.
fn in_name_of_expression_with_type_arguments_or_heritage_type_reference(node: P<Node>) -> bool {
    let mut node = node;
    while let Some(parent) = node.parent()
        && (parent.kind() == Kind::PropertyAccessExpression || parent.kind() == Kind::QualifiedName)
    {
        node = parent;
    }
    node.parent()
        .is_some_and(|parent| parent.kind() == Kind::ExpressionWithTypeArguments)
        || is_name_of_heritage_clause_type_reference(node)
}

/// `utilities.rs` `is_import_type_qualifier_part`.
fn in_import_type_qualifier(node: P<Node>) -> bool {
    let mut node = node;
    let mut parent = node.parent();
    while let Some(candidate) = parent
        && is_qualified_name(candidate)
    {
        node = candidate;
        parent = candidate.parent();
    }
    parent.is_some_and(|parent| {
        parent.kind() == Kind::ImportType && parent.as_import_type_node().qualifier == Some(node)
    })
}
