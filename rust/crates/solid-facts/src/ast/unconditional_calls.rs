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
mod tests {
    use super::*;

    fn unconditional(source: &str, name: &str) -> Vec<String> {
        let mut start = source
            .find(&format!("function {name}"))
            .expect("the function");
        if source[..start].ends_with("async ") {
            start -= "async ".len();
        }
        let end = source[start..]
            .find("\n}")
            .map(|offset| start + offset + 2)
            .expect("the function's end");
        let span = Span::new(start as u32, end as u32);
        unconditional_calls(Path::new("test.js"), source, span, &[])
            .expect("parses")
            .into_iter()
            .map(|span| source[span.start as usize..span.end as usize].to_owned())
            .collect()
    }

    fn synchronous(source: &str, name: &str) -> Vec<String> {
        let start = source
            .find(&format!("function {name}"))
            .expect("the function");
        let start = if source[..start].ends_with("async ") {
            start - "async ".len()
        } else {
            start
        };
        let end = source[start..]
            .find("\n}")
            .map(|offset| start + offset + 2)
            .expect("the end");
        synchronously_unconditional_calls(
            Path::new("test.js"),
            source,
            Span::new(start as u32, end as u32),
            &[],
        )
        .expect("parses")
        .into_iter()
        .map(|span| source[span.start as usize..span.end as usize].to_owned())
        .collect()
    }

    /// ADR 0275: an `async` function returns at its own first suspension.
    #[test]
    fn an_async_function_states_its_calls_before_its_first_suspension() {
        let source = "async function before(cb) {\n  onCleanup(cb);\n  await load();\n  onCleanup(cb);\n}\n\
async function inside(cb) {\n  onCleanup(await load(cb));\n}\n\
async function looped(cb, list) {\n  for await (const x of list) {}\n  onCleanup(cb);\n}\n\
async function nested(cb) {\n  const later = async () => { await load(); };\n  onCleanup(cb);\n}\n\
async function *generated(cb) {\n  onCleanup(cb);\n}\n\
function plain(cb) {\n  onCleanup(cb);\n}\n\
function guarded(cb, ready) {\n  if (!ready) throw new Error('x');\n  onCleanup(cb);\n}\n";
        assert_eq!(synchronous(source, "before"), ["onCleanup(cb)"]);
        assert!(synchronous(source, "inside").is_empty());
        assert!(synchronous(source, "looped").is_empty());
        assert_eq!(synchronous(source, "nested"), ["onCleanup(cb)"]);
        assert!(synchronous(source, "*generated").is_empty());
        assert_eq!(synchronous(source, "plain"), ["onCleanup(cb)"]);
        assert!(unconditional(source, "before").is_empty());
        // A caller may catch the thrown guard and carry on (ADR 0276).
        assert!(synchronous(source, "guarded").is_empty());
        assert_eq!(unconditional(source, "guarded"), ["onCleanup(cb)"]);
    }

    #[test]
    fn only_a_call_on_every_completion_is_unconditional() {
        let source = "function straight(cb) {\n  onCleanup(cb);\n  const x = createEffect(a, b);\n}\n\
function guarded(cb, flag) {\n  if (flag) onCleanup(cb);\n  flag && onCleanup(cb);\n  flag ? onCleanup(cb) : 0;\n  cb?.();\n}\n\
function early(cb, flag) {\n  if (flag) return;\n  onCleanup(cb);\n}\n\
function thrown(cb, flag) {\n  if (!flag) throw new Error('x');\n  onCleanup(cb);\n}\n\
function looped(cb) {\n  for (;;) onCleanup(cb);\n  try { onCleanup(cb); } catch {}\n}\n\
function nested(cb) {\n  queueMicrotask(() => onCleanup(cb));\n}\n\
async function later(cb) {\n  onCleanup(cb);\n}\n";
        assert_eq!(
            unconditional(source, "straight"),
            ["onCleanup(cb)", "createEffect(a, b)"]
        );
        assert!(unconditional(source, "guarded").is_empty());
        assert!(unconditional(source, "early").is_empty());
        assert_eq!(unconditional(source, "thrown"), ["onCleanup(cb)"]);
        assert!(unconditional(source, "looped").is_empty());
        assert_eq!(
            unconditional(source, "nested"),
            ["queueMicrotask(() => onCleanup(cb))"]
        );
        assert!(unconditional(source, "later").is_empty());
    }

    /// The function bodies of two published exports, byte for byte:
    /// `@solid-primitives/gestures@3.0.0-next.3` `dist/tap.js` (sha256
    /// 270f7a35...9174) and `@solid-primitives/mutation-observer@3.0.0-next.2`
    /// `dist/index.js` (sha256 8d970f9c...8f20). `tap` registers its
    /// `onCleanup` on every call; `createMutationObserver` registers
    /// `onSettled` and `onCleanup` only `if (isSupported)`.
    #[test]
    fn a_published_export_registers_on_every_call_only_without_a_guard() {
        const TAP: &str = r#"function tap(props) {
	let cleanup;
	onCleanup(() => cleanup?.());
	return (node) => {
		let x;
		let y;
		let time;
		let startPointerId = null;
		const downCallback = (activeEvents, event) => {
			if (activeEvents.length !== 1) return;
			startPointerId = event.pointerId;
			time = Date.now();
			x = event.clientX;
			y = event.clientY;
		};
		const upCallback = (_, event) => {
			if (event.pointerId !== startPointerId) return;
			startPointerId = null;
			const now = Date.now();
			if (Math.abs(event.clientX - x) < 4 && Math.abs(event.clientY - y) < 4 && now - time >= (props.minimumTapLength ?? 0) && (props.maximumTapLength === void 0 || now - time < props.maximumTapLength)) {
				const rect = node.getBoundingClientRect();
				const tapX = Math.round(event.clientX - rect.left);
				const tapY = Math.round(event.clientY - rect.top);
				props.callback({
					x: tapX,
					y: tapY
				});
			}
		};
		cleanup = registerPointerListener(node, downCallback, void 0, upCallback);
	};
}
"#;
        const MUTATION_OBSERVER: &str = r#"function createMutationObserver(initial, b, c) {
	let defaultOptions, callback;
	const isSupported = !isServer;
	if (typeof b === "function") {
		defaultOptions = {};
		callback = b;
	} else {
		defaultOptions = b;
		callback = c;
	}
	const instance = isSupported ? new MutationObserver(callback) : void 0;
	const add = (el, options) => instance?.observe(el, access(options) ?? defaultOptions);
	const start = () => {
		if (!isSupported) return;
		asArray(access(initial)).forEach((item) => {
			item instanceof Node ? add(item, defaultOptions) : add(item[0], item[1]);
		});
	};
	const stop = () => instance?.disconnect();
	if (isSupported) {
		onSettled(start);
		onCleanup(stop);
	}
	return [add, {
		start,
		stop,
		instance,
		isSupported
	}];
}
"#;
        let tap = unconditional(TAP, "tap");
        assert!(
            tap.iter()
                .any(|call| call == "onCleanup(() => cleanup?.())")
        );
        let observer = unconditional(MUTATION_OBSERVER, "createMutationObserver");
        assert!(!observer.iter().any(|call| call == "onSettled(start)"));
        assert!(!observer.iter().any(|call| call == "onCleanup(stop)"));
    }

    /// ADR 0166: under a declared host, `isServer` is the value the host's
    /// resolution of `@solidjs/web` binds (`false` in `dist/web.js`, `true` in
    /// `dist/server.js`). The bodies are byte for byte
    /// `@solid-primitives/memo@2.0.0-next.2` `dist/index.js` `createPureReaction`
    /// and `@solid-primitives/lifecycle@1.0.0-next.2` `dist/index.js`
    /// `onElementConnect`, under the import line both modules write.
    #[test]
    fn a_folded_host_constant_decides_the_guard() {
        const MEMO: &str = r#"import { isServer } from "@solidjs/web";
function createPureReaction(onInvalidate, options) {
	if (isServer) return () => void 0;
	const owner = getOwner();
	let trackers = 0;
	let disposed = false;
	onCleanup(() => {
		disposed = true;
	});
	return (tracking) => {
		if (disposed) {
			untrack(tracking);
			return;
		}
		trackers++;
		runWithOwner(owner, () => createReaction(() => {
			if (--trackers === 0) untrack(onInvalidate);
		}, options))(tracking);
	};
}"#;
        const LIFECYCLE: &str = r#"import { isServer } from "@solidjs/web";
function onElementConnect(el, fn) {
	if (isServer) return;
	if (el.isConnected) return fn();
	const observer = new ResizeObserver(() => el.isConnected && (observer.disconnect(), fn()));
	observer.observe(el);
	onCleanup(() => observer.disconnect());
}"#;
        let fold = |value| {
            vec![HostConstantFold {
                specifier: "@solidjs/web".into(),
                imported: "isServer".into(),
                value,
            }]
        };
        let calls = |source: &str, name: &str, folds: &[HostConstantFold]| {
            let start = source
                .find(&format!("function {name}"))
                .expect("the function");
            let end = source[start..]
                .find("\n}")
                .map(|offset| start + offset + 2)
                .expect("end");
            unconditional_calls(
                Path::new("index.js"),
                source,
                Span::new(start as u32, end as u32),
                folds,
            )
            .expect("parses")
            .into_iter()
            .map(|span| source[span.start as usize..span.end as usize].to_owned())
            .collect::<Vec<_>>()
        };
        let cleanup = |calls: &[String]| calls.iter().any(|call| call.starts_with("onCleanup("));
        // Host free: the guard may return, so nothing after it is unconditional.
        assert!(!cleanup(&calls(MEMO, "createPureReaction", &[])));
        // Browser: the guard never returns.
        let browser = calls(MEMO, "createPureReaction", &fold(false));
        assert!(cleanup(&browser), "{browser:?}");
        assert!(browser.iter().any(|call| call == "getOwner()"));
        // Node: the guard always returns, so nothing after it runs at all.
        assert!(calls(MEMO, "createPureReaction", &fold(true)).is_empty());
        // `onElementConnect` also returns under `el.isConnected`, which no fold
        // decides: its `onCleanup` stays conditional under every host.
        for folds in [Vec::new(), fold(false), fold(true)] {
            assert!(!cleanup(&calls(LIFECYCLE, "onElementConnect", &folds)));
        }
    }
}
