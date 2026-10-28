//! Code no execution reaches, read from the syntax alone (facts schema 77,
//! ADR 0458).
//!
//! Two shapes, each decided by what is written and nothing else:
//!
//! - A statement after one that never completes normally in the same list
//!   (`return`, `throw`, `break`, `continue`, a block holding one, an `if`
//!   both of whose arms do, or whose literal test takes one that does). A
//!   function declaration is hoisted and is not dead there: it can be called
//!   before the exit. A class declaration is not hoisted, so its static
//!   blocks and initializers never run.
//! - The arm a literal test rules out: an `if` or `?:` arm, a `while`/`for`
//!   body, the right operand of `&&`, `||` or `??`. A literal is a boolean,
//!   `null`, a number, a string, a `void` expression, a function, object or
//!   array literal, and `!` or `&&`/`||` over those. An identifier (even
//!   `undefined`) decides nothing: it may be shadowed.
//!
//! A region holds every node written in it, nested functions and classes
//! included: a function the region would create is never created.

use oxc_ast::ast::{
    ConditionalExpression, Expression, ForStatement, IfStatement, LogicalExpression,
    LogicalOperator, Program, Statement, UnaryOperator, WhileStatement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::GetSpan;

use crate::core::Span;

/// The dead regions of `program`, sorted and without duplicates.
pub(super) fn unreachable_regions(program: &Program<'_>) -> Vec<Span> {
    let mut collector = Regions::default();
    collector.visit_program(program);
    let mut regions = collector.regions;
    regions.sort_unstable();
    regions.dedup();
    regions
}

#[derive(Default)]
struct Regions {
    regions: Vec<Span>,
}

impl Regions {
    fn push(&mut self, span: oxc_span::Span) {
        self.regions.push(Span::new(span.start, span.end));
    }

    /// Every statement of `statements` after the first that never completes
    /// normally, but hoisted function declarations.
    fn dead_suffix(&mut self, statements: &[Statement<'_>]) {
        let Some(exit) = statements.iter().position(|statement| exits(statement)) else {
            return;
        };
        for statement in &statements[exit + 1..] {
            if !hoisted(statement) {
                self.push(statement.span());
            }
        }
    }
}

impl<'a> Visit<'a> for Regions {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.dead_suffix(&program.body);
        walk::walk_program(self, program);
    }

    fn visit_function_body(&mut self, body: &oxc_ast::ast::FunctionBody<'a>) {
        self.dead_suffix(&body.statements);
        walk::walk_function_body(self, body);
    }

    fn visit_block_statement(&mut self, block: &oxc_ast::ast::BlockStatement<'a>) {
        self.dead_suffix(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_static_block(&mut self, block: &oxc_ast::ast::StaticBlock<'a>) {
        self.dead_suffix(&block.body);
        walk::walk_static_block(self, block);
    }

    fn visit_switch_case(&mut self, case: &oxc_ast::ast::SwitchCase<'a>) {
        self.dead_suffix(&case.consequent);
        walk::walk_switch_case(self, case);
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        match truthy(&statement.test) {
            Some(true) => {
                if let Some(alternate) = &statement.alternate {
                    self.push(alternate.span());
                }
            }
            Some(false) => self.push(statement.consequent.span()),
            None => {}
        }
        walk::walk_if_statement(self, statement);
    }

    fn visit_conditional_expression(&mut self, expression: &ConditionalExpression<'a>) {
        match truthy(&expression.test) {
            Some(true) => self.push(expression.alternate.span()),
            Some(false) => self.push(expression.consequent.span()),
            None => {}
        }
        walk::walk_conditional_expression(self, expression);
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        let right_dead = match expression.operator {
            LogicalOperator::And => truthy(&expression.left) == Some(false),
            LogicalOperator::Or => truthy(&expression.left) == Some(true),
            LogicalOperator::Coalesce => nullish(&expression.left) == Some(false),
        };
        if right_dead {
            self.push(expression.right.span());
        }
        walk::walk_logical_expression(self, expression);
    }

    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        if truthy(&statement.test) == Some(false) {
            self.push(statement.body.span());
        }
        walk::walk_while_statement(self, statement);
    }

    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        if statement
            .test
            .as_ref()
            .is_some_and(|test| truthy(test) == Some(false))
        {
            self.push(statement.body.span());
            if let Some(update) = &statement.update {
                self.push(update.span());
            }
        }
        walk::walk_for_statement(self, statement);
    }
}

/// A declaration the binder hoists with its value: a `function` declaration,
/// exported or not. Its body runs when it is called, which may be before an
/// exit that precedes it.
fn hoisted(statement: &Statement<'_>) -> bool {
    use oxc_ast::ast::{Declaration, ExportDefaultDeclarationKind};
    match statement {
        Statement::FunctionDeclaration(_) => true,
        Statement::ExportNamedDeclaration(export) => {
            matches!(
                export.declaration,
                Some(Declaration::FunctionDeclaration(_))
            )
        }
        Statement::ExportDefaultDeclaration(export) => matches!(
            export.declaration,
            ExportDefaultDeclarationKind::FunctionDeclaration(_)
        ),
        _ => false,
    }
}

/// Whether `statement` never completes normally, so the statement after it
/// in its list never runs. A labelled statement may complete through a
/// `break` to its own label, and `try`, `switch` and loops are not read.
fn exits(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_) => true,
        Statement::BlockStatement(block) => block.body.iter().any(exits),
        Statement::IfStatement(statement) => {
            let alternate = || statement.alternate.as_ref().is_some_and(exits);
            match truthy(&statement.test) {
                Some(true) => exits(&statement.consequent),
                Some(false) => alternate(),
                None => exits(&statement.consequent) && alternate(),
            }
        }
        _ => false,
    }
}

fn peel<'e, 'a>(mut expression: &'e Expression<'a>) -> &'e Expression<'a> {
    loop {
        expression = match expression {
            Expression::ParenthesizedExpression(inner) => &inner.expression,
            Expression::TSAsExpression(inner) => &inner.expression,
            Expression::TSSatisfiesExpression(inner) => &inner.expression,
            Expression::TSNonNullExpression(inner) => &inner.expression,
            Expression::TSTypeAssertion(inner) => &inner.expression,
            _ => return expression,
        };
    }
}

/// The truthiness of `expression` when the literal written decides it.
fn truthy(expression: &Expression<'_>) -> Option<bool> {
    match peel(expression) {
        Expression::BooleanLiteral(literal) => Some(literal.value),
        Expression::NullLiteral(_) => Some(false),
        Expression::NumericLiteral(literal) => {
            Some(literal.value != 0.0 && !literal.value.is_nan())
        }
        Expression::StringLiteral(literal) => Some(!literal.value.is_empty()),
        Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_) => Some(true),
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::LogicalNot => truthy(&unary.argument).map(|value| !value),
            UnaryOperator::Void => Some(false),
            _ => None,
        },
        Expression::LogicalExpression(logical) => {
            let left = truthy(&logical.left);
            match logical.operator {
                LogicalOperator::And => match left {
                    Some(false) => Some(false),
                    Some(true) => truthy(&logical.right),
                    None => None,
                },
                LogicalOperator::Or => match left {
                    Some(true) => Some(true),
                    Some(false) => truthy(&logical.right),
                    None => None,
                },
                LogicalOperator::Coalesce => match nullish(&logical.left) {
                    Some(true) => truthy(&logical.right),
                    Some(false) => left,
                    None => None,
                },
            }
        }
        _ => None,
    }
}

/// Whether `expression` is `null` or `undefined`, when the literal written
/// decides it.
fn nullish(expression: &Expression<'_>) -> Option<bool> {
    match peel(expression) {
        Expression::NullLiteral(_) => Some(true),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void => Some(true),
        Expression::BooleanLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_) => Some(false),
        _ => None,
    }
}
