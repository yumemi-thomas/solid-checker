//! Normal-path reachability of a site inside one function, by the abrupt
//! completion rules of structured control flow (the JLS 14.22 "can complete
//! normally" shape, specialized to JavaScript).
//!
//! The question is whether a path from the function's entry to a site exists
//! that no statement before it cuts off: a `return`, `throw`, `break` or
//! `continue` that always executes, a call the caller states never returns,
//! an infinite loop with no `break`, a `try` whose blocks only leave abruptly,
//! a `switch` every arm of which returns. It answers `true` only when every
//! enclosing construct is passed by one of the modelled rules, and a condition
//! it cannot decide is taken either way (the engine's feasible-path premise);
//! only a literal condition is decided.
//!
//! Every approximation errs toward `false` for what it cannot model (an
//! unmodelled loop form that may not terminate normally is treated as not
//! completing), so a `true` is a path that exists in the modelled language and a
//! `false` withholds a claim. `None` is a source that does not parse or a
//! function that is not found.
//!
//! `diverging` are call expressions the caller proves never return normally
//! (a `never` signature, a project function that only throws): a statement
//! that *is* such a call, or declares a variable from it, or returns it, does
//! not complete.

use std::collections::HashSet;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{Expression, FunctionBody, Statement, UnaryOperator};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};

use super::host_execution::inner;
use crate::core::Span;

/// Whether a normal path from the entry of the function at `function` reaches
/// `site`. See the module comment. `None` when the source does not parse or no
/// function has that span.
#[must_use]
pub fn site_reachable(
    path: &Path,
    source: &str,
    function: Span,
    site: Span,
    diverging: &[Span],
) -> Option<bool> {
    with_body(path, source, function, |body, expression, _| {
        if expression {
            return true;
        }
        let walk = Walk::new(diverging);
        walk.reach_seq(&body.statements, site, true)
            .unwrap_or(false)
    })
}

/// Whether a call of the function at `function` can return a value: its body
/// can complete normally, or a reachable `return` does not return a diverging
/// call. `false` is a function that always throws (or never ends). An `async`
/// function or a generator answers `true`: calling one never throws
/// synchronously. `None` when the source does not parse or no function has
/// that span.
#[must_use]
pub fn function_may_return(
    path: &Path,
    source: &str,
    function: Span,
    diverging: &[Span],
) -> Option<bool> {
    with_body(path, source, function, |body, expression, lenient| {
        if lenient {
            return true;
        }
        let walk = Walk::new(diverging);
        if expression {
            return match body.statements.first() {
                Some(Statement::ExpressionStatement(statement)) => {
                    !walk.diverges(&statement.expression)
                }
                _ => true,
            };
        }
        let out = walk.seq(&body.statements);
        out.normal || out.returns
    })
}

fn with_body<T>(
    path: &Path,
    source: &str,
    function: Span,
    answer: impl FnOnce(&FunctionBody<'_>, bool, bool) -> T,
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
            let body = function.body.as_ref()?;
            Some(answer(body, false, function.r#async || function.generator))
        }
        AstKind::ArrowFunctionExpression(arrow) => {
            Some(answer(&arrow.body, arrow.expression, arrow.r#async))
        }
        _ => None,
    }
}

/// How a statement can end, as far as the rules model it.
#[derive(Clone, Default)]
struct Out {
    /// Control can fall out of the end of the statement.
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

struct Walk {
    diverging: HashSet<Span>,
}

fn span_of(span: oxc_span::Span) -> Span {
    Span::new(span.start, span.end)
}

fn contains(outer: oxc_span::Span, site: Span) -> bool {
    outer.start <= site.start && site.end <= outer.end
}

impl Walk {
    fn new(diverging: &[Span]) -> Self {
        Self {
            diverging: diverging.iter().copied().collect(),
        }
    }

    /// Whether the expression is, behind sugar and an `await`, a call the
    /// caller proves never returns.
    fn diverges(&self, expression: &Expression<'_>) -> bool {
        let Some(mut expression) = inner(expression) else {
            return false;
        };
        loop {
            match expression {
                Expression::AwaitExpression(await_) => {
                    let Some(next) = inner(&await_.argument) else {
                        return false;
                    };
                    expression = next;
                }
                Expression::CallExpression(call) => {
                    return self.diverging.contains(&span_of(call.span));
                }
                _ => return false,
            }
        }
    }

    fn truth(expression: &Expression<'_>) -> Option<bool> {
        match inner(expression)? {
            Expression::BooleanLiteral(value) => Some(value.value),
            Expression::NumericLiteral(value) => Some(value.value != 0.0),
            Expression::NullLiteral(_) => Some(false),
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                Self::truth(&unary.argument).map(|value| !value)
            }
            _ => None,
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
                    .is_none_or(|argument| !self.diverges(argument)),
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
                if self.diverges(&statement.expression) {
                    Out::default()
                } else {
                    Out::normal()
                }
            }
            Statement::VariableDeclaration(declaration) => {
                if declaration.declarations.iter().any(|declarator| {
                    declarator
                        .init
                        .as_ref()
                        .is_some_and(|init| self.diverges(init))
                }) {
                    Out::default()
                } else {
                    Out::normal()
                }
            }
            Statement::BlockStatement(block) => self.seq(&block.body),
            Statement::IfStatement(statement) => {
                let truth = Self::truth(&statement.test);
                let consequent = || self.stmt(&statement.consequent);
                let alternate = || {
                    statement
                        .alternate
                        .as_ref()
                        .map_or_else(Out::normal, |alternate| self.stmt(alternate))
                };
                match truth {
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
                self.looping(Self::truth(&statement.test), &statement.body, false)
            }
            Statement::ForStatement(statement) => {
                let truth = statement
                    .test
                    .as_ref()
                    .map_or(Some(true), |test| Self::truth(test));
                self.looping(truth, &statement.body, false)
            }
            Statement::DoWhileStatement(statement) => {
                self.looping(Self::truth(&statement.test), &statement.body, true)
            }
            Statement::ForInStatement(statement) => self.each(&statement.body),
            Statement::ForOfStatement(statement) => self.each(&statement.body),
            Statement::SwitchStatement(statement) => {
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
                let block = self.seq(&statement.block.body);
                let handler = statement
                    .handler
                    .as_ref()
                    .map(|handler| self.seq(&handler.body.body));
                let mut out = Out {
                    normal: block.normal || handler.as_ref().is_some_and(|handler| handler.normal),
                    ..Out::default()
                };
                out.absorb(&block);
                if let Some(handler) = &handler {
                    out.absorb(handler);
                }
                if let Some(finalizer) = &statement.finalizer {
                    let finalizer = self.seq(&finalizer.body);
                    if finalizer.normal {
                        out.absorb(&finalizer);
                    } else {
                        // A `finally` that always leaves overrides every
                        // completion of the blocks.
                        out = finalizer;
                    }
                }
                out
            }
            Statement::WithStatement(statement) => self.stmt(&statement.body),
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

    fn each(&self, body: &Statement<'_>) -> Out {
        let body = self.stmt(body);
        let mut out = Out::normal();
        out.absorb(&body);
        out.brk = false;
        out
    }

    fn reach_seq(&self, statements: &[Statement<'_>], site: Span, mut entry: bool) -> Option<bool> {
        for statement in statements {
            if contains(statement.span(), site) {
                return self.reach_stmt(statement, site, entry);
            }
            if entry {
                entry = self.stmt(statement).normal;
            }
        }
        None
    }

    fn reach_stmt(&self, statement: &Statement<'_>, site: Span, entry: bool) -> Option<bool> {
        let in_body = |body: &Statement<'_>, entry: bool| {
            if contains(body.span(), site) {
                self.reach_stmt(body, site, entry)
            } else {
                Some(entry)
            }
        };
        match statement {
            Statement::BlockStatement(block) => self.reach_seq(&block.body, site, entry),
            Statement::IfStatement(statement) => {
                let truth = Self::truth(&statement.test);
                if contains(statement.consequent.span(), site) {
                    return self.reach_stmt(
                        &statement.consequent,
                        site,
                        entry && truth != Some(false),
                    );
                }
                if let Some(alternate) = &statement.alternate
                    && contains(alternate.span(), site)
                {
                    return self.reach_stmt(alternate, site, entry && truth != Some(true));
                }
                Some(entry)
            }
            Statement::LabeledStatement(statement) => in_body(&statement.body, entry),
            Statement::WhileStatement(statement) => in_body(
                &statement.body,
                entry && Self::truth(&statement.test) != Some(false),
            ),
            Statement::ForStatement(statement) => {
                let truth = statement
                    .test
                    .as_ref()
                    .map_or(Some(true), |t| Self::truth(t));
                in_body(&statement.body, entry && truth != Some(false))
            }
            Statement::DoWhileStatement(statement) => in_body(&statement.body, entry),
            Statement::ForInStatement(statement) => in_body(&statement.body, entry),
            Statement::ForOfStatement(statement) => in_body(&statement.body, entry),
            Statement::WithStatement(statement) => in_body(&statement.body, entry),
            Statement::SwitchStatement(statement) => {
                for case in &statement.cases {
                    if contains(case.span, site) {
                        return self
                            .reach_seq(&case.consequent, site, entry)
                            .or(Some(entry));
                    }
                }
                Some(entry)
            }
            Statement::TryStatement(statement) => {
                if contains(statement.block.span, site) {
                    return self
                        .reach_seq(&statement.block.body, site, entry)
                        .or(Some(entry));
                }
                if let Some(handler) = &statement.handler
                    && contains(handler.span, site)
                {
                    return self
                        .reach_seq(&handler.body.body, site, entry)
                        .or(Some(entry));
                }
                if let Some(finalizer) = &statement.finalizer
                    && contains(finalizer.span, site)
                {
                    return self.reach_seq(&finalizer.body, site, entry).or(Some(entry));
                }
                Some(entry)
            }
            _ => Some(entry),
        }
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
            ("try { return; } catch {} hit();", true),
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
        ] {
            let source = arrow(body);
            assert_eq!(reach(&source, "hit()", &[]), Some(expected), "{body}");
        }
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
    }
}
