//! Closed, non-evaluating classic-script grammar for HTML entry independence.
//! A text blacklist is insufficient: aliases/computed calls can navigate, stop
//! parsing or remove module scripts. Unknown executable shapes remain refused.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingPattern, Expression, Statement, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::BTreeSet;

fn member(value: &Expression<'_>, chain: &[&str]) -> bool {
    if let [name] = chain {
        return matches!(value.get_inner_expression(), Expression::Identifier(id) if id.name == *name);
    }
    let Some((last, prefix)) = chain.split_last() else {
        return false;
    };
    matches!(value.get_inner_expression(), Expression::StaticMemberExpression(value)
        if !value.optional && value.property.name == *last && member(&value.object, prefix))
}

fn data(value: &Expression<'_>, locals: &BTreeSet<String>) -> bool {
    match value.get_inner_expression() {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::Identifier(id) => locals.contains(id.name.as_str()),
        Expression::BinaryExpression(value) => {
            // Equality on these scalar inputs cannot invoke a user hook.
            matches!(value.operator.as_str(), "===" | "!==" | "==" | "!=")
                && data(&value.left, locals)
                && data(&value.right, locals)
        }
        Expression::LogicalExpression(value) => {
            data(&value.left, locals) && data(&value.right, locals)
        }
        Expression::ConditionalExpression(value) => {
            data(&value.test, locals)
                && data(&value.consequent, locals)
                && data(&value.alternate, locals)
        }
        Expression::CallExpression(value) => {
            !value.optional
                && member(&value.callee, &["localStorage", "getItem"])
                && matches!(value.arguments.as_slice(), [Argument::StringLiteral(_)])
        }
        _ => false,
    }
}

fn statement(value: &Statement<'_>, locals: &mut BTreeSet<String>) -> bool {
    match value {
        Statement::EmptyStatement(_) => true,
        Statement::BlockStatement(block) => {
            let mut nested = locals.clone();
            block
                .body
                .iter()
                .all(|statement| self::statement(statement, &mut nested))
        }
        Statement::VariableDeclaration(declaration) => {
            declaration.kind == VariableDeclarationKind::Const
                && declaration.declarations.iter().all(|value| {
                    let BindingPattern::BindingIdentifier(id) = &value.id else {
                        return false;
                    };
                    // Fresh scalar locals cannot shadow ambient object identity.
                    !["document", "window", "location", "localStorage"].contains(&id.name.as_str())
                        && value.init.as_ref().is_some_and(|value| data(value, locals))
                        && locals.insert(id.name.to_string())
                })
        }
        Statement::IfStatement(branch) => {
            data(&branch.test, locals)
                && statement(&branch.consequent, &mut locals.clone())
                && branch
                    .alternate
                    .as_ref()
                    .is_none_or(|value| statement(value, &mut locals.clone()))
        }
        Statement::ThrowStatement(value) => data(&value.argument, locals),
        Statement::ExpressionStatement(value) => {
            if data(&value.expression, locals) {
                return true;
            }
            let Expression::AssignmentExpression(value) = value.expression.get_inner_expression()
            else {
                return false;
            };
            let AssignmentTarget::StaticMemberExpression(target) = &value.left else {
                return false;
            };
            value.operator.as_str() == "="
                && !target.optional
                && target.property.name == "theme"
                && member(&target.object, &["document", "documentElement", "dataset"])
                && data(&value.right, locals)
        }
        _ => false,
    }
}

pub(super) fn independent(source: &str) -> bool {
    if source.len() > 8192 {
        return false;
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default().with_module(false)).parse();
    let mut locals = BTreeSet::new();
    !parsed.panicked
        && parsed.errors.is_empty()
        && parsed
            .program
            .directives
            .iter()
            .all(|directive| directive.directive == "use strict")
        && parsed
            .program
            .body
            .iter()
            .all(|value| statement(value, &mut locals))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_classic_script_and_interference_twins() {
        // Keep locals across statements, including Jar Hell's actual bootstrap.
        assert!(independent("throw 'error';"));
        assert!(independent("const x = 'light';"));
        assert!(independent(
            "const theme = localStorage.getItem('theme'); if (theme === 'light' || theme === 'dark') document.documentElement.dataset.theme = theme;"
        ));
        for source in [
            "document.write('new page')",
            "document.open()",
            "location = '/elsewhere'",
            "location.href = '/elsewhere'",
            "window.location = '/elsewhere'",
            "window.stop()",
            "document.querySelector('script').remove()",
            "document.body.innerHTML = ''",
            "const d = document; d['write']('x')",
            "const stop = window.stop; stop()",
            "eval('window.stop()')",
            "while(true){}",
            "unknown()",
        ] {
            assert!(!independent(source), "{source}");
        }
    }
}
