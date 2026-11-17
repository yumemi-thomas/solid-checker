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

use std::collections::{HashMap, HashSet};
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
    calls_on_every_completion(
        path,
        source,
        function,
        folds,
        false,
        false,
        Mode::ExactlyOnce,
    )
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
    calls_on_every_completion(
        path,
        source,
        function,
        folds,
        true,
        false,
        Mode::AtLeastOnce,
    )
}

/// ADR 0550: the calls the first `next()` of the generator at `function`
/// runs on every completion its caller can continue from: those no `yield`
/// (and, in an async generator, no `await`, `for await` or `await using`) of
/// its own precedes or contains, with an earlier `throw` a way out as for
/// [`synchronously_unconditional_calls`]. `None` when the source does not
/// parse, no function node has that span, or the function is no generator.
#[must_use]
pub fn first_step_unconditional_calls(
    path: &Path,
    source: &str,
    function: Span,
    folds: &[HostConstantFold],
) -> Option<Vec<Span>> {
    calls_on_every_completion(path, source, function, folds, true, true, Mode::AtLeastOnce)
}

/// ADR 0714: in one parse, every call and `new` expression that runs exactly
/// once on every normal completion of its innermost function, or of the
/// module's evaluation when no function contains it, with an earlier `throw`
/// a way out. An `await` is an ordinary expression here: the continuation of
/// a completing call runs (the premise the owner pass's after-await arm takes
/// too). A generator's set is its first step's ([`first_step_unconditional_calls`]).
/// At module scope a top-level statement admits its calls as a function body
/// does, and so does a static field initializer of a class declared there.
/// `None` when the source does not parse.
#[must_use]
pub fn every_unconditional_call(path: &Path, source: &str) -> Option<HashSet<Span>> {
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
    let scope = HostConstantScope::new(&parsed.program, &built.semantic, &[]);
    let scope = scope.as_ref();
    // Each generator's first `yield`: a later step runs only if it is asked for.
    let mut suspension = HashMap::<Option<NodeId>, u32>::new();
    for node in nodes.iter() {
        if matches!(node.kind(), AstKind::YieldExpression(_)) {
            let owner = innermost_function(nodes, node.id());
            let start = node.kind().span().start;
            suspension
                .entry(owner)
                .and_modify(|first| *first = (*first).min(start))
                .or_insert(start);
        }
    }
    let calls = nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind(),
                AstKind::CallExpression(_) | AstKind::NewExpression(_)
            )
        })
        .filter(|node| {
            let owner = innermost_function(nodes, node.id());
            let span = node.kind().span();
            suspension
                .get(&owner)
                .is_none_or(|suspension| span.end <= *suspension)
                && match owner {
                    Some(owner) => runs_on_every_completion(
                        nodes,
                        scope,
                        node.id(),
                        owner,
                        true,
                        Mode::AtLeastOnce,
                    ),
                    None => runs_on_every_module_evaluation(nodes, scope, node.id()),
                }
        })
        .map(|node| {
            let span = node.kind().span();
            Span::new(span.start, span.end)
        })
        .collect();
    Some(calls)
}

/// A call no function contains, admitted up to its top-level statement.
fn runs_on_every_module_evaluation(
    nodes: &AstNodes<'_>,
    scope: Option<&HostConstantScope<'_, '_>>,
    call: NodeId,
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
        match nodes.kind(parent) {
            AstKind::Program(program) => {
                return statements_before_exit_free(
                    scope,
                    &program.body,
                    nodes.kind(child).span(),
                    true,
                );
            }
            AstKind::ExportNamedDeclaration(_)
            | AstKind::ExportDefaultDeclaration(_)
            | AstKind::ClassBody(_)
            | AstKind::Class(_) => {}
            AstKind::PropertyDefinition(property)
                if property.r#static
                    && property
                        .value
                        .as_ref()
                        .is_some_and(|value| value.span() == nodes.kind(child).span()) => {}
            _ if !evaluates_child_once(nodes, scope, parent, child, true, Mode::AtLeastOnce) => {
                return false;
            }
            _ => {}
        }
        child = parent;
    }
}

fn calls_on_every_completion(
    path: &Path,
    source: &str,
    function: Span,
    folds: &[HostConstantFold],
    synchronous_prefix: bool,
    generator_first_step: bool,
    mode: Mode,
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
    if generator_first_step && !generator {
        return None;
    }
    if (generator && !generator_first_step) || (asynchronous && !synchronous_prefix) {
        return Some(Vec::new());
    }
    // Where this function's own first suspension point starts: an `await`,
    // a `for await` loop or an `await using` declaration not nested in
    // another function.
    let first_suspension = nodes
        .iter()
        .filter(|_| asynchronous || generator_first_step)
        .filter(|node| match node.kind() {
            AstKind::YieldExpression(_) => generator_first_step,
            AstKind::AwaitExpression(_) => asynchronous,
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
            runs_on_every_completion(
                nodes,
                scope,
                node.id(),
                owner.id(),
                synchronous_prefix,
                mode,
            )
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
    mode: Mode,
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
        if !evaluates_child_once(nodes, scope, parent, child, throws_exit, mode) {
            return false;
        }
        child = parent;
    }
}

/// What a walk proves of a call it admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    /// It runs exactly once on every completion: [`unconditional_calls`], the producer's whitelist (ADR 0161).
    ExactlyOnce,
    /// ADR 1042: it runs at least once on every completion. Admits, besides, a label's body, a `try`'s finalizer and the
    /// block of a `try` with no `catch`, a `do` body, the body of a `while` or `for` whose first test is decided true (or
    /// a `for` with no test), and the parts a loop evaluates once before its body (a `for`'s initializer and first test, a
    /// `while`'s first test, a `for … in`/`of` object). Every reader of this mode asks whether the call runs, never how
    /// often: the owner graph's edges (ADR 0714), a leaf scope's reach (`cleanup.rs`), a rerun's read
    /// (`write_roles.rs`, `read_context.rs`).
    AtLeastOnce,
}

/// Whether evaluating `parent` evaluates `child` exactly once (or, under [`Mode::AtLeastOnce`], at least once), whatever
/// the values involved.
fn evaluates_child_once(
    nodes: &AstNodes<'_>,
    scope: Option<&HostConstantScope<'_, '_>>,
    parent: NodeId,
    child: NodeId,
    throws_exit: bool,
    mode: Mode,
) -> bool {
    let child_span = nodes.kind(child).span();
    let is = |span: oxc_span::Span| span == child_span;
    let decided = |test: &Expression<'_>| scope.and_then(|scope| scope.truthiness(test));
    if mode == Mode::AtLeastOnce
        && let Some(admitted) =
            evaluates_child_at_least_once(nodes.kind(parent), child_span, &decided)
    {
        return admitted;
    }
    // ADR 1046: a statement of an entered case's consequent, no earlier one of which leaves (a `break` included).
    if mode == Mode::AtLeastOnce
        && let AstKind::SwitchCase(case) = nodes.kind(parent)
    {
        return statements_before_exit_free(scope, &case.consequent, child_span, throws_exit);
    }
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
        | AstKind::TSTypeAssertion(_)
        // Only [`every_unconditional_call`] reaches one: the other walks stop
        // at an `async` function's first suspension.
        | AstKind::AwaitExpression(_) => true,
        _ => false,
    }
}

/// ADR 1042: the parents [`Mode::AtLeastOnce`] reads beyond the exactly-once whitelist: `Some` for a loop, label or
/// `try`, `None` for every other kind (the whitelist answers). An earlier exit inside the admitted statement (a `break`,
/// `continue` or `return` before the call in a loop body, a label's block, a finalizer) is the enclosing block's to
/// refuse, as for any block.
fn evaluates_child_at_least_once(
    parent: AstKind<'_>,
    child: oxc_span::Span,
    decided: &dyn Fn(&Expression<'_>) -> Option<(bool, bool)>,
) -> Option<bool> {
    let is = |span: oxc_span::Span| span == child;
    // A first test a literal or a fold decides true: the body is entered.
    let entered = |test: Option<&Expression<'_>>| {
        test.is_none_or(|test| matches!(decided(test), Some((true, _))))
    };
    Some(match parent {
        // A labelled statement evaluates its body once.
        AstKind::LabeledStatement(statement) => is(statement.body.span()),
        // The finalizer runs whenever the `try` is entered; the block runs as a plain block when nothing catches. The
        // handler runs only on a throw, and a block a `catch` guards may complete without its later statements.
        AstKind::TryStatement(statement) => {
            statement
                .finalizer
                .as_ref()
                .is_some_and(|finalizer| is(finalizer.span()))
                || (statement.handler.is_none() && is(statement.block.span()))
        }
        // The body's first pass runs before any test.
        AstKind::DoWhileStatement(statement) => is(statement.body.span()),
        AstKind::WhileStatement(statement) => {
            is(statement.test.span())
                || (is(statement.body.span()) && entered(Some(&statement.test)))
        }
        AstKind::ForStatement(statement) => {
            statement.init.as_ref().is_some_and(|init| is(init.span()))
                || statement.test.as_ref().is_some_and(|test| is(test.span()))
                || (is(statement.body.span())
                    && (entered(statement.test.as_ref()) || counter_entered(statement)))
        }
        // ADR 1046: the discriminant is evaluated once; a case is entered when literals decide it the first equal one.
        AstKind::SwitchStatement(statement) => {
            is(statement.discriminant.span())
                || statement
                    .cases
                    .iter()
                    .position(|case| is(case.span))
                    .is_some_and(|index| literal_switch_enters(statement, index))
        }
        AstKind::ForInStatement(statement) => is(statement.right.span()),
        AstKind::ForOfStatement(statement) => is(statement.right.span()),
        _ => return None,
    })
}

/// ADR 1046: a literal a switch or a counter loop compares: a number, a string or a boolean, through parentheses and
/// TypeScript's wrappers (`1 as number`), a negated number included.
#[derive(Clone, Debug, PartialEq)]
enum Literal {
    Number(f64),
    String(String),
    Boolean(bool),
}

fn literal(expression: &Expression<'_>) -> Option<Literal> {
    match expression.get_inner_expression() {
        Expression::NumericLiteral(number) => Some(Literal::Number(number.value)),
        Expression::StringLiteral(string) => Some(Literal::String(string.value.to_string())),
        Expression::BooleanLiteral(boolean) => Some(Literal::Boolean(boolean.value)),
        Expression::UnaryExpression(unary)
            if unary.operator == oxc_syntax::operator::UnaryOperator::UnaryNegation =>
        {
            match literal(&unary.argument)? {
                Literal::Number(value) => Some(Literal::Number(-value)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// ADR 1046: whether the case at `index` is the one a `switch` enters: its discriminant and every case test up to it are
/// literals of one primitive type, none before it strictly equal to the discriminant, and its own test strictly equal. A
/// `default` and a case reached by fall-through are not read.
fn literal_switch_enters(statement: &oxc_ast::ast::SwitchStatement<'_>, index: usize) -> bool {
    let Some(discriminant) = literal(&statement.discriminant) else {
        return false;
    };
    let same_kind =
        |other: &Literal| std::mem::discriminant(other) == std::mem::discriminant(&discriminant);
    for (at, case) in statement.cases.iter().enumerate().take(index + 1) {
        let Some(test) = case.test.as_ref().and_then(literal) else {
            return false;
        };
        if !same_kind(&test) {
            return false;
        }
        let equal = test == discriminant;
        if at == index {
            return equal;
        }
        if equal {
            return false;
        }
    }
    false
}

/// ADR 1046: whether a counter loop's first test passes: the initializer declares one binding (`let`, `const` or `var`)
/// bound to a numeric literal, and the test compares that name (the loop's own binding: nothing runs between the
/// initializer and the first test) with a numeric literal by `<`, `<=`, `>`, `>=`, `!=` or `!==`, true for those values.
fn counter_entered(statement: &oxc_ast::ast::ForStatement<'_>) -> bool {
    use oxc_ast::ast::{BindingPattern, ForStatementInit};
    use oxc_syntax::operator::BinaryOperator;
    let Some(ForStatementInit::VariableDeclaration(declaration)) = &statement.init else {
        return false;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return false;
    };
    let BindingPattern::BindingIdentifier(name) = &declarator.id else {
        return false;
    };
    let Some(Literal::Number(start)) = declarator.init.as_ref().and_then(literal) else {
        return false;
    };
    let Some(Expression::BinaryExpression(test)) = statement
        .test
        .as_ref()
        .map(Expression::get_inner_expression)
    else {
        return false;
    };
    let Expression::Identifier(left) = test.left.get_inner_expression() else {
        return false;
    };
    let Some(Literal::Number(bound)) = literal(&test.right) else {
        return false;
    };
    if left.name != name.name || start.is_nan() || bound.is_nan() {
        return false;
    }
    match test.operator {
        BinaryOperator::LessThan => start < bound,
        BinaryOperator::LessEqualThan => start <= bound,
        BinaryOperator::GreaterThan => start > bound,
        BinaryOperator::GreaterEqualThan => start >= bound,
        BinaryOperator::Inequality | BinaryOperator::StrictInequality => start != bound,
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
