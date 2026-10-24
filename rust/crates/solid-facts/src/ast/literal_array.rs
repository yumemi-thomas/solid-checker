//! Exact array-literal slots without exporting parser nodes. Spreads and
//! holes refuse; an absent slot is not evidence of an unrelated value.

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{ArrayExpression, ArrayExpressionElement, ThisExpression};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

/// Exact keyword spans for consumers that census lexical/dynamic receiver
/// uses. This supplies syntax only, never the identity of the receiver.
#[must_use]
pub fn this_expression_spans(path: &Path, source: &str) -> Option<Vec<Span>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::from_path(path).ok()?).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut uses = ThisUses(Vec::new());
    uses.visit_program(&parsed.program);
    Some(uses.0)
}

struct ThisUses(Vec<Span>);

impl<'a> Visit<'a> for ThisUses {
    fn visit_this_expression(&mut self, expression: &ThisExpression) {
        self.0
            .push(Span::new(expression.span.start, expression.span.end));
    }
}

#[must_use]
pub fn literal_array_element(
    path: &Path,
    source: &str,
    literal: Span,
    index: usize,
) -> Option<Span> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::from_path(path).ok()?).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut slots = Slot {
        literal,
        index,
        result: None,
    };
    slots.visit_program(&parsed.program);
    slots.result
}

/// What [`this_expression_spans`] and [`literal_array_element`] read, in
/// source order: every `this` keyword (tag zero), and every array literal
/// (tag one) followed by its elements (tag two for an expression, three for
/// a spread, four for a hole). Two sources with the same list answer both
/// the same way at every span. `None` when the source does not parse (both
/// then answer `None`).
#[must_use]
pub fn receiver_and_array_syntax(path: &Path, source: &str) -> Option<Vec<(u8, Span)>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::from_path(path).ok()?).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut syntax = Syntax(Vec::new());
    syntax.visit_program(&parsed.program);
    Some(syntax.0)
}

struct Syntax(Vec<(u8, Span)>);

impl<'a> Visit<'a> for Syntax {
    fn visit_this_expression(&mut self, expression: &ThisExpression) {
        self.0
            .push((0, Span::new(expression.span.start, expression.span.end)));
    }

    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        self.0
            .push((1, Span::new(array.span.start, array.span.end)));
        for element in &array.elements {
            let tag = match element {
                ArrayExpressionElement::SpreadElement(_) => 3,
                ArrayExpressionElement::Elision(_) => 4,
                _ => 2,
            };
            let span = element.span();
            self.0.push((tag, Span::new(span.start, span.end)));
        }
        walk::walk_array_expression(self, array);
    }
}

struct Slot {
    literal: Span,
    index: usize,
    result: Option<Span>,
}

impl<'a> Visit<'a> for Slot {
    fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
        if array.span.start == self.literal.start && array.span.end == self.literal.end {
            if array
                .elements
                .iter()
                .any(|e| matches!(e, ArrayExpressionElement::SpreadElement(_)))
            {
                return;
            }
            if let Some(element) = array.elements.get(self.index)
                && !matches!(element, ArrayExpressionElement::Elision(_))
            {
                let span = element.span();
                self.result = Some(Span::new(span.start, span.end));
            }
        } else {
            walk::walk_array_expression(self, array);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_exact_and_spreads_or_holes_refuse() {
        for (literal, index, expected) in [
            ("[location]", 0, Some("location")),
            ("[, { target: location }]", 1, Some("{ target: location }")),
            ("[, location]", 0, None),
            ("[...input, location]", 1, None),
            ("[]", 0, None),
        ] {
            let source = format!("const values = {literal};");
            let start = u32::try_from(source.find('[').unwrap()).unwrap();
            let end = start + u32::try_from(literal.len()).unwrap();
            let result = literal_array_element(
                Path::new("fixture.ts"),
                &source,
                Span::new(start, end),
                index,
            );
            assert_eq!(
                result.map(|s| &source[s.start as usize..s.end as usize]),
                expected
            );
        }
    }

    /// A same-length edit that adds a `this` use or reshapes an array literal
    /// moves the syntax both helpers read.
    #[test]
    fn receiver_and_array_syntax_moves_with_this_and_array_slots() {
        let syntax = |source: &str| receiver_and_array_syntax(Path::new("fixture.ts"), source);
        let base = syntax("const o = { m() { void 0; } };\nconst a = [x, 1, 2];\n");
        assert!(base.is_some());
        assert_ne!(
            base,
            syntax("const o = { m() { this;   } };\nconst a = [x, 1, 2];\n")
        );
        assert_ne!(
            base,
            syntax("const o = { m() { void 0; } };\nconst a = [x, 12  ];\n")
        );
        // A literal's text is not syntax either reads.
        assert_eq!(
            base,
            syntax("const o = { m() { void 1; } };\nconst a = [x, 3, 4];\n")
        );
        assert_eq!(syntax("const o = {"), None);
    }
}
