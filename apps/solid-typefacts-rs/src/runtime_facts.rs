//! Runtime identity and runtime binding kinds (Go: `runtimeIdentitySymbol` of
//! `tsgo/semantic_entities.go` and `tsgo/runtime_binding_kinds.go`).
//!
//! The binding kind closes the runtime kind of one exact identifier by
//! following its canonical symbol to the runtime declaration and inspecting
//! every direct write to that binding. It deliberately does not treat property
//! mutation (`value.x = ...`) as reassignment of `value`. A write whose value
//! is dynamic stays open; disagreement between closed writes stays mixed.
//! Nothing here infers behavior from a name, a declaration's text, or a
//! package: a relative import is followed only to the exact source file the
//! program already holds, and everything else stays open.

use crate::durable::durable_runtime_ref_for;
use crate::gocompat::clean;
use crate::identity::Env;
use crate::type_facts::{callability_of_type, constructability_of_type};
use std::collections::HashSet;
use tsrs_ast::{
    Kind, Node, Symbol, SymbolFlags, get_assignment_target, get_source_file_of_node,
    is_array_literal_expression, is_arrow_function, is_as_expression, is_binary_expression,
    is_block, is_call_expression, is_class_declaration, is_class_expression,
    is_conditional_expression, is_enum_declaration, is_function_declaration,
    is_function_expression, is_identifier, is_import_clause, is_import_declaration,
    is_import_specifier, is_logical_or_coalescing_assignment_operator, is_module_declaration,
    is_namespace_import, is_no_substitution_template_literal, is_non_null_expression,
    is_numeric_literal, is_object_literal_expression, is_parenthesized_expression,
    is_part_of_type_only_import_or_export_declaration, is_postfix_unary_expression,
    is_prefix_unary_expression, is_return_statement, is_satisfies_expression, is_string_literal,
    is_variable_declaration,
};
use tsrs_core::P;
use typefacts::{Callability, Constructability, RuntimeBindingKind};

/// Go: `typefacts.RuntimeBindingKind`, including the internal "absent" rung
/// that never reaches the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind3 {
    Absent,
    Callable,
    NonCallable,
    Mixed,
    Open,
}

impl Kind3 {
    fn wire(self) -> Option<RuntimeBindingKind> {
        match self {
            Self::Absent => None,
            Self::Callable => Some(RuntimeBindingKind::Callable),
            Self::NonCallable => Some(RuntimeBindingKind::NonCallable),
            Self::Mixed => Some(RuntimeBindingKind::Mixed),
            Self::Open => Some(RuntimeBindingKind::Open),
        }
    }
}

/// Go: `mergeRuntimeBindingKinds`.
fn merge(left: Kind3, right: Kind3) -> Kind3 {
    if left == Kind3::Absent {
        return right;
    }
    if right == Kind3::Absent || left == right {
        return left;
    }
    if left == Kind3::Open || right == Kind3::Open {
        return Kind3::Open;
    }
    Kind3::Mixed
}

/// Go: `runtimeIdentitySymbol` and the `runtimeID` it feeds: the identity of
/// the runtime value `symbol` denotes, or empty when it denotes none. It
/// rejects aliases and declarations which the compiler represents
/// semantically but which cannot denote a runtime value: type-only
/// import/export aliases and type-level property signatures such as JSX
/// intrinsic entries.
pub fn runtime_identity(env: &mut Env<'_>, symbol: P<Symbol>) -> String {
    if symbol.flags().intersects(SymbolFlags::Alias)
        && symbol
            .declarations()
            .iter()
            .any(|declaration| is_part_of_type_only_import_or_export_declaration(*declaration))
    {
        return String::new();
    }
    let symbol = env.canonical_symbol(symbol);
    let Some(value_declaration) = symbol.value_declaration() else {
        return String::new();
    };
    if !symbol.flags().intersects(SymbolFlags::Value)
        || value_declaration.kind() == Kind::PropertySignature
    {
        return String::new();
    }
    let Some(reference) = durable_runtime_ref_for(symbol) else {
        return String::new();
    };
    let path = env.runtime_path(&reference.path);
    reference.runtime_id(&path)
}

/// Go: `runtimeBindingKindLocked`, mapped to the wire value (`None` is absent).
pub fn runtime_binding_kind(env: &mut Env<'_>, node: P<Node>) -> Option<RuntimeBindingKind> {
    Kinds { env }.binding_kind(node).wire()
}

struct Kinds<'a, 'e> {
    env: &'a mut Env<'e>,
}

type Seen = HashSet<usize>;

impl Kinds<'_, '_> {
    fn binding_kind(&mut self, node: P<Node>) -> Kind3 {
        if !is_identifier(node) {
            return Kind3::Absent;
        }
        let symbol = self.env.checker.get_symbol_at_location_exported(node);
        self.symbol_kind(symbol, &mut Seen::new())
    }

    /// Go: `runtimeSymbolKindLocked`.
    #[allow(clippy::too_many_lines)]
    fn symbol_kind(&mut self, symbol: Option<P<Symbol>>, seen: &mut Seen) -> Kind3 {
        let mut symbol = symbol;
        while let Some(current) = symbol
            && current.flags().intersects(SymbolFlags::Alias)
        {
            if let Some(imported) = self.imported_alias_kind(current, seen) {
                return imported;
            }
            match self.env.checker.get_immediate_aliased_symbol(current) {
                Some(next) if next.addr() != current.addr() => symbol = Some(next),
                _ => return Kind3::Open,
            }
        }
        let Some(symbol) = symbol else {
            return Kind3::Open;
        };
        if !symbol.flags().intersects(SymbolFlags::Value) {
            return Kind3::Open;
        }
        if !seen.insert(symbol.addr()) {
            return Kind3::Open;
        }
        let kind = self.value_symbol_kind(symbol, seen);
        seen.remove(&symbol.addr());
        kind
    }

    fn value_symbol_kind(&mut self, symbol: P<Symbol>, seen: &mut Seen) -> Kind3 {
        let declaration = symbol
            .value_declaration()
            .or_else(|| symbol.declarations().first().copied());
        let Some(declaration) = declaration else {
            return Kind3::Open;
        };
        let source_file = get_source_file_of_node(declaration);
        let Some(source_file) = source_file.filter(|file| !file.is_declaration_file()) else {
            return self.expression_kind(Some(declaration), seen);
        };

        let mut kind = if is_function_declaration(declaration) || is_class_declaration(declaration)
        {
            Kind3::Callable
        } else if is_enum_declaration(declaration) || is_module_declaration(declaration) {
            // Both declarations materialize namespace-like objects at runtime.
            Kind3::NonCallable
        } else if is_variable_declaration(declaration) {
            match declaration.initializer() {
                // `var x` and `let x` begin as undefined. Later writes are
                // folded in below; retaining this state matters for
                // conditionally written vars.
                None => Kind3::NonCallable,
                initializer => self.expression_kind(initializer, seen),
            }
        } else {
            self.expression_kind(Some(declaration), seen)
        };

        // A module-scope exported binding cannot be reassigned from another
        // source file. Scan its complete declaring file, including nested
        // closures, because any such closure may execute after import. Symbol
        // equality, not spelling, distinguishes shadowed identifiers.
        for statement in source_file.as_node().statements() {
            self.visit_writes(*statement, symbol, &mut kind, seen);
        }
        if kind == Kind3::Absent {
            return Kind3::Open;
        }
        kind
    }

    /// The `visit` closure of Go's `runtimeSymbolKindLocked`: folds every
    /// direct write to `symbol` under `node` into `kind`.
    fn visit_writes(
        &mut self,
        node: P<Node>,
        symbol: P<Symbol>,
        kind: &mut Kind3,
        seen: &mut Seen,
    ) {
        if is_identifier(node) {
            let candidate = self
                .env
                .checker
                .get_symbol_at_location_exported(node)
                .map(|found| self.env.canonical_symbol(found));
            if candidate.is_some_and(|candidate| candidate.addr() == symbol.addr())
                && let Some(target) = get_assignment_target(node)
            {
                let mut write = Kind3::Open;
                if is_binary_expression(target) {
                    let binary = target.as_binary_expression();
                    if is_identifier(binary.left()) && binary.left().addr() == node.addr() {
                        let operator = binary.operator_token().kind();
                        write = if operator == Kind::EqualsToken
                            || is_logical_or_coalescing_assignment_operator(operator)
                        {
                            self.expression_kind(Some(binary.right()), seen)
                        } else {
                            // Arithmetic/bitwise/update assignment always
                            // writes a primitive, even when its operands were
                            // any.
                            Kind3::NonCallable
                        };
                    }
                } else if is_prefix_unary_expression(target) || is_postfix_unary_expression(target)
                {
                    write = Kind3::NonCallable;
                }
                *kind = merge(*kind, write);
            }
        }
        // `ForEachChild` visits children in order; the callback never stops it.
        node.for_each_child(&mut |child| {
            self.visit_writes(child, symbol, kind, seen);
            false
        });
    }

    /// Go: `runtimeImportedAliasKindLocked`: follows only an explicit relative
    /// runtime module specifier to the exact source file already held by the
    /// configured program. TypeScript commonly redirects `./x.mjs` to
    /// `./x.d.mts`; using that declaration target would turn published typings
    /// into runtime proof. The literal runtime path plus program membership
    /// avoids that substitution. `None` is Go's `exact == false`.
    #[allow(clippy::too_many_lines)]
    fn imported_alias_kind(&mut self, symbol: P<Symbol>, seen: &mut Seen) -> Option<Kind3> {
        if !symbol.flags().intersects(SymbolFlags::Alias) {
            return None;
        }
        for declaration in symbol.declarations() {
            let declaration = *declaration;
            let imported_name: String = if is_import_specifier(declaration) {
                if declaration.as_import_specifier().is_type_only {
                    continue;
                }
                if let Some(property) = declaration.property_name() {
                    property.text().to_owned()
                } else if let Some(name) = declaration.name() {
                    name.text().to_owned()
                } else {
                    String::new()
                }
            } else if is_import_clause(declaration) {
                if declaration.name().is_none() {
                    continue;
                }
                "default".to_owned()
            } else if is_namespace_import(declaration) {
                // A module namespace exotic object is noncallable by language
                // definition, independently of any member it exposes.
                return Some(Kind3::NonCallable);
            } else {
                continue;
            };
            if imported_name.is_empty() {
                continue;
            }
            let mut owner = declaration.parent();
            while let Some(current) = owner
                && !is_import_declaration(current)
            {
                owner = current.parent();
            }
            let Some(owner) = owner else {
                continue;
            };
            let Some(specifier) = owner.module_specifier() else {
                continue;
            };
            let text = specifier.text();
            if (!text.starts_with("./") && !text.starts_with("../")) || extension(text).is_empty() {
                continue;
            }
            let Some(importer) = get_source_file_of_node(declaration) else {
                continue;
            };
            let directory = match clean(importer.file_name()).rsplit_once('/') {
                Some(("", _)) => "/".to_owned(),
                Some((directory, _)) => directory.to_owned(),
                None => ".".to_owned(),
            };
            let target_path = clean(&format!("{directory}/{text}"));
            let Some(target) = self.env.program.get_source_file(&target_path) else {
                continue;
            };
            let Some(module_symbol) = target.symbol().filter(|_| !target.is_declaration_file())
            else {
                continue;
            };
            let exports = self
                .env
                .checker
                .get_exports_of_module_exported(module_symbol);
            for exported in exports {
                if exported.name() == imported_name {
                    return Some(self.symbol_kind(Some(exported), seen));
                }
            }
            return Some(Kind3::Open);
        }
        None
    }

    /// Go: `runtimeExpressionKindLocked`.
    #[allow(clippy::too_many_lines)]
    fn expression_kind(&mut self, node: Option<P<Node>>, seen: &mut Seen) -> Kind3 {
        let Some(node) = node else {
            return Kind3::Open;
        };
        let value = self.env.checker.get_type_at_location(node);
        let callability = callability_of_type(self.env.checker, value);
        let constructability = constructability_of_type(self.env.checker, value);
        if callability == Callability::Callable
            || callability == Callability::UntypedCallable
            || constructability == Constructability::Constructable
        {
            return Kind3::Callable;
        }
        if callability == Callability::NonCallable
            && constructability == Constructability::NonConstructable
        {
            return Kind3::NonCallable;
        }
        if callability == Callability::Mixed || constructability == Constructability::Mixed {
            return Kind3::Mixed;
        }

        if is_parenthesized_expression(node)
            || is_as_expression(node)
            || is_satisfies_expression(node)
            || is_non_null_expression(node)
        {
            return self.expression_kind(node.expression(), seen);
        }
        if is_arrow_function(node) || is_function_expression(node) || is_class_expression(node) {
            return Kind3::Callable;
        }
        if is_object_literal_expression(node)
            || is_array_literal_expression(node)
            || is_string_literal(node)
            || is_numeric_literal(node)
            || is_no_substitution_template_literal(node)
            || is_prefix_unary_expression(node)
            || is_postfix_unary_expression(node)
        {
            return Kind3::NonCallable;
        }
        if is_conditional_expression(node) {
            let conditional = node.as_conditional_expression();
            let when_true = self.expression_kind(Some(conditional.when_true()), seen);
            let when_false = self.expression_kind(Some(conditional.when_false()), seen);
            return merge(when_true, when_false);
        }
        if is_binary_expression(node) {
            let binary = node.as_binary_expression();
            return match binary.operator_token().kind() {
                Kind::EqualsToken | Kind::CommaToken => {
                    self.expression_kind(Some(binary.right()), seen)
                }
                // `left && right` can expose left only when left is falsy.
                // Every callable/constructable JavaScript value is truthy, so
                // the exposed short-circuit partition is necessarily
                // noncallable. This is what closes chains such as
                // `!!host && host.fn && host.fn.name === x` without trusting
                // the unknown host property.
                Kind::AmpersandAmpersandToken => {
                    let right = self.expression_kind(Some(binary.right()), seen);
                    merge(Kind3::NonCallable, right)
                }
                Kind::BarBarToken | Kind::QuestionQuestionToken => {
                    let left = self.expression_kind(Some(binary.left()), seen);
                    let right = self.expression_kind(Some(binary.right()), seen);
                    merge(left, right)
                }
                // All remaining JavaScript binary operators produce primitives.
                _ => Kind3::NonCallable,
            };
        }
        if is_identifier(node) {
            let symbol = self.env.checker.get_symbol_at_location_exported(node);
            return self.symbol_kind(symbol, seen);
        }
        if is_call_expression(node) {
            return self.iife_kind(node, seen);
        }
        Kind3::Open
    }

    /// Go: `runtimeIIFEKindLocked`.
    fn iife_kind(&mut self, call: P<Node>, seen: &mut Seen) -> Kind3 {
        let mut callee = call.expression();
        while let Some(current) = callee
            && is_parenthesized_expression(current)
        {
            callee = current.expression();
        }
        let Some(callee) = callee.filter(|c| is_arrow_function(*c) || is_function_expression(*c))
        else {
            // An external helper's result is intentionally not inferred from
            // its name, declaration text, or arguments.
            return Kind3::Open;
        };
        let Some(body) = callee.body() else {
            return Kind3::Open;
        };
        if !is_block(body) {
            return self.expression_kind(Some(body), seen);
        }
        let mut kind = Kind3::Absent;
        body.for_each_child(&mut |child| {
            self.collect_returns(child, body, &mut kind, seen);
            false
        });
        let statements = body.statements();
        if statements
            .last()
            .is_none_or(|last| !is_return_statement(*last))
        {
            // Without a terminal return, falling through to undefined remains
            // a possible noncallable value. This may over-refuse unreachable
            // tails but can never manufacture a closed kind.
            kind = merge(kind, Kind3::NonCallable);
        }
        if kind == Kind3::Absent {
            return Kind3::NonCallable;
        }
        kind
    }

    fn collect_returns(
        &mut self,
        current: P<Node>,
        body: P<Node>,
        kind: &mut Kind3,
        seen: &mut Seen,
    ) {
        if current.addr() != body.addr()
            && (is_arrow_function(current)
                || is_function_expression(current)
                || is_function_declaration(current)
                || is_class_declaration(current))
        {
            return;
        }
        if is_return_statement(current) {
            *kind = match current.expression() {
                Some(expression) => {
                    let value = self.expression_kind(Some(expression), seen);
                    merge(*kind, value)
                }
                None => merge(*kind, Kind3::NonCallable),
            };
            return;
        }
        current.for_each_child(&mut |child| {
            self.collect_returns(child, body, kind, seen);
            false
        });
    }
}

/// Go: `filepath.Ext`: the suffix starting at the final dot of the last path
/// element, or empty.
fn extension(path: &str) -> &str {
    let element = path.rsplit('/').next().unwrap_or(path);
    element.rfind('.').map_or("", |dot| &element[dot..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_follows_the_rung_order() {
        use Kind3::*;
        assert_eq!(merge(Absent, Callable), Callable);
        assert_eq!(merge(Callable, Absent), Callable);
        assert_eq!(merge(Callable, Callable), Callable);
        assert_eq!(merge(Callable, Open), Open);
        assert_eq!(merge(Open, NonCallable), Open);
        assert_eq!(merge(Callable, NonCallable), Mixed);
        assert_eq!(merge(Mixed, Callable), Mixed);
    }

    #[test]
    fn extension_matches_filepath_ext() {
        assert_eq!(extension("./x.js"), ".js");
        assert_eq!(extension("./x"), "");
        assert_eq!(extension("../a.b/x"), "");
        assert_eq!(extension("./x.d.mts"), ".mts");
        assert_eq!(extension("./.hidden"), ".hidden");
    }
}
