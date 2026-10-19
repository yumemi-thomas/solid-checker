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

/// [`completion_return_cover`] over statements the caller already parsed and
/// bound: whether every normal completion of `statements` returns one of the
/// exact peeled value spans in `candidates`. A throw completes without a
/// return and is neither; fallthrough, other returns, and unmodeled control
/// flow refuse.
pub(super) fn statements_return_cover(
    statements: &[Statement<'_>],
    candidates: HashSet<(u32, u32)>,
) -> Option<bool> {
    if candidates.is_empty() {
        return None;
    }
    let mut walk = Cover {
        candidates,
        dead_returns: HashSet::new(),
        returns: 0,
        budget: 4096,
        return_sites: true,
    };
    let fallthrough = walk.statements(statements, 1, 0)?;
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
mod tests;
