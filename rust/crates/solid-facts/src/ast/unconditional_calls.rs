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
use oxc_ast::ast::{AssignmentOperator, Statement};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};

/// The spans of the call and `new` expressions written in the function whose
/// node is exactly `function` that run exactly once on every normal
/// completion of it. `None` when the source does not parse or no function
/// node has that span.
#[must_use]
pub fn unconditional_calls(path: &Path, source: &str, function: Span) -> Option<Vec<Span>> {
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
    let owner = nodes.iter().find(|node| {
        let span = node.kind().span();
        span.start == function.start
            && span.end == function.end
            && matches!(
                node.kind(),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
    })?;
    let plain = match owner.kind() {
        AstKind::Function(function) => !function.r#async && !function.generator,
        AstKind::ArrowFunctionExpression(arrow) => !arrow.r#async,
        _ => false,
    };
    if !plain {
        return Some(Vec::new());
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
            let span = node.kind().span();
            span.start >= function.start && span.end <= function.end
        })
        .filter(|node| runs_on_every_completion(nodes, node.id(), owner.id()))
        .map(|node| {
            let span = node.kind().span();
            Span::new(span.start, span.end)
        })
        .collect();
    Some(calls)
}

fn runs_on_every_completion(nodes: &AstNodes<'_>, call: NodeId, owner: NodeId) -> bool {
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
        if !evaluates_child_once(nodes, parent, child) {
            return false;
        }
        child = parent;
    }
}

/// Whether evaluating `parent` evaluates `child` exactly once, whatever the
/// values involved.
fn evaluates_child_once(nodes: &AstNodes<'_>, parent: NodeId, child: NodeId) -> bool {
    let child_span = nodes.kind(child).span();
    let is = |span: oxc_span::Span| span == child_span;
    match nodes.kind(parent) {
        AstKind::FunctionBody(body) => statements_before_exit_free(&body.statements, child_span),
        AstKind::BlockStatement(block) => statements_before_exit_free(&block.body, child_span),
        AstKind::IfStatement(statement) => is(statement.test.span()),
        AstKind::ConditionalExpression(expression) => is(expression.test.span()),
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
fn statements_before_exit_free(statements: &[Statement<'_>], child: oxc_span::Span) -> bool {
    for statement in statements {
        if statement.span() == child {
            return true;
        }
        let mut exits = EarlyExit::default();
        exits.visit_statement(statement);
        if exits.found {
            return false;
        }
    }
    false
}

/// Finds a `return`, `break` or `continue` outside every function nested in
/// the statement visited.
#[derive(Default)]
struct EarlyExit {
    found: bool,
}

impl<'a> Visit<'a> for EarlyExit {
    fn visit_return_statement(&mut self, _: &oxc_ast::ast::ReturnStatement<'a>) {
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
        unconditional_calls(Path::new("test.js"), source, span)
            .expect("parses")
            .into_iter()
            .map(|span| source[span.start as usize..span.end as usize].to_owned())
            .collect()
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
}
