//! A bounded syntactic cover of normal completions by exact call sites.
//! Symbols and unreachable-return evidence belong to the caller. This walk
//! joins alternative sites; it never upgrades either site's individual bound.
use std::{collections::HashSet, path::Path};

use oxc_allocator::Allocator;
use oxc_ast::{AstKind, ast::Statement};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};

use crate::core::Span;

/// Whether every normal completion of this exact synchronous block body
/// executes at least one candidate call. `None` means unmodeled syntax or an
/// unbound frame. Throwing-only bodies do not prove a positive occurrence.
///
/// Unreachable returns are exact, separately authenticated producer facts.
/// Treating them as fallthrough overapproximates paths, so it cannot manufacture
/// a cover. Candidate expressions must be direct, non-optional call statements.
#[must_use]
pub fn completion_call_cover(
    path: &Path,
    source: &str,
    body: Span,
    candidates: &[Span],
    unreachable_returns: &[Span],
) -> Option<bool> {
    completion_cover(path, source, body, candidates, unreachable_returns, false)
}

/// Whether every normal completion of this synchronous block body exits at
/// a return of one of these exact peeled value spans. Candidate identity belongs to
/// the caller. Fallthrough, bare returns, and unmodeled control flow refuse.
/// Unlike a call cover, running a candidate initializer never closes this
/// proof: a later finally/catch or fallthrough can still change the return.
#[must_use]
pub fn completion_return_cover(
    path: &Path,
    source: &str,
    body: Span,
    candidates: &[Span],
) -> Option<bool> {
    completion_cover(path, source, body, candidates, &[], true)
}

fn completion_cover(
    path: &Path,
    source: &str,
    body: Span,
    candidates: &[Span],
    unreachable_returns: &[Span],
    return_sites: bool,
) -> Option<bool> {
    if source.len() > 4 * 1024 * 1024 || candidates.is_empty() {
        return None;
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        source,
        SourceType::from_path(path).ok()?.with_module(true),
    )
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let nodes = built.semantic.nodes();
    let bound = nodes.iter().find_map(|node| match node.kind() {
        AstKind::Function(function) if !function.r#async && !function.generator => function
            .body
            .as_ref()
            .filter(|value| same(value.span, body)),
        // An expression body (`x => a()`) is one expression statement that
        // runs exactly once and completes the arrow (ADR 0183).
        AstKind::ArrowFunctionExpression(arrow) if !arrow.r#async => {
            same(arrow.body.span, body).then_some(&arrow.body)
        }
        _ => None,
    })?;
    let mut walk = Cover {
        candidates: candidates
            .iter()
            .map(|span| (span.start, span.end))
            .collect(),
        dead_returns: unreachable_returns
            .iter()
            .map(|span| (span.start, span.end))
            .collect(),
        returns: 0,
        budget: 4096,
        return_sites,
    };
    let fallthrough = walk.statements(&bound.statements, 1, 0)?;
    // Bit 1: an uncovered completion. Bit 2: a covered completion.
    Some((fallthrough | walk.returns) == 2)
}

fn same(actual: oxc_span::Span, expected: Span) -> bool {
    actual.start == expected.start && actual.end == expected.end
}

struct Cover {
    candidates: HashSet<(u32, u32)>,
    dead_returns: HashSet<(u32, u32)>,
    returns: u8,
    budget: usize,
    return_sites: bool,
}

impl Cover {
    /// Whether `expression` is, whole, one candidate call. Optional members
    /// can skip a non-optional outer call too, so callees are restricted to
    /// identifiers; namespace/member witnesses remain open rather than
    /// guessing optional-chain semantics.
    fn candidate(&self, expression: &oxc_ast::ast::Expression<'_>) -> bool {
        !self.return_sites
            && matches!(super::peel_ts_sugar(expression), oxc_ast::ast::Expression::CallExpression(call)
            if !call.optional
            && matches!(super::peel_ts_sugar(&call.callee), oxc_ast::ast::Expression::Identifier(_))
            && self.candidates.contains(&(call.span.start, call.span.end)))
    }

    fn statements(
        &mut self,
        statements: &[Statement<'_>],
        mut paths: u8,
        depth: usize,
    ) -> Option<u8> {
        for statement in statements {
            // No live path, or every live path has already run a candidate:
            // what follows cannot change either answer (ADR 0183), so it is
            // not walked, and syntax this walk does not model there does not
            // refuse a cover already made.
            if paths == 0 || paths == 2 {
                break;
            }
            paths = self.statement(statement, paths, depth)?;
        }
        Some(paths)
    }

    fn statement(&mut self, statement: &Statement<'_>, paths: u8, depth: usize) -> Option<u8> {
        if depth > 32 || self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        match statement {
            Statement::BlockStatement(block) => self.statements(&block.body, paths, depth + 1),
            Statement::IfStatement(branch) => {
                let yes = self.statement(&branch.consequent, paths, depth + 1)?;
                let no = match &branch.alternate {
                    Some(alternate) => self.statement(alternate, paths, depth + 1)?,
                    None => paths,
                };
                Some(yes | no)
            }
            Statement::ReturnStatement(returned) => {
                if self
                    .dead_returns
                    .contains(&(returned.span.start, returned.span.end))
                {
                    return Some(paths);
                }
                // `return a()` runs the call before it completes (ADR 0183).
                let hit = if self.return_sites {
                    returned.argument.as_ref().is_some_and(|argument| {
                        let span = super::peel_ts_sugar(argument).span();
                        self.candidates.contains(&(span.start, span.end))
                    })
                } else {
                    returned
                        .argument
                        .as_ref()
                        .is_some_and(|argument| self.candidate(argument))
                };
                self.returns |= if hit { 2 } else { paths };
                Some(0)
            }
            Statement::ThrowStatement(_) => Some(0),
            Statement::ExpressionStatement(expression) => {
                let hit = self.candidate(&expression.expression);
                Some(if hit { 2 } else { paths })
            }
            // `const x = a()` runs the call whenever the declaration completes
            // (ADR 0183). Only a whole initializer counts; a call nested in one
            // (`const x = flag && a()`) does not.
            Statement::VariableDeclaration(declaration) => {
                let hit = declaration.declarations.iter().any(|declarator| {
                    declarator
                        .init
                        .as_ref()
                        .is_some_and(|init| self.candidate(init))
                });
                Some(if hit { 2 } else { paths })
            }
            Statement::FunctionDeclaration(_)
            | Statement::ClassDeclaration(_)
            | Statement::EmptyStatement(_) => Some(paths),
            // Catch/finally, loops, switch, labels, with and jumps need their
            // own execution model. Refuse them even when they appear harmless.
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_cover_requires_a_candidate_on_every_normal_completion() {
        for (body, expected) in [
            ("return state;", Some(true)),
            ("if(flag) return state; else return state;", Some(true)),
            ("if(flag) return state; return state;", Some(true)),
            ("if(flag) throw 0; return state;", Some(true)),
            ("if(flag) return state;", Some(false)),
            ("if(flag) return; return state;", Some(false)),
            ("if(flag) return other; return state;", Some(false)),
            ("try { return state; } finally { return other; }", None),
            ("try { throw 0; } catch { return state; }", None),
            ("while(flag) { return state; }", None),
            ("switch(flag) { default: return state; }", None),
        ] {
            let source = format!("function test(flag) {{ {body} }}");
            let candidates = source
                .match_indices("return state;")
                .map(|(at, text)| {
                    Span::new((at + "return ".len()) as u32, (at + text.len() - 1) as u32)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                completion_return_cover(
                    Path::new("test.js"),
                    &source,
                    Span::new(source.find('{').unwrap() as u32, source.len() as u32),
                    &candidates,
                ),
                expected,
                "{body}"
            );
        }
    }

    fn cover(body: &str, targets: &[&str], dead: &[&str]) -> Option<bool> {
        let source = format!("function test(flag) {{ {body} }}");
        let spans = |needles: &[&str]| {
            needles
                .iter()
                .flat_map(|needle| {
                    source
                        .match_indices(needle)
                        .map(|(start, _)| Span::new(start as u32, (start + needle.len()) as u32))
                })
                .collect::<Vec<_>>()
        };
        completion_call_cover(
            Path::new("test.js"),
            &source,
            Span::new(source.find('{').unwrap() as u32, source.len() as u32),
            &spans(targets),
            &spans(dead),
        )
    }

    #[test]
    fn completion_cover_joins_both_branches_but_not_missing_or_early_paths() {
        assert_eq!(
            cover("if(flag) a(); else b();", &["a()", "b()"], &[]),
            Some(true)
        );
        assert_eq!(cover("if(flag) a();", &["a()"], &[]), Some(false));
        assert_eq!(
            cover("if(flag) a(); else other();", &["a()"], &[]),
            Some(false)
        );
        assert_eq!(
            cover(
                "if(flag) return; if(flag) a(); else b();",
                &["a()", "b()"],
                &[]
            ),
            Some(false)
        );
        assert_eq!(
            cover("if(flag) { a(); return; } else b();", &["a()", "b()"], &[]),
            Some(true)
        );
        assert_eq!(cover("if(flag) throw 0; a();", &["a()"], &[]), Some(true));
        assert_eq!(cover("throw 0; a();", &["a()"], &[]), Some(false));
    }

    #[test]
    fn completion_cover_refuses_unmodeled_flow_and_nested_or_optional_calls() {
        for body in [
            "try { a(); } catch {}",
            "for(;;) a();",
            "switch(flag) { default: a(); }",
        ] {
            assert_eq!(cover(body, &["a()"], &[]), None);
        }
        for body in [
            "const later = () => a();",
            "flag && a();",
            "a?.();",
            "obj?.a();",
        ] {
            assert_eq!(cover(body, &["a()", "a?.()", "obj?.a()"], &[]), Some(false));
        }
    }

    #[test]
    fn completion_cover_counts_a_whole_initializer_or_returned_call() {
        assert_eq!(cover("const x = a();", &["a()"], &[]), Some(true));
        assert_eq!(cover("const y = 1, x = a();", &["a()"], &[]), Some(true));
        assert_eq!(cover("return a();", &["a()"], &[]), Some(true));
        assert_eq!(
            cover("if(flag) return a(); b();", &["a()", "b()"], &[]),
            Some(true)
        );
        assert_eq!(
            cover("if(flag) return 1; return a();", &["a()"], &[]),
            Some(false)
        );
        assert_eq!(
            cover("const x = a(); for (const k in x) b(k);", &["a()"], &[]),
            Some(true)
        );
        assert_eq!(
            cover("for (const k in flag) b(k); const x = a();", &["a()"], &[]),
            None
        );
        for body in [
            "const x = flag && a();",
            "const x = () => a();",
            "return flag ? a() : 0;",
        ] {
            assert_eq!(cover(body, &["a()"], &[]), Some(false), "{body}");
        }
    }

    #[test]
    fn completion_cover_reads_an_expression_bodied_arrow_as_its_one_statement() {
        let source = "const f = flag => a();";
        let call = source.find("a()").unwrap() as u32;
        let body = Span::new(call, call + 3);
        assert_eq!(
            completion_call_cover(Path::new("test.js"), source, body, &[body], &[]),
            Some(true)
        );
        let source = "const f = flag => flag && a();";
        let call = source.find("a()").unwrap() as u32;
        let start = source.find("flag &&").unwrap() as u32;
        assert_eq!(
            completion_call_cover(
                Path::new("test.js"),
                source,
                Span::new(start, call + 3),
                &[Span::new(call, call + 3)],
                &[]
            ),
            Some(false)
        );
    }

    #[test]
    fn completion_cover_uses_dead_returns_only_as_extra_fallthrough() {
        assert_eq!(
            cover("if(flag) return; a();", &["a()"], &["return;"]),
            Some(true)
        );
        assert_eq!(
            cover("if(flag) return; else a();", &["a()"], &["return;"]),
            Some(false)
        );
    }
}
