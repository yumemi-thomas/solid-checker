//! A bounded syntactic cover of normal completions by exact call sites.
//! Symbols and unreachable-return evidence belong to the caller. This walk
//! joins alternative sites; it never upgrades either site's individual bound.
use std::{collections::HashSet, path::Path};

use oxc_allocator::Allocator;
use oxc_ast::{AstKind, ast::Statement};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;

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
        AstKind::ArrowFunctionExpression(arrow) if !arrow.r#async && !arrow.expression => {
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
    };
    let fallthrough = walk.statements(&bound.statements, 1, 0)?;
    // Bit 1: a path without a call. Bit 2: a path with a call.
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
}

impl Cover {
    fn statements(
        &mut self,
        statements: &[Statement<'_>],
        mut paths: u8,
        depth: usize,
    ) -> Option<u8> {
        for statement in statements {
            if paths == 0 {
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
                self.returns |= paths;
                Some(0)
            }
            Statement::ThrowStatement(_) => Some(0),
            Statement::ExpressionStatement(expression) => {
                let expression = super::peel_ts_sugar(&expression.expression);
                // Optional members can skip a non-optional outer call too.
                // Restrict to identifier callees; namespace/member witnesses
                // remain open rather than guessing optional-chain semantics.
                let hit = matches!(expression, oxc_ast::ast::Expression::CallExpression(call)
                    if !call.optional
                    && matches!(super::peel_ts_sugar(&call.callee), oxc_ast::ast::Expression::Identifier(_))
                    && self.candidates.contains(&(call.span.start, call.span.end)));
                Some(if hit { 2 } else { paths })
            }
            Statement::VariableDeclaration(_)
            | Statement::FunctionDeclaration(_)
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
