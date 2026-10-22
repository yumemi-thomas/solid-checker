//! Source facts and async function facts (Go: `tsgo/async_flow.go` and the
//! `Source*` half of `tsgo/project.go`).
//!
//! `SourceCalls`, `SourceBindings` and `SourceFunctions` are parser-derived
//! facts with a symbol identity attached where a call has a target;
//! `SourceAsyncFunctions` and `AsyncFunctionsAt` add the checker's view of
//! which functions can return async work and the syntactic-dominance scan of
//! calls that run after an `await` on every reachable path.
//!
//! # The memo
//!
//! Go keeps one `fileFactsMemo` per `Source*` path argument across
//! generations. A fact set is stored only when every symbol identity it
//! carries is durable (`symbol:h:`), so a reused set never hands out an id the
//! current generation cannot resolve. The memo holds ids and locations, never
//! a tsrs handle, so it survives a rebase and `ReleaseAnalysisState`. An
//! incremental update drops the affected files and the files that left the
//! program; any other update clears it (`Project::evict_source_memo`).
//!
//! # Differences from Go
//!
//! `SourceFunctions` needs no checker, as in Go. Source text is `&str` by
//! construction, so the "source is not valid UTF-8" check of
//! `validateLocation` has no counterpart.

use crate::backend::BackendError;
use crate::context::Context;
use crate::gocompat::{absolute, clean};
use crate::identity::Env;
use crate::model::durable_symbol_id;
use crate::nodes::node_at;
use crate::project::{Project, validate_location};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tsrs_ast::{
    Kind, ModifierFlags, Node, SourceFile, get_source_file_of_node, has_syntactic_modifier,
    is_array_binding_pattern, is_array_literal_expression, is_arrow_function, is_await_expression,
    is_binary_expression, is_binding_element, is_block, is_break_statement, is_call_expression,
    is_conditional_expression, is_function_declaration, is_function_expression, is_identifier,
    is_if_statement, is_iteration_statement, is_labeled_statement,
    is_logical_or_coalescing_assignment_operator, is_method_declaration, is_optional_chain,
    is_property_assignment, is_return_statement, is_shorthand_property_assignment,
    is_switch_statement, is_throw_statement, is_try_statement, is_variable_declaration,
    is_variable_statement,
};
use tsrs_checker::types::{SignatureKind, Type};
use tsrs_compiler::Checker;
use tsrs_core::P;
use tsrs_scanner::skip_trivia;
use typefacts::{AsyncFunctionFact, Location, SourceBinding, SourceCall, SourceFunction};

/// Go: `fileFactsMemo`: one `Source*` path argument's fact sets.
#[derive(Clone, Debug, Default)]
pub struct FileFactsMemo {
    /// The cleaned absolute path, retained for affected-set eviction.
    pub(crate) abs_path: String,
    pub(crate) calls: Option<Vec<SourceCall>>,
    pub(crate) bindings: Option<Vec<SourceBinding>>,
    pub(crate) functions: Option<Vec<SourceFunction>>,
    pub(crate) async_functions: Option<Vec<AsyncFunctionFact>>,
    pub(crate) async_at: HashMap<(u64, u64), Vec<AsyncFunctionFact>>,
}

impl FileFactsMemo {
    #[must_use]
    pub fn has_calls(&self) -> bool {
        self.calls.is_some()
    }
    #[must_use]
    pub fn has_bindings(&self) -> bool {
        self.bindings.is_some()
    }
    #[must_use]
    pub fn has_functions(&self) -> bool {
        self.functions.is_some()
    }
    #[must_use]
    pub fn has_async(&self) -> bool {
        self.async_functions.is_some()
    }
    #[must_use]
    pub fn async_at_len(&self) -> usize {
        self.async_at.len()
    }
}

/// Go: `asyncFactsDurable`.
fn async_facts_durable(facts: &[AsyncFunctionFact]) -> bool {
    facts
        .iter()
        .all(|fact| durable_symbol_id(&fact.symbol) && durable_symbol_id(&fact.target))
}

/// Go: `sourceCallsDurable`.
fn source_calls_durable(calls: &[SourceCall]) -> bool {
    calls.iter().all(|call| durable_symbol_id(&call.target))
}

/// Go: `sourceBindingsDurable`.
fn source_bindings_durable(bindings: &[SourceBinding]) -> bool {
    bindings
        .iter()
        .all(|binding| durable_symbol_id(&binding.initializer.target))
}

/// `scanner.SkipTrivia(sourceFile.Text(), node.Pos())`.
fn start_of(text: &str, node: P<Node>) -> u64 {
    u64::from(skip_trivia(
        text,
        node.pos().source_offset().unwrap_or_default(),
    ))
}

/// `node.End()`.
fn end_of(node: P<Node>) -> u64 {
    node.end().source_offset().map_or(0, u64::from)
}

fn span(path: &Arc<str>, text: &str, node: P<Node>) -> Location {
    Location {
        path: Arc::clone(path),
        start_byte: start_of(text, node),
        end_byte: end_of(node),
    }
}

/// Go: `p.program.GetSourceFile` + `validateLocation` (`sourceFileFor`).
fn source_file_for(env: &Env<'_>, location: &Location) -> Result<P<SourceFile>, BackendError> {
    let absolute_path = absolute(&location.path)
        .map_err(|error| BackendError(format!("resolve source path: {error}")))?;
    let file = env
        .program
        .get_source_file(&absolute_path)
        .ok_or_else(|| BackendError(format!("not found: source file {absolute_path}")))?;
    validate_location(location, file.text())?;
    Ok(file)
}

/// Go: `sourceCallFact`. A call whose callee has no symbol is no fact.
fn source_call_fact(
    env: &mut Env<'_>,
    path: &Arc<str>,
    file: P<SourceFile>,
    node: P<Node>,
) -> Option<SourceCall> {
    let call = node.as_call_expression();
    let target = env
        .checker
        .get_symbol_at_location_exported(call.expression)?;
    let target = env.canonical_symbol(target);
    let text = file.text();
    let arguments = call
        .arguments
        .nodes()
        .iter()
        .map(|argument| span(path, text, *argument))
        .collect();
    Some(SourceCall {
        location: span(path, text, node),
        callee: span(path, text, call.expression),
        arguments,
        target: env.id_for(target),
    })
}

/// Go: the `visit` closure of every `Source*` walk: pre-order over the
/// statements.
fn walk(node: P<Node>, visit: &mut dyn FnMut(P<Node>)) {
    visit(node);
    node.for_each_child(&mut |child| {
        walk(child, visit);
        false
    });
}

fn walk_statements(file: P<SourceFile>, visit: &mut dyn FnMut(P<Node>)) {
    for statement in file.statements.nodes() {
        walk(*statement, visit);
    }
}

/// Go: `bindingNameLocations`.
fn binding_name_locations(path: &Arc<str>, text: &str, name: P<Node>) -> (bool, Vec<Location>) {
    if is_identifier(name) {
        return (false, vec![span(path, text, name)]);
    }
    if !is_array_binding_pattern(name) {
        return (false, Vec::new());
    }
    let locations = name
        .as_binding_pattern()
        .elements
        .nodes()
        .iter()
        .map(|element| {
            if !is_binding_element(*element) {
                return Location::default();
            }
            match element.as_binding_element().name {
                Some(bound) if is_identifier(bound) => span(path, text, bound),
                _ => Location::default(),
            }
        })
        .collect();
    (true, locations)
}

/// Go: `sourceFunctionFact`.
fn source_function_fact(
    path: &Arc<str>,
    text: &str,
    name: P<Node>,
    body: P<Node>,
    parameters: &[P<Node>],
    owner: P<Node>,
) -> SourceFunction {
    SourceFunction {
        name: span(path, text, name),
        body: Location {
            path: Arc::clone(path),
            start_byte: start_of(text, body),
            end_byte: end_of(body).saturating_sub(1),
        },
        parameters: parameters
            .iter()
            .map(|parameter| span(path, text, *parameter))
            .collect(),
        exported: has_syntactic_modifier(owner, ModifierFlags::Export),
        r#async: has_syntactic_modifier(owner, ModifierFlags::Async),
        arrow: false,
    }
}

/// Go: `isAsyncFunctionNode`.
fn is_async_function_node(node: P<Node>) -> bool {
    is_arrow_function(node)
        || is_function_expression(node)
        || is_function_declaration(node)
        || is_method_declaration(node)
}

/// Go: `asyncReturnType`.
fn async_return_type(checker: &mut Checker, return_type: P<Type>) -> bool {
    if return_type.is_union() || return_type.is_intersection() {
        return return_type
            .types()
            .iter()
            .any(|constituent| async_return_type(checker, *constituent));
    }
    if return_type.is_type_parameter() {
        return match checker.get_base_constraint_of_type_exported(return_type) {
            Some(constraint) => {
                constraint.addr() != return_type.addr() && async_return_type(checker, constraint)
            }
            None => false,
        };
    }
    if let Some(awaited) = checker.get_awaited_type_exported(return_type)
        && !checker.is_type_identical_to(return_type, awaited)
    {
        return true;
    }
    if let Some(symbol) = return_type.symbol()
        && matches!(symbol.name(), "AsyncIterable" | "AsyncIterator")
    {
        return true;
    }
    for base in checker.get_base_types_exported(return_type) {
        if base.addr() != return_type.addr() && async_return_type(checker, *base) {
            return true;
        }
    }
    false
}

/// Go: `asyncFlowState`.
#[derive(Clone, Copy)]
struct FlowState {
    awaited: bool,
    reachable: bool,
}

/// Go: `mergeAsyncBranches`.
fn merge(left: FlowState, right: FlowState) -> FlowState {
    if !left.reachable {
        return right;
    }
    if !right.reachable {
        return left;
    }
    FlowState {
        reachable: true,
        awaited: left.awaited && right.awaited,
    }
}

/// Go: `scanAsyncFlow`'s environment.
struct FlowScan<'a, 'e> {
    env: &'a mut Env<'e>,
    path: &'a Arc<str>,
    file: P<SourceFile>,
    root: P<Node>,
    calls: Vec<Location>,
}

impl FlowScan<'_, '_> {
    fn record_call(&mut self, node: P<Node>) {
        if let Some(call) = source_call_fact(self.env, self.path, self.file, node) {
            self.calls.push(call.callee);
        }
    }

    fn children(&mut self, node: P<Node>, state: &mut FlowState) {
        node.for_each_child(&mut |child| {
            self.scan(child, state);
            false
        });
    }

    /// Go: `scanAsyncFlow`.
    #[allow(clippy::too_many_lines)]
    fn scan(&mut self, node: P<Node>, state: &mut FlowState) {
        if !state.reachable {
            return;
        }
        if node != self.root && is_async_function_node(node) {
            return;
        }
        if is_block(node) {
            for statement in node.as_block().statements.nodes() {
                self.scan(*statement, state);
                if !state.reachable {
                    break;
                }
            }
            return;
        }
        if is_if_statement(node) {
            let statement = node.as_if_statement();
            self.scan(statement.expression, state);
            let (mut then_state, mut else_state) = (*state, *state);
            self.scan(statement.then_statement, &mut then_state);
            if let Some(else_statement) = statement.else_statement {
                self.scan(else_statement, &mut else_state);
            }
            *state = merge(then_state, else_state);
            return;
        }
        if is_conditional_expression(node) {
            let expression = node.as_conditional_expression();
            self.scan(expression.condition, state);
            let (mut true_state, mut false_state) = (*state, *state);
            self.scan(expression.when_true, &mut true_state);
            self.scan(expression.when_false, &mut false_state);
            *state = merge(true_state, false_state);
            return;
        }
        // ADR 0320: constructs that may skip an await. A labelled statement's
        // body can be left by `break label` past an await written in it; the
        // right side of a logical assignment runs only when the target
        // allows; a destructuring default runs only for `undefined`; an
        // optional chain skips its arguments and keys when its base is
        // nullish. None of their awaits dominates what follows them.
        if is_labeled_statement(node) {
            let mut body_state = *state;
            self.scan(node.as_labeled_statement().statement, &mut body_state);
            *state = merge(*state, body_state);
            return;
        }
        if is_optional_chain(node) {
            if is_call_expression(node) && state.awaited {
                self.record_call(node);
            }
            let mut chain_state = *state;
            self.children(node, &mut chain_state);
            *state = merge(*state, chain_state);
            return;
        }
        if is_binding_element(node)
            && let Some(initializer) = node.as_binding_element().initializer()
        {
            let mut default_state = *state;
            self.scan(initializer, &mut default_state);
            *state = merge(*state, default_state);
            return;
        }
        if is_shorthand_property_assignment(node)
            && let Some(initializer) = node
                .as_shorthand_property_assignment()
                .object_assignment_initializer
                .get()
        {
            let mut default_state = *state;
            self.scan(initializer, &mut default_state);
            *state = merge(*state, default_state);
            return;
        }
        if is_binary_expression(node) {
            let expression = node.as_binary_expression();
            let operator = expression.operator_token.kind();
            let destructuring_default = operator == Kind::EqualsToken
                && node.parent().is_some_and(|parent| {
                    is_array_literal_expression(parent) || is_property_assignment(parent)
                });
            if is_logical_or_coalescing_assignment_operator(operator)
                || destructuring_default
                || matches!(
                    operator,
                    Kind::AmpersandAmpersandToken | Kind::BarBarToken | Kind::QuestionQuestionToken
                )
            {
                self.scan(expression.left, state);
                let mut right_state = *state;
                self.scan(expression.right.get(), &mut right_state);
                *state = merge(*state, right_state);
                return;
            }
        }
        if is_switch_statement(node) {
            self.scan_switch(node, state);
            return;
        }
        if is_try_statement(node) {
            let statement = node.as_try_statement();
            let (mut try_state, mut catch_state) = (*state, *state);
            self.scan(statement.try_block, &mut try_state);
            if let Some(clause) = statement.catch_clause {
                self.scan(clause.as_catch_clause().block, &mut catch_state);
            } else {
                catch_state = try_state;
            }
            *state = merge(try_state, catch_state);
            if let Some(finally_block) = statement.finally_block {
                self.scan(finally_block, state);
            }
            return;
        }
        if is_iteration_statement(node, true) {
            let entry = *state;
            let mut loop_state = entry;
            self.children(node, &mut loop_state);
            state.awaited = entry.awaited;
            state.reachable = entry.reachable;
            return;
        }
        if is_await_expression(node) {
            self.children(node, state);
            state.awaited = true;
            return;
        }
        if is_call_expression(node) && state.awaited {
            self.record_call(node);
        }
        if is_return_statement(node) || is_throw_statement(node) {
            self.children(node, state);
            state.reachable = false;
            return;
        }
        self.children(node, state);
    }

    /// Go: `scanAsyncSwitchFlow`.
    fn scan_switch(&mut self, node: P<Node>, state: &mut FlowState) {
        let statement = node.as_switch_statement();
        self.scan(statement.expression, state);
        let entry = *state;
        let mut flow_through: Option<FlowState> = None;
        let clauses = statement.case_block.as_case_block().clauses.nodes();
        let mut exits: Vec<FlowState> = Vec::with_capacity(clauses.len() + 1);
        let mut has_default = false;
        for clause_node in clauses {
            let clause = clause_node.as_case_or_default_clause();
            if clause_node.kind() == Kind::DefaultClause {
                has_default = true;
            } else if let Some(expression) = clause.expression {
                // A case expression is conditionally evaluated while matching.
                // Preserve calls already dominated before the switch, but do
                // not let an await in one case expression dominate a sibling
                // case or its body.
                let mut case_state = entry;
                self.scan(expression, &mut case_state);
            }
            let mut clause_state = entry;
            if let Some(through) = flow_through {
                clause_state = merge(clause_state, through);
            }
            let mut broke = false;
            for child in clause.statements.nodes() {
                if is_break_statement(*child) {
                    exits.push(clause_state);
                    broke = true;
                    break;
                }
                self.scan(*child, &mut clause_state);
                if !clause_state.reachable {
                    break;
                }
            }
            flow_through = if broke || !clause_state.reachable {
                None
            } else {
                Some(clause_state)
            };
        }
        if let Some(through) = flow_through {
            exits.push(through);
        }
        if !has_default {
            exits.push(entry);
        }
        let Some((first, rest)) = exits.split_first() else {
            state.reachable = false;
            return;
        };
        *state = rest
            .iter()
            .fold(*first, |merged, exit| merge(merged, *exit));
    }
}

/// Go: `asyncFunctionBodyAndSymbol`.
fn async_function_body_and_symbol(
    env: &mut Env<'_>,
    node: P<Node>,
) -> (Option<P<Node>>, crate::model::SymbolId) {
    let body = node.body();
    let name = if is_arrow_function(node) {
        node.parent()
            .filter(|parent| is_variable_declaration(*parent))
            .map(|parent| parent.as_variable_declaration().name)
    } else if is_function_declaration(node) {
        node.as_function_declaration().name
    } else if is_method_declaration(node) {
        node.name()
    } else {
        None
    };
    if let Some(name) = name
        && let Some(symbol) = env.checker.get_symbol_at_location_exported(name)
    {
        let canonical = env.canonical_symbol(symbol);
        return (body, env.id_for(canonical));
    }
    (body, "".into())
}

/// Go: `asyncFunctionFact`.
fn async_function_fact(
    env: &mut Env<'_>,
    path: &Arc<str>,
    file: P<SourceFile>,
    node: P<Node>,
) -> Option<AsyncFunctionFact> {
    let (body, symbol) = async_function_body_and_symbol(env, node);
    let body = body?;
    let mut fact = AsyncFunctionFact {
        expression: span(path, file.text(), node),
        symbol,
        target: "".into(),
        can_return_async: has_syntactic_modifier(node, ModifierFlags::Async),
        calls_after_await: Vec::new(),
    };
    if !fact.can_return_async {
        let function_type = env.checker.get_type_at_location(node);
        for signature in env
            .checker
            .get_signatures_of_type_exported(function_type, SignatureKind::Call)
        {
            let return_type = env
                .checker
                .get_return_type_of_signature_exported(*signature);
            if async_return_type(env.checker, return_type) {
                fact.can_return_async = true;
                break;
            }
        }
    }
    let mut state = FlowState {
        awaited: false,
        reachable: true,
    };
    let mut scan = FlowScan {
        env,
        path,
        file,
        root: body,
        calls: Vec::new(),
    };
    scan.scan(body, &mut state);
    fact.calls_after_await = scan.calls;
    Some(fact)
}

/// Go: `asyncAliasFact`.
fn async_alias_fact(
    env: &mut Env<'_>,
    path: &Arc<str>,
    file: P<SourceFile>,
    node: P<Node>,
) -> Option<AsyncFunctionFact> {
    let declaration = node.as_variable_declaration();
    let initializer = declaration.initializer.get()?;
    if !is_identifier(declaration.name) || !is_identifier(initializer) {
        return None;
    }
    let alias = env
        .checker
        .get_symbol_at_location_exported(declaration.name)?;
    let target = env.checker.get_symbol_at_location_exported(initializer)?;
    let alias = env.canonical_symbol(alias);
    let symbol = env.id_for(alias);
    let target = env.canonical_symbol(target);
    let target = env.id_for(target);
    Some(AsyncFunctionFact {
        expression: span(path, file.text(), initializer),
        symbol,
        target,
        can_return_async: false,
        calls_after_await: Vec::new(),
    })
}

/// Go: `appendAsyncSymbolFacts`.
fn append_async_symbol_facts(
    env: &mut Env<'_>,
    facts: &mut Vec<AsyncFunctionFact>,
    symbol: Option<P<tsrs_ast::Symbol>>,
    depth: u32,
) {
    let Some(symbol) = symbol else { return };
    if depth > 32 {
        return;
    }
    let Some(declaration) = symbol.value_declaration() else {
        return;
    };
    let Some(file) = get_source_file_of_node(declaration) else {
        return;
    };
    let path: Arc<str> = clean(file.file_name()).into();
    if is_async_function_node(declaration) {
        if let Some(fact) = async_function_fact(env, &path, file, declaration) {
            facts.push(fact);
        }
        return;
    }
    if !is_variable_declaration(declaration) {
        return;
    }
    if let Some(fact) = async_alias_fact(env, &path, file, declaration) {
        facts.push(fact);
    }
    let Some(initializer) = declaration.as_variable_declaration().initializer.get() else {
        return;
    };
    if is_async_function_node(initializer) {
        if let Some(fact) = async_function_fact(env, &path, file, initializer) {
            facts.push(fact);
        }
        return;
    }
    if is_identifier(initializer) {
        let next = env.checker.get_symbol_at_location_exported(initializer);
        append_async_symbol_facts(env, facts, next, depth + 1);
    }
}

/// Go: `asyncFunctionsAtLocation`.
fn async_functions_at_location(
    env: &mut Env<'_>,
    location: &Location,
    file: P<SourceFile>,
) -> Vec<AsyncFunctionFact> {
    let Ok(position) = u32::try_from(location.start_byte) else {
        return Vec::new();
    };
    let node = node_at(file, position);
    let mut facts = Vec::new();
    if is_identifier(node) {
        let symbol = env.checker.get_symbol_at_location_exported(node);
        append_async_symbol_facts(env, &mut facts, symbol, 0);
        return facts;
    }
    let mut owner = Some(node);
    while let Some(current) = owner {
        if is_async_function_node(current) {
            if let Some(fact) = async_function_fact(env, &location.path, file, current) {
                facts.push(fact);
            }
            break;
        }
        owner = current.parent();
    }
    facts
}

type FactKey = (String, u64, u64, Arc<str>, Arc<str>);

impl Project {
    /// Go: `memoFor`: the memo entry for a `Source*` path argument, keyed by
    /// the argument itself so memoized facts repeat the caller's own path
    /// form, with the normalized path retained for affected-set eviction.
    /// `None` when the path cannot be made absolute.
    fn memo_for(&mut self, path: &str) -> Option<&mut FileFactsMemo> {
        if !self.source_memo.contains_key(path) {
            let abs_path = clean(&absolute(path).ok()?);
            self.source_memo.insert(
                path.to_owned(),
                FileFactsMemo {
                    abs_path,
                    ..FileFactsMemo::default()
                },
            );
        }
        self.source_memo.get_mut(path)
    }

    /// The source-fact memo, for tests.
    #[must_use]
    pub fn source_memo(&self) -> &HashMap<String, FileFactsMemo> {
        &self.source_memo
    }

    /// Go: the memo half of `Update`. `kept` is `Some` on the incremental
    /// path: a file's entries survive when its text is unchanged and every
    /// durable identity they reference declares in an unchanged file (a
    /// changed declaring file would have put the referencing file in the
    /// affected set). Anything else fails closed and clears the memo.
    pub(crate) fn evict_source_memo(&mut self, kept: Option<&dyn Fn(&str) -> bool>) {
        match kept {
            Some(kept) => self.source_memo.retain(|_, memo| kept(&memo.abs_path)),
            None => self.source_memo.clear(),
        }
    }

    /// Go: `SourceCalls`.
    pub fn source_calls(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<SourceCall>, BackendError> {
        check(ctx)?;
        self.live()?;
        if let Some(calls) = self.memo_for(path).and_then(|memo| memo.calls.clone()) {
            return Ok(calls);
        }
        let location = Location {
            path: path.into(),
            ..Location::default()
        };
        let shared: Arc<str> = path.into();
        let calls = self.with_env(|env, _| {
            let file = source_file_for(env, &location)?;
            let mut calls = Vec::new();
            walk_statements(file, &mut |node| {
                if is_call_expression(node)
                    && let Some(call) = source_call_fact(env, &shared, file, node)
                {
                    calls.push(call);
                }
            });
            Ok::<_, BackendError>(calls)
        })??;
        if source_calls_durable(&calls)
            && let Some(memo) = self.memo_for(path)
        {
            memo.calls = Some(calls.clone());
        }
        Ok(calls)
    }

    /// Go: `SourceBindings`.
    pub fn source_bindings(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<SourceBinding>, BackendError> {
        check(ctx)?;
        self.live()?;
        if let Some(bindings) = self.memo_for(path).and_then(|memo| memo.bindings.clone()) {
            return Ok(bindings);
        }
        let location = Location {
            path: path.into(),
            ..Location::default()
        };
        let shared: Arc<str> = path.into();
        let bindings = self.with_env(|env, _| {
            let file = source_file_for(env, &location)?;
            let text = file.text();
            let mut bindings = Vec::new();
            walk_statements(file, &mut |node| {
                if !is_variable_declaration(node) {
                    return;
                }
                let declaration = node.as_variable_declaration();
                let Some(initializer) = declaration.initializer.get() else {
                    return;
                };
                if is_call_expression(initializer)
                    && let Some(call) = source_call_fact(env, &shared, file, initializer)
                {
                    let (array, names) = binding_name_locations(&shared, text, declaration.name);
                    bindings.push(SourceBinding {
                        array,
                        names,
                        initializer: call,
                    });
                }
            });
            Ok::<_, BackendError>(bindings)
        })??;
        if source_bindings_durable(&bindings)
            && let Some(memo) = self.memo_for(path)
        {
            memo.bindings = Some(bindings.clone());
        }
        Ok(bindings)
    }

    /// Go: `SourceFunctions`. Syntax only: no checker, and no durability
    /// condition because the facts carry no symbol.
    pub fn source_functions(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<SourceFunction>, BackendError> {
        check(ctx)?;
        self.live()?;
        if let Some(functions) = self.memo_for(path).and_then(|memo| memo.functions.clone()) {
            return Ok(functions);
        }
        let location = Location {
            path: path.into(),
            ..Location::default()
        };
        let absolute_path = absolute(&location.path)
            .map_err(|error| BackendError(format!("resolve source path: {error}")))?;
        let file = self
            .live()?
            .program()
            .get_source_file(&absolute_path)
            .ok_or_else(|| BackendError(format!("not found: source file {absolute_path}")))?;
        validate_location(&location, file.text())?;
        let shared: Arc<str> = path.into();
        let text = file.text();
        let mut functions = Vec::new();
        walk_statements(file, &mut |node| {
            if is_function_declaration(node) {
                let declaration = node.as_function_declaration();
                if let (Some(name), Some(body)) = (declaration.name, node.body()) {
                    functions.push(source_function_fact(
                        &shared,
                        text,
                        name,
                        body,
                        node.parameters(),
                        node,
                    ));
                }
            } else if is_variable_declaration(node) {
                let declaration = node.as_variable_declaration();
                let Some(initializer) = declaration.initializer.get() else {
                    return;
                };
                if is_identifier(declaration.name)
                    && is_arrow_function(initializer)
                    && let Some(body) = initializer.body()
                    && is_block(body)
                {
                    let mut owner = node;
                    while !is_variable_statement(owner) {
                        let Some(parent) = owner.parent() else { break };
                        owner = parent;
                    }
                    let mut function = source_function_fact(
                        &shared,
                        text,
                        declaration.name,
                        body,
                        initializer.parameters(),
                        owner,
                    );
                    function.r#async = has_syntactic_modifier(initializer, ModifierFlags::Async);
                    function.arrow = true;
                    functions.push(function);
                }
            }
        });
        if let Some(memo) = self.memo_for(path) {
            memo.functions = Some(functions.clone());
        }
        Ok(functions)
    }

    /// Go: `SourceAsyncFunctions`.
    pub fn source_async_functions(
        &mut self,
        ctx: &Context,
        path: &str,
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        check(ctx)?;
        self.live()?;
        if let Some(facts) = self
            .memo_for(path)
            .and_then(|memo| memo.async_functions.clone())
        {
            return Ok(facts);
        }
        let location = Location {
            path: path.into(),
            ..Location::default()
        };
        let shared: Arc<str> = path.into();
        let facts = self.with_env(|env, _| {
            let file = source_file_for(env, &location)?;
            let mut facts = Vec::new();
            walk_statements(file, &mut |node| {
                if is_async_function_node(node) {
                    if let Some(fact) = async_function_fact(env, &shared, file, node) {
                        facts.push(fact);
                    }
                } else if is_variable_declaration(node)
                    && let Some(fact) = async_alias_fact(env, &shared, file, node)
                {
                    facts.push(fact);
                }
            });
            Ok::<_, BackendError>(facts)
        })??;
        if async_facts_durable(&facts)
            && let Some(memo) = self.memo_for(path)
        {
            memo.async_functions = Some(facts.clone());
        }
        Ok(facts)
    }

    /// Go: `AsyncFunctionsAt`: each location selects an inline or containing
    /// function, or follows an identifier through local variable aliases to
    /// its function declaration. One checker lease covers the batch, and
    /// per-location results survive unchanged generations.
    pub fn async_functions_at_locations(
        &mut self,
        ctx: &Context,
        locations: &[Location],
    ) -> Result<Vec<AsyncFunctionFact>, BackendError> {
        check(ctx)?;
        self.live()?;
        let mut facts: Vec<AsyncFunctionFact> = Vec::with_capacity(locations.len());
        let mut seen: HashSet<FactKey> = HashSet::with_capacity(locations.len());
        for location in locations {
            check(ctx)?;
            let location = Location {
                path: clean(&location.path).into(),
                ..location.clone()
            };
            let key = (location.start_byte, location.end_byte);
            let memoized = self
                .memo_for(&location.path)
                .and_then(|memo| memo.async_at.get(&key).cloned());
            let selected = match memoized {
                Some(selected) => selected,
                None => {
                    let selected = self.with_env(|env, _| {
                        let file = source_file_for(env, &location)?;
                        Ok::<_, BackendError>(async_functions_at_location(env, &location, file))
                    })??;
                    if async_facts_durable(&selected)
                        && let Some(memo) = self.memo_for(&location.path)
                    {
                        memo.async_at.insert(key, selected.clone());
                    }
                    selected
                }
            };
            for mut fact in selected {
                let fact_key: FactKey = (
                    clean(&fact.expression.path),
                    fact.expression.start_byte,
                    fact.expression.end_byte,
                    fact.symbol.clone(),
                    fact.target.clone(),
                );
                if !seen.insert(fact_key.clone()) {
                    continue;
                }
                fact.expression.path = fact_key.0.into();
                for call in &mut fact.calls_after_await {
                    call.path = clean(&call.path).into();
                }
                facts.push(fact);
            }
        }
        facts.sort_by(|left, right| {
            (
                &*left.expression.path,
                left.expression.start_byte,
                left.expression.end_byte,
                &*left.symbol,
                &*left.target,
            )
                .cmp(&(
                    &*right.expression.path,
                    right.expression.start_byte,
                    right.expression.end_byte,
                    &*right.symbol,
                    &*right.target,
                ))
        });
        Ok(facts)
    }
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}
