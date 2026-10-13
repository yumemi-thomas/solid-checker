//! Argument-sensitive completion (ADR 0280): whether a site written directly
//! in a function runs on a call of it, given what that call's arguments
//! prove about the function's parameters.
//!
//! [`super::AstFacts::completion_sites`] answers the question for every call
//! at once, so any branch makes a site merely possible. A call knows more:
//! `createTimer(fn, 30_000)` passes a number, so `typeof delay === "number"`
//! is decided, the `onCleanup` arm always runs and the `createEffect` after
//! its `return` never does. This walk folds exactly such tests and nothing
//! else. A test it cannot decide leaves the site possible, never decided.
//!
//! A parameter takes part only when it is a single identifier the function
//! never writes. A function that names `arguments`, a module with a direct
//! `eval`, and a generator state nothing. In an `async` function only the
//! prefix before its first own suspension is decided.

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    BinaryOperator, BindingPattern, Expression, LogicalOperator, Statement, UnaryOperator,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType};

use crate::core::Span;

/// What a call's argument proves about one parameter's value. Every field is
/// optional: `None` decides nothing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParameterValue {
    pub index: usize,
    /// The exact `typeof` of the value.
    pub type_of: Option<&'static str>,
    pub truthy: Option<bool>,
    pub nullish: Option<bool>,
    /// The value is `undefined` (as opposed to `null`), when nullish.
    pub undefined: Option<bool>,
}

impl ParameterValue {
    /// A value the call omits, for a parameter with no default.
    #[must_use]
    pub fn omitted(index: usize) -> Self {
        Self {
            index,
            type_of: Some("undefined"),
            truthy: Some(false),
            nullish: Some(true),
            undefined: Some(true),
        }
    }
}

/// How a site stands on one call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiteStanding {
    /// Runs on every completion the caller can continue from.
    Guaranteed,
    /// May or may not run.
    Possible,
    /// Cannot run on this call.
    Unreachable,
}

/// The standing of each of `sites` (call, `new` or member expressions written
/// directly in the function at `function`) on a call whose arguments prove
/// `parameters`. `exits` are calls the caller knows may leave by throwing
/// (`never` or `asserts` signatures); one before a site leaves it possible.
/// `None` when the source does not parse, the function is not found or is a
/// generator, or the walk refuses (see the module comment). A site that is no
/// node of the function is [`SiteStanding::Possible`].
#[must_use]
pub fn argument_site_standings(
    path: &Path,
    source: &str,
    function: Span,
    parameters: &[ParameterValue],
    exits: &[Span],
    sites: &[Span],
) -> Option<Vec<SiteStanding>> {
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
    let semantic = &built.semantic;
    let nodes = semantic.nodes();
    let scoping = semantic.scoping();
    let owner = nodes.iter().find(|node| {
        let span = node.kind().span();
        span.start == function.start
            && span.end == function.end
            && matches!(
                node.kind(),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
    })?;
    let (params, asynchronous) = match owner.kind() {
        AstKind::Function(function) if !function.generator => (&function.params, function.r#async),
        AstKind::ArrowFunctionExpression(arrow) => (&arrow.params, arrow.r#async),
        _ => return None,
    };
    // `arguments` aliases every parameter; a direct `eval` may write one.
    let mut refusal = Refusal::default();
    refusal.visit_program(&parsed.program);
    if refusal.eval
        || refusal
            .arguments
            .iter()
            .any(|span| function.start <= span.start && span.end <= function.end)
    {
        return None;
    }
    let mut values = Vec::<(SymbolId, &ParameterValue)>::new();
    for value in parameters {
        let Some(parameter) = params.items.get(value.index) else {
            continue;
        };
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            continue;
        };
        let Some(symbol) = identifier.symbol_id.get() else {
            continue;
        };
        // Written, redeclared (`var p = …`) or defaulted parameters may hold
        // another value when tested; a default replaces an `undefined`.
        if scoping
            .get_resolved_references(symbol)
            .any(|reference| reference.is_write())
            || !scoping.symbol_redeclarations(symbol).is_empty()
            || (parameter.initializer.is_some() && value.undefined != Some(false))
        {
            continue;
        }
        values.push((symbol, value));
    }
    let scope = Scope {
        scoping,
        values,
        exits,
    };
    let suspensions = if asynchronous {
        nodes
            .iter()
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
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let exit_nodes = exits
        .iter()
        .filter_map(|exit| {
            nodes
                .iter()
                .find(|node| {
                    let span = node.kind().span();
                    matches!(node.kind(), AstKind::CallExpression(_))
                        && span.start == exit.start
                        && span.end == exit.end
                })
                .filter(|node| innermost_function(nodes, node.id()) == Some(owner.id()))
                .map(|node| (*exit, node.id()))
        })
        .collect::<Vec<_>>();
    Some(
        sites
            .iter()
            .map(|site| {
                let Some(node) = nodes.iter().find(|node| {
                    let span = node.kind().span();
                    span.start == site.start
                        && span.end == site.end
                        && matches!(
                            node.kind(),
                            AstKind::CallExpression(_)
                                | AstKind::NewExpression(_)
                                | AstKind::StaticMemberExpression(_)
                                | AstKind::ComputedMemberExpression(_)
                        )
                }) else {
                    return SiteStanding::Possible;
                };
                let site_standing = standing(nodes, &scope, node.id(), owner.id());
                // A `never`/`asserts` call that may run before the site, in
                // the same statement or the parameter list included, may
                // throw first (review round 4). Only one the arguments rule
                // out does not count.
                let exit_first = || {
                    exit_nodes.iter().any(|(span, exit)| {
                        span.end <= site.start
                            && standing(nodes, &scope, *exit, owner.id())
                                != SiteStanding::Unreachable
                    })
                };
                if site_standing == SiteStanding::Guaranteed
                    && (suspensions.iter().any(|start| *start < site.end) || exit_first())
                {
                    SiteStanding::Possible
                } else {
                    site_standing
                }
            })
            .collect(),
    )
}

#[derive(Default)]
struct Refusal {
    eval: bool,
    arguments: Vec<oxc_span::Span>,
}

impl<'a> Visit<'a> for Refusal {
    fn visit_identifier_reference(&mut self, identifier: &oxc_ast::ast::IdentifierReference<'a>) {
        match identifier.name.as_str() {
            "arguments" => self.arguments.push(identifier.span),
            "eval" => self.eval = true,
            _ => {}
        }
    }
}

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

struct Scope<'s, 'v> {
    scoping: &'s Scoping,
    values: Vec<(SymbolId, &'v ParameterValue)>,
    exits: &'v [Span],
}

impl Scope<'_, '_> {
    fn parameter(&self, expression: &Expression<'_>) -> Option<&ParameterValue> {
        let Expression::Identifier(identifier) = peel(expression) else {
            return None;
        };
        let symbol = self
            .scoping
            .get_reference(identifier.reference_id.get()?)
            .symbol_id()?;
        self.values
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
            .map(|(_, value)| *value)
    }

    /// The truthiness of `expression` on this call, when decided.
    fn truthy(&self, expression: &Expression<'_>) -> Option<bool> {
        let expression = peel(expression);
        if let Some(value) = self.parameter(expression) {
            return value.truthy;
        }
        match expression {
            Expression::BooleanLiteral(literal) => Some(literal.value),
            Expression::NullLiteral(_) => Some(false),
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.truthy(&unary.argument).map(|value| !value)
            }
            Expression::LogicalExpression(logical) => {
                let left = self.truthy(&logical.left);
                match logical.operator {
                    LogicalOperator::And => match left {
                        Some(false) => Some(false),
                        Some(true) => self.truthy(&logical.right),
                        None => (self.truthy(&logical.right) == Some(false)).then_some(false),
                    },
                    LogicalOperator::Or => match left {
                        Some(true) => Some(true),
                        Some(false) => self.truthy(&logical.right),
                        None => (self.truthy(&logical.right) == Some(true)).then_some(true),
                    },
                    LogicalOperator::Coalesce => None,
                }
            }
            Expression::BinaryExpression(binary) => self.compare(binary),
            _ => None,
        }
    }

    /// Whether `expression` is nullish on this call, when decided.
    fn nullish(&self, expression: &Expression<'_>) -> Option<bool> {
        let expression = peel(expression);
        if let Some(value) = self.parameter(expression) {
            return value.nullish;
        }
        match expression {
            Expression::NullLiteral(_) => Some(true),
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

    /// `typeof p === "kind"`, `p === null`, `p == null`, `p === undefined`
    /// and their negations, either way round.
    fn compare(&self, binary: &oxc_ast::ast::BinaryExpression<'_>) -> Option<bool> {
        let (negated, loose) = match binary.operator {
            BinaryOperator::StrictEquality => (false, false),
            BinaryOperator::StrictInequality => (true, false),
            BinaryOperator::Equality => (false, true),
            BinaryOperator::Inequality => (true, true),
            _ => return None,
        };
        let decided = self
            .compare_sides(&binary.left, &binary.right, loose)
            .or_else(|| self.compare_sides(&binary.right, &binary.left, loose))?;
        Some(decided != negated)
    }

    fn compare_sides(
        &self,
        subject: &Expression<'_>,
        other: &Expression<'_>,
        loose: bool,
    ) -> Option<bool> {
        let other = peel(other);
        if let Expression::UnaryExpression(unary) = peel(subject)
            && unary.operator == UnaryOperator::Typeof
        {
            let Expression::StringLiteral(kind) = other else {
                return None;
            };
            let value = self.parameter(&unary.argument)?;
            return value.type_of.map(|type_of| type_of == kind.value.as_str());
        }
        let value = self.parameter(subject)?;
        let is_undefined = matches!(other, Expression::Identifier(identifier)
            if identifier.name == "undefined"
                && identifier
                    .reference_id
                    .get()
                    .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                    .is_none());
        match (other, loose) {
            (Expression::NullLiteral(_), true) => value.nullish,
            _ if is_undefined && loose => value.nullish,
            (Expression::NullLiteral(_), false) => match (value.nullish, value.undefined) {
                (Some(false), _) => Some(false),
                (Some(true), Some(undefined)) => Some(!undefined),
                _ => None,
            },
            _ if is_undefined => match (value.nullish, value.undefined) {
                (Some(false), _) => Some(false),
                (Some(true), Some(undefined)) => Some(undefined),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_exit_call(&self, span: oxc_span::Span) -> bool {
        self.exits
            .iter()
            .any(|exit| exit.start >= span.start && exit.end <= span.end)
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

/// How a statement completes on this call, for what follows it.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Completion {
    Normal,
    Maybe,
    Exits,
}

fn sequence<'s, 'a: 's>(
    scope: &Scope<'_, '_>,
    statements: impl IntoIterator<Item = &'s Statement<'a>>,
) -> Completion {
    let mut result = Completion::Normal;
    for statement in statements {
        match completion(scope, statement) {
            Completion::Exits => return Completion::Exits,
            Completion::Maybe => result = Completion::Maybe,
            Completion::Normal => {}
        }
    }
    result
}

fn completion(scope: &Scope<'_, '_>, statement: &Statement<'_>) -> Completion {
    match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => Completion::Exits,
        Statement::BlockStatement(block) => sequence(scope, &block.body),
        Statement::IfStatement(statement) => {
            if scope.is_exit_call(statement.test.span()) {
                return Completion::Maybe;
            }
            let consequent = || completion(scope, &statement.consequent);
            let alternate = || {
                statement
                    .alternate
                    .as_ref()
                    .map_or(Completion::Normal, |alternate| completion(scope, alternate))
            };
            match scope.truthy(&statement.test) {
                Some(true) => consequent(),
                Some(false) => alternate(),
                None => match (consequent(), alternate()) {
                    (Completion::Exits, Completion::Exits) => Completion::Exits,
                    (Completion::Normal, Completion::Normal) => Completion::Normal,
                    _ => Completion::Maybe,
                },
            }
        }
        Statement::FunctionDeclaration(_)
        | Statement::ClassDeclaration(_)
        | Statement::EmptyStatement(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_) => Completion::Normal,
        _ => {
            let mut jumps = Jumps::default();
            jumps.visit_statement(statement);
            if jumps.found || scope.is_exit_call(statement.span()) {
                Completion::Maybe
            } else {
                Completion::Normal
            }
        }
    }
}

/// A `return`, `throw`, `break` or `continue` outside every nested function.
#[derive(Default)]
struct Jumps {
    found: bool,
}

impl<'a> Visit<'a> for Jumps {
    fn visit_return_statement(&mut self, _: &oxc_ast::ast::ReturnStatement<'a>) {
        self.found = true;
    }
    fn visit_throw_statement(&mut self, _: &oxc_ast::ast::ThrowStatement<'a>) {
        self.found = true;
    }
    fn visit_break_statement(&mut self, _: &oxc_ast::ast::BreakStatement<'a>) {
        self.found = true;
    }
    fn visit_continue_statement(&mut self, _: &oxc_ast::ast::ContinueStatement<'a>) {
        self.found = true;
    }
    fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {
    }
    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &oxc_ast::ast::Class<'a>) {}
}

fn standing(
    nodes: &AstNodes<'_>,
    scope: &Scope<'_, '_>,
    site: NodeId,
    owner: NodeId,
) -> SiteStanding {
    if let AstKind::CallExpression(call) = nodes.kind(site)
        && call.optional
    {
        return SiteStanding::Possible;
    }
    let mut result = SiteStanding::Guaranteed;
    let mut child = site;
    loop {
        let parent = nodes.parent_id(child);
        if parent == child {
            return SiteStanding::Possible;
        }
        if parent == owner {
            return if matches!(nodes.kind(child), AstKind::FunctionBody(_))
                || matches!(nodes.kind(owner), AstKind::ArrowFunctionExpression(arrow) if arrow.expression)
            {
                result
            } else {
                // A parameter list: defaults depend on the argument.
                SiteStanding::Possible
            };
        }
        let child_span = nodes.kind(child).span();
        let is = |span: oxc_span::Span| span == child_span;
        let step = match nodes.kind(parent) {
            AstKind::FunctionBody(body) => before(scope, &body.statements, child_span),
            AstKind::BlockStatement(block) => before(scope, &block.body, child_span),
            AstKind::IfStatement(statement) => {
                if is(statement.test.span()) {
                    SiteStanding::Guaranteed
                } else {
                    arm(
                        scope.truthy(&statement.test),
                        is(statement.consequent.span()),
                    )
                }
            }
            AstKind::ConditionalExpression(expression) => {
                if is(expression.test.span()) {
                    SiteStanding::Guaranteed
                } else {
                    arm(
                        scope.truthy(&expression.test),
                        is(expression.consequent.span()),
                    )
                }
            }
            AstKind::LogicalExpression(expression) => {
                if is(expression.left.span()) {
                    SiteStanding::Guaranteed
                } else {
                    let runs = match expression.operator {
                        LogicalOperator::And => scope.truthy(&expression.left),
                        LogicalOperator::Or => scope.truthy(&expression.left).map(|value| !value),
                        LogicalOperator::Coalesce => scope.nullish(&expression.left),
                    };
                    arm(runs, true)
                }
            }
            AstKind::CallExpression(expression) => {
                if expression.optional && !is(expression.callee.span()) {
                    SiteStanding::Possible
                } else {
                    SiteStanding::Guaranteed
                }
            }
            AstKind::StaticMemberExpression(expression) if expression.optional => {
                SiteStanding::Possible
            }
            AstKind::ComputedMemberExpression(expression) => {
                if expression.optional && !is(expression.object.span()) {
                    SiteStanding::Possible
                } else {
                    SiteStanding::Guaranteed
                }
            }
            AstKind::StaticMemberExpression(_)
            | AstKind::NewExpression(_)
            | AstKind::ExpressionStatement(_)
            | AstKind::ReturnStatement(_)
            | AstKind::ThrowStatement(_)
            | AstKind::VariableDeclaration(_)
            | AstKind::ParenthesizedExpression(_)
            | AstKind::SequenceExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::UnaryExpression(_)
            | AstKind::TemplateLiteral(_)
            | AstKind::ArrayExpression(_)
            | AstKind::ObjectExpression(_)
            | AstKind::ObjectProperty(_)
            | AstKind::SpreadElement(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_) => SiteStanding::Guaranteed,
            AstKind::VariableDeclarator(declarator) => {
                if declarator.init.as_ref().is_some_and(|init| is(init.span())) {
                    SiteStanding::Guaranteed
                } else {
                    SiteStanding::Possible
                }
            }
            AstKind::AssignmentExpression(expression) => {
                use oxc_ast::ast::AssignmentOperator;
                if matches!(
                    expression.operator,
                    AssignmentOperator::LogicalAnd
                        | AssignmentOperator::LogicalOr
                        | AssignmentOperator::LogicalNullish
                ) {
                    SiteStanding::Possible
                } else {
                    SiteStanding::Guaranteed
                }
            }
            // What a `?.` link may skip, whatever the link (review round 4:
            // oxc marks the member, not the call, in `a?.put(x)`).
            AstKind::ChainExpression(chain) => {
                let mut skipped_sites = Vec::new();
                let mut skipped_ranges = Vec::new();
                super::skippable_chain_parts(
                    &chain.expression,
                    &mut skipped_sites,
                    &mut skipped_ranges,
                );
                let site_span = nodes.kind(site).span();
                let site_span = crate::core::Span::new(site_span.start, site_span.end);
                if skipped_sites.contains(&site_span)
                    || skipped_ranges.iter().any(|range| range.contains(site_span))
                {
                    SiteStanding::Possible
                } else {
                    SiteStanding::Guaranteed
                }
            }
            _ => SiteStanding::Possible,
        };
        match step {
            SiteStanding::Unreachable => return SiteStanding::Unreachable,
            SiteStanding::Possible => result = SiteStanding::Possible,
            SiteStanding::Guaranteed => {}
        }
        child = parent;
    }
}

/// A branch arm whose evaluation `runs` decides: `this_arm` names the arm
/// taken when it holds.
fn arm(runs: Option<bool>, this_arm: bool) -> SiteStanding {
    match runs {
        Some(value) if value == this_arm => SiteStanding::Guaranteed,
        Some(_) => SiteStanding::Unreachable,
        None => SiteStanding::Possible,
    }
}

/// The standing the statements before `child` in a list leave it.
fn before(
    scope: &Scope<'_, '_>,
    statements: &[Statement<'_>],
    child: oxc_span::Span,
) -> SiteStanding {
    let mut result = SiteStanding::Guaranteed;
    for statement in statements {
        if statement.span() == child {
            return result;
        }
        match completion(scope, statement) {
            Completion::Exits => return SiteStanding::Unreachable,
            Completion::Maybe => result = SiteStanding::Possible,
            Completion::Normal => {}
        }
    }
    SiteStanding::Possible
}

#[cfg(test)]
mod tests {
    use super::{ParameterValue, SiteStanding, argument_site_standings};
    use crate::core::Span;
    use std::path::Path;

    fn standings(
        source: &str,
        function: &str,
        values: &[ParameterValue],
        sites: &[&str],
    ) -> Vec<SiteStanding> {
        let start = source
            .find(&format!("function {function}"))
            .expect("function");
        let end = source[start..]
            .find("\n}")
            .map(|offset| start + offset + 2)
            .expect("end");
        let spans = sites
            .iter()
            .map(|site| {
                let at = source[start..end].find(site).expect("site") + start;
                Span::new(at as u32, (at + site.len()) as u32)
            })
            .collect::<Vec<_>>();
        argument_site_standings(
            Path::new("test.ts"),
            source,
            Span::new(start as u32, end as u32),
            values,
            &[],
            &spans,
        )
        .expect("decides")
    }

    fn number(index: usize) -> ParameterValue {
        ParameterValue {
            index,
            type_of: Some("number"),
            truthy: Some(true),
            nullish: Some(false),
            undefined: Some(false),
        }
    }

    fn function(index: usize) -> ParameterValue {
        ParameterValue {
            index,
            type_of: Some("function"),
            truthy: Some(true),
            nullish: Some(false),
            undefined: Some(false),
        }
    }

    const TIMER: &str = "declare function onCleanup(f: () => void): void;\n\
declare function createEffect(a: () => unknown, b: () => void): void;\n\
function createTimer(fn: () => void, delay: number | (() => number)) {\n  if (typeof delay === \"number\") {\n    onCleanup(() => fn());\n    return;\n  }\n  createEffect(() => delay(), () => fn());\n}\n";

    #[test]
    fn a_number_takes_the_cleanup_arm_and_skips_the_effect() {
        assert_eq!(
            standings(
                TIMER,
                "createTimer",
                &[number(1)],
                &[
                    "onCleanup(() => fn())",
                    "createEffect(() => delay(), () => fn())"
                ]
            ),
            [SiteStanding::Guaranteed, SiteStanding::Unreachable]
        );
        assert_eq!(
            standings(
                TIMER,
                "createTimer",
                &[function(1)],
                &[
                    "onCleanup(() => fn())",
                    "createEffect(() => delay(), () => fn())"
                ]
            ),
            [SiteStanding::Unreachable, SiteStanding::Guaranteed]
        );
        assert_eq!(
            standings(
                TIMER,
                "createTimer",
                &[],
                &[
                    "onCleanup(() => fn())",
                    "createEffect(() => delay(), () => fn())"
                ]
            ),
            [SiteStanding::Possible, SiteStanding::Possible]
        );
    }

    /// Review round 4: defaults, redeclarations, optional chains and exits
    /// that may run first.
    #[test]
    fn review_round_four_refusals() {
        let source = "declare function readA(): number;\ndeclare function fail(m: string): never;\n\
declare const sink: { put(x: number): void } | undefined;\n\
function defaulted(flag = true) {\n  if (flag) readA();\n}\n\
function redeclared(flag: boolean) {\n  var flag = true;\n  if (flag) readA();\n}\n\
function chained(flag: boolean) {\n  if (flag) return;\n  sink?.put(readA());\n}\n\
function exitFirst(ok: boolean, n = fail(\"x\")) {\n  if (ok) return;\n  readA();\n}\n";
        let truthy = ParameterValue {
            index: 0,
            truthy: Some(true),
            ..ParameterValue::default()
        };
        let falsy = ParameterValue {
            index: 0,
            truthy: Some(false),
            ..ParameterValue::default()
        };
        // `f(undefined)` takes the default, so the omitted value decides nothing.
        assert_eq!(
            standings(
                source,
                "defaulted",
                &[ParameterValue::omitted(0)],
                &["readA()"]
            ),
            [SiteStanding::Possible]
        );
        assert_eq!(
            standings(
                source,
                "redeclared",
                std::slice::from_ref(&falsy),
                &["readA()"]
            ),
            [SiteStanding::Possible]
        );
        assert_eq!(
            standings(
                source,
                "chained",
                std::slice::from_ref(&falsy),
                &["readA()"]
            ),
            [SiteStanding::Possible]
        );
        assert_eq!(
            standings(source, "chained", &[truthy], &["readA()"]),
            [SiteStanding::Unreachable]
        );
        let start = source.find("function exitFirst").unwrap();
        let end = source[start..].find("\n}").unwrap() + start + 2;
        let at = |text: &str| {
            let offset = source[start..end].find(text).unwrap() + start;
            Span::new(offset as u32, (offset + text.len()) as u32)
        };
        assert_eq!(
            argument_site_standings(
                Path::new("test.ts"),
                source,
                Span::new(start as u32, end as u32),
                &[falsy],
                &[at("fail(\"x\")")],
                &[at("readA()")],
            ),
            Some(vec![SiteStanding::Possible]),
            "a `never` default may throw before the body"
        );
    }

    #[test]
    fn truthiness_nullishness_and_refusals() {
        let source = "declare function readA(): number;\ndeclare function readB(): number;\n\
function when(flag: boolean, value?: number) {\n  if (!flag) return 0;\n  const fallback = value ?? readA();\n  return flag && readB();\n}\n\
function written(flag: boolean) {\n  flag = !flag;\n  if (flag) readA();\n}\n\
function aliased(flag: boolean) {\n  if (arguments[0]) readA();\n}\n";
        let truthy = ParameterValue {
            index: 0,
            truthy: Some(true),
            ..ParameterValue::default()
        };
        let falsy = ParameterValue {
            index: 0,
            truthy: Some(false),
            ..ParameterValue::default()
        };
        assert_eq!(
            standings(
                source,
                "when",
                &[truthy.clone(), ParameterValue::omitted(1)],
                &["readA()", "readB()"]
            ),
            [SiteStanding::Guaranteed, SiteStanding::Guaranteed]
        );
        assert_eq!(
            standings(source, "when", &[falsy], &["readA()", "readB()"]),
            [SiteStanding::Unreachable, SiteStanding::Unreachable]
        );
        // Without values the same sites are only possible.
        assert_eq!(
            standings(source, "when", &[], &["readA()", "readB()"]),
            [SiteStanding::Possible, SiteStanding::Possible]
        );
        // A written parameter decides nothing.
        assert_eq!(
            standings(source, "written", &[truthy], &["readA()"]),
            [SiteStanding::Possible]
        );
        let start = source.find("function aliased").unwrap();
        let end = source[start..].find("\n}").unwrap() + start + 2;
        assert!(
            argument_site_standings(
                Path::new("test.ts"),
                source,
                Span::new(start as u32, end as u32),
                &[],
                &[],
                &[]
            )
            .is_none(),
            "`arguments` refuses"
        );
    }
}
