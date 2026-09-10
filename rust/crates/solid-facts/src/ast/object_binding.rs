//! Exact local object-binding facts, without package or receipt authority.

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{BindingPattern, Declaration, Expression, IdentifierReference, Statement};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use sha2::{Digest as _, Sha256};

/// Positive recognition of a module-level object initializer and its unwritten
/// lexical binding. Members and initialization effects are deliberately unproved.
/// Constructed locally from complete source, never deserialized as authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnwrittenObjectBinding {
    source_digest: String,
    binding: Span,
}

impl UnwrittenObjectBinding {
    #[must_use]
    pub fn matches(&self, source: &str, binding: Span) -> bool {
        self.binding == binding
            && self.source_digest == format!("{:x}", Sha256::digest(source.as_bytes()))
    }
}

/// Recognize an exact declaration-name span, not a containing or same-name node.
/// Only JavaScript ESM grammar is accepted. Dynamic lexical evaluation is outside
/// this proof, including shadowed `eval` (a conservative refusal).
#[must_use]
pub fn unwritten_object_binding(source: &str, binding: Span) -> Option<UnwrittenObjectBinding> {
    if source.len() > 1024 * 1024 {
        return None;
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    if !built.errors.is_empty() {
        return None;
    }
    let mut dynamic = EvalReference(false);
    dynamic.visit_program(&parsed.program);
    if dynamic.0 {
        return None;
    }
    let scoping = built.semantic.scoping();
    for statement in &parsed.program.body {
        let declaration = match statement {
            Statement::VariableDeclaration(declaration) => declaration,
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::VariableDeclaration(declaration)) => declaration,
                _ => continue,
            },
            _ => continue,
        };
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            if identifier.span.start != binding.start || identifier.span.end != binding.end {
                continue;
            }
            if !matches!(
                declarator.init.as_ref()?.get_inner_expression(),
                Expression::ObjectExpression(_)
            ) {
                return None;
            }
            let symbol = identifier.symbol_id.get()?;
            if !scoping.symbol_redeclarations(symbol).is_empty()
                || scoping
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
            {
                return None;
            }
            return Some(UnwrittenObjectBinding {
                source_digest: format!("{:x}", Sha256::digest(source.as_bytes())),
                binding,
            });
        }
    }
    None
}

struct EvalReference(bool);

impl<'a> Visit<'a> for EvalReference {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.0 |= identifier.name == "eval";
        walk::walk_identifier_reference(self, identifier);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recognize(source: &str) -> Option<UnwrittenObjectBinding> {
        let start = u32::try_from(source.find("value").unwrap()).unwrap();
        unwritten_object_binding(
            source,
            Span {
                start,
                end: start + 5,
            },
        )
    }

    #[test]
    fn object_binding_binds_exact_bytes_and_declaration() {
        let source = "export var value = { method() {} }; function f() { let value = 1; value++; }";
        let proof = recognize(source).unwrap();
        assert!(proof.matches(source, Span { start: 11, end: 16 }));
        assert!(!proof.matches(&format!("{source} "), Span { start: 11, end: 16 }));
        assert!(!proof.matches(source, Span { start: 10, end: 16 }));
        assert!(recognize("const value = {}; value.member = () => {}; ").is_some());
    }

    #[test]
    fn object_binding_refuses_writes_redeclarations_and_dynamic_scope() {
        for source in [
            "var value = {}; value = () => {};",
            "var value = {}; function f() { value = () => {}; }",
            "var value = {}; ({ value } = other);",
            "var value = {}; for (value of items) {}",
            "var value = {}; var value = () => {};",
            "const value = {}; eval('value');",
            "const value = () => {};",
            "const value = new Proxy({}, {});",
            "function f() { const value = {}; }",
            "declare const value: {};",
        ] {
            assert!(recognize(source).is_none(), "{source}");
        }
    }
}
