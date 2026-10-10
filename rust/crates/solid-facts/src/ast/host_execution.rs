//! Host-sensitive execution predicates. No package or dialect meaning is guessed.
//! Import constants carry the exact declaration selected by Oxc's binder; the
//! consumer must authenticate their runtime values. Vite SSR is a structural
//! import.meta identity whose value likewise requires a Vite host premise.

use oxc_ast::AstKind;
use oxc_ast::ast::{Expression, LogicalOperator, Statement, UnaryOperator};
use oxc_semantic::{AstNodes, NodeId, Semantic};
use oxc_span::GetSpan;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::host_call_completion::{
    State as CallState, argument_completion, assignment_completion, expression_completion,
};
use crate::core::Span;

// Bounds apply to one whole fact extraction, not independently to each site.
// Predicate trees remain the existing wire format, but have constant maximum
// size; cloning prefixes therefore cannot multiply work or serialized bytes.
const PREDICATE_NODES: usize = 64;
const EXECUTION_STEPS: usize = 131_072;
const EDGE_DEPTH: usize = 64;
const WIDTH: usize = 256;

struct ExtractionState {
    calls: CallState,
    statements: HashMap<Span, HostExecutionPredicate>,
    classes: HashMap<Span, HostExecutionPredicate>,
    prefixes: HashMap<Span, HostExecutionPredicate>,
    composites: HashMap<Span, Option<HostExecutionPredicate>>,
    execution: HashMap<NodeId, (Option<Span>, HostExecutionPredicate)>,
    remaining: usize,
    steps: usize,
}

impl ExtractionState {
    fn new() -> Self {
        Self {
            calls: CallState::new(),
            statements: HashMap::new(),
            classes: HashMap::new(),
            prefixes: HashMap::new(),
            composites: HashMap::new(),
            execution: HashMap::new(),
            remaining: EXECUTION_STEPS,
            steps: 0,
        }
    }

    fn step(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        self.steps += 1;
        true
    }
}

impl HostExecutionPredicate {
    fn nodes(&self) -> usize {
        match self {
            Self::All(items) | Self::Any(items) => 1 + items.iter().map(Self::nodes).sum::<usize>(),
            Self::CallCompletion(item) => 1 + item.nodes(),
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HostConstantIdentity {
    ViteSsr,
    /// Binding-name span, not the local name or a contained reference.
    Import {
        declaration: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum HostExecutionPredicate {
    Live,
    Dead,
    /// Both arms may execute; no positive execution witness.
    Unknown,
    Constant {
        identity: HostConstantIdentity,
        expected: bool,
    },
    All(Vec<Self>),
    Any(Vec<Self>),
    /// A call continuation is feasible if the call returns (ADR 0270 A).
    /// Only a certain synchronous exit can make it dead; missing constants
    /// and bounded/unsupported completion never assert certain non-return.
    CallCompletion(Box<Self>),
}

impl HostExecutionPredicate {
    /// Three-valued evaluation: None is possible execution, never dead code.
    #[must_use]
    pub fn evaluate(&self, value: &impl Fn(&HostConstantIdentity) -> Option<bool>) -> Option<bool> {
        match self {
            Self::Live => Some(true),
            Self::Dead => Some(false),
            Self::Unknown => None,
            Self::CallCompletion(predicate) => Some(predicate.evaluate(value) != Some(false)),
            Self::Constant { identity, expected } => {
                value(identity).map(|value| value == *expected)
            }
            Self::All(items) => {
                let mut unknown = false;
                for item in items {
                    match item.evaluate(value) {
                        Some(false) => return Some(false),
                        None => unknown = true,
                        _ => {}
                    }
                }
                if unknown { None } else { Some(true) }
            }
            Self::Any(items) => {
                let mut unknown = false;
                for item in items {
                    match item.evaluate(value) {
                        Some(true) => return Some(true),
                        None => unknown = true,
                        _ => {}
                    }
                }
                if unknown { None } else { Some(false) }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostExecutionSiteKind {
    Call,
    Jsx,
    Member,
    DynamicImport,
    /// Execution regions allow projection to a diagnostic's subspan. Regions
    /// never supply symbol identity or a guessed callable target.
    Region,
    /// Whole-module normal completion, not eligibility of its live prefix.
    ModuleCompletion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostExecutionFact {
    pub span: Span,
    pub kind: HostExecutionSiteKind,
    /// None is module initialization; otherwise a function or default extent.
    pub scope: Option<Span>,
    pub predicate: HostExecutionPredicate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostDefaultInitializer {
    pub span: Span,
    pub function: Span,
    /// None for a nested destructuring default: omission of the whole
    /// argument does not establish that the nested property is undefined.
    pub argument_index: Option<usize>,
    /// Literal/function allocation only. Its undefined-input premise remains
    /// mandatory; this says nothing about whether the initializer executes.
    pub inert: bool,
}

/// Ordered calls without a task boundary or an effectful intervening statement.
/// This is syntax/order only: consumers authenticate both APIs and handles.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCallSequence {
    pub first: Span,
    pub second: Span,
}

pub(super) fn call_sequences(semantic: &Semantic<'_>) -> Vec<HostCallSequence> {
    fn direct_call(statement: &Statement<'_>) -> Option<Span> {
        let expression = match statement {
            Statement::ExpressionStatement(statement) => &statement.expression,
            Statement::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => {
                declaration.declarations[0].init.as_ref()?
            }
            _ => return None,
        };
        match inner(expression)? {
            Expression::CallExpression(call) if !call.optional => Some(span(call.span)),
            _ => None,
        }
    }
    fn cancellation_call(statement: &Statement<'_>) -> Option<Span> {
        match statement {
            Statement::IfStatement(statement) if statement.alternate.is_none() => {
                cancellation_call(&statement.consequent)
            }
            Statement::BlockStatement(block) if block.body.len() == 1 => {
                cancellation_call(&block.body[0])
            }
            _ => direct_call(statement),
        }
    }
    fn inert(expression: &Expression<'_>) -> bool {
        matches!(
            inner(expression),
            Some(
                Expression::BooleanLiteral(_)
                    | Expression::NumericLiteral(_)
                    | Expression::StringLiteral(_)
                    | Expression::NullLiteral(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
            )
        )
    }
    fn inert_statement(statement: &Statement<'_>) -> bool {
        match statement {
            Statement::EmptyStatement(_) => true,
            Statement::ExpressionStatement(statement) => inert(&statement.expression),
            Statement::VariableDeclaration(declaration) => {
                declaration.declarations.iter().all(|declarator| {
                    matches!(
                        declarator.id,
                        oxc_ast::ast::BindingPattern::BindingIdentifier(_)
                    ) && declarator.init.as_ref().is_some_and(inert)
                })
            }
            _ => false,
        }
    }
    let mut result = Vec::new();
    for node in semantic.nodes().iter() {
        let statements = match node.kind() {
            AstKind::Program(program) => program.body.as_slice(),
            AstKind::FunctionBody(body) => body.statements.as_slice(),
            AstKind::BlockStatement(block) => block.body.as_slice(),
            _ => continue,
        };
        for (index, statement) in statements.iter().enumerate() {
            if let Some(first) = direct_call(statement) {
                for next in &statements[index + 1..] {
                    if let Some(second) = cancellation_call(next) {
                        result.push(HostCallSequence { first, second });
                    }
                    if !inert_statement(next) {
                        break;
                    }
                }
            }
        }
    }
    // Nested immediate cancellation consumes exactly the returned handle.
    for node in semantic.nodes().iter() {
        if let AstKind::CallExpression(call) = node.kind()
            && !call.optional
            && call.arguments.len() == 1
            && let oxc_ast::ast::Argument::CallExpression(first) = &call.arguments[0]
            && !first.optional
        {
            result.push(HostCallSequence {
                first: span(first.span),
                second: span(call.span),
            });
        }
    }
    result.sort_by_key(|pair| (pair.first, pair.second));
    result.dedup();
    result
}

pub(super) fn span(span: oxc_span::Span) -> Span {
    Span::new(span.start, span.end)
}

pub(super) fn empty_and_zero_values(semantic: &Semantic<'_>) -> (Vec<Span>, Vec<Span>) {
    let mut empty = Vec::new();
    let mut zero = Vec::new();
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::ArrayExpression(array) if array.elements.is_empty() => {
                empty.push(span(array.span))
            }
            AstKind::NumericLiteral(literal) if literal.value == 0.0 => {
                zero.push(span(literal.span))
            }
            _ => {}
        }
    }
    empty.sort_unstable();
    zero.sort_unstable();
    (empty, zero)
}

pub(super) fn all(items: Vec<HostExecutionPredicate>) -> HostExecutionPredicate {
    use HostExecutionPredicate::{All, Dead, Live};
    if items.contains(&Dead) {
        return Dead;
    }
    let mut items = items
        .into_iter()
        .filter(|item| *item != Live)
        .collect::<Vec<_>>();
    match items.len() {
        0 => Live,
        1 if items[0].nodes() <= PREDICATE_NODES => items.remove(0),
        1 => HostExecutionPredicate::Unknown,
        _ if 1 + items
            .iter()
            .map(HostExecutionPredicate::nodes)
            .sum::<usize>()
            > PREDICATE_NODES =>
        {
            HostExecutionPredicate::Unknown
        }
        _ => All(items),
    }
}

pub(super) fn any(items: Vec<HostExecutionPredicate>) -> HostExecutionPredicate {
    use HostExecutionPredicate::{Any, Dead, Live};
    if items.contains(&Live) {
        return Live;
    }
    let mut items = items
        .into_iter()
        .filter(|item| *item != Dead)
        .collect::<Vec<_>>();
    match items.len() {
        0 => Dead,
        1 if items[0].nodes() <= PREDICATE_NODES => items.remove(0),
        1 => HostExecutionPredicate::Unknown,
        _ if 1 + items
            .iter()
            .map(HostExecutionPredicate::nodes)
            .sum::<usize>()
            > PREDICATE_NODES =>
        {
            HostExecutionPredicate::Unknown
        }
        _ => Any(items),
    }
}

/// Direct C / !C only. Wrappers preserve the runtime expression. No aliases,
/// boolean comparisons, nested logical tests, DEV/PROD, or computed dispatch.
pub(super) fn inner<'a, 'b>(mut expression: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    for _ in 0..EDGE_DEPTH {
        expression = match expression {
            Expression::ParenthesizedExpression(value) => &value.expression,
            Expression::TSAsExpression(value) => &value.expression,
            Expression::TSSatisfiesExpression(value) => &value.expression,
            Expression::TSInstantiationExpression(value) => &value.expression,
            Expression::TSNonNullExpression(value) => &value.expression,
            Expression::TSTypeAssertion(value) => &value.expression,
            _ => return Some(expression),
        };
    }
    None
}

pub(super) fn test(
    semantic: &Semantic<'_>,
    expression: &Expression<'_>,
    expected: bool,
) -> HostExecutionPredicate {
    let Some(expression) = inner(expression) else {
        return HostExecutionPredicate::Unknown;
    };
    let (expression, expected) = match expression {
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let Some(argument) = inner(&unary.argument) else {
                return HostExecutionPredicate::Unknown;
            };
            (argument, !expected)
        }
        _ => (expression, expected),
    };
    let identity = match expression {
        Expression::BooleanLiteral(literal) => {
            return if literal.value == expected {
                HostExecutionPredicate::Live
            } else {
                HostExecutionPredicate::Dead
            };
        }
        Expression::StaticMemberExpression(member)
            if !member.optional && member.property.name == "SSR" =>
        {
            let Some(Expression::StaticMemberExpression(env)) = inner(&member.object) else {
                return HostExecutionPredicate::Unknown;
            };
            if env.optional || env.property.name != "env" {
                return HostExecutionPredicate::Unknown;
            }
            let Some(Expression::MetaProperty(meta)) = inner(&env.object) else {
                return HostExecutionPredicate::Unknown;
            };
            if meta.meta.name != "import" || meta.property.name != "meta" {
                return HostExecutionPredicate::Unknown;
            }
            HostConstantIdentity::ViteSsr
        }
        Expression::Identifier(reference) => {
            let scoping = semantic.scoping();
            let Some(symbol) = reference
                .reference_id
                .get()
                .and_then(|id| scoping.get_reference(id).symbol_id())
            else {
                return HostExecutionPredicate::Unknown;
            };
            let declaration = scoping.symbol_span(symbol);
            // Exact import declaration, value space only, unwritten. The
            // backend decides which package/export (if any) is a constant.
            let imported = matches!(semantic.symbol_declaration(symbol).kind(),
                AstKind::ImportSpecifier(specifier)
                    if specifier.local.span == declaration
                        && specifier.import_kind == oxc_ast::ast::ImportOrExportKind::Value);
            if !imported
                || scoping.get_resolved_reference_ids(symbol).len() > WIDTH
                || scoping
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
            {
                return HostExecutionPredicate::Unknown;
            }
            HostConstantIdentity::Import {
                declaration: span(declaration),
            }
        }
        _ => return HostExecutionPredicate::Unknown,
    };
    HostExecutionPredicate::Constant { identity, expected }
}

/// Class allocation does not run instance field initializers or methods.
/// Static work, decorators and computed coercion remain separate eager effects.
fn class_completion<'a>(
    semantic: &Semantic<'a>,
    class: &oxc_ast::ast::Class<'a>,
    depth: usize,
    state: &mut ExtractionState,
) -> HostExecutionPredicate {
    use oxc_ast::ast::{ClassElement, PropertyKey};
    let extent = span(class.span);
    if let Some(result) = state.classes.get(&extent) {
        return result.clone();
    }
    // A cache entry before recursion also refuses cycles/TDZ heritage.
    state
        .classes
        .insert(extent, HostExecutionPredicate::Unknown);
    if depth >= EDGE_DEPTH
        || !state.step()
        || !class.decorators.is_empty()
        || class.body.body.len() > WIDTH
    {
        return HostExecutionPredicate::Unknown;
    }
    let inert_key = |key: &PropertyKey<'_>, computed: bool| {
        !computed
            || matches!(
                key.as_expression().and_then(inner),
                Some(Expression::StringLiteral(_) | Expression::NumericLiteral(_))
            )
    };
    for element in &class.body.body {
        if !state.step() {
            return HostExecutionPredicate::Unknown;
        }
        let inert = match element {
            ClassElement::MethodDefinition(item) => {
                item.decorators.is_empty()
                    && inert_key(&item.key, item.computed)
                    && item.value.params.items.len() <= WIDTH
                    && item
                        .value
                        .params
                        .items
                        .iter()
                        .all(|parameter| parameter.decorators.is_empty())
            }
            ClassElement::PropertyDefinition(item) => {
                item.decorators.is_empty()
                    && inert_key(&item.key, item.computed)
                    && (!item.r#static || item.value.is_none())
            }
            ClassElement::AccessorProperty(item) => {
                item.decorators.is_empty()
                    && inert_key(&item.key, item.computed)
                    && (!item.r#static || item.value.is_none())
            }
            ClassElement::StaticBlock(_) => false,
            ClassElement::TSIndexSignature(_) => true,
        };
        if !inert {
            return HostExecutionPredicate::Unknown;
        }
    }
    if let Some(heritage) = &class.super_class {
        let Some(Expression::Identifier(reference)) = inner(heritage) else {
            return HostExecutionPredicate::Unknown;
        };
        let Some(symbol) = reference
            .reference_id
            .get()
            .and_then(|id| semantic.scoping().get_reference(id).symbol_id())
        else {
            return HostExecutionPredicate::Unknown;
        };
        let references = semantic.scoping().get_resolved_reference_ids(symbol);
        if references.len() > WIDTH || !semantic.scoping().symbol_redeclarations(symbol).is_empty()
        {
            return HostExecutionPredicate::Unknown;
        }
        // Even an unwritten function identifier can have a mutated prototype.
        // Admit only a private constructor whose uses are these exact heritage
        // reads; exports, aliases, method calls and member writes are refused.
        for reference in semantic.scoping().get_resolved_references(symbol) {
            if !reference.is_value() || reference.flags().is_value_as_type() {
                continue;
            }
            if reference.is_write() {
                return HostExecutionPredicate::Unknown;
            }
            let node = reference.node_id();
            let at = semantic.nodes().kind(node).span();
            let mut ancestors = semantic.nodes().ancestor_kinds(node);
            let owner = ancestors
                .by_ref()
                .take(EDGE_DEPTH)
                .find_map(|kind| match kind {
                    AstKind::Class(owner) => Some(Some(owner)),
                    AstKind::ParenthesizedExpression(_)
                    | AstKind::TSAsExpression(_)
                    | AstKind::TSTypeAssertion(_)
                    | AstKind::TSNonNullExpression(_)
                    | AstKind::TSSatisfiesExpression(_)
                    | AstKind::TSInstantiationExpression(_) => None,
                    _ => Some(None),
                })
                .flatten();
            let Some(owner) = owner else {
                return HostExecutionPredicate::Unknown;
            };
            if owner
                .super_class
                .as_ref()
                .is_none_or(|value| inner(value).is_none_or(|value| value.span() != at))
            {
                return HostExecutionPredicate::Unknown;
            }
        }
        let declaration = semantic.symbol_declaration(symbol);
        if semantic
            .nodes()
            .ancestor_kinds(declaration.id())
            .next()
            .is_some_and(|parent| {
                matches!(
                    parent,
                    AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_)
                )
            })
        {
            return HostExecutionPredicate::Unknown;
        }
        let proved = match declaration.kind() {
            AstKind::Class(base) => {
                !base.declare
                    && base.span.end < class.span.start
                    && class_completion(semantic, base, depth + 1, state)
                        == HostExecutionPredicate::Live
            }
            AstKind::Function(base) => {
                !base.declare && !base.r#async && !base.generator && base.body.is_some()
            }
            _ => false,
        };
        if !proved {
            return HostExecutionPredicate::Unknown;
        }
    }
    state.classes.insert(extent, HostExecutionPredicate::Live);
    HostExecutionPredicate::Live
}

/// Normal fallthrough, host by host. Deliberately no catch/finally, loop,
/// switch, label or abrupt-completion guess. Only a same-block straight exit
/// or direct-test if can prove that the rest of that block is dead.
fn completion<'a>(
    semantic: &Semantic<'a>,
    suspensions: &[(oxc_span::Span, Option<oxc_span::Span>)],
    statement: &Statement<'a>,
    depth: usize,
    state: &mut ExtractionState,
) -> HostExecutionPredicate {
    let extent = span(statement.span());
    if let Some(result) = state.statements.get(&extent) {
        return result.clone();
    }
    if depth >= EDGE_DEPTH || !state.step() {
        return HostExecutionPredicate::Unknown;
    }
    let result = completion_inner(semantic, suspensions, statement, depth, state);
    state.statements.insert(extent, result.clone());
    result
}

fn completion_inner<'a>(
    semantic: &Semantic<'a>,
    suspensions: &[(oxc_span::Span, Option<oxc_span::Span>)],
    statement: &Statement<'a>,
    depth: usize,
    state: &mut ExtractionState,
) -> HostExecutionPredicate {
    if depth >= EDGE_DEPTH {
        return HostExecutionPredicate::Unknown;
    }
    let width = match statement {
        Statement::VariableDeclaration(item) => item.declarations.len(),
        Statement::ExportNamedDeclaration(item) => match &item.declaration {
            Some(oxc_ast::ast::Declaration::VariableDeclaration(item)) => item.declarations.len(),
            _ => 0,
        },
        _ => 0,
    };
    if width > WIDTH {
        return HostExecutionPredicate::Unknown;
    }
    if may_suspend(suspensions, statement.span()) {
        return HostExecutionPredicate::Unknown;
    }
    match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => {
            HostExecutionPredicate::Dead
        }
        Statement::ExpressionStatement(statement) => {
            expression_completion(semantic, &statement.expression, &mut state.calls)
        }
        Statement::VariableDeclaration(statement) => all(statement
            .declarations
            .iter()
            .filter_map(|declaration| declaration.init.as_ref())
            .map(|value| expression_completion(semantic, value, &mut state.calls))
            .collect()),
        Statement::BlockStatement(block) => {
            let mut result = HostExecutionPredicate::Live;
            for item in &block.body {
                if !state.step() {
                    return HostExecutionPredicate::Unknown;
                }
                result = all(vec![
                    result,
                    completion(semantic, suspensions, item, depth + 1, state),
                ]);
            }
            result
        }
        Statement::ExportNamedDeclaration(export) => match export.declaration.as_ref() {
            Some(oxc_ast::ast::Declaration::VariableDeclaration(declaration)) => all(declaration
                .declarations
                .iter()
                .filter_map(|item| item.init.as_ref())
                .map(|value| expression_completion(semantic, value, &mut state.calls))
                .collect()),
            Some(oxc_ast::ast::Declaration::ClassDeclaration(class)) => {
                class_completion(semantic, class, depth, state)
            }
            Some(oxc_ast::ast::Declaration::TSModuleDeclaration(_)) => {
                HostExecutionPredicate::Unknown
            }
            _ => HostExecutionPredicate::Live,
        },
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                class_completion(semantic, class, depth, state)
            }
            declaration => declaration
                .as_expression()
                .map_or(HostExecutionPredicate::Live, |value| {
                    expression_completion(semantic, value, &mut state.calls)
                }),
        },
        Statement::IfStatement(statement) => {
            let yes = completion(
                semantic,
                suspensions,
                &statement.consequent,
                depth + 1,
                state,
            );
            let no = statement
                .alternate
                .as_ref()
                .map_or(HostExecutionPredicate::Live, |statement| {
                    completion(semantic, suspensions, statement, depth + 1, state)
                });
            let evaluated = expression_completion(semantic, &statement.test, &mut state.calls);
            if yes == no {
                return all(vec![evaluated, yes]);
            }
            all(vec![
                evaluated,
                any(vec![
                    all(vec![test(semantic, &statement.test, true), yes]),
                    all(vec![test(semantic, &statement.test, false), no]),
                ]),
            ])
        }
        Statement::ClassDeclaration(class) => class_completion(semantic, class, depth, state),
        Statement::ForStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::WhileStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::TryStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::LabeledStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::WithStatement(_) => HostExecutionPredicate::Unknown,
        _ => HostExecutionPredicate::Live,
    }
}

/// A continuation cannot inherit its prefix's execution witness. An await can
/// remain pending forever. Nested functions are allocations, not suspension of
/// this expression/statement. Do not guess settlement from a Promise's type.
fn may_suspend(
    suspensions: &[(oxc_span::Span, Option<oxc_span::Span>)],
    extent: oxc_span::Span,
) -> bool {
    let start = suspensions.partition_point(|(span, _)| span.start < extent.start);
    for (index, (span, function)) in suspensions[start..].iter().enumerate() {
        if span.start >= extent.end {
            return false;
        }
        if index == EDGE_DEPTH {
            return true;
        }
        if extent.contains_inclusive(*span)
            && function.is_none_or(|function| !extent.contains_inclusive(function))
        {
            return true;
        }
    }
    false
}

fn preceding<'a>(
    semantic: &Semantic<'a>,
    suspensions: &[(oxc_span::Span, Option<oxc_span::Span>)],
    statements: &[Statement<'a>],
    child: oxc_span::Span,
    state: &mut ExtractionState,
) -> HostExecutionPredicate {
    if let Some(result) = state.prefixes.get(&span(child)) {
        return result.clone();
    }
    let mut prefix = HostExecutionPredicate::Live;
    for statement in statements {
        if !state.step() {
            return HostExecutionPredicate::Unknown;
        }
        state
            .prefixes
            .insert(span(statement.span()), prefix.clone());
        prefix = all(vec![
            prefix,
            completion(semantic, suspensions, statement, 0, state),
        ]);
    }
    state
        .prefixes
        .get(&span(child))
        .cloned()
        .unwrap_or(HostExecutionPredicate::Unknown)
}

fn default_initializer(nodes: &AstNodes<'_>, id: NodeId) -> Option<HostDefaultInitializer> {
    let (initializer, parameter) = match nodes.kind(id) {
        AstKind::FormalParameter(parameter) => {
            (parameter.initializer.as_deref()?, Some(parameter.span))
        }
        AstKind::AssignmentPattern(pattern) => (&pattern.right, None),
        _ => return None,
    };
    for owner in nodes.ancestor_kinds(id).take(EDGE_DEPTH) {
        let (function, parameters) = match owner {
            AstKind::Function(function) => (function.span, &function.params),
            AstKind::ArrowFunctionExpression(function) => (function.span, &function.params),
            // A local destructuring default in a body is not a parameter.
            AstKind::FunctionBody(_) => return None,
            _ => continue,
        };
        return Some(HostDefaultInitializer {
            span: span(initializer.span()),
            function: span(function),
            argument_index: parameter.and_then(|parameter| {
                parameters
                    .items
                    .iter()
                    .position(|item| item.span == parameter)
            }),
            inert: matches!(
                inner(initializer),
                Some(
                    Expression::BooleanLiteral(_)
                        | Expression::NumericLiteral(_)
                        | Expression::StringLiteral(_)
                        | Expression::NullLiteral(_)
                        | Expression::FunctionExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                )
            ),
        });
    }
    None
}

fn composite_completion<'a>(
    semantic: &Semantic<'a>,
    kind: AstKind<'a>,
    state: &mut ExtractionState,
) -> Option<HostExecutionPredicate> {
    let extent = span(kind.span());
    if let Some(result) = state.composites.get(&extent) {
        return result.clone();
    }
    let width = match kind {
        AstKind::ArrayExpression(item) => item.elements.len(),
        AstKind::ObjectExpression(item) => item.properties.len(),
        AstKind::TemplateLiteral(item) => item.expressions.len(),
        AstKind::TaggedTemplateExpression(item) => item.quasi.expressions.len(),
        _ => 0,
    };
    if width > WIDTH || !state.step() {
        return Some(HostExecutionPredicate::Unknown);
    }
    let result = composite_completion_inner(semantic, kind, state);
    state.composites.insert(extent, result.clone());
    result
}

fn composite_completion_inner<'a>(
    semantic: &Semantic<'a>,
    kind: AstKind<'a>,
    state: &mut ExtractionState,
) -> Option<HostExecutionPredicate> {
    match kind {
        AstKind::ArrayExpression(expression) => Some(all(expression
            .elements
            .iter()
            .map(|item| match item {
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                    expression_completion(semantic, &spread.argument, &mut state.calls)
                }
                _ => item
                    .as_expression()
                    .map_or(HostExecutionPredicate::Live, |value| {
                        expression_completion(semantic, value, &mut state.calls)
                    }),
            })
            .collect())),
        AstKind::ObjectExpression(expression) => Some(all(expression
            .properties
            .iter()
            .map(|item| match item {
                oxc_ast::ast::ObjectPropertyKind::SpreadProperty(spread) => {
                    expression_completion(semantic, &spread.argument, &mut state.calls)
                }
                oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) => all(vec![
                    if property.computed {
                        property
                            .key
                            .as_expression()
                            .map_or(HostExecutionPredicate::Live, |value| {
                                expression_completion(semantic, value, &mut state.calls)
                            })
                    } else {
                        HostExecutionPredicate::Live
                    },
                    expression_completion(semantic, &property.value, &mut state.calls),
                ]),
            })
            .collect())),
        AstKind::TemplateLiteral(expression) => Some(all(expression
            .expressions
            .iter()
            .map(|value| expression_completion(semantic, value, &mut state.calls))
            .collect())),
        AstKind::AssignmentExpression(expression) => Some(assignment_completion(
            semantic,
            expression,
            &mut state.calls,
        )),
        AstKind::TaggedTemplateExpression(expression) => Some(all(std::iter::once(
            expression_completion(semantic, &expression.tag, &mut state.calls),
        )
        .chain(
            expression
                .quasi
                .expressions
                .iter()
                .map(|value| expression_completion(semantic, value, &mut state.calls)),
        )
        .collect())),
        _ => None,
    }
}

fn execution<'a>(
    semantic: &Semantic<'a>,
    suspensions: &[(oxc_span::Span, Option<oxc_span::Span>)],
    id: NodeId,
    defaults: &[HostDefaultInitializer],
    state: &mut ExtractionState,
) -> (Option<Span>, HostExecutionPredicate) {
    if let Some(result) = state.execution.get(&id) {
        return result.clone();
    }
    let result = execution_inner(semantic, suspensions, id, defaults, state);
    state.execution.insert(id, result.clone());
    result
}

fn execution_inner<'a>(
    semantic: &Semantic<'a>,
    suspensions: &[(oxc_span::Span, Option<oxc_span::Span>)],
    id: NodeId,
    defaults: &[HostDefaultInitializer],
    state: &mut ExtractionState,
) -> (Option<Span>, HostExecutionPredicate) {
    let nodes = semantic.nodes();
    if !state.step() {
        return (None, HostExecutionPredicate::Unknown);
    }
    let original = span(nodes.kind(id).span());
    if defaults.len() > WIDTH {
        return (None, HostExecutionPredicate::Unknown);
    }
    if let Some(default) = defaults.iter().find(|default| default.span == original) {
        return (Some(default.span), HostExecutionPredicate::Live);
    }
    let mut child = id;
    let mut conditions = Vec::new();
    for (depth, parent) in nodes.ancestor_ids(id).enumerate() {
        if depth >= EDGE_DEPTH || !state.step() || defaults.len() > WIDTH {
            return (None, HostExecutionPredicate::Unknown);
        }
        let child_span = nodes.kind(child).span();
        let is = |candidate: oxc_span::Span| candidate == child_span;
        match nodes.kind(parent) {
            AstKind::Function(function) => {
                let function = span(function.span);
                let scope = defaults
                    .iter()
                    .filter(|default| {
                        default.function == function && default.span.contains(original)
                    })
                    .min_by_key(|default| default.span.end - default.span.start)
                    .map_or(function, |default| default.span);
                return (Some(scope), all(conditions));
            }
            AstKind::ArrowFunctionExpression(function) => {
                let function = span(function.span);
                let scope = defaults
                    .iter()
                    .filter(|default| {
                        default.function == function && default.span.contains(original)
                    })
                    .min_by_key(|default| default.span.end - default.span.start)
                    .map_or(function, |default| default.span);
                return (Some(scope), all(conditions));
            }
            AstKind::Program(program) => conditions.push(preceding(
                semantic,
                suspensions,
                &program.body,
                child_span,
                state,
            )),
            AstKind::FunctionBody(body) => conditions.push(preceding(
                semantic,
                suspensions,
                &body.statements,
                child_span,
                state,
            )),
            AstKind::BlockStatement(block) => conditions.push(preceding(
                semantic,
                suspensions,
                &block.body,
                child_span,
                state,
            )),
            AstKind::IfStatement(statement) => {
                if is(statement.consequent.span()) {
                    conditions.push(test(semantic, &statement.test, true));
                } else if statement
                    .alternate
                    .as_ref()
                    .is_some_and(|alternate| is(alternate.span()))
                {
                    conditions.push(test(semantic, &statement.test, false));
                }
            }
            AstKind::ConditionalExpression(expression) => {
                if is(expression.consequent.span()) {
                    conditions.push(test(semantic, &expression.test, true));
                } else if is(expression.alternate.span()) {
                    conditions.push(test(semantic, &expression.test, false));
                }
            }
            AstKind::LogicalExpression(expression) if is(expression.right.span()) => {
                conditions.push(match expression.operator {
                    LogicalOperator::And => test(semantic, &expression.left, true),
                    LogicalOperator::Or => test(semantic, &expression.left, false),
                    LogicalOperator::Coalesce => HostExecutionPredicate::Unknown,
                });
            }
            AstKind::SequenceExpression(expression) => {
                if expression.expressions.len() > WIDTH {
                    return (None, HostExecutionPredicate::Unknown);
                }
                for item in &expression.expressions {
                    if is(item.span()) {
                        break;
                    }
                    if may_suspend(suspensions, item.span()) {
                        conditions.push(HostExecutionPredicate::Unknown);
                    }
                    conditions.push(expression_completion(semantic, item, &mut state.calls));
                }
            }
            AstKind::VariableDeclaration(declaration) => {
                if declaration.declarations.len() > WIDTH {
                    return (None, HostExecutionPredicate::Unknown);
                }
                for item in &declaration.declarations {
                    if is(item.span) {
                        break;
                    }
                    if may_suspend(suspensions, item.span) {
                        conditions.push(HostExecutionPredicate::Unknown);
                    }
                    if let Some(value) = &item.init {
                        conditions.push(expression_completion(semantic, value, &mut state.calls));
                    }
                }
            }
            AstKind::BinaryExpression(expression) if is(expression.right.span()) => {
                if may_suspend(suspensions, expression.left.span()) {
                    conditions.push(HostExecutionPredicate::Unknown);
                }
                conditions.push(expression_completion(
                    semantic,
                    &expression.left,
                    &mut state.calls,
                ));
            }
            AstKind::ArrayExpression(_)
            | AstKind::ObjectExpression(_)
            | AstKind::TemplateLiteral(_)
            | AstKind::TaggedTemplateExpression(_)
            | AstKind::AssignmentExpression(_) => {
                if let Some(completion) = composite_completion(semantic, nodes.kind(parent), state)
                {
                    conditions.push(completion);
                }
            }
            AstKind::ComputedMemberExpression(member)
                if !member.optional && is(member.expression.span()) =>
            {
                conditions.push(expression_completion(
                    semantic,
                    &member.object,
                    &mut state.calls,
                ));
            }
            AstKind::JSXElement(element)
                if !matches!(&element.opening_element.name,
                    oxc_ast::ast::JSXElementName::Identifier(name)
                        if name.name.chars().next().is_some_and(|c| c.is_ascii_lowercase())) =>
            {
                // Component prop/child expressions can be lazy and never read.
                // Exact callback invocation is modeled by the consumer instead.
                // Stop at each nested function above: this is an evaluation
                // barrier, not a claim that a supplied closure can never run.
                conditions.push(HostExecutionPredicate::Unknown);
            }
            AstKind::AssignmentPattern(pattern) if is(pattern.right.span()) => {
                // Nested destructuring defaults have their own withheld scope.
                if !defaults
                    .iter()
                    .any(|default| default.span == span(pattern.right.span()))
                {
                    conditions.push(HostExecutionPredicate::Unknown);
                }
            }
            AstKind::CallExpression(call) if call.optional && !is(call.callee.span()) => {
                conditions.push(HostExecutionPredicate::Unknown)
            }
            AstKind::CallExpression(call) => {
                if call.arguments.len() > WIDTH {
                    return (None, HostExecutionPredicate::Unknown);
                }
                if call.arguments.iter().any(|argument| {
                    argument.span().end <= child_span.start
                        && may_suspend(suspensions, argument.span())
                }) || (!is(call.callee.span()) && may_suspend(suspensions, call.callee.span()))
                {
                    conditions.push(HostExecutionPredicate::Unknown);
                }
                for argument in &call.arguments {
                    if argument.span().end <= child_span.start {
                        conditions.push(argument_completion(semantic, argument, &mut state.calls));
                    }
                }
                if !is(call.callee.span()) {
                    conditions.push(expression_completion(
                        semantic,
                        &call.callee,
                        &mut state.calls,
                    ));
                }
            }
            AstKind::NewExpression(call) => {
                if call.arguments.len() > WIDTH {
                    return (None, HostExecutionPredicate::Unknown);
                }
                if call.arguments.iter().any(|argument| {
                    argument.span().end <= child_span.start
                        && may_suspend(suspensions, argument.span())
                }) || (!is(call.callee.span()) && may_suspend(suspensions, call.callee.span()))
                {
                    conditions.push(HostExecutionPredicate::Unknown);
                }
                for argument in &call.arguments {
                    if argument.span().end <= child_span.start {
                        conditions.push(argument_completion(semantic, argument, &mut state.calls));
                    }
                }
                if !is(call.callee.span()) {
                    conditions.push(expression_completion(
                        semantic,
                        &call.callee,
                        &mut state.calls,
                    ));
                }
            }
            AstKind::ComputedMemberExpression(member)
                if member.optional && is(member.expression.span()) =>
            {
                conditions.push(HostExecutionPredicate::Unknown)
            }
            AstKind::ChainExpression(_)
            | AstKind::Class(_)
            | AstKind::TSModuleBlock(_)
            | AstKind::TryStatement(_)
            | AstKind::CatchClause(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::SwitchStatement(_)
            | AstKind::WithStatement(_) => conditions.push(HostExecutionPredicate::Unknown),
            _ => {}
        }
        // Include the child-to-parent edge first: the predicate of an if node
        // itself does not include the test controlling its consequent.
        if let Some((scope, prefix)) = state.execution.get(&parent) {
            conditions.push(prefix.clone());
            return (*scope, all(conditions));
        }
        child = parent;
    }
    (None, all(conditions))
}

pub(super) fn undefined_values(semantic: &Semantic<'_>) -> Vec<Span> {
    let mut values = semantic.nodes().iter().filter_map(|node| match node.kind() {
        AstKind::UnaryExpression(expression) if expression.operator == UnaryOperator::Void
            && matches!(inner(&expression.argument), Some(Expression::NumericLiteral(literal)) if literal.value == 0.0) =>
            Some(span(expression.span)),
        _ => None,
    }).collect::<Vec<_>>();
    values.sort_unstable();
    values
}

/// Vite dev injects an ordinary mutable env object. Direct property reads are
/// constants only while that object/meta identity cannot escape or be written.
/// Refuse the file conservatively, including mutations in uncalled helpers;
/// do not guess reflective builtin semantics or perform alias analysis here.
fn opaque_host_environment(semantic: &Semantic<'_>) -> bool {
    let mut writes = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::AssignmentExpression(item) => Some(item.left.span()),
            AstKind::UpdateExpression(item) => Some(item.argument.span()),
            AstKind::UnaryExpression(item) if item.operator == UnaryOperator::Delete => {
                Some(item.argument.span())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    writes.sort_by_key(|extent| extent.start);
    // Prefix maximum end makes containment an O(log n) query, including nested
    // assignment spans. No AST scan per import.meta expression.
    let mut maximum = 0;
    for extent in &mut writes {
        maximum = maximum.max(extent.end);
        extent.end = maximum;
    }
    let written = |extent: oxc_span::Span| {
        let count = writes.partition_point(|write| write.start <= extent.start);
        count != 0 && writes[count - 1].end >= extent.end
    };
    semantic.nodes().iter().any(|node| {
        let AstKind::MetaProperty(meta) = node.kind() else {
            return false;
        };
        if meta.meta.name != "import" || meta.property.name != "meta" {
            return false;
        }
        let mut ancestors = semantic.nodes().ancestor_kinds(node.id());
        let Some(AstKind::StaticMemberExpression(member)) = ancestors.next() else {
            return true;
        };
        if member.optional || written(member.span) {
            return true;
        }
        if member.property.name != "env" {
            // Literal metadata reads cannot expose the meta/env object. The
            // HMR object is not an env constant, and supplies no host edges.
            return !matches!(
                member.property.name.as_str(),
                "url" | "filename" | "dirname" | "main" | "resolve" | "hot"
            );
        }
        let Some(AstKind::StaticMemberExpression(property)) = ancestors.next() else {
            return true;
        };
        // A readonly optional scalar access cannot expose the env object.
        // Its host-test predicate can still be Unknown; clearing the escape
        // veto supplies neither a constant value nor branch execution.
        if !matches!(
            property.property.name.as_str(),
            "SSR" | "DEV" | "PROD" | "MODE" | "BASE_URL"
        ) {
            return true;
        }
        written(property.span)
    })
}

pub(super) fn host_execution(
    semantic: &Semantic<'_>,
) -> (Vec<HostExecutionFact>, Vec<HostDefaultInitializer>) {
    host_execution_with_state(semantic, &mut ExtractionState::new())
}

fn host_execution_with_state(
    semantic: &Semantic<'_>,
    state: &mut ExtractionState,
) -> (Vec<HostExecutionFact>, Vec<HostDefaultInitializer>) {
    let mut suspensions = semantic
        .nodes()
        .iter()
        .filter(|node| {
            matches!(
                node.kind(),
                AstKind::AwaitExpression(_) | AstKind::YieldExpression(_)
            )
        })
        .map(|node| {
            let function = semantic
                .nodes()
                .ancestor_kinds(node.id())
                .take(EDGE_DEPTH)
                .find_map(|ancestor| {
                    matches!(
                        ancestor,
                        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                    )
                    .then(|| ancestor.span())
                });
            (node.kind().span(), function)
        })
        .collect::<Vec<_>>();
    suspensions.sort_by_key(|(span, _)| span.start);
    let mut defaults = semantic
        .nodes()
        .iter()
        .filter_map(|node| default_initializer(semantic.nodes(), node.id()))
        .collect::<Vec<_>>();
    defaults.sort_by_key(|default| default.span);
    let mut facts = Vec::new();
    for node in semantic.nodes().iter() {
        let (scope, mut predicate) = execution(semantic, &suspensions, node.id(), &defaults, state);
        if let AstKind::Program(program) = node.kind() {
            let mut module_completion = HostExecutionPredicate::Live;
            for statement in &program.body {
                if !state.step() {
                    module_completion = HostExecutionPredicate::Unknown;
                    break;
                }
                module_completion = all(vec![
                    module_completion,
                    completion(semantic, &suspensions, statement, 0, state),
                ]);
            }
            facts.push(HostExecutionFact {
                span: span(program.span),
                kind: HostExecutionSiteKind::ModuleCompletion,
                scope: None,
                predicate: module_completion,
            });
        }
        let kind = match node.kind() {
            AstKind::CallExpression(_) | AstKind::NewExpression(_) => HostExecutionSiteKind::Call,
            AstKind::JSXElement(_) => HostExecutionSiteKind::Jsx,
            AstKind::StaticMemberExpression(_)
            | AstKind::ComputedMemberExpression(_)
            | AstKind::PrivateFieldExpression(_) => HostExecutionSiteKind::Member,
            AstKind::ImportExpression(_) => HostExecutionSiteKind::DynamicImport,
            AstKind::Program(_)
            | AstKind::FunctionBody(_)
            | AstKind::ExpressionStatement(_)
            | AstKind::VariableDeclarator(_)
            | AstKind::ReturnStatement(_)
            | AstKind::ThrowStatement(_)
            | AstKind::BlockStatement(_)
            | AstKind::IfStatement(_)
            | AstKind::ConditionalExpression(_)
            | AstKind::LogicalExpression(_)
            | AstKind::SequenceExpression(_)
            | AstKind::ArrayExpression(_)
            | AstKind::ObjectExpression(_)
            | AstKind::TemplateLiteral(_)
            | AstKind::TaggedTemplateExpression(_)
            | AstKind::AssignmentExpression(_) => HostExecutionSiteKind::Region,
            _ => continue,
        };
        // Composite expressions lack a separately indexed sibling-order table.
        // Conservatively gate their entire projection on bounded completion;
        // separate-statement prefixes and the exiting call itself stay eligible.
        let composite = composite_completion(semantic, node.kind(), state);
        if let Some(completion) = composite {
            predicate = all(vec![predicate, completion]);
        }
        if matches!(node.kind(), AstKind::CallExpression(call) if call.optional) {
            predicate = all(vec![predicate, HostExecutionPredicate::Unknown]);
        }
        // Until expression-order continuation facts are complete, a containing
        // statement/expression with suspension supplies no projection authority.
        // Do not apply this to whole program/function/block containers: their
        // independently proved prefixes remain eligible.
        if matches!(
            node.kind(),
            AstKind::ExpressionStatement(_)
                | AstKind::VariableDeclarator(_)
                | AstKind::ReturnStatement(_)
                | AstKind::ThrowStatement(_)
                | AstKind::ConditionalExpression(_)
                | AstKind::LogicalExpression(_)
                | AstKind::SequenceExpression(_)
        ) && may_suspend(&suspensions, node.kind().span())
        {
            predicate = all(vec![predicate, HostExecutionPredicate::Unknown]);
        }
        facts.push(HostExecutionFact {
            span: span(node.kind().span()),
            kind,
            scope,
            predicate,
        });
    }
    // An explicit default region covers even a primary span that is neither
    // a call nor a member. Its scope must be reached independently of the body.
    for default in &defaults {
        facts.push(HostExecutionFact {
            span: default.span,
            kind: HostExecutionSiteKind::Region,
            scope: Some(default.span),
            predicate: HostExecutionPredicate::Live,
        });
    }
    facts.sort_by_key(|fact| fact.span);
    if state.remaining == 0 || state.calls.exhausted() || defaults.len() > WIDTH {
        for fact in &mut facts {
            fact.predicate = HostExecutionPredicate::Unknown;
        }
    } else if opaque_host_environment(semantic) {
        for fact in &mut facts {
            fact.predicate = all(vec![
                fact.predicate.clone(),
                HostExecutionPredicate::Unknown,
            ]);
        }
    }
    (facts, defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::extract;

    fn measured(
        source: &str,
        exhaust: bool,
    ) -> (Vec<HostExecutionFact>, usize, usize, usize, usize) {
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        assert!(built.errors.is_empty(), "{:?}", built.errors);
        let mut state = ExtractionState::new();
        if exhaust {
            state.remaining = 0;
        }
        let (facts, _) = host_execution_with_state(&built.semantic, &mut state);
        (
            facts,
            built.semantic.nodes().len(),
            state.calls.visits,
            state.calls.body_visits,
            state.steps,
        )
    }

    #[test]
    fn deep_wide_completion_visits_and_predicate_storage_are_linear() {
        // The parser/binder have their own stack needs. Count this pass's work,
        // never elapsed time, and test both pathological expression shapes.
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            for deep in [format!("{}0", "true?0:".repeat(2000)), format!("{}true", "true&&".repeat(2000))] {
                let source = format!("function local(flag:boolean){{if(flag)return;throw 0}} const value={deep}; {}", "local(true);".repeat(500));
                let (facts, nodes, visits, bodies, steps) = measured(&source, false);
                assert_eq!(bodies, 1, "one body memo for 500 exact local calls");
                assert!(visits <= 2 * nodes, "{visits} completion visits / {nodes} AST nodes");
                assert!(visits <= super::super::host_call_completion::FILE_NODE_BUDGET);
                assert!(steps <= EXECUTION_STEPS && steps <= 8 * nodes + 32);
                assert!(facts.iter().all(|fact| fact.predicate.nodes() <= PREDICATE_NODES));
                assert!(facts.iter().map(|fact| fact.predicate.nodes()).sum::<usize>() <= PREDICATE_NODES * facts.len());
            }
            // Wide symbolic prefixes used to clone the whole accumulated tree
            // into every following call fact; the encoded size is bounded too.
            let source = "if(import.meta.env.SSR) throw 0; ordinary();".repeat(2000);
            let (facts, nodes, visits, _, steps) = measured(&source, false);
            assert!(visits <= 2 * nodes && steps <= EXECUTION_STEPS);
            assert!(facts.iter().all(|fact| fact.predicate.nodes() <= PREDICATE_NODES));
            assert!(serde_json::to_vec(&facts).unwrap().len() <= facts.len() * 8192);
        }).unwrap().join().unwrap();
    }

    #[test]
    fn overflow_never_creates_an_execution_witness() {
        let (facts, _, _, _, steps) = measured("if(false) closed(); ordinary();", true);
        assert_eq!(steps, 0);
        assert!(
            facts
                .iter()
                .all(|fact| fact.predicate.evaluate(&|_| Some(false)).is_none())
        );
        let branch = HostExecutionPredicate::Constant {
            identity: HostConstantIdentity::ViteSsr,
            expected: true,
        };
        let overflow = all(vec![branch; PREDICATE_NODES + 1]);
        assert_eq!(overflow, HostExecutionPredicate::Unknown);
        assert_eq!(overflow.evaluate(&|_| Some(false)), None);
        assert_eq!(
            HostExecutionPredicate::CallCompletion(Box::new(overflow)).evaluate(&|_| None),
            Some(true)
        );
        // Literal specializations must not reuse a non-return proof for a
        // different invocation environment.
        let source = "function local(flag:boolean){if(flag)throw 0} local(true); function caller(){local(false);open()}";
        assert_eq!(observed(source, "open()", false), Some(true));
        let source = format!(
            "if({}import.meta.env.SSR{}) closed();",
            "(".repeat(100),
            " as boolean)".repeat(100)
        );
        assert_eq!(observed(&source, "closed()", false), None);
    }

    #[test]
    fn inert_classes_complete_and_eager_or_unresolved_heritage_stays_unknown() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/reactive-ir/inferred-host-spa/class-cases.json");
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
        for case in cases {
            let expected = case["normal"].as_bool().unwrap().then_some(true);
            let source = case["source"].as_str().unwrap();
            assert_eq!(
                observed(source, "closed()", false),
                expected,
                "{}",
                case["name"]
            );
            let (facts, _, _, _, _) = measured(source, false);
            let completion = facts
                .iter()
                .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                .unwrap();
            assert_eq!(
                completion.predicate.evaluate(&|_| None),
                expected,
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn nullish_completion_composes_left_and_withholds_unknown_selection() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../../../../fixtures/reactive-ir/inferred-host-spa/nullish-cases.json"
        ))
        .unwrap();
        for case in cases {
            for form in ["{};", "const value = {};", "consume({});"] {
                let source = format!(
                    "{} function consume(value: unknown) {{}} prefix(); {} closed();",
                    case["declaration"].as_str().unwrap(),
                    form.replace("{}", case["expression"].as_str().unwrap())
                );
                let expected = case["completion"].as_bool();
                assert_eq!(observed(&source, "prefix()", false), Some(true), "{source}");
                assert_eq!(observed(&source, "closed()", false), expected, "{source}");
                let (facts, _, _, _, _) = measured(&source, false);
                let completion = facts
                    .iter()
                    .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                    .unwrap();
                assert_eq!(
                    completion.predicate.evaluate(&|_| None),
                    expected,
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn unproved_constructor_completion_preserves_its_prefix_and_withholds_tails() {
        for source in [
            "class Stop {constructor(){throw 0}} prefix(); new Stop(); closed();",
            "function Stop(){throw 0} prefix(); new Stop(); closed();",
            "class Normal {} prefix(); new Normal(); closed();",
        ] {
            assert_eq!(observed(source, "prefix()", false), Some(true), "{source}");
            assert_eq!(observed(source, "closed()", false), None, "{source}");
            let (facts, _, _, _, _) = measured(source, false);
            let completion = facts
                .iter()
                .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                .unwrap();
            assert_eq!(completion.predicate.evaluate(&|_| None), None, "{source}");
        }
    }

    #[test]
    fn mutable_or_escaped_vite_env_never_authenticates_a_browser_continuation() {
        for source in [
            "Object.assign(import.meta.env,{SSR:true}); if(import.meta.env.SSR) throw 0; closed();",
            "const env=import.meta.env; env.SSR=true; closed();",
            "Object.assign(import.meta,{env:{SSR:true}}); closed();",
            "import.meta.env.SSR=true; closed();",
            "import.meta.env['SSR']=true; closed();",
            "delete import.meta.env.SSR; closed();",
            "Object.assign(import.meta.env,{SSR:true}); function stop(){if(import.meta.env.SSR) throw 0} stop(); closed();",
            "const env=import.meta?.env; closed();",
            "const value=import.meta.env?.unknown; closed();",
            "const env=import.meta.env; if(import.meta.env?.DEV) env.SSR=true; closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), None, "{source}");
        }
    }

    #[test]
    fn readonly_optional_vite_env_scalars_do_not_escape_the_environment() {
        for (source, expected) in [
            ("void import.meta.env?.DEV; closed();", None),
            (
                "function deferred(){if(import.meta.env?.DEV) warn()} closed();",
                Some(true),
            ),
            ("void import.meta.env?.SSR; closed();", None),
        ] {
            let allocator = oxc_allocator::Allocator::default();
            let parsed =
                oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
            let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
            assert!(!opaque_host_environment(&built.semantic), "{source}");
            // Eager optional-chain completion remains unproved. Only a
            // deferred read permits the surrounding initialization to finish.
            assert_eq!(observed(source, "closed()", false), expected, "{source}");
            let (facts, _, _, _, _) = measured(source, false);
            let completion = facts
                .iter()
                .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                .unwrap();
            assert_eq!(
                completion.predicate.evaluate(&|_| None),
                expected,
                "{source}"
            );
        }
        assert_eq!(
            observed("if(import.meta.env?.SSR) closed();", "closed()", false),
            None
        );
    }

    #[test]
    fn exact_call_completion_instantiates_client_constants_and_preserves_normal_returns() {
        for source in [
            "function stop(){if(!import.meta.env.SSR) throw 0} stop(); closed();",
            "function stop(){if(true) throw 0} stop(); closed();",
            "function stop(){throw 0} stop(); closed();",
            "function stop(){while(true){}} stop(); closed();",
            "function stop(x:boolean){if(x) throw 0} stop(true); closed();",
            "function stop(x:unknown){void x; throw 0} stop({}); closed();",
            "const stop=(x:boolean)=>{if(!x) throw 0}; stop(false); closed();",
            "function inner(){if(!import.meta.env.SSR) throw 0} function stop(){inner()} stop(); closed();",
            "function stop(){if(import.meta.env.SSR) return; throw 0} stop(); closed();",
            "function stop(){if(true){return (()=>{throw 0})()}} stop(); closed();",
            "function stop(){if(true) throw 0} if(stop()) {} closed();",
            "function stop(){throw 0} true && stop(); closed();",
            "function stop(){throw 0} [stop(),closed()];",
            "function stop(){throw 0} ({a:stop(), b:closed()});",
            "function stop(){throw 0} outer(stop(),closed());",
            "function stop(){throw 0} const a=stop(), b=closed();",
            "function stop(){throw 0} let x=0; x+=stop(); closed();",
            "function stop(){throw 0} let obj:any={}; obj[stop()]=closed();",
            "function stop(){throw 0} stop()[closed()];",
            "function stop(){throw 0} outer(...stop()); closed();",
            "function stop(){throw 0} outer(...stop(),closed());",
            "function stop(){throw 0} tag`${stop()}`; closed();",
            "import {isServer as ssr} from '@solidjs/web'; function stop(){if(!ssr) throw 0} stop(); closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(false), "{source}");
        }
        for source in [
            "function stop(){if(import.meta.env.SSR) throw 0} stop(); closed();",
            "function stop(){if(true) return; throw 0} stop(); closed();",
            "function stop(){if(flag) return; throw 0} stop(); closed();",
            "function stop(x:boolean){if(x) throw 0} stop(false); closed();",
            "function stop(x:boolean){if(x) throw 0} stop(flag); closed();",
            "function stop(x:boolean){x=false;if(x) throw 0} stop(true); closed();",
            "function stop(x=flag){if(x) throw 0} stop(); closed();",
            "function stop(){while(true){break}} stop(); closed();",
            "async function stop(){if(true) throw 0} stop(); closed();",
            "function* stop(){throw 0} stop(); closed();",
            "function stop(){stop();throw 0} stop(); closed();",
            "function a(){b();throw 0} function b(){a()} a(); closed();",
            "function a(){b()} function b(){c()} function c(){d()} function d(){e()} function e(){throw 0} a(); closed();",
            "const obj={stop(){throw 0}}; obj.stop(); closed();",
            "declare const process:{exit():never}; process.exit(); closed();",
            "unknownCall(); closed();",
            "let stop=()=>{throw 0}; stop=()=>{}; stop(); closed();",
            "function stop(){throw 0} false && stop(); closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(true), "{source}");
        }
        let source = "function stop(){if(!import.meta.env.SSR) throw 0} stop(); closed();";
        assert_eq!(observed(source, "closed()", true), Some(true));
        assert_eq!(
            observed(
                "prefix(); function stop(){if(!import.meta.env.SSR) throw 0} stop(); closed();",
                "prefix()",
                false
            ),
            Some(true)
        );
        let ast = extract("host.ts", source).unwrap();
        let completion = ast
            .host_execution
            .iter()
            .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
            .unwrap();
        assert_eq!(completion.predicate.evaluate(&|_| Some(false)), Some(false));
        let encoded = serde_json::to_string(&ast).unwrap();
        let decoded: crate::ast::AstFacts = serde_json::from_str(&encoded).unwrap();
        assert_eq!(ast, decoded);
        let source = format!(
            "function stop(){{{} throw 0}} stop(); closed();",
            "void 0;".repeat(300)
        );
        assert_eq!(observed(&source, "closed()", false), Some(true));
        let source = format!(
            "function stop(){{/*{}*/ throw 0}} stop(); closed();",
            "x".repeat(9000)
        );
        assert_eq!(observed(&source, "closed()", false), Some(true));
        assert_eq!(
            HostExecutionPredicate::CallCompletion(Box::new(HostExecutionPredicate::Unknown))
                .evaluate(&|_| None),
            Some(true)
        );
    }

    #[test]
    fn class_and_namespace_initialization_cannot_supply_a_tail_completion_premise() {
        for source in [
            "class C{static x=(()=>{throw 0})()} closed();",
            "export class C{static x=(()=>{throw 0})()} closed();",
            "export default class C{static x=(()=>{throw 0})()} closed();",
            "const C=class{static x=(()=>{throw 0})()}; closed();",
            "namespace N{throw 0} closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), None, "{source}");
        }
    }

    #[test]
    fn module_completion_is_separate_from_prefix_and_ordinary_calls_remain_feasible() {
        for (source, expected) in [
            ("closed(); throw 0;", Some(false)),
            ("closed(); await new Promise(()=>{});", None),
            ("ordinaryCall(); closed();", Some(true)),
            ("if(import.meta.env.SSR) throw 0; closed();", Some(true)),
        ] {
            let ast = extract("host.ts", source).unwrap();
            let completion = ast
                .host_execution
                .iter()
                .find(|fact| fact.kind == HostExecutionSiteKind::ModuleCompletion)
                .unwrap();
            assert_eq!(
                completion.predicate.evaluate(&|identity| match identity {
                    HostConstantIdentity::ViteSsr => Some(false),
                    _ => None,
                }),
                expected
            );
            assert_eq!(observed(source, "closed()", false), Some(true));
        }
        for source in [
            "((()=>{throw 0})(), closed());",
            "const a=(()=>{throw 0})(), b=closed();",
            "void ((()=>{throw 0})(), 1); closed();",
            "const stop=()=>{throw 0}; stop(); closed();",
            "function stop(){void 0; throw 0} stop(); closed();",
            "export const value=(()=>{throw 0})(); closed();",
            "export default (()=>{throw 0})(); closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(false));
        }
    }

    #[test]
    fn lazy_jsx_values_and_unproved_continuations_withhold_host() {
        for (source, needle) in [
            ("const v=<Show when={false}><Dead/></Show>;", "<Dead/>"),
            ("const v=<Ignore content={closed()}/>;", "closed()"),
            (
                "const v=<Ignore>{import('./never.ts')}</Ignore>;",
                "import('./never.ts')",
            ),
            (
                "async function f(){await new Promise(()=>{}); closed();} f();",
                "closed()",
            ),
            (
                "async function f(){return (await new Promise(()=>{}), closed());} f();",
                "closed()",
            ),
            (
                "async function f(){const x=await new Promise(()=>{}), y=closed();} f();",
                "closed()",
            ),
        ] {
            assert_eq!(observed(source, needle, false), None, "{source}");
        }
        for (source, needle) in [
            ("const v=<div><Live/></div>;", "<Live/>"),
            (
                "async function f(){closed(); await new Promise(()=>{});} f();",
                "closed()",
            ),
            ("const cb=async()=>{await step()}; closed();", "closed()"),
            (
                "const v=<Show when={true}>{()=>closed()}</Show>;",
                "closed()",
            ),
        ] {
            assert_eq!(observed(source, needle, false), Some(true), "{source}");
        }
    }

    #[test]
    fn exact_synchronous_throwing_calls_withhold_their_tail_only() {
        for source in [
            "(()=>{throw 0})(); closed();",
            "function stop(){throw 0} stop(); closed();",
            "const result=(()=>{throw 0})(); closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(false));
        }
        for source in [
            "(()=>{return 0})(); closed();",
            "(async()=>{throw 0})(); closed();",
            "(function*(){throw 0})(); closed();",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(true));
        }
    }

    #[test]
    fn synchronous_order_and_literal_dead_guards_are_normalized() {
        for source in [
            "const h=setTimeout(fn,0); clearTimeout(h);",
            "const h=setTimeout(fn,0); const n=1; if(!import.meta.env.SSR) clearTimeout(h);",
            "clearTimeout(setTimeout(fn,0));",
        ] {
            let facts = extract("host.ts", source).unwrap();
            assert_eq!(facts.host_call_sequences.len(), 1, "{source}");
        }
        for source in [
            "const h=setTimeout(fn,0); await step(); clearTimeout(h);",
            "const h=setTimeout(fn,0); opaque(); clearTimeout(h);",
        ] {
            let facts = extract("host.ts", source).unwrap();
            assert!(
                !facts.host_call_sequences.iter().any(|pair| {
                    source[pair.first.start as usize..pair.first.end as usize]
                        .starts_with("setTimeout")
                        && source[pair.second.start as usize..pair.second.end as usize]
                            .starts_with("clearTimeout")
                }),
                "{source}"
            );
        }
        assert_eq!(
            observed("if(false) closed()", "closed()", false),
            Some(false)
        );
        assert_eq!(observed("if(true) closed()", "closed()", false), Some(true));
    }

    fn observed(source: &str, needle: &str, server: bool) -> Option<bool> {
        let ast = extract("host.tsx", source).unwrap();
        let start = u32::try_from(source.find(needle).unwrap()).unwrap();
        let site = ast
            .host_execution
            .iter()
            .find(|fact| {
                fact.span == Span::new(start, start + u32::try_from(needle.len()).unwrap())
                    && !matches!(
                        fact.kind,
                        HostExecutionSiteKind::Region | HostExecutionSiteKind::ModuleCompletion
                    )
            })
            .unwrap();
        site.predicate.evaluate(&|identity| match identity {
            HostConstantIdentity::ViteSsr => Some(server),
            HostConstantIdentity::Import { declaration } => ast
                .imports
                .iter()
                .flat_map(|import| &import.bindings)
                .any(|binding| {
                    binding.local.span == *declaration
                        && binding.imported.as_deref() == Some("isServer")
                })
                .then_some(server),
        })
    }

    #[test]
    fn direct_host_tests_cover_both_hosts_and_all_required_sites() {
        for (source, needle, client) in [
            ("if(import.meta.env.SSR) closed()", "closed()", false),
            ("if(!import.meta.env.SSR) closed()", "closed()", true),
            (
                "import.meta.env.SSR ? closed() : other()",
                "closed()",
                false,
            ),
            ("import.meta.env.SSR && closed()", "closed()", false),
            ("!import.meta.env.SSR && closed()", "closed()", true),
            ("import.meta.env.SSR || closed()", "closed()", true),
            ("!import.meta.env.SSR || closed()", "closed()", false),
            ("if(import.meta.env.SSR) <View/>", "<View/>", false),
            (
                "if(import.meta.env.SSR) object.field",
                "object.field",
                false,
            ),
            (
                "if(import.meta.env.SSR) import('./server.ts')",
                "import('./server.ts')",
                false,
            ),
        ] {
            assert_eq!(observed(source, needle, false), Some(client), "{source}");
            assert_eq!(observed(source, needle, true), Some(!client), "{source}");
        }
    }

    #[test]
    fn early_guards_and_same_block_abrupt_completions() {
        for source in [
            "function f(){ if(import.meta.env.SSR) return; closed(); }",
            "function f(){ if(import.meta.env.SSR) {throw 0;} closed(); }",
            "function f(){ if(!import.meta.env.SSR) {} else {return;} closed(); }",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(true));
            assert_eq!(observed(source, "closed()", true), Some(false));
        }
        for source in [
            "function f(){return; closed();}",
            "function f(){throw 0; closed();}",
        ] {
            assert_eq!(observed(source, "closed()", false), Some(false));
            assert_eq!(observed(source, "closed()", true), Some(false));
        }
        // Lexical position does not execute a hoisted function's body.
        assert_eq!(
            observed(
                "function f(){return; function g(){closed();}}",
                "closed()",
                false
            ),
            Some(true)
        );
    }

    #[test]
    fn exact_import_identity_preserves_aliases_and_refuses_shadowing() {
        assert_eq!(
            observed(
                "import {isServer as ssr} from '@solidjs/web'; if(ssr) closed()",
                "closed()",
                false
            ),
            Some(false)
        );
        for source in [
            "const isServer = false; if(isServer) closed()",
            "import {isServer} from '@solidjs/web'; function f(isServer:boolean){if(isServer) closed()}",
            "import * as web from '@solidjs/web'; if(web.isServer) closed()",
            "if(fake.env.SSR) closed()",
            "if(import.meta.env['SSR']) closed()",
            "if(import.meta.env.SSR === true) closed()",
            "if(!!import.meta.env.SSR) closed()",
            "if(import.meta.env.SSR && flag) closed()",
            "if(import.meta.env.DEV) closed()",
            "if(import.meta.env.PROD) closed()",
        ] {
            assert_eq!(observed(source, "closed()", false), None, "{source}");
        }
    }

    #[test]
    fn parameters_are_separate_scopes_and_never_unconditional_body_execution() {
        let source = "function f(x = closed(), {y = nested()} = {}) { body(); } f(false);";
        let ast = extract("host.ts", source).unwrap();
        let function = ast.functions.first().unwrap();
        assert_eq!(ast.host_defaults.len(), 3);
        let call = ast
            .calls
            .iter()
            .find(|call| source[call.span.start as usize..call.span.end as usize] == *"closed()")
            .unwrap();
        let default = ast
            .host_defaults
            .iter()
            .find(|default| default.span == call.span)
            .unwrap();
        assert_eq!(default.argument_index, Some(0));
        let fact = ast
            .host_execution
            .iter()
            .find(|fact| fact.span == call.span && fact.kind == HostExecutionSiteKind::Call)
            .unwrap();
        assert_eq!(fact.scope, Some(default.span));
        assert_ne!(fact.scope, Some(function.span));
        assert!(
            ast.host_defaults
                .iter()
                .any(|default| default.argument_index.is_none())
        );
        let encoded = serde_json::to_string(&ast).unwrap();
        let decoded: crate::ast::AstFacts = serde_json::from_str(&encoded).unwrap();
        assert_eq!(ast, decoded);
    }

    #[test]
    fn unknown_constants_preserve_both_possible_arms_and_wrappers_preserve_identity() {
        assert_eq!(
            observed(
                "if((import.meta.env.SSR as boolean)) closed()",
                "closed()",
                false
            ),
            Some(false)
        );
        assert_eq!(
            observed(
                "if(!(import.meta.env.SSR satisfies boolean)) closed()",
                "closed()",
                false
            ),
            Some(true)
        );
        let ast = extract(
            "host.ts",
            "import {isServer as ssr} from '@other/web'; if(ssr) a(); else b();",
        )
        .unwrap();
        for fact in ast
            .host_execution
            .iter()
            .filter(|fact| fact.kind == HostExecutionSiteKind::Call)
        {
            assert_eq!(fact.predicate.evaluate(&|_| None), None);
            assert_ne!(fact.predicate.evaluate(&|_| None), Some(false));
        }
    }

    #[test]
    fn dead_outer_statement_does_not_execute_nested_defaults_or_bodies() {
        let ast = extract(
            "host.ts",
            "function f(){return; function g(x = nested()){body()}} const h=(x=arrow())=>x;",
        )
        .unwrap();
        assert_eq!(ast.host_defaults.len(), 2);
        for default in &ast.host_defaults {
            let site = ast
                .host_execution
                .iter()
                .find(|fact| fact.span == default.span && fact.kind == HostExecutionSiteKind::Call)
                .unwrap();
            assert_eq!(site.scope, Some(default.span));
            assert_eq!(site.predicate.evaluate(&|_| None), Some(true));
        }
    }

    #[test]
    fn only_void_zero_is_positive_undefined_value_evidence() {
        let ast = extract("host.ts", "f(void 0); f(undefined); function g(undefined:boolean){f(undefined)} f(false as unknown as undefined);").unwrap();
        assert_eq!(ast.host_undefined_arguments.len(), 1);
        let source = "f(void 0)";
        assert_eq!(
            ast.host_undefined_arguments[0],
            Span::new(2, u32::try_from(source.len() - 1).unwrap())
        );
    }

    #[test]
    fn unknown_execution_is_not_dead_and_does_not_discard_following_simple_sites() {
        for source in [
            "if(flag) closed()",
            "while(flag) closed()",
            "try{closed()}catch{}",
            "obj?.method(closed())",
            "class Dormant { field = closed(); }",
            "namespace Dormant { closed(); }",
        ] {
            assert_eq!(observed(source, "closed()", false), None);
        }
        assert_eq!(
            observed("if(flag) {} closed()", "closed()", false),
            Some(true)
        );
        assert_eq!(
            observed("function f(){if(flag) return; closed()}", "closed()", false),
            None
        );
    }
}
