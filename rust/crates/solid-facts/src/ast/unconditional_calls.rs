//! The calls a function runs exactly once on every normal completion
//! (ADR 0161): the generator's syntax half of the producer's
//! `ImplementationCall::unconditional` (ADR 0152), used only to decide what to
//! propose. The certifier reads the producer's fact and never this one.
//!
//! The walk goes up from a call to the function's body and admits each step
//! only when it is a position its parent evaluates exactly once,
//! unconditionally, every time the parent itself is evaluated -- the same
//! whitelist the producer applies. Any other kind of parent makes the call
//! conditional, which claims nothing; so does an earlier statement in an
//! enclosing block that can leave the function by `return`, `break` or
//! `continue`. A `throw` before the call is admitted: a completion that throws
//! is not a normal one. An `async` function or a generator states nothing.

use std::path::Path;

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{AssignmentOperator, Expression, Statement};

use super::host_constants::{HostConstantFold, HostConstantScope};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};

/// The spans of the call and `new` expressions written in the function whose
/// node is exactly `function` that run exactly once on every normal
/// completion of it. `None` when the source does not parse or no function
/// node has that span.
#[must_use]
pub fn unconditional_calls(
    path: &Path,
    source: &str,
    function: Span,
    folds: &[HostConstantFold],
) -> Option<Vec<Span>> {
    calls_on_every_completion(path, source, function, folds, false)
}

/// The calls a call of the function at `function` runs before it returns, on
/// every completion its caller can continue from (ADR 0275). These are
/// [`unconditional_calls`], except that an earlier `throw` is a way out
/// (ADR 0276): a caller may catch it. An `async` function returns at its first `await`,
/// so only the calls on every completion that no `await`, `for await` or
/// `await using` of its own precedes or contains run before it returns. A
/// generator states nothing.
#[must_use]
pub fn synchronously_unconditional_calls(
    path: &Path,
    source: &str,
    function: Span,
    folds: &[HostConstantFold],
) -> Option<Vec<Span>> {
    calls_on_every_completion(path, source, function, folds, true)
}

fn calls_on_every_completion(
    path: &Path,
    source: &str,
    function: Span,
    folds: &[HostConstantFold],
    synchronous_prefix: bool,
) -> Option<Vec<Span>> {
    if source.len() > 4 * 1024 * 1024 {
        return None;
    }
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let nodes = built.semantic.nodes();
    // ADR 0166: a condition a fold (or a literal) decides leaves one arm of an
    // `if` or `?:` dead, exactly as the producer's walk reads it.
    let scope = HostConstantScope::new(&parsed.program, &built.semantic, folds);
    let scope = scope.as_ref();
    let owner = nodes.iter().find(|node| {
        let span = node.kind().span();
        span.start == function.start
            && span.end == function.end
            && matches!(
                node.kind(),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
    })?;
    let (asynchronous, generator) = match owner.kind() {
        AstKind::Function(function) => (function.r#async, function.generator),
        AstKind::ArrowFunctionExpression(arrow) => (arrow.r#async, false),
        _ => return Some(Vec::new()),
    };
    if generator || (asynchronous && !synchronous_prefix) {
        return Some(Vec::new());
    }
    // Where this function's own first suspension point starts: an `await`,
    // a `for await` loop or an `await using` declaration not nested in
    // another function.
    let first_suspension = nodes
        .iter()
        .filter(|_| asynchronous)
        .filter(|node| match node.kind() {
            AstKind::AwaitExpression(_) => true,
            AstKind::ForOfStatement(statement) => statement.r#await,
            AstKind::VariableDeclaration(declaration) => {
                declaration.kind == oxc_ast::ast::VariableDeclarationKind::AwaitUsing
            }
            _ => false,
        })
        .filter(|node| innermost_function(nodes, node.id()) == Some(owner.id()))
        .map(|node| node.kind().span().start)
        .min();
    let calls = nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind(),
                AstKind::CallExpression(_) | AstKind::NewExpression(_)
            )
        })
        .filter(|node| {
            let span = node.kind().span();
            span.start >= function.start
                && span.end <= function.end
                && first_suspension.is_none_or(|suspension| span.end <= suspension)
        })
        .filter(|node| {
            runs_on_every_completion(nodes, scope, node.id(), owner.id(), synchronous_prefix)
        })
        .map(|node| {
            let span = node.kind().span();
            Span::new(span.start, span.end)
        })
        .collect();
    Some(calls)
}

/// The nearest function node strictly enclosing `node`.
fn innermost_function(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    let mut child = node;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return None;
        }
        if matches!(
            nodes.kind(parent),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            return Some(parent);
        }
        child = parent;
    }
}

/// `throws_exit`: a `throw` before the call counts as a way out (ADR 0276),
/// because a caller may catch it and carry on.
fn runs_on_every_completion(
    nodes: &AstNodes<'_>,
    scope: Option<&HostConstantScope<'_, '_>>,
    call: NodeId,
    owner: NodeId,
    throws_exit: bool,
) -> bool {
    if let AstKind::CallExpression(expression) = nodes.kind(call)
        && expression.optional
    {
        return false;
    }
    let mut child = call;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return false;
        }
        if parent == owner {
            return matches!(nodes.kind(child), AstKind::FunctionBody(_))
                || matches!(nodes.kind(owner), AstKind::ArrowFunctionExpression(arrow) if arrow.expression);
        }
        if !evaluates_child_once(nodes, scope, parent, child, throws_exit) {
            return false;
        }
        child = parent;
    }
}

/// Whether evaluating `parent` evaluates `child` exactly once, whatever the
/// values involved.
fn evaluates_child_once(
    nodes: &AstNodes<'_>,
    scope: Option<&HostConstantScope<'_, '_>>,
    parent: NodeId,
    child: NodeId,
    throws_exit: bool,
) -> bool {
    let child_span = nodes.kind(child).span();
    let is = |span: oxc_span::Span| span == child_span;
    let decided = |test: &Expression<'_>| scope.and_then(|scope| scope.truthiness(test));
    match nodes.kind(parent) {
        AstKind::FunctionBody(body) => {
            statements_before_exit_free(scope, &body.statements, child_span, throws_exit)
        }
        AstKind::BlockStatement(block) => {
            statements_before_exit_free(scope, &block.body, child_span, throws_exit)
        }
        AstKind::IfStatement(statement) => {
            is(statement.test.span())
                || match decided(&statement.test) {
                    Some((true, _)) => is(statement.consequent.span()),
                    Some((false, _)) => statement
                        .alternate
                        .as_ref()
                        .is_some_and(|alternate| is(alternate.span())),
                    None => false,
                }
        }
        AstKind::ConditionalExpression(expression) => {
            is(expression.test.span())
                || match decided(&expression.test) {
                    Some((true, _)) => is(expression.consequent.span()),
                    Some((false, _)) => is(expression.alternate.span()),
                    None => false,
                }
        }
        AstKind::LogicalExpression(expression) => is(expression.left.span()),
        AstKind::AssignmentExpression(expression) => {
            !matches!(
                expression.operator,
                AssignmentOperator::LogicalAnd
                    | AssignmentOperator::LogicalOr
                    | AssignmentOperator::LogicalNullish
            ) || !is(expression.right.span())
        }
        AstKind::CallExpression(expression) => !expression.optional,
        AstKind::NewExpression(_) => true,
        AstKind::StaticMemberExpression(expression) => !expression.optional,
        AstKind::ComputedMemberExpression(expression) => !expression.optional,
        AstKind::VariableDeclarator(declarator) => {
            declarator.init.as_ref().is_some_and(|init| is(init.span()))
        }
        AstKind::ExpressionStatement(_)
        | AstKind::ReturnStatement(_)
        | AstKind::ThrowStatement(_)
        | AstKind::VariableDeclaration(_)
        | AstKind::ParenthesizedExpression(_)
        | AstKind::SequenceExpression(_)
        | AstKind::BinaryExpression(_)
        | AstKind::UnaryExpression(_)
        | AstKind::UpdateExpression(_)
        | AstKind::TemplateLiteral(_)
        | AstKind::ArrayExpression(_)
        | AstKind::ObjectExpression(_)
        | AstKind::ObjectProperty(_)
        | AstKind::SpreadElement(_)
        | AstKind::TSAsExpression(_)
        | AstKind::TSSatisfiesExpression(_)
        | AstKind::TSNonNullExpression(_)
        | AstKind::TSTypeAssertion(_) => true,
        _ => false,
    }
}

/// Whether the statement at `child` in `statements` exists and no earlier one
/// can leave the enclosing function early.
fn statements_before_exit_free(
    scope: Option<&HostConstantScope<'_, '_>>,
    statements: &[Statement<'_>],
    child: oxc_span::Span,
    throws_exit: bool,
) -> bool {
    for statement in statements {
        if statement.span() == child {
            return true;
        }
        let mut exits = EarlyExit {
            scope,
            throws_exit,
            found: false,
        };
        exits.visit_statement(statement);
        if exits.found {
            return false;
        }
    }
    false
}

/// Finds a `return`, `break` or `continue` outside every function nested in
/// the statement visited.
/// An `if` whose condition is decided is read through its condition and its
/// live arm only: a `return` in the dead arm is no path out (ADR 0166).
struct EarlyExit<'f, 's, 'a> {
    scope: Option<&'f HostConstantScope<'s, 'a>>,
    throws_exit: bool,
    found: bool,
}

impl<'v> Visit<'v> for EarlyExit<'_, '_, '_> {
    fn visit_if_statement(&mut self, statement: &oxc_ast::ast::IfStatement<'v>) {
        match self
            .scope
            .and_then(|scope| scope.truthiness(&statement.test))
        {
            Some((true, _)) => {
                self.visit_expression(&statement.test);
                self.visit_statement(&statement.consequent);
            }
            Some((false, _)) => {
                self.visit_expression(&statement.test);
                if let Some(alternate) = &statement.alternate {
                    self.visit_statement(alternate);
                }
            }
            None => oxc_ast_visit::walk::walk_if_statement(self, statement),
        }
    }
    fn visit_return_statement(&mut self, _: &oxc_ast::ast::ReturnStatement<'v>) {
        self.found = true;
    }
    fn visit_break_statement(&mut self, _: &oxc_ast::ast::BreakStatement<'v>) {
        self.found = true;
    }
    fn visit_continue_statement(&mut self, _: &oxc_ast::ast::ContinueStatement<'v>) {
        self.found = true;
    }
    fn visit_throw_statement(&mut self, statement: &oxc_ast::ast::ThrowStatement<'v>) {
        self.found |= self.throws_exit;
        oxc_ast_visit::walk::walk_throw_statement(self, statement);
    }
    fn visit_function(&mut self, _: &oxc_ast::ast::Function<'v>, _: oxc_syntax::scope::ScopeFlags) {
    }
    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'v>) {}
    fn visit_class(&mut self, _: &oxc_ast::ast::Class<'v>) {}
}

#[cfg(test)]
mod tests;
