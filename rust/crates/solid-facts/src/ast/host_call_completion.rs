//! Bounded synchronous call completion. Unknown means feasible-if-returning,
//! never proof of non-return. Exact host constants are instantiated downstream.

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

    fn maybe_normal() -> Self {
        Self {
            next: Predicate::Live,
            returns: Predicate::Live,
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
            return Flow::maybe_normal();
        }
        let item = statement_flow(semantic, statement, inputs, state);
        result.returns = any(vec![
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
        return Flow::maybe_normal();
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
        return Flow::maybe_normal();
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
        Statement::VariableDeclaration(declaration) => Flow::next(all(declaration
            .declarations
            .iter()
            .map(|item| {
                item.init.as_ref().map_or(Predicate::Live, |value| {
                    expression_flow(semantic, value, inputs, state)
                })
            })
            .collect())),
        Statement::BlockStatement(block) => sequence(semantic, &block.body, inputs, state),
        Statement::IfStatement(statement) => {
            let yes = statement_flow(semantic, &statement.consequent, inputs, state);
            let no = statement.alternate.as_ref().map_or_else(
                || Flow::next(Predicate::Live),
                |item| statement_flow(semantic, item, inputs, state),
            );
            let select = |yes, no| {
                any(vec![
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
            // A stable literal/authenticated host/input guard remains true on
            // every iteration. Unsupported control (including break) supplies
            // a possible normal return, so cannot prove a non-returning loop.
            if guard(semantic, &statement.test, true, inputs) == Predicate::Live
                && guard(semantic, &statement.test, false, inputs) == Predicate::Live
            {
                return Flow::maybe_normal();
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
        Statement::EmptyStatement(_) | Statement::FunctionDeclaration(_) => {
            Flow::next(Predicate::Live)
        }
        _ => Flow::maybe_normal(),
    }
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
        return Predicate::Live;
    }
    if parameters.items.len() > 32
        || parameters.rest.is_some()
        || parameters.items.iter().any(|parameter| {
            parameter.initializer.is_some()
                || !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_))
        })
    {
        return Predicate::Live;
    }
    let mut bound = Inputs::new();
    for (index, parameter) in parameters.items.iter().enumerate() {
        let BindingPattern::BindingIdentifier(binding) = &parameter.pattern else {
            return Predicate::Live;
        };
        let Some(symbol) = binding.symbol_id.get() else {
            return Predicate::Live;
        };
        if state.written(semantic, symbol) {
            return Predicate::Live;
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
            Predicate::Live
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
            _ => Predicate::Live,
        }
    } else {
        let flow = sequence(semantic, &body.statements, &bound, state);
        any(vec![flow.next, flow.returns])
    };
    state.stack.pop();
    state.body_starts.pop();
    // The entire body becomes MaybeNormal if a branch (possibly containing an
    // early return) was truncated. Keeping a later throw would be unsound.
    let result = if state.refusals != refusals
        || state.body_refusals != body_refusals
        || state.visits - start > NODE_BUDGET
    {
        // This is a call-completion refusal, not a positive execution witness.
        // Isolate it from the expression's execution-order budget: ADR A makes
        // the call's continuation feasible, while direct expression overflow
        // still withholds a host witness. Enclosing bodies see the refusal.
        state.refusals = refusals;
        state.body_refusals = state.body_refusals.saturating_add(1);
        Predicate::Live
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
        return Predicate::Live;
    }
    let Some(mut callee) = inner(&call.callee) else {
        return Predicate::Live;
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
        // External/builtin/method/dynamic dispatch, async promise return and
        // generator allocation are all MaybeNormal for the call continuation.
        _ => Predicate::Live,
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
        Expression::CallExpression(call) if !call.optional => {
            let mut evaluated = vec![expression_flow(semantic, &call.callee, inputs, state)];
            for argument in &call.arguments {
                if let oxc_ast::ast::Argument::SpreadElement(spread) = argument {
                    evaluated.push(expression_flow(semantic, &spread.argument, inputs, state));
                } else if let Some(value) = argument.as_expression() {
                    evaluated.push(expression_flow(semantic, value, inputs, state));
                }
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
        Expression::UnaryExpression(unary) => {
            expression_flow(semantic, &unary.argument, inputs, state)
        }
        Expression::BinaryExpression(binary) => all(vec![
            expression_flow(semantic, &binary.left, inputs, state),
            expression_flow(semantic, &binary.right, inputs, state),
        ]),
        Expression::LogicalExpression(logical) => {
            let take = match logical.operator {
                oxc_ast::ast::LogicalOperator::And => true,
                oxc_ast::ast::LogicalOperator::Or => false,
                oxc_ast::ast::LogicalOperator::Coalesce => return Predicate::Live,
            };
            all(vec![
                expression_flow(semantic, &logical.left, inputs, state),
                any(vec![
                    guard(semantic, &logical.left, !take, inputs),
                    all(vec![
                        guard(semantic, &logical.left, take, inputs),
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
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                    expression_flow(semantic, &spread.argument, inputs, state)
                }
                _ => item.as_expression().map_or(Predicate::Live, |value| {
                    expression_flow(semantic, value, inputs, state)
                }),
            })
            .collect()),
        Expression::ObjectExpression(object) => all(object
            .properties
            .iter()
            .map(|item| match item {
                oxc_ast::ast::ObjectPropertyKind::SpreadProperty(spread) => {
                    expression_flow(semantic, &spread.argument, inputs, state)
                }
                oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) => {
                    let key = if property.computed {
                        property
                            .key
                            .as_expression()
                            .map_or(Predicate::Live, |value| {
                                expression_flow(semantic, value, inputs, state)
                            })
                    } else {
                        Predicate::Live
                    };
                    all(vec![
                        key,
                        expression_flow(semantic, &property.value, inputs, state),
                    ])
                }
            })
            .collect()),
        Expression::TemplateLiteral(template) => all(template
            .expressions
            .iter()
            .map(|value| expression_flow(semantic, value, inputs, state))
            .collect()),
        Expression::TaggedTemplateExpression(template) => {
            all(
                std::iter::once(expression_flow(semantic, &template.tag, inputs, state))
                    .chain(
                        template
                            .quasi
                            .expressions
                            .iter()
                            .map(|value| expression_flow(semantic, value, inputs, state)),
                    )
                    .collect(),
            )
        }
        Expression::ImportExpression(import) => all(vec![
            expression_flow(semantic, &import.source, inputs, state),
            import.options.as_ref().map_or(Predicate::Live, |value| {
                expression_flow(semantic, value, inputs, state)
            }),
        ]),
        // Unsupported chain ordering must not lend the enclosing statement a
        // completion premise. Its nested sites already carry an unknown guard.
        Expression::ChainExpression(_) => Predicate::Unknown,
        Expression::StaticMemberExpression(member) => {
            expression_flow(semantic, &member.object, inputs, state)
        }
        Expression::ComputedMemberExpression(member) if !member.optional => all(vec![
            expression_flow(semantic, &member.object, inputs, state),
            expression_flow(semantic, &member.expression, inputs, state),
        ]),
        Expression::NewExpression(call) => {
            all(
                // Constructor bodies are not part of the exact local-call
                // completion census. In particular a local constructor may
                // certainly throw or never return. Eager inputs can execute,
                // but construction supplies no continuation/completion proof.
                std::iter::once(Predicate::Unknown)
                    .chain(std::iter::once(expression_flow(
                        semantic,
                        &call.callee,
                        inputs,
                        state,
                    )))
                    .chain(call.arguments.iter().map(|argument| match argument {
                        oxc_ast::ast::Argument::SpreadElement(spread) => {
                            expression_flow(semantic, &spread.argument, inputs, state)
                        }
                        _ => argument.as_expression().map_or(Predicate::Live, |value| {
                            expression_flow(semantic, value, inputs, state)
                        }),
                    }))
                    .collect(),
            )
        }
        Expression::ClassExpression(_) => Predicate::Unknown,
        Expression::ConditionalExpression(conditional) => all(vec![
            expression_flow(semantic, &conditional.test, inputs, state),
            any(vec![
                all(vec![
                    guard(semantic, &conditional.test, true, inputs),
                    expression_flow(semantic, &conditional.consequent, inputs, state),
                ]),
                all(vec![
                    guard(semantic, &conditional.test, false, inputs),
                    expression_flow(semantic, &conditional.alternate, inputs, state),
                ]),
            ]),
        ]),
        // Allocations are not calls. Unsupported eager/short-circuit forms do
        // not prove non-return; premise A admits the continuation if returning.
        _ => Predicate::Live,
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
        oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member) => {
            expression_flow(semantic, &member.object, inputs, state)
        }
        oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(member) => all(vec![
            expression_flow(semantic, &member.object, inputs, state),
            expression_flow(semantic, &member.expression, inputs, state),
        ]),
        // Destructuring initialization needs an independent completion premise.
        _ => Predicate::Unknown,
    };
    if assignment.operator.is_logical() {
        return left;
    }
    all(vec![
        left,
        expression_flow(semantic, &assignment.right, inputs, state),
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
        oxc_ast::ast::Argument::SpreadElement(spread) => {
            expression_completion(semantic, &spread.argument, state)
        }
        _ => argument.as_expression().map_or(Predicate::Live, |value| {
            expression_completion(semantic, value, state)
        }),
    }
}
