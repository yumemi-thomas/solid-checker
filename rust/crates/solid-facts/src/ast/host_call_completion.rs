//! Bounded synchronous completion with an explicit allowlist. Unsupported forms
//! withhold authority; ordinary dispatch retains ADR 0270 A. Exact host constants
//! are instantiated downstream.

use oxc_ast::AstKind;
use oxc_ast::ast::{BindingPattern, Expression, FormalParameters, FunctionBody, Statement};
use oxc_semantic::Semantic;
use oxc_span::GetSpan;
use std::collections::{BTreeMap, HashMap, HashSet};

use super::host_execution::{HostExecutionPredicate as Predicate, all, any, inner, span, test};
use crate::core::Span;

const CALL_DEPTH: usize = 4;
const NODE_BUDGET: usize = 256;
const BODY_BYTES: u32 = 8192;
const EXPRESSION_DEPTH: usize = 64;
pub(super) const FILE_NODE_BUDGET: usize = 32_768;

// One exact input environment per span. A different specialization is refused,
// rather than rewalking a body once for every call site. Unknown input choices
// never borrow a cached literal-specific non-return proof.
type Memo = HashMap<Span, (Inputs, Predicate)>;

/// Fallthrough and function return are separate: a return in an early branch
/// must survive a later throw. Unsupported control may contain such a return.
struct Flow {
    next: Predicate,
    returns: Predicate,
}

impl Flow {
    fn next(next: Predicate) -> Self {
        Self {
            next,
            returns: Predicate::Dead,
        }
    }

    fn unproved() -> Self {
        Self {
            next: Predicate::Unknown,
            returns: Predicate::Unknown,
        }
    }
}

pub(super) struct State {
    remaining: usize,
    stack: Vec<Span>,
    body_starts: Vec<usize>,
    refusals: usize,
    body_refusals: usize,
    depth: usize,
    expressions: Memo,
    assignments: Memo,
    bodies: Memo,
    // Cache even write checks: scanning 500 references at 500 call sites is
    // quadratic. The binder-selected declaration is the key, never its name.
    writes: HashMap<Span, bool>,
    statements: HashSet<Span>,
    pub(super) visits: usize,
    pub(super) body_visits: usize,
}

impl State {
    pub(super) fn exhausted(&self) -> bool {
        self.remaining == 0
    }

    pub(super) fn new() -> Self {
        Self {
            remaining: FILE_NODE_BUDGET,
            stack: Vec::new(),
            body_starts: Vec::new(),
            refusals: 0,
            body_refusals: 0,
            depth: 0,
            expressions: HashMap::new(),
            assignments: HashMap::new(),
            bodies: HashMap::new(),
            writes: HashMap::new(),
            statements: HashSet::new(),
            visits: 0,
            body_visits: 0,
        }
    }

    fn refuse(&mut self) {
        self.refusals = self.refusals.saturating_add(1);
    }

    fn visit(&mut self) -> bool {
        if self.remaining == 0
            || self.depth >= EXPRESSION_DEPTH
            || self
                .body_starts
                .iter()
                .any(|start| self.visits - start >= NODE_BUDGET)
        {
            self.refuse();
            return false;
        }
        self.remaining -= 1;
        self.visits += 1;
        true
    }

    fn written(&mut self, semantic: &Semantic<'_>, symbol: oxc_syntax::symbol::SymbolId) -> bool {
        let extent = span(semantic.scoping().symbol_span(symbol));
        *self.writes.entry(extent).or_insert_with(|| {
            semantic
                .scoping()
                .get_resolved_references(symbol)
                .any(|reference| reference.is_write())
                || !semantic.scoping().symbol_redeclarations(symbol).is_empty()
        })
    }
}

type Inputs = BTreeMap<Span, bool>;

// A feasible normal arm is not proof that an unproved competing arm completes.
// Dead alternatives have already been removed by `all`.
fn completion_choice(items: Vec<Predicate>) -> Predicate {
    fn unproved(item: &Predicate) -> bool {
        match item {
            Predicate::Unknown => true,
            Predicate::All(items) | Predicate::Any(items) => items.iter().any(unproved),
            Predicate::CallCompletion(item) => unproved(item),
            _ => false,
        }
    }
    if items.iter().any(unproved) {
        Predicate::Unknown
    } else {
        any(items)
    }
}

fn literal(expression: &Expression<'_>) -> Option<bool> {
    match inner(expression)? {
        Expression::BooleanLiteral(value) => Some(value.value),
        Expression::NumericLiteral(value) => Some(value.value != 0.0 && !value.value.is_nan()),
        Expression::StringLiteral(value) => Some(!value.value.is_empty()),
        Expression::NullLiteral(_) => Some(false),
        _ => None,
    }
}

fn guard<'a>(
    semantic: &Semantic<'a>,
    expression: &Expression<'a>,
    mut expected: bool,
    inputs: &Inputs,
) -> Predicate {
    let Some(mut expression) = inner(expression) else {
        return Predicate::Live;
    };
    for _ in 0..32 {
        let Expression::UnaryExpression(unary) = expression else {
            break;
        };
        if unary.operator != oxc_ast::ast::UnaryOperator::LogicalNot {
            break;
        }
        let Some(argument) = inner(&unary.argument) else {
            return Predicate::Live;
        };
        expression = argument;
        expected = !expected;
    }
    if matches!(expression, Expression::UnaryExpression(unary) if unary.operator == oxc_ast::ast::UnaryOperator::LogicalNot)
    {
        return Predicate::Live;
    }
    let input = literal(expression).or_else(|| {
        let Expression::Identifier(reference) = expression else {
            return None;
        };
        let symbol = reference
            .reference_id
            .get()
            .and_then(|id| semantic.scoping().get_reference(id).symbol_id())?;
        inputs
            .get(&span(semantic.scoping().symbol_span(symbol)))
            .copied()
    });
    if let Some(value) = input {
        return if value == expected {
            Predicate::Live
        } else {
            Predicate::Dead
        };
    }
    // Unknown branch choices can take either arm. This is an overapproximation
    // of normal returns, rather than the positive site-execution predicate.
    match test(semantic, expression, expected) {
        Predicate::Unknown => Predicate::Live,
        predicate => predicate,
    }
}

fn sequence<'a>(
    semantic: &Semantic<'a>,
    statements: &[Statement<'a>],
    inputs: &Inputs,
    state: &mut State,
) -> Flow {
    let mut result = Flow::next(Predicate::Live);
    let start = state.visits;
    for statement in statements {
        if state.visits - start >= NODE_BUDGET || state.remaining == 0 {
            state.refuse();
            return Flow::unproved();
        }
        let item = statement_flow(semantic, statement, inputs, state);
        result.returns = completion_choice(vec![
            result.returns,
            all(vec![result.next.clone(), item.returns]),
        ]);
        result.next = all(vec![result.next, item.next]);
    }
    result
}

fn statement_flow<'a>(
    semantic: &Semantic<'a>,
    statement: &Statement<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Flow {
    let extent = span(statement.span());
    // A second input environment may reach a statement via another expression
    // body. It cannot reuse an input-specific result or trigger another walk.
    if !state.statements.insert(extent) || !state.visit() {
        state.refuse();
        return Flow::unproved();
    }
    state.depth += 1;
    let result = statement_flow_inner(semantic, statement, inputs, state);
    state.depth -= 1;
    result
}

fn statement_flow_inner<'a>(
    semantic: &Semantic<'a>,
    statement: &Statement<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Flow {
    if matches!(statement, Statement::VariableDeclaration(item) if item.declarations.len() > NODE_BUDGET)
    {
        state.refuse();
        return Flow::unproved();
    }
    match statement {
        Statement::ThrowStatement(_) => Flow::next(Predicate::Dead),
        Statement::ReturnStatement(statement) => Flow {
            next: Predicate::Dead,
            returns: statement
                .argument
                .as_ref()
                .map_or(Predicate::Live, |value| {
                    expression_flow(semantic, value, inputs, state)
                }),
        },
        Statement::ExpressionStatement(statement) => Flow::next(expression_flow(
            semantic,
            &statement.expression,
            inputs,
            state,
        )),
        Statement::VariableDeclaration(declaration) => Flow::next(variable_completion_with_inputs(
            semantic,
            declaration,
            inputs,
            state,
        )),
        Statement::BlockStatement(block) => sequence(semantic, &block.body, inputs, state),
        Statement::IfStatement(statement) => {
            let yes = statement_flow(semantic, &statement.consequent, inputs, state);
            let no = statement.alternate.as_ref().map_or_else(
                || Flow::next(Predicate::Live),
                |item| statement_flow(semantic, item, inputs, state),
            );
            let select = |yes, no| {
                completion_choice(vec![
                    all(vec![guard(semantic, &statement.test, true, inputs), yes]),
                    all(vec![guard(semantic, &statement.test, false, inputs), no]),
                ])
            };
            let evaluated = expression_flow(semantic, &statement.test, inputs, state);
            Flow {
                next: all(vec![evaluated.clone(), select(yes.next, no.next)]),
                returns: all(vec![evaluated, select(yes.returns, no.returns)]),
            }
        }
        Statement::WhileStatement(statement) => {
            if guard(semantic, &statement.test, true, inputs) == Predicate::Live
                && guard(semantic, &statement.test, false, inputs) == Predicate::Live
            {
                return Flow::unproved();
            }
            let body = statement_flow(semantic, &statement.body, inputs, state);
            let evaluated = expression_flow(semantic, &statement.test, inputs, state);
            Flow {
                next: all(vec![
                    evaluated.clone(),
                    guard(semantic, &statement.test, false, inputs),
                ]),
                returns: all(vec![
                    evaluated,
                    guard(semantic, &statement.test, true, inputs),
                    body.returns,
                ]),
            }
        }
        Statement::EmptyStatement(_)
        | Statement::FunctionDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_) => Flow::next(Predicate::Live),
        _ => Flow::unproved(),
    }
}

fn variable_completion_with_inputs<'a>(
    semantic: &Semantic<'a>,
    declaration: &oxc_ast::ast::VariableDeclaration<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    use oxc_ast::ast::VariableDeclarationKind;
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return Predicate::Unknown;
    }
    all(declaration
        .declarations
        .iter()
        .map(|item| declarator_completion_with_inputs(semantic, item, inputs, state))
        .collect())
}

fn declarator_completion_with_inputs<'a>(
    semantic: &Semantic<'a>,
    item: &oxc_ast::ast::VariableDeclarator<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    let evaluated = item.init.as_ref().map_or(Predicate::Live, |value| {
        expression_flow(semantic, value, inputs, state)
    });
    all(vec![
        evaluated,
        if matches!(item.id, BindingPattern::BindingIdentifier(_)) {
            Predicate::Live
        } else {
            Predicate::Unknown
        },
    ])
}

pub(super) fn declarator_completion<'a>(
    semantic: &Semantic<'a>,
    item: &oxc_ast::ast::VariableDeclarator<'a>,
    state: &mut State,
) -> Predicate {
    declarator_completion_with_inputs(semantic, item, &Inputs::new(), state)
}

pub(super) fn variable_completion<'a>(
    semantic: &Semantic<'a>,
    declaration: &oxc_ast::ast::VariableDeclaration<'a>,
    state: &mut State,
) -> Predicate {
    variable_completion_with_inputs(semantic, declaration, &Inputs::new(), state)
}

#[allow(clippy::too_many_arguments)]
fn body_flow<'a>(
    semantic: &Semantic<'a>,
    parameters: &FormalParameters<'a>,
    body: &FunctionBody<'a>,
    expression: bool,
    call: &oxc_ast::ast::CallExpression<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    let extent = span(body.span);
    if state.stack.len() == CALL_DEPTH
        || body.span.size() > BODY_BYTES
        || state.stack.contains(&extent)
    {
        state.body_refusals = state.body_refusals.saturating_add(1);
        return Predicate::Unknown;
    }
    if parameters.items.len() > 32
        || parameters.rest.is_some()
        || parameters.items.iter().any(|parameter| {
            parameter.initializer.is_some()
                || !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
        })
    {
        return Predicate::Unknown;
    }
    let mut bound = Inputs::new();
    for (index, parameter) in parameters.items.iter().enumerate() {
        let BindingPattern::BindingIdentifier(binding) = &parameter.pattern else {
            return Predicate::Unknown;
        };
        let Some(symbol) = binding.symbol_id.get() else {
            return Predicate::Unknown;
        };
        if state.written(semantic, symbol) {
            return Predicate::Unknown;
        }
        let referenced = semantic
            .scoping()
            .get_resolved_references(symbol)
            .next()
            .is_some();
        if !referenced {
            continue;
        }
        // Only literals (or already instantiated, unwritten literal parameters)
        // are substituted. Other simple inputs retain unknown branch choices;
        // a parameter used only outside control cannot manufacture a return.
        // Defaults/destructuring/rest are refused before this body analysis.
        let value = call
            .arguments
            .get(index)
            .and_then(|argument| argument.as_expression())
            .and_then(|value| {
                literal(value).or_else(|| {
                    let Some(Expression::Identifier(reference)) = inner(value) else {
                        return None;
                    };
                    let symbol = reference
                        .reference_id
                        .get()
                        .and_then(|id| semantic.scoping().get_reference(id).symbol_id())?;
                    inputs
                        .get(&span(semantic.scoping().symbol_span(symbol)))
                        .copied()
                })
            });
        if let Some(value) = value {
            bound.insert(span(binding.span), value);
        }
    }
    if let Some((cached_inputs, result)) = state.bodies.get(&extent) {
        return if *cached_inputs == bound {
            result.clone()
        } else {
            Predicate::Unknown
        };
    }
    state.body_visits += 1;
    let start = state.visits;
    let refusals = state.refusals;
    let body_refusals = state.body_refusals;
    state.stack.push(extent);
    state.body_starts.push(start);
    let result = if expression {
        match body.statements.first() {
            Some(Statement::ExpressionStatement(statement)) => {
                expression_flow(semantic, &statement.expression, &bound, state)
            }
            _ => Predicate::Unknown,
        }
    } else {
        let flow = sequence(semantic, &body.statements, &bound, state);
        completion_choice(vec![flow.next, flow.returns])
    };
    state.stack.pop();
    state.body_starts.pop();
    // The entire body becomes unproved if a branch (possibly containing an
    // early return) was truncated. Keeping a later throw would be unsound.
    let result = if state.refusals != refusals
        || state.body_refusals != body_refusals
        || state.visits - start > NODE_BUDGET
    {
        // Truncation cannot prove either normal return or certain exit.
        // Isolate the walk budget while retaining an unproved body result.
        state.refusals = refusals;
        state.body_refusals = state.body_refusals.saturating_add(1);
        Predicate::Unknown
    } else {
        result
    };
    state.bodies.insert(extent, (bound, result.clone()));
    result
}

fn invocation<'a>(
    semantic: &Semantic<'a>,
    call: &oxc_ast::ast::CallExpression<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    if call.optional {
        return Predicate::Unknown;
    }
    let Some(mut callee) = inner(&call.callee) else {
        return Predicate::Unknown;
    };
    if let Expression::Identifier(reference) = callee {
        let scoping = semantic.scoping();
        let Some(symbol) = reference
            .reference_id
            .get()
            .and_then(|id| scoping.get_reference(id).symbol_id())
        else {
            return Predicate::Live;
        };
        if state.written(semantic, symbol) {
            return Predicate::Live;
        }
        let declaration = scoping.symbol_span(symbol);
        match semantic.symbol_declaration(symbol).kind() {
            AstKind::VariableDeclarator(variable) if matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.span == declaration) =>
            {
                let Some(value) = variable.init.as_ref() else {
                    return Predicate::Live;
                };
                let Some(value) = inner(value) else {
                    return Predicate::Live;
                };
                callee = value;
            }
            AstKind::Function(function) if function.generator => {
                return generator_allocation(&function.params);
            }
            AstKind::Function(function) if function.r#async => {
                return function.body.as_ref().map_or(Predicate::Unknown, |body| {
                    async_completion(semantic, &function.params, body, false, state)
                });
            }
            AstKind::Function(function)
                if function
                    .id
                    .as_ref()
                    .is_some_and(|id| id.span == declaration)
                    && !function.r#async
                    && !function.generator =>
            {
                return function.body.as_ref().map_or(Predicate::Live, |body| {
                    body_flow(semantic, &function.params, body, false, call, inputs, state)
                });
            }
            _ => return Predicate::Live,
        }
    }
    match callee {
        Expression::FunctionExpression(function) if function.generator => {
            generator_allocation(&function.params)
        }
        Expression::FunctionExpression(function) if function.r#async => {
            function.body.as_ref().map_or(Predicate::Unknown, |body| {
                async_completion(semantic, &function.params, body, false, state)
            })
        }
        Expression::ArrowFunctionExpression(function) if function.r#async => async_completion(
            semantic,
            &function.params,
            &function.body,
            function.expression,
            state,
        ),
        Expression::FunctionExpression(function) if !function.r#async && !function.generator => {
            function.body.as_ref().map_or(Predicate::Live, |body| {
                body_flow(semantic, &function.params, body, false, call, inputs, state)
            })
        }
        Expression::ArrowFunctionExpression(function) if !function.r#async => body_flow(
            semantic,
            &function.params,
            &function.body,
            function.expression,
            call,
            inputs,
            state,
        ),
        // Ordinary opaque dispatch retains premise A. This cannot override
        // the separate required proof for eager callee/argument evaluation.
        _ => Predicate::Live,
    }
}

fn async_completion<'a>(
    semantic: &Semantic<'a>,
    parameters: &FormalParameters<'a>,
    body: &FunctionBody<'a>,
    expression: bool,
    state: &mut State,
) -> Predicate {
    // An async function executes synchronously until suspension/termination.
    // Its return type is not proof that this prefix returns. Only simple empty,
    // literal-return and proved throw bodies establish promise allocation here;
    // in particular unknown return values may have a non-returning then getter.
    let extent = span(body.span);
    if let Some((_, cached)) = state.bodies.get(&extent) {
        return cached.clone();
    }
    if body.span.size() > BODY_BYTES
        || body.statements.len() > NODE_BUDGET
        || parameters.items.len() > 32
        || generator_allocation(parameters) != Predicate::Live
    {
        return Predicate::Unknown;
    }
    state
        .bodies
        .insert(extent, (Inputs::new(), Predicate::Unknown));
    state.body_visits += 1;
    let mut result = Predicate::Live;
    for statement in &body.statements {
        if !state.visit() {
            result = Predicate::Unknown;
            break;
        }
        match statement {
            Statement::EmptyStatement(_)
            | Statement::FunctionDeclaration(_)
            | Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_) => {}
            Statement::ThrowStatement(item) => {
                result = if expression_flow(semantic, &item.argument, &Inputs::new(), state)
                    == Predicate::Live
                {
                    Predicate::Live
                } else {
                    Predicate::Unknown
                };
                break;
            }
            Statement::ReturnStatement(item) => {
                result = item
                    .argument
                    .as_ref()
                    .map_or(Predicate::Live, primitive_conversion);
                break;
            }
            Statement::ExpressionStatement(item) if expression => {
                result = primitive_conversion(&item.expression);
                break;
            }
            _ => {
                result = Predicate::Unknown;
                break;
            }
        }
    }
    state.bodies.insert(extent, (Inputs::new(), result.clone()));
    result
}

fn generator_allocation(parameters: &FormalParameters<'_>) -> Predicate {
    // Generator bodies are deferred, but defaults/destructuring run at call
    // time and can prevent allocation from completing.
    if parameters.rest.is_none()
        && parameters.items.iter().all(|parameter| {
            parameter.initializer.is_none()
                && matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
        })
    {
        Predicate::Live
    } else {
        Predicate::Unknown
    }
}

fn expression_flow<'a>(
    semantic: &Semantic<'a>,
    expression: &Expression<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    let extent = span(expression.span());
    if let Some((cached_inputs, result)) = state.expressions.get(&extent) {
        return if cached_inputs == inputs {
            result.clone()
        } else {
            Predicate::Unknown
        };
    }
    if !state.visit() {
        return Predicate::Unknown;
    }
    let refusals = state.refusals;
    state.depth += 1;
    let result = expression_flow_inner(semantic, expression, inputs, state);
    state.depth -= 1;
    let result = if state.refusals != refusals {
        Predicate::Unknown
    } else {
        result
    };
    state
        .expressions
        .insert(extent, (inputs.clone(), result.clone()));
    result
}

fn expression_flow_inner<'a>(
    semantic: &Semantic<'a>,
    expression: &Expression<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    let Some(expression) = inner(expression) else {
        state.refuse();
        return Predicate::Unknown;
    };
    let width = match expression {
        Expression::CallExpression(call) => call.arguments.len(),
        Expression::NewExpression(call) => call.arguments.len(),
        Expression::SequenceExpression(sequence) => sequence.expressions.len(),
        Expression::ArrayExpression(array) => array.elements.len(),
        Expression::ObjectExpression(object) => object.properties.len(),
        Expression::TemplateLiteral(template) => template.expressions.len(),
        Expression::TaggedTemplateExpression(template) => template.quasi.expressions.len(),
        _ => 0,
    };
    if width > NODE_BUDGET {
        state.refuse();
        return Predicate::Unknown;
    }
    match expression {
        Expression::BooleanLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::MetaProperty(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_) => Predicate::Live,
        Expression::CallExpression(call) if !call.optional => {
            let mut evaluated = vec![expression_flow(semantic, &call.callee, inputs, state)];
            for argument in &call.arguments {
                evaluated.push(match argument {
                    oxc_ast::ast::Argument::SpreadElement(spread) => all(vec![
                        expression_flow(semantic, &spread.argument, inputs, state),
                        Predicate::Unknown,
                    ]),
                    _ => argument
                        .as_expression()
                        .map_or(Predicate::Unknown, |value| {
                            expression_flow(semantic, value, inputs, state)
                        }),
                });
            }
            evaluated.push(Predicate::CallCompletion(Box::new(invocation(
                semantic, call, inputs, state,
            ))));
            all(evaluated)
        }
        Expression::SequenceExpression(sequence) => all(sequence
            .expressions
            .iter()
            .map(|item| expression_flow(semantic, item, inputs, state))
            .collect()),
        Expression::UnaryExpression(unary) => all(vec![
            expression_flow(semantic, &unary.argument, inputs, state),
            if matches!(
                unary.operator,
                oxc_ast::ast::UnaryOperator::LogicalNot
                    | oxc_ast::ast::UnaryOperator::Void
                    | oxc_ast::ast::UnaryOperator::Typeof
            ) {
                Predicate::Live
            } else {
                Predicate::Unknown
            },
        ]),
        Expression::BinaryExpression(binary) => all(vec![
            expression_flow(semantic, &binary.left, inputs, state),
            expression_flow(semantic, &binary.right, inputs, state),
            if matches!(
                binary.operator,
                oxc_ast::ast::BinaryOperator::StrictEquality
                    | oxc_ast::ast::BinaryOperator::StrictInequality
            ) {
                Predicate::Live
            } else {
                Predicate::Unknown
            },
        ]),
        Expression::LogicalExpression(logical) => {
            let take = match logical.operator {
                oxc_ast::ast::LogicalOperator::And => true,
                oxc_ast::ast::LogicalOperator::Or => false,
                oxc_ast::ast::LogicalOperator::Coalesce => {
                    return all(vec![
                        expression_flow(semantic, &logical.left, inputs, state),
                        Predicate::Unknown,
                    ]);
                }
            };
            all(vec![
                expression_flow(semantic, &logical.left, inputs, state),
                any(vec![
                    test(semantic, &logical.left, !take),
                    all(vec![
                        test(semantic, &logical.left, take),
                        expression_flow(semantic, &logical.right, inputs, state),
                    ]),
                ]),
            ])
        }
        Expression::AssignmentExpression(assignment) => {
            assignment_flow(semantic, assignment, inputs, state)
        }
        Expression::ArrayExpression(array) => all(array
            .elements
            .iter()
            .map(|item| match item {
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => all(vec![
                    expression_flow(semantic, &spread.argument, inputs, state),
                    Predicate::Unknown,
                ]),
                oxc_ast::ast::ArrayExpressionElement::Elision(_) => Predicate::Live,
                _ => item.as_expression().map_or(Predicate::Unknown, |value| {
                    expression_flow(semantic, value, inputs, state)
                }),
            })
            .collect()),
        Expression::ObjectExpression(object) => all(object
            .properties
            .iter()
            .map(|item| match item {
                oxc_ast::ast::ObjectPropertyKind::SpreadProperty(spread) => all(vec![
                    expression_flow(semantic, &spread.argument, inputs, state),
                    Predicate::Unknown,
                ]),
                oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) => all(vec![
                    if property.computed {
                        property
                            .key
                            .as_expression()
                            .map_or(Predicate::Unknown, |value| {
                                all(vec![
                                    expression_flow(semantic, value, inputs, state),
                                    primitive_conversion(value),
                                ])
                            })
                    } else {
                        Predicate::Live
                    },
                    expression_flow(semantic, &property.value, inputs, state),
                ]),
            })
            .collect()),
        Expression::TemplateLiteral(template) => all(template
            .expressions
            .iter()
            .map(|value| {
                all(vec![
                    expression_flow(semantic, value, inputs, state),
                    primitive_conversion(value),
                ])
            })
            .collect()),
        // The exact authenticated Vite constant is not an arbitrary getter.
        Expression::StaticMemberExpression(member) => static_member_completion(member),
        Expression::ConditionalExpression(conditional) => all(vec![
            expression_flow(semantic, &conditional.test, inputs, state),
            any(vec![
                all(vec![
                    test(semantic, &conditional.test, true),
                    expression_flow(semantic, &conditional.consequent, inputs, state),
                ]),
                all(vec![
                    test(semantic, &conditional.test, false),
                    expression_flow(semantic, &conditional.alternate, inputs, state),
                ]),
            ]),
        ]),
        // Completion is an allowlist. New Oxc forms never inherit authority.
        _ => Predicate::Unknown,
    }
}

fn assignment_flow<'a>(
    semantic: &Semantic<'a>,
    assignment: &oxc_ast::ast::AssignmentExpression<'a>,
    inputs: &Inputs,
    state: &mut State,
) -> Predicate {
    let left = match &assignment.left {
        oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(_) => Predicate::Live,
        oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member) => all(vec![
            expression_flow(semantic, &member.object, inputs, state),
            Predicate::Unknown,
        ]),
        oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(member) => all(vec![
            expression_flow(semantic, &member.object, inputs, state),
            expression_flow(semantic, &member.expression, inputs, state),
            Predicate::Unknown,
        ]),
        _ => Predicate::Unknown,
    };
    if assignment.operator.is_logical() {
        // Exact target selection is absent for every logical assignment.
        return all(vec![left, Predicate::Unknown]);
    }
    all(vec![
        left,
        expression_flow(semantic, &assignment.right, inputs, state),
        if assignment.operator == oxc_ast::ast::AssignmentOperator::Assign {
            Predicate::Live
        } else {
            Predicate::Unknown
        },
    ])
}

pub(super) fn assignment_completion<'a>(
    semantic: &Semantic<'a>,
    assignment: &oxc_ast::ast::AssignmentExpression<'a>,
    state: &mut State,
) -> Predicate {
    let extent = span(assignment.span);
    let inputs = Inputs::new();
    if let Some((cached_inputs, result)) = state.assignments.get(&extent) {
        return if *cached_inputs == inputs {
            result.clone()
        } else {
            Predicate::Unknown
        };
    }
    if assignment.span.size() > BODY_BYTES || !state.visit() {
        return Predicate::Unknown;
    }
    let refusals = state.refusals;
    state.depth += 1;
    let result = assignment_flow(semantic, assignment, &inputs, state);
    state.depth -= 1;
    let result = if state.refusals != refusals {
        Predicate::Unknown
    } else {
        result
    };
    state.assignments.insert(extent, (inputs, result.clone()));
    result
}

pub(super) fn expression_completion<'a>(
    semantic: &Semantic<'a>,
    expression: &Expression<'a>,
    state: &mut State,
) -> Predicate {
    if expression.span().size() > BODY_BYTES {
        return Predicate::Unknown;
    }
    expression_flow(semantic, expression, &Inputs::new(), state)
}

pub(super) fn argument_completion<'a>(
    semantic: &Semantic<'a>,
    argument: &oxc_ast::ast::Argument<'a>,
    state: &mut State,
) -> Predicate {
    match argument {
        oxc_ast::ast::Argument::SpreadElement(spread) => all(vec![
            expression_completion(semantic, &spread.argument, state),
            Predicate::Unknown,
        ]),
        _ => argument
            .as_expression()
            .map_or(Predicate::Unknown, |value| {
                expression_completion(semantic, value, state)
            }),
    }
}

// Conversion of other values can invoke user code.
pub(super) fn primitive_conversion(expression: &Expression<'_>) -> Predicate {
    match inner(expression) {
        Some(
            Expression::BooleanLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_),
        ) => Predicate::Live,
        _ => Predicate::Unknown,
    }
}

pub(super) fn static_member_completion(
    member: &oxc_ast::ast::StaticMemberExpression<'_>,
) -> Predicate {
    if !member.optional
        && member.property.name == "SSR"
        && let Some(Expression::StaticMemberExpression(env)) = inner(&member.object)
        && !env.optional
        && env.property.name == "env"
        && let Some(Expression::MetaProperty(meta)) = inner(&env.object)
        && meta.meta.name == "import"
        && meta.property.name == "meta"
    {
        Predicate::Live
    } else {
        Predicate::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_oxc_expression_variant_has_an_explicit_completion_classification() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../../../../fixtures/reactive-ir/inferred-host-spa/completion-variants.json"
        ))
        .unwrap();
        let mut variants = HashSet::new();
        for case in &cases {
            let source = case["source"].as_str().unwrap();
            let allocator = oxc_allocator::Allocator::default();
            let source_type = if case["jsx"] == true {
                oxc_span::SourceType::tsx()
            } else {
                oxc_span::SourceType::ts()
            };
            let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
                .with_options(oxc_parser::ParseOptions {
                    allow_v8_intrinsics: true,
                    ..Default::default()
                })
                .parse();
            assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
            let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
            let mut expression = built.semantic.nodes().iter().find_map(|node| match node.kind() {
                AstKind::VariableDeclarator(item) if matches!(&item.id, BindingPattern::BindingIdentifier(id) if id.name == "sample") => item.init.as_ref(),
                _ => None,
            }).unwrap();
            if case["select"] == "callee" {
                let Expression::CallExpression(call) = expression else {
                    panic!("{source}")
                };
                expression = &call.callee;
            }
            if case["variant"] == "SequenceExpression" {
                expression = inner(expression).unwrap();
            }
            let variant = case["variant"].as_str().unwrap();
            assert!(
                format!("{expression:?}").starts_with(variant),
                "{variant}: {expression:?}"
            );
            assert!(
                variants.insert(format!("{:?}", std::mem::discriminant(expression))),
                "duplicate variant: {variant}"
            );
            assert_eq!(
                expression_completion(&built.semantic, expression, &mut State::new())
                    .evaluate(&|_| None),
                case["normal"].as_bool(),
                "{variant}: {source}"
            );
        }
        // Oxc 0.118: 40 direct + 3 inherited member variants. On an upgrade,
        // audit the catalog again; the production fallback remains Unknown.
        assert_eq!(variants.len(), 43);
    }
}
