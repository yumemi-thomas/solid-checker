//! Exact local object- and primitive-binding facts, without package or
//! receipt authority.

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{BindingPattern, Declaration, Expression, IdentifierReference, Statement};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_syntax::operator::UnaryOperator;
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
    unwritten_binding(source, binding, |initializer| {
        matches!(initializer, Expression::ObjectExpression(_))
    })
}

/// Positive recognition of a module-level primitive initializer and its
/// unwritten lexical binding (ADR 0130).
///
/// The same exact-span, resolved-write and dynamic-scope rules as
/// [`UnwrittenObjectBinding`]; only the initializer differs. It must be a
/// primitive literal -- string, number, bigint, boolean, `null`, or a template
/// with no substitutions -- or `void` applied to one. Each evaluates to a
/// primitive with no observable effect, so the binding never holds a value
/// with `[[Call]]` or `[[Construct]]`. The identifier `undefined` is refused:
/// it names a binding, not a literal. Nothing about the declared type is
/// claimed. Constructed locally from complete source, never deserialized as
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnwrittenPrimitiveBinding {
    source_digest: String,
    binding: Span,
}

impl UnwrittenPrimitiveBinding {
    #[must_use]
    pub fn matches(&self, source: &str, binding: Span) -> bool {
        self.binding == binding
            && self.source_digest == format!("{:x}", Sha256::digest(source.as_bytes()))
    }
}

/// Recognize an exact declaration-name span bound to a primitive literal; see
/// [`UnwrittenPrimitiveBinding`].
#[must_use]
pub fn unwritten_primitive_binding(
    source: &str,
    binding: Span,
) -> Option<UnwrittenPrimitiveBinding> {
    unwritten_binding(source, binding, |initializer| match initializer {
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void => {
            primitive_literal(unary.argument.get_inner_expression())
        }
        other => primitive_literal(other),
    })
    .map(
        |UnwrittenObjectBinding {
             source_digest,
             binding,
         }| UnwrittenPrimitiveBinding {
            source_digest,
            binding,
        },
    )
}

fn primitive_literal(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::TemplateLiteral(template) => template.expressions.is_empty(),
        _ => false,
    }
}

/// The walk both recognizers share: `initializer` decides only whether the
/// declarator's initializer, with transparent wrappers removed, has the
/// recognized shape.
fn unwritten_binding(
    source: &str,
    binding: Span,
    initializer: impl Fn(&Expression<'_>) -> bool,
) -> Option<UnwrittenObjectBinding> {
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
            if !initializer(declarator.init.as_ref()?.get_inner_expression()) {
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

    fn recognize_primitive(source: &str) -> Option<UnwrittenPrimitiveBinding> {
        let start = u32::try_from(source.find("value").unwrap()).unwrap();
        unwritten_primitive_binding(
            source,
            Span {
                start,
                end: start + 5,
            },
        )
    }

    #[test]
    fn primitive_binding_binds_exact_bytes_and_declaration() {
        // The shape of `@tanstack/router-core@1.171.22`'s
        // `dist/esm/isServer/client.js`, whose declaration says `never`.
        let source = "const value = void 0;\nexport { value };\n";
        let proof = recognize_primitive(source).unwrap();
        assert!(proof.matches(source, Span { start: 6, end: 11 }));
        assert!(!proof.matches(&format!("{source} "), Span { start: 6, end: 11 }));
        assert!(!proof.matches(source, Span { start: 5, end: 11 }));
        for source in [
            "export const value = false;",
            "export let value = 0;",
            "export var value = \"x\";",
            "export const value = null;",
            "export const value = 1n;",
            "export const value = `plain`;",
            "export const value = (void \"\");",
        ] {
            assert!(recognize_primitive(source).is_some(), "{source}");
        }
    }

    #[test]
    fn primitive_binding_refuses_everything_but_an_unwritten_primitive_literal() {
        for source in [
            "var value = void 0; value = () => {};",
            "var value = void 0; function f() { value = () => {}; }",
            "var value = void 0; ({ value } = other);",
            "var value = void 0; var value = () => {};",
            "const value = void 0; eval('value');",
            "const value = undefined;",
            "const value = void f();",
            "const value = void other;",
            "const value = `${other}`;",
            "const value = -1;",
            "const value = {};",
            "const value = () => {};",
            "function f() { const value = void 0; }",
            "declare const value: never;",
        ] {
            assert!(recognize_primitive(source).is_none(), "{source}");
        }
        // The shared walk does not widen the object recognizer.
        assert!(recognize("export const value = void 0;").is_none());
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
