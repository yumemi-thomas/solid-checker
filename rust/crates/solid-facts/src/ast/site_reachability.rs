//! Normal-path reachability of a site inside one function (or the module
//! body), proven by a positive allowlist of syntactic shapes.
//!
//! A site is reachable only when every construct on the syntactic path from
//! the scope entry to the site is an allowlisted shape, and every statement and
//! expression evaluated before the site on that path has no proven abrupt
//! completion. The allowlist:
//!
//! - blocks, labeled statements, expression statements, declarations;
//! - `if`/`else`, `while`, `for`: the branch or body is entered only when the
//!   test does not fold to the constant that skips it (a missing `for` test is
//!   constant true);
//! - `for`-`of`/`in` over an iterable that is not a literal;
//! - `switch` over a discriminant that does not fold to a constant;
//! - `try` blocks and `finally` blocks, never `catch` handlers (no throw is
//!   proven to reach one);
//! - inside an expression: operands of calls, `new`, `await`, unary, binary and
//!   sequence expressions, simple assignments, and the right of `&&`/`||`/`??`
//!   or an arm of `?:` unless the left side or test folds to the constant that
//!   skips it.
//!
//! Anything else on the path (a `do`-`while` test, a `for` update, a `with`, a
//! `catch`, an optional chain, a computed member key, a spread) is not claimed.
//!
//! Abrupt completion is proven by: `return`, `throw`, `break`, `continue`; a
//! call the caller lists in `diverging` (it never returns normally); a read of
//! a name that a same-file getter (object literal or class) which cannot return
//! is declared under, or any computed read, spread or destructuring while such
//! a getter exists; a loop with a constant-true test and no `break`; a
//! `switch` over a literal or one whose arms all leave; a `try` whose block
//! does not complete normally (its `catch` is not credited).
//!
//! Constant folding is exact ([`Walk::fold`]): literals, `!`, `typeof`, `void`,
//! unary minus, `&&`/`||`/`??`, `?:` and comparisons of two folded primitives.
//! Anything it cannot fold is non-constant.
//!
//! The one remaining assumption is the feasible-path premise (ADR 0341): a
//! non-constant condition may take either branch, and a call not proven never
//! to return returns normally.
//!
//! `None` is a source that does not parse or a function that is not found.

use std::collections::HashSet;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentOperator, AssignmentTarget, BinaryOperator,
    BindingPattern, ComputedMemberExpression, Declaration, Expression, ForStatementInit,
    FunctionBody, LogicalOperator, MethodDefinition, MethodDefinitionKind, ObjectProperty,
    PropertyKind, Statement, StaticMemberExpression, UnaryOperator, VariableDeclaration,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::scope::ScopeFlags;

use super::host_execution::inner;
use crate::core::Span;

/// Whether the site is on a proven normal path from the entry of the function
/// at `function`. See the module comment. `None` when the source does not parse
/// or no function has that span.
#[must_use]
pub fn site_reachable(
    path: &Path,
    source: &str,
    function: Span,
    site: Span,
    diverging: &[Span],
) -> Option<bool> {
    with_scope(
        path,
        source,
        Some(function),
        diverging,
        |walk, statements, _| walk.reach_seq(statements, site),
    )
}

/// [`site_reachable`] for a site written directly in the module body (outside
/// every function).
#[must_use]
pub fn module_site_reachable(
    path: &Path,
    source: &str,
    site: Span,
    diverging: &[Span],
) -> Option<bool> {
    with_scope(path, source, None, diverging, |walk, statements, _| {
        walk.reach_seq(statements, site)
    })
}

/// Whether a call of the function at `function` can return a value: its body
/// proves a normal completion, or a reachable `return` that does not return a
/// diverging call. `false` is a function that always throws (or never ends) as
/// far as the rules prove. An `async` function or a generator answers `true`:
/// calling one never throws synchronously. `None` when the source does not
/// parse or no function has that span.
#[must_use]
pub fn function_may_return(
    path: &Path,
    source: &str,
    function: Span,
    diverging: &[Span],
) -> Option<bool> {
    with_scope(
        path,
        source,
        Some(function),
        diverging,
        |walk, statements, kind| {
            if kind.lenient {
                return true;
            }
            if kind.expression {
                return match statements.first() {
                    Some(Statement::ExpressionStatement(statement)) => {
                        !walk.abrupt(&statement.expression)
                    }
                    _ => true,
                };
            }
            let out = walk.seq(statements);
            out.normal || out.returns
        },
    )
}

#[derive(Clone, Copy)]
struct ScopeKind {
    /// An expression-bodied arrow.
    expression: bool,
    /// An `async` function or a generator.
    lenient: bool,
}

fn with_scope<T>(
    path: &Path,
    source: &str,
    function: Option<Span>,
    diverging: &[Span],
    answer: impl FnOnce(&Walk, &[Statement<'_>], ScopeKind) -> T,
) -> Option<T> {
    if source.len() > 4 * 1024 * 1024 {
        return None;
    }
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut walk = Walk::new(diverging);
    // Getters that cannot return, from a first pass without them (a getter
    // that reads another throwing getter is not chased).
    let mut scan = GetterScan {
        walk: &walk,
        throwing: Vec::new(),
    };
    scan.visit_program(&parsed.program);
    walk.getters = scan.throwing;
    let Some(function) = function else {
        return Some(answer(
            &walk,
            &parsed.program.body,
            ScopeKind {
                expression: false,
                lenient: false,
            },
        ));
    };
    let built = SemanticBuilder::new().build(&parsed.program);
    let nodes = built.semantic.nodes();
    let owner = nodes.iter().find(|node| {
        let span = node.kind().span();
        span.start == function.start
            && span.end == function.end
            && matches!(
                node.kind(),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
    })?;
    match owner.kind() {
        AstKind::Function(function) => {
            let body: &FunctionBody<'_> = function.body.as_ref()?;
            Some(answer(
                &walk,
                &body.statements,
                ScopeKind {
                    expression: false,
                    lenient: function.r#async || function.generator,
                },
            ))
        }
        AstKind::ArrowFunctionExpression(arrow) => Some(answer(
            &walk,
            &arrow.body.statements,
            ScopeKind {
                expression: arrow.expression,
                lenient: arrow.r#async,
            },
        )),
        _ => None,
    }
}

/// How a statement can end, as far as the rules prove it.
#[derive(Clone, Default)]
struct Out {
    /// Control provably falls out of the end of the statement.
    normal: bool,
    /// A reachable `return` that returns a value or nothing (not a diverging
    /// call).
    returns: bool,
    /// A reachable unlabeled `break`.
    brk: bool,
    /// Reachable `break label` statements.
    labels: Vec<String>,
}

impl Out {
    fn normal() -> Self {
        Self {
            normal: true,
            ..Self::default()
        }
    }

    fn absorb(&mut self, other: &Self) {
        self.returns |= other.returns;
        self.brk |= other.brk;
        for label in &other.labels {
            if !self.labels.contains(label) {
                self.labels.push(label.clone());
            }
        }
    }
}

/// A folded constant.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Null,
    Undefined,
    Bool(bool),
    Num(f64),
    Str(String),
    /// An object, array, regular expression: truthy, identity unknown.
    Object,
    /// A function or class: truthy.
    Function,
}

impl Value {
    fn truthy(&self) -> bool {
        match self {
            Self::Null | Self::Undefined => false,
            Self::Bool(value) => *value,
            Self::Num(value) => *value != 0.0 && !value.is_nan(),
            Self::Str(value) => !value.is_empty(),
            Self::Object | Self::Function => true,
        }
    }

    fn nullish(&self) -> bool {
        matches!(self, Self::Null | Self::Undefined)
    }

    fn type_of(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null | Self::Object => "object",
            Self::Bool(_) => "boolean",
            Self::Num(_) => "number",
            Self::Str(_) => "string",
            Self::Function => "function",
        }
    }
}

struct Walk {
    diverging: HashSet<Span>,
    /// Names of same-file getters that cannot return; `None` is a getter whose
    /// name is not static.
    getters: Vec<Option<String>>,
}

fn span_of(span: oxc_span::Span) -> Span {
    Span::new(span.start, span.end)
}

fn contains(outer: oxc_span::Span, site: Span) -> bool {
    outer.start <= site.start && site.end <= outer.end
}

/// Finds a proven abrupt completion inside an expression, not entering nested
/// functions or classes (they do not run when the expression is evaluated).
struct Finder<'w> {
    walk: &'w Walk,
    found: bool,
}

impl<'a> Visit<'a> for Finder<'_> {
    fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}

    fn visit_class(&mut self, _: &oxc_ast::ast::Class<'a>) {}

    fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
        if self.walk.diverging.contains(&span_of(call.span)) {
            self.found = true;
            return;
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if self.walk.getter_throws(Some(member.property.name.as_str())) {
            self.found = true;
            return;
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        let name = match inner(&member.expression) {
            Some(Expression::StringLiteral(text)) => Some(text.value.as_str()),
            _ => None,
        };
        if self.walk.getter_throws(name) {
            self.found = true;
            return;
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_spread_element(&mut self, spread: &oxc_ast::ast::SpreadElement<'a>) {
        if !self.walk.getters.is_empty() {
            self.found = true;
            return;
        }
        walk::walk_spread_element(self, spread);
    }

    fn visit_binding_property(&mut self, property: &oxc_ast::ast::BindingProperty<'a>) {
        let name = property.key.static_name();
        if self.walk.getter_throws(name.as_deref()) {
            self.found = true;
            return;
        }
        walk::walk_binding_property(self, property);
    }
}

/// Collects the same-file getters (object literal or class) that cannot return.
struct GetterScan<'w> {
    walk: &'w Walk,
    throwing: Vec<Option<String>>,
}

impl GetterScan<'_> {
    fn check(&mut self, key: Option<String>, function: &oxc_ast::ast::Function<'_>) {
        if function.r#async || function.generator {
            return;
        }
        let Some(body) = &function.body else {
            return;
        };
        let out = self.walk.seq(&body.statements);
        if !(out.normal || out.returns) {
            self.throwing.push(key);
        }
    }
}

impl<'a> Visit<'a> for GetterScan<'_> {
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.kind == PropertyKind::Get
            && let Expression::FunctionExpression(function) = &property.value
        {
            let key = property.key.static_name().map(|name| name.into_owned());
            self.check(key, function);
        }
        walk::walk_object_property(self, property);
    }

    fn visit_method_definition(&mut self, method: &MethodDefinition<'a>) {
        if method.kind == MethodDefinitionKind::Get {
            let key = method.key.static_name().map(|name| name.into_owned());
            self.check(key, &method.value);
        }
        walk::walk_method_definition(self, method);
    }
}

/// Whether the expression, peeled of sugar, is a literal value written in
/// place: its content is known where it is written.
fn literal_like(expression: &Expression<'_>) -> bool {
    match inner(expression) {
        Some(
            Expression::ArrayExpression(_)
            | Expression::ObjectExpression(_)
            | Expression::StringLiteral(_)
            | Expression::TemplateLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_),
        ) => true,
        Some(_) => false,
        None => true,
    }
}

impl Walk {
    fn new(diverging: &[Span]) -> Self {
        Self {
            diverging: diverging.iter().copied().collect(),
            getters: Vec::new(),
        }
    }

    fn getter_throws(&self, name: Option<&str>) -> bool {
        self.getters.iter().any(|getter| match (getter, name) {
            (Some(getter), Some(name)) => getter == name,
            _ => true,
        })
    }

    /// Exact constant folding; `None` is anything it cannot fold.
    fn fold(expression: &Expression<'_>) -> Option<Value> {
        match inner(expression)? {
            Expression::BooleanLiteral(value) => Some(Value::Bool(value.value)),
            Expression::NumericLiteral(value) => Some(Value::Num(value.value)),
            Expression::StringLiteral(value) => Some(Value::Str(value.value.to_string())),
            Expression::NullLiteral(_) => Some(Value::Null),
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                let cooked = template.quasis.first()?.value.cooked.as_ref()?;
                Some(Value::Str(cooked.to_string()))
            }
            Expression::ArrayExpression(_)
            | Expression::ObjectExpression(_)
            | Expression::RegExpLiteral(_) => Some(Value::Object),
            Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::ClassExpression(_) => Some(Value::Function),
            Expression::UnaryExpression(unary) => {
                let argument = Self::fold(&unary.argument)?;
                match unary.operator {
                    UnaryOperator::LogicalNot => Some(Value::Bool(!argument.truthy())),
                    UnaryOperator::Typeof => Some(Value::Str(argument.type_of().to_owned())),
                    UnaryOperator::Void => Some(Value::Undefined),
                    UnaryOperator::UnaryNegation => match argument {
                        Value::Num(value) => Some(Value::Num(-value)),
                        _ => None,
                    },
                    UnaryOperator::UnaryPlus => match argument {
                        Value::Num(value) => Some(Value::Num(value)),
                        _ => None,
                    },
                    _ => None,
                }
            }
            Expression::LogicalExpression(logical) => {
                let left = Self::fold(&logical.left)?;
                let take_right = match logical.operator {
                    LogicalOperator::And => left.truthy(),
                    LogicalOperator::Or => !left.truthy(),
                    LogicalOperator::Coalesce => left.nullish(),
                };
                if take_right {
                    Self::fold(&logical.right)
                } else {
                    Some(left)
                }
            }
            Expression::ConditionalExpression(conditional) => {
                if Self::fold(&conditional.test)?.truthy() {
                    Self::fold(&conditional.consequent)
                } else {
                    Self::fold(&conditional.alternate)
                }
            }
            Expression::BinaryExpression(binary) => {
                let left = Self::fold(&binary.left)?;
                let right = Self::fold(&binary.right)?;
                let primitive = |value: &Value| {
                    matches!(
                        value,
                        Value::Null
                            | Value::Undefined
                            | Value::Bool(_)
                            | Value::Num(_)
                            | Value::Str(_)
                    )
                };
                if !primitive(&left) || !primitive(&right) {
                    return None;
                }
                let same_kind = std::mem::discriminant(&left) == std::mem::discriminant(&right);
                match binary.operator {
                    BinaryOperator::StrictEquality => Some(Value::Bool(left == right)),
                    BinaryOperator::StrictInequality => Some(Value::Bool(left != right)),
                    BinaryOperator::Equality if same_kind => Some(Value::Bool(left == right)),
                    BinaryOperator::Inequality if same_kind => Some(Value::Bool(left != right)),
                    BinaryOperator::LessThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterEqualThan => match (left, right) {
                        (Value::Num(a), Value::Num(b)) => {
                            Some(Value::Bool(match binary.operator {
                                BinaryOperator::LessThan => a < b,
                                BinaryOperator::LessEqualThan => a <= b,
                                BinaryOperator::GreaterThan => a > b,
                                _ => a >= b,
                            }))
                        }
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn truth(expression: &Expression<'_>) -> Option<bool> {
        Self::fold(expression).map(|value| value.truthy())
    }

    /// Whether evaluating the expression has a proven abrupt completion.
    fn abrupt(&self, expression: &Expression<'_>) -> bool {
        let Some(expression) = inner(expression) else {
            return true;
        };
        match expression {
            Expression::LogicalExpression(logical) => {
                if self.abrupt(&logical.left) {
                    return true;
                }
                let right_runs =
                    Self::fold(&logical.left).is_some_and(|left| match logical.operator {
                        LogicalOperator::And => left.truthy(),
                        LogicalOperator::Or => !left.truthy(),
                        LogicalOperator::Coalesce => left.nullish(),
                    });
                right_runs && self.abrupt(&logical.right)
            }
            Expression::ConditionalExpression(conditional) => {
                self.abrupt(&conditional.test)
                    || match Self::truth(&conditional.test) {
                        Some(true) => self.abrupt(&conditional.consequent),
                        Some(false) => self.abrupt(&conditional.alternate),
                        None => {
                            self.abrupt(&conditional.consequent)
                                && self.abrupt(&conditional.alternate)
                        }
                    }
            }
            Expression::SequenceExpression(sequence) => {
                sequence.expressions.iter().any(|item| self.abrupt(item))
            }
            other => {
                let mut finder = Finder {
                    walk: self,
                    found: false,
                };
                finder.visit_expression(other);
                finder.found
            }
        }
    }

    fn pattern_abrupt(&self, pattern: &BindingPattern<'_>) -> bool {
        let mut finder = Finder {
            walk: self,
            found: false,
        };
        finder.visit_binding_pattern(pattern);
        finder.found
    }

    fn declaration_abrupt(&self, declaration: &VariableDeclaration<'_>) -> bool {
        declaration.declarations.iter().any(|declarator| {
            declarator
                .init
                .as_ref()
                .is_some_and(|init| self.abrupt(init) || self.pattern_abrupt(&declarator.id))
        })
    }

    fn for_init_abrupt(&self, init: &ForStatementInit<'_>) -> bool {
        match init {
            ForStatementInit::VariableDeclaration(declaration) => {
                self.declaration_abrupt(declaration)
            }
            other => other.as_expression().is_some_and(|init| self.abrupt(init)),
        }
    }

    fn seq(&self, statements: &[Statement<'_>]) -> Out {
        let mut out = Out::normal();
        for statement in statements {
            if !out.normal {
                break;
            }
            let next = self.stmt(statement);
            out.normal = next.normal;
            out.absorb(&next);
        }
        out
    }

    fn stmt(&self, statement: &Statement<'_>) -> Out {
        match statement {
            Statement::ReturnStatement(statement) => Out {
                returns: statement
                    .argument
                    .as_ref()
                    .is_none_or(|argument| !self.abrupt(argument)),
                ..Out::default()
            },
            Statement::ThrowStatement(_) | Statement::ContinueStatement(_) => Out::default(),
            Statement::BreakStatement(statement) => match &statement.label {
                None => Out {
                    brk: true,
                    ..Out::default()
                },
                Some(label) => Out {
                    labels: vec![label.name.as_str().to_owned()],
                    ..Out::default()
                },
            },
            Statement::ExpressionStatement(statement) => {
                if self.abrupt(&statement.expression) {
                    Out::default()
                } else {
                    Out::normal()
                }
            }
            Statement::VariableDeclaration(declaration) => {
                if self.declaration_abrupt(declaration) {
                    Out::default()
                } else {
                    Out::normal()
                }
            }
            Statement::BlockStatement(block) => self.seq(&block.body),
            Statement::IfStatement(statement) => {
                if self.abrupt(&statement.test) {
                    return Out::default();
                }
                let consequent = || self.stmt(&statement.consequent);
                let alternate = || {
                    statement
                        .alternate
                        .as_ref()
                        .map_or_else(Out::normal, |alternate| self.stmt(alternate))
                };
                match Self::truth(&statement.test) {
                    Some(true) => consequent(),
                    Some(false) => alternate(),
                    None => {
                        let (a, b) = (consequent(), alternate());
                        let mut out = Out {
                            normal: a.normal || b.normal,
                            ..Out::default()
                        };
                        out.absorb(&a);
                        out.absorb(&b);
                        out
                    }
                }
            }
            Statement::LabeledStatement(statement) => {
                let mut out = self.stmt(&statement.body);
                let label = statement.label.name.as_str();
                if out.labels.iter().any(|candidate| candidate == label) {
                    out.normal = true;
                    out.labels.retain(|candidate| candidate != label);
                }
                out
            }
            Statement::WhileStatement(statement) => {
                if self.abrupt(&statement.test) {
                    return Out::default();
                }
                self.looping(Self::truth(&statement.test), &statement.body, false)
            }
            Statement::ForStatement(statement) => {
                if statement
                    .init
                    .as_ref()
                    .is_some_and(|init| self.for_init_abrupt(init))
                    || statement
                        .test
                        .as_ref()
                        .is_some_and(|test| self.abrupt(test))
                {
                    return Out::default();
                }
                let truth = statement
                    .test
                    .as_ref()
                    .map_or(Some(true), |test| Self::truth(test));
                self.looping(truth, &statement.body, false)
            }
            Statement::DoWhileStatement(statement) => {
                let mut out = self.looping(Self::truth(&statement.test), &statement.body, true);
                if self.abrupt(&statement.test) {
                    // The test runs after a normal body; a `break` still leaves.
                    out.normal = false;
                }
                out
            }
            Statement::ForInStatement(statement) => self.each(&statement.right, &statement.body),
            Statement::ForOfStatement(statement) => self.each(&statement.right, &statement.body),
            Statement::SwitchStatement(statement) => {
                if literal_like(&statement.discriminant)
                    || self.abrupt(&statement.discriminant)
                    || statement
                        .cases
                        .iter()
                        .any(|case| case.test.as_ref().is_some_and(|test| self.abrupt(test)))
                {
                    return Out::default();
                }
                let mut out = Out::default();
                let mut any_break = false;
                let mut last_normal = true;
                for case in &statement.cases {
                    let group = self.seq(&case.consequent);
                    any_break |= group.brk;
                    last_normal = group.normal;
                    out.absorb(&group);
                }
                let has_default = statement.cases.iter().any(|case| case.test.is_none());
                out.brk = false;
                out.normal = !has_default || statement.cases.is_empty() || last_normal || any_break;
                out
            }
            Statement::TryStatement(statement) => {
                // No throw is proven to reach a `catch`: its normal completion
                // and its returns are not credited.
                let block = self.seq(&statement.block.body);
                let mut out = Out {
                    normal: block.normal,
                    ..Out::default()
                };
                out.absorb(&block);
                if let Some(finalizer) = &statement.finalizer {
                    let finalizer = self.seq(&finalizer.body);
                    if finalizer.normal {
                        out.absorb(&finalizer);
                    } else {
                        // A `finally` that always leaves overrides every
                        // completion of the block.
                        out = finalizer;
                    }
                }
                out
            }
            Statement::WithStatement(_) => Out::default(),
            _ => Out::normal(),
        }
    }

    /// `while`, `for` and `do`: with a literally true test the loop ends only
    /// through a `break`; a literally false test never runs the body (but a
    /// `do` body runs once).
    fn looping(&self, truth: Option<bool>, body: &Statement<'_>, runs_once: bool) -> Out {
        if truth == Some(false) && !runs_once {
            return Out::normal();
        }
        let body = self.stmt(body);
        let mut out = Out {
            normal: if runs_once {
                (truth != Some(true) && body.normal) || body.brk
            } else {
                truth != Some(true) || body.brk
            },
            ..Out::default()
        };
        out.absorb(&body);
        out.brk = false;
        out
    }

    /// `for`-`in`/`of`: the body runs only over an iterable that is not a
    /// literal (a literal may be empty); evaluating the iterable must not leave.
    fn each(&self, right: &Expression<'_>, body: &Statement<'_>) -> Out {
        if self.abrupt(right) {
            return Out::default();
        }
        let mut out = Out::normal();
        if !literal_like(right) {
            let body = self.stmt(body);
            out.absorb(&body);
            out.brk = false;
        }
        out
    }

    fn reach_seq(&self, statements: &[Statement<'_>], site: Span) -> bool {
        for statement in statements {
            if contains(statement.span(), site) {
                return self.reach_stmt(statement, site);
            }
            if !self.stmt(statement).normal {
                return false;
            }
        }
        false
    }

    fn reach_declaration(&self, declaration: &VariableDeclaration<'_>, site: Span) -> bool {
        for declarator in &declaration.declarations {
            if let Some(init) = &declarator.init
                && contains(init.span(), site)
            {
                return self.reach_expr(init, site);
            }
            if contains(declarator.span, site) {
                return false;
            }
            if let Some(init) = &declarator.init
                && (self.abrupt(init) || self.pattern_abrupt(&declarator.id))
            {
                return false;
            }
        }
        false
    }

    fn reach_stmt(&self, statement: &Statement<'_>, site: Span) -> bool {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.reach_expr(&statement.expression, site)
            }
            Statement::VariableDeclaration(declaration) => {
                self.reach_declaration(declaration, site)
            }
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::VariableDeclaration(declaration)) => {
                    self.reach_declaration(declaration, site)
                }
                _ => false,
            },
            Statement::ReturnStatement(statement) => statement
                .argument
                .as_ref()
                .is_some_and(|argument| self.reach_expr(argument, site)),
            Statement::ThrowStatement(statement) => self.reach_expr(&statement.argument, site),
            Statement::BlockStatement(block) => self.reach_seq(&block.body, site),
            Statement::LabeledStatement(statement) => self.reach_stmt(&statement.body, site),
            Statement::IfStatement(statement) => {
                if contains(statement.test.span(), site) {
                    return self.reach_expr(&statement.test, site);
                }
                if self.abrupt(&statement.test) {
                    return false;
                }
                let truth = Self::truth(&statement.test);
                if contains(statement.consequent.span(), site) {
                    return truth != Some(false) && self.reach_stmt(&statement.consequent, site);
                }
                if let Some(alternate) = &statement.alternate
                    && contains(alternate.span(), site)
                {
                    return truth != Some(true) && self.reach_stmt(alternate, site);
                }
                false
            }
            Statement::WhileStatement(statement) => {
                if contains(statement.test.span(), site) {
                    return self.reach_expr(&statement.test, site);
                }
                contains(statement.body.span(), site)
                    && !self.abrupt(&statement.test)
                    && Self::truth(&statement.test) != Some(false)
                    && self.reach_stmt(&statement.body, site)
            }
            Statement::ForStatement(statement) => {
                if let Some(init) = &statement.init
                    && contains(init.span(), site)
                {
                    return match init {
                        ForStatementInit::VariableDeclaration(declaration) => {
                            self.reach_declaration(declaration, site)
                        }
                        other => other
                            .as_expression()
                            .is_some_and(|init| self.reach_expr(init, site)),
                    };
                }
                if statement
                    .init
                    .as_ref()
                    .is_some_and(|init| self.for_init_abrupt(init))
                {
                    return false;
                }
                if let Some(test) = &statement.test
                    && contains(test.span(), site)
                {
                    return self.reach_expr(test, site);
                }
                let truth = statement
                    .test
                    .as_ref()
                    .map_or(Some(true), |test| Self::truth(test));
                contains(statement.body.span(), site)
                    && statement
                        .test
                        .as_ref()
                        .is_none_or(|test| !self.abrupt(test))
                    && truth != Some(false)
                    && self.reach_stmt(&statement.body, site)
            }
            Statement::DoWhileStatement(statement) => {
                contains(statement.body.span(), site) && self.reach_stmt(&statement.body, site)
            }
            Statement::ForInStatement(statement) => {
                self.reach_each(&statement.right, &statement.body, site)
            }
            Statement::ForOfStatement(statement) => {
                self.reach_each(&statement.right, &statement.body, site)
            }
            Statement::SwitchStatement(statement) => {
                if contains(statement.discriminant.span(), site) {
                    return self.reach_expr(&statement.discriminant, site);
                }
                if literal_like(&statement.discriminant)
                    || self.abrupt(&statement.discriminant)
                    || statement
                        .cases
                        .iter()
                        .any(|case| case.test.as_ref().is_some_and(|test| self.abrupt(test)))
                {
                    return false;
                }
                statement
                    .cases
                    .iter()
                    .any(|case| contains(case.span, site) && self.reach_seq(&case.consequent, site))
            }
            Statement::TryStatement(statement) => {
                if contains(statement.block.span, site) {
                    return self.reach_seq(&statement.block.body, site);
                }
                // A catch handler is not claimed: no throw is proven to reach
                // it.
                if let Some(finalizer) = &statement.finalizer
                    && contains(finalizer.span, site)
                {
                    return self.reach_seq(&finalizer.body, site);
                }
                false
            }
            _ => false,
        }
    }

    fn reach_each(&self, right: &Expression<'_>, body: &Statement<'_>, site: Span) -> bool {
        if contains(right.span(), site) {
            return self.reach_expr(right, site);
        }
        contains(body.span(), site)
            && !self.abrupt(right)
            && !literal_like(right)
            && self.reach_stmt(body, site)
    }

    /// Whether evaluating `expression` reaches the sub-expression `site`.
    fn reach_expr(&self, expression: &Expression<'_>, site: Span) -> bool {
        let Some(peeled) = inner(expression) else {
            return false;
        };
        if span_of(peeled.span()) == site || span_of(expression.span()) == site {
            return true;
        }
        match peeled {
            Expression::CallExpression(call) => {
                if call.optional {
                    return false;
                }
                self.reach_call(&call.callee, &call.arguments, site)
            }
            Expression::NewExpression(new) => self.reach_call(&new.callee, &new.arguments, site),
            Expression::AwaitExpression(await_) => self.reach_expr(&await_.argument, site),
            Expression::UnaryExpression(unary) => self.reach_expr(&unary.argument, site),
            Expression::StaticMemberExpression(member) if !member.optional => {
                self.reach_expr(&member.object, site)
            }
            Expression::BinaryExpression(binary) => {
                if contains(binary.left.span(), site) {
                    self.reach_expr(&binary.left, site)
                } else {
                    contains(binary.right.span(), site)
                        && !self.abrupt(&binary.left)
                        && self.reach_expr(&binary.right, site)
                }
            }
            Expression::AssignmentExpression(assignment) => {
                matches!(
                    assignment.left,
                    AssignmentTarget::AssignmentTargetIdentifier(_)
                ) && assignment.operator == AssignmentOperator::Assign
                    && contains(assignment.right.span(), site)
                    && self.reach_expr(&assignment.right, site)
            }
            Expression::SequenceExpression(sequence) => {
                for (index, item) in sequence.expressions.iter().enumerate() {
                    if contains(item.span(), site) {
                        return sequence.expressions[..index]
                            .iter()
                            .all(|earlier| !self.abrupt(earlier))
                            && self.reach_expr(item, site);
                    }
                }
                false
            }
            Expression::LogicalExpression(logical) => {
                if contains(logical.left.span(), site) {
                    return self.reach_expr(&logical.left, site);
                }
                if !contains(logical.right.span(), site) || self.abrupt(&logical.left) {
                    return false;
                }
                let skipped =
                    Self::fold(&logical.left).is_some_and(|left| match logical.operator {
                        LogicalOperator::And => !left.truthy(),
                        LogicalOperator::Or => left.truthy(),
                        LogicalOperator::Coalesce => !left.nullish(),
                    });
                !skipped && self.reach_expr(&logical.right, site)
            }
            Expression::ConditionalExpression(conditional) => {
                if contains(conditional.test.span(), site) {
                    return self.reach_expr(&conditional.test, site);
                }
                if self.abrupt(&conditional.test) {
                    return false;
                }
                let truth = Self::truth(&conditional.test);
                if contains(conditional.consequent.span(), site) {
                    truth != Some(false) && self.reach_expr(&conditional.consequent, site)
                } else {
                    contains(conditional.alternate.span(), site)
                        && truth != Some(true)
                        && self.reach_expr(&conditional.alternate, site)
                }
            }
            Expression::ArrayExpression(array) => {
                for (index, element) in array.elements.iter().enumerate() {
                    let Some(item) = element.as_expression() else {
                        if matches!(element, ArrayExpressionElement::Elision(_)) {
                            continue;
                        }
                        return false;
                    };
                    if contains(item.span(), site) {
                        return array.elements[..index].iter().all(|earlier| {
                            earlier
                                .as_expression()
                                .is_none_or(|earlier| !self.abrupt(earlier))
                        }) && self.reach_expr(item, site);
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn reach_call(&self, callee: &Expression<'_>, arguments: &[Argument<'_>], site: Span) -> bool {
        if contains(callee.span(), site) {
            return self.reach_expr(callee, site);
        }
        if self.abrupt(callee) {
            return false;
        }
        for argument in arguments {
            let Some(expression) = argument.as_expression() else {
                // A spread shifts and evaluates arbitrary code.
                return false;
            };
            if contains(expression.span(), site) {
                return self.reach_expr(expression, site);
            }
            if self.abrupt(expression) {
                return false;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reach(source: &str, marker: &str, diverging_marker: &[&str]) -> Option<bool> {
        let marker_at = source.find(marker).expect("marker") as u32;
        let site = Span::new(marker_at, marker_at + marker.len() as u32);
        let diverging = diverging_marker
            .iter()
            .map(|text| {
                let at = source.find(text).expect("diverging") as u32;
                Span::new(at, at + text.len() as u32)
            })
            .collect::<Vec<_>>();
        // The function under test is the first arrow in the source.
        let start = source.find("(").map(|outer| outer + 1).unwrap() as u32;
        let end = (source.len() - 1) as u32;
        site_reachable(
            Path::new("a.ts"),
            source,
            Span::new(start, end),
            site,
            &diverging,
        )
    }

    fn arrow(body: &str) -> String {
        format!("(() => {{ {body} }})")
    }

    #[test]
    fn abrupt_completion_cuts_the_rest_of_a_block() {
        for (body, expected) in [
            ("hit();", true),
            ("if (x()) return; hit();", true),
            ("if (true) return; hit();", false),
            ("return; hit();", false),
            ("throw 1; hit();", false),
            ("try { return; } finally { } hit();", false),
            ("try { return; } catch {} hit();", false),
            ("try { throw 1; } catch {} hit();", false),
            ("try { x(); } catch {} hit();", true),
            ("try { return; } catch { hit(); }", false),
            ("try { x(); } finally { hit(); }", true),
            ("try { x(); } finally { return; } hit();", false),
            ("for (;;) {} hit();", false),
            ("while (true) { if (x()) break; } hit();", true),
            ("while (1) {} hit();", false),
            ("for (const a of b) { return; } hit();", true),
            ("switch (1) { default: return; } hit();", false),
            (
                "switch (x) { case 1: return; default: return; } hit();",
                false,
            ),
            ("switch (x) { case 1: return; } hit();", true),
            ("switch (x) { default: break; } hit();", true),
            ("a: { break a; } hit();", true),
            ("{ return; } hit();", false),
            ("if (x()) { return; } else { throw 1; } hit();", false),
            ("do { return; } while (x()); hit();", false),
            ("while (false) { hit(); }", false),
            ("if (false) { hit(); }", false),
            ("if (x()) { hit(); }", true),
            ("try { hit(); } catch {}", true),
            ("false && hit();", false),
            ("true || hit();", false),
            ("true && hit();", true),
            ("x() && hit();", true),
            ("x() ?? hit();", true),
            ("null ?? hit();", true),
            ("1 ?? hit();", false),
            ("true ? 1 : hit();", false),
            ("false ? 1 : hit();", true),
            ("!0 ? hit() : 1;", true),
            ("typeof 1 === 'string' ? hit() : 1;", false),
            ("typeof 1 === 'number' ? hit() : 1;", true),
            ("x() ? hit() : 1;", true),
            ("(1, hit());", true),
            ("switch (1) { case 1: break; default: hit(); }", false),
            ("switch (x()) { case 1: break; default: hit(); }", true),
            ("for (const v of [] as number[]) { hit(); }", false),
            ("for (const v of x()) { hit(); }", true),
            ("for (const v in {}) { hit(); }", false),
            ("do { hit(); } while (false);", true),
            ("for (let i = 0; false; ) { hit(); }", false),
            ("for (let i = 0; i < 1; i++) { hit(); }", true),
            ("with (x) { hit(); }", false),
            ("const a = 1, b = hit();", true),
            (
                "const { value } = { get value(): number { throw 1; } }; hit();",
                false,
            ),
            (
                "const o = { get value(): number { throw 1; } }; o.value; hit();",
                false,
            ),
            (
                "const o = { get value(): number { return 1; } }; o.value; hit();",
                true,
            ),
            (
                "const o = { get value(): number { throw 1; } }; o.other; hit();",
                true,
            ),
            (
                "const o = { get value(): number { throw 1; } }; o[k()]; hit();",
                false,
            ),
        ] {
            let source = arrow(body);
            assert_eq!(reach(&source, "hit()", &[]), Some(expected), "{body}");
        }
    }

    #[test]
    fn module_sites_follow_the_same_rules() {
        let at = |source: &str| {
            let start = source.find("hit()").unwrap() as u32;
            module_site_reachable(Path::new("a.ts"), source, Span::new(start, start + 5), &[])
        };
        assert_eq!(at("hit();"), Some(true));
        assert_eq!(at("if (false) hit();"), Some(false));
        assert_eq!(at("if (x()) hit();"), Some(true));
        assert_eq!(at("export const a = hit();"), Some(true));
    }

    #[test]
    fn a_diverging_call_cuts_the_rest_of_a_block() {
        let source = arrow("never(); hit();");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(false));
        assert_eq!(reach(&source, "hit()", &[]), Some(true));
        let source = arrow("const v = never(); hit();");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(false));
        let source = arrow("x() && never(); hit();");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(true));
        let source = arrow("(never(), hit());");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(false));
        let source = arrow("for (let i = never();;) { hit(); }");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(false));
        let source = arrow("while (never()) { hit(); }");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(false));
        let source = arrow("true && never(); hit();");
        assert_eq!(reach(&source, "hit()", &["never()"]), Some(false));
    }

    #[test]
    fn a_function_that_only_throws_may_not_return() {
        let may = |source: &str, diverging: &[&str]| {
            let start = source.find("(").map(|outer| outer + 1).unwrap() as u32;
            let end = (source.len() - 1) as u32;
            let diverging = diverging
                .iter()
                .map(|text| {
                    let at = source.find(text).unwrap() as u32;
                    Span::new(at, at + text.len() as u32)
                })
                .collect::<Vec<_>>();
            function_may_return(Path::new("a.ts"), source, Span::new(start, end), &diverging)
        };
        assert_eq!(may("(() => { throw new Error(); })", &[]), Some(false));
        assert_eq!(may("(() => { for (;;) {} })", &[]), Some(false));
        assert_eq!(may("(() => { x(); })", &[]), Some(true));
        assert_eq!(may("(() => { return 1; })", &[]), Some(true));
        assert_eq!(
            may("(() => { if (x()) throw 1; return 1; })", &[]),
            Some(true)
        );
        assert_eq!(may("(() => { return fail(); })", &["fail()"]), Some(false));
        assert_eq!(may("(() => fail())", &["fail()"]), Some(false));
        assert_eq!(may("(() => 1)", &[]), Some(true));
        assert_eq!(may("(async () => { throw 1; })", &[]), Some(true));
        assert_eq!(
            may("(() => { try { throw 1; } catch {} })", &[]),
            Some(false)
        );
        assert_eq!(
            may(
                "(() => { const o = { get v(): number { throw 1; } }; return o.v; })",
                &[]
            ),
            Some(false)
        );
    }
}
