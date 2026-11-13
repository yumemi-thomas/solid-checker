//! Exact array-literal slots without exporting parser nodes. Spreads and
//! holes refuse; an absent slot is not evidence of an unrelated value.

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{ArrayExpression, ArrayExpressionElement, Super, ThisExpression};
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

/// Exact keyword spans of every expression that reads the object a function was
/// called on: each `this` and each `super` (`super.x` reads the prototype's `x`
/// with the same `this`, so a getter there runs against the object). Syntax only,
/// never the identity of the receiver. `None` when the file does not parse.
#[must_use]
pub fn receiver_expression_spans(path: &Path, source: &str) -> Option<Vec<Span>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::from_path(path).ok()?).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut uses = ReceiverUses(Vec::new());
    uses.visit_program(&parsed.program);
    Some(uses.0)
}

struct ReceiverUses(Vec<Span>);

impl<'a> Visit<'a> for ReceiverUses {
    fn visit_this_expression(&mut self, expression: &ThisExpression) {
        self.0
            .push(Span::new(expression.span.start, expression.span.end));
    }

    fn visit_super(&mut self, expression: &Super) {
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

    /// `this` and `super` both read the object a function was called on. A type
    /// annotation's `this: T` parameter, a string and a comment name neither.
    #[test]
    fn receiver_spans_hold_every_this_and_super_and_nothing_else() {
        let source = concat!(
            "const base = { get x() { return 1; } };\n",
            "const o = { m(this: { g: number }) { return this.g + super.x; }, k: () => 1 };\n",
            "const text = 'this super';\n",
            "// this super\n",
        );
        let words = |spans: Vec<Span>| {
            spans
                .into_iter()
                .map(|span| {
                    (
                        span.start as usize,
                        &source[span.start as usize..span.end as usize],
                    )
                })
                .collect::<Vec<_>>()
        };
        let receivers = words(receiver_expression_spans(Path::new("fixture.ts"), source).unwrap());
        let this_only = words(this_expression_spans(Path::new("fixture.ts"), source).unwrap());
        assert_eq!(
            receivers.iter().map(|(_, word)| *word).collect::<Vec<_>>(),
            ["this", "super"]
        );
        assert_eq!(
            this_only.iter().map(|(_, word)| *word).collect::<Vec<_>>(),
            ["this"]
        );
        assert!(receiver_expression_spans(Path::new("fixture.ts"), "const = ;").is_none());
    }
}
