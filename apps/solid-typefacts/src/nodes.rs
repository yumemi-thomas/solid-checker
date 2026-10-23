//! Node lookup by byte position (Go: `deepestNodeAt` of `tsgo/project.go` and
//! `semanticNodeCursor`, `jsxTagNameAt`, `symbolSpanCovers` of
//! `tsgo/semantic_runs.go` and `semantic_entities.go`).
//!
//! Every function here reads syntax only and returns nodes that belong to the
//! generation whose file they were found in. Callers use them inside one
//! backend call and copy results out (`unsafe_boundary` invariant 1).

use tsrs_ast::{
    Kind, Node, SourceFile, get_node_at_position, is_call_expression, is_expression,
    is_new_expression,
};
use tsrs_core::P;
use tsrs_scanner::skip_trivia;

/// Go: `node.Pos() <= position && position < node.End()`. A synthesized
/// position never contains one.
fn contains(node: P<Node>, position: u32) -> bool {
    match (node.pos().source_offset(), node.end().source_offset()) {
        (Some(pos), Some(end)) => pos <= position && position < end,
        _ => false,
    }
}

/// Go: `deepestNodeAt`.
#[must_use]
pub fn deepest_node_at(node: P<Node>, position: u32) -> P<Node> {
    let mut best = node;
    node.for_each_child(&mut |child| {
        if contains(child, position) {
            best = deepest_node_at(child, position);
            return true;
        }
        false
    });
    best
}

/// Go: `deepestNodeAt(ast.GetNodeAtPosition(file, position, false), position)`.
#[must_use]
pub fn node_at(file: P<SourceFile>, position: u32) -> P<Node> {
    deepest_node_at(get_node_at_position(file, position, false), position)
}

/// Go: `deepestNodeCovering`: the smallest node containing `[start, end)`, or
/// `None` when `node` does not contain it.
#[must_use]
pub fn deepest_node_covering(node: P<Node>, start: u32, end: u32) -> Option<P<Node>> {
    let (pos, node_end) = (node.pos().source_offset()?, node.end().source_offset()?);
    if pos > start || node_end < end {
        return None;
    }
    let mut best = node;
    node.for_each_child(&mut |child| {
        let (Some(child_pos), Some(child_end)) =
            (child.pos().source_offset(), child.end().source_offset())
        else {
            return false;
        };
        if child_pos <= start && end <= child_end {
            if let Some(candidate) = deepest_node_covering(child, start, end) {
                best = candidate;
            }
            return true;
        }
        false
    });
    Some(best)
}

/// Go: `isCallLikeExpression`.
#[must_use]
pub fn is_call_like_expression(node: P<Node>) -> bool {
    is_call_expression(node) || is_new_expression(node)
}

/// The position of the first token of `node`, with leading trivia skipped
/// (`scanner.SkipTrivia(text, node.Pos())`, which is how every producer
/// location is spelled).
fn token_start(file: P<SourceFile>, node: P<Node>) -> Option<u32> {
    Some(skip_trivia(file.text(), node.pos().source_offset()?))
}

/// A top-level statement with its source offsets: (pos, end, node).
type Statement = (u32, u32, P<Node>);

/// Go: `semanticNodeCursor`. Demands in a run arrive in position order and
/// often repeat a start byte, so the last answer is remembered.
pub struct NodeCursor {
    file: Option<P<SourceFile>>,
    last: Option<(u32, Option<P<Node>>)>,
    /// The file's top-level statements with their source offsets, built on the
    /// first covering query; `Some(None)` when they are not usable (see
    /// [`NodeCursor::top_level`]).
    top: Option<Option<Vec<Statement>>>,
}

impl NodeCursor {
    #[must_use]
    pub fn new(file: Option<P<SourceFile>>) -> Self {
        Self {
            file,
            last: None,
            top: None,
        }
    }

    /// Go: `at`. `None` for a missing file or a position past `u32`.
    pub fn at(&mut self, position: u64) -> Option<P<Node>> {
        let file = self.file?;
        let position = u32::try_from(position).ok()?;
        if let Some((cached, node)) = self.last
            && cached == position
        {
            return node;
        }
        let node = Some(node_at(file, position));
        self.last = Some((position, node));
        node
    }

    /// Go: `covering`: the smallest node containing the complete demanded
    /// range. Type and value queries describe expressions, not merely the
    /// deepest token at their first byte: `factory()` and `factory` start at
    /// the same offset but have different runtime domains and callability.
    pub fn covering(&mut self, start: u64, end: u64) -> Option<P<Node>> {
        if end <= start {
            return self.at(start);
        }
        let file = self.file?;
        let (start, end) = (u32::try_from(start).ok()?, u32::try_from(end).ok()?);
        let root = file.as_node();
        if let Some(statements) = self.top_level(file) {
            // `deepest_node_covering(root, ..)` descends into the first child
            // that holds the range. Statement ends never decrease here, so
            // every statement before the first one ending at or after `end`
            // cannot hold it, and that statement is the first candidate.
            // Anything else (no statement holds it, a position outside the
            // root) takes the full walk.
            let (root_pos, root_end) = (root.pos().source_offset()?, root.end().source_offset()?);
            if root_pos <= start && end <= root_end {
                let index =
                    statements.partition_point(|(_, statement_end, _)| *statement_end < end);
                if let Some((statement_pos, _, statement)) = statements.get(index)
                    && *statement_pos <= start
                {
                    return deepest_node_covering(*statement, start, end).or(Some(root));
                }
            }
        }
        deepest_node_covering(root, start, end)
    }

    /// The top-level statements of `file` as (pos, end, node), when every one
    /// has a source position and their ends are non-decreasing (always true of
    /// a parsed file; checked rather than assumed). Built once per cursor.
    fn top_level(&mut self, file: P<SourceFile>) -> Option<&[Statement]> {
        self.top
            .get_or_insert_with(|| {
                let mut statements = Vec::with_capacity(file.as_node().statements().len());
                let mut previous_end = 0;
                for statement in file.as_node().statements() {
                    let (pos, end) = (
                        statement.pos().source_offset()?,
                        statement.end().source_offset()?,
                    );
                    if end < previous_end {
                        return None;
                    }
                    previous_end = end;
                    statements.push((pos, end, *statement));
                }
                Some(statements)
            })
            .as_deref()
    }

    /// Go: `exactCallLikeAt`: the call-like expression reached from `start`
    /// only when its emitted source span is exactly `[start, end)`. `Pos`
    /// includes leading trivia, while all producer locations skip it. Walking
    /// parents preserves `resolvedCall`'s call-like lookup, but the end check
    /// keeps a callee or enclosing expression from answering.
    pub fn exact_call_like_at(&mut self, start: u64, end: u64) -> Option<P<Node>> {
        self.exact_at(start, end, is_call_like_expression)
    }

    /// Go: `exactExpressionAt`: only an expression whose trivia-normalized
    /// span equals the demand. Unlike [`NodeCursor::covering`], it can never
    /// answer from a child of the requested expression.
    pub fn exact_expression_at(&mut self, start: u64, end: u64) -> Option<P<Node>> {
        if end <= start {
            return None;
        }
        self.exact_at(start, end, is_expression)
    }

    fn exact_at(
        &mut self,
        start: u64,
        end: u64,
        accepts: impl Fn(P<Node>) -> bool,
    ) -> Option<P<Node>> {
        let file = self.file?;
        let (start32, end32) = (u32::try_from(start).ok()?, u32::try_from(end).ok()?);
        let mut node = self.at(start);
        while let Some(current) = node {
            if accepts(current)
                && current.end().source_offset() == Some(end32)
                && token_start(file, current) == Some(start32)
            {
                return Some(current);
            }
            node = current.parent();
        }
        None
    }

    /// Go: `resolvedCallLikeAt`: keeps the historical start-byte anchor for
    /// `resolvedCall`, but chooses the outermost call-like ancestor contained
    /// by the demanded span. Chained calls share a start byte, so stopping at
    /// the first ancestor would describe `factory()` instead of
    /// `factory()(value)`.
    pub fn resolved_call_like_at(&mut self, start: u64, end: u64) -> Option<P<Node>> {
        let mut first: Option<P<Node>> = None;
        let mut best: Option<P<Node>> = None;
        let mut node = self.at(start);
        while let Some(current) = node {
            if is_call_like_expression(current) {
                if first.is_none() {
                    first = Some(current);
                }
                let current_end = current.end().source_offset();
                if current_end.is_some_and(|value| u64::from(value) <= end)
                    && best.is_none_or(|held| current_end > held.end().source_offset())
                {
                    best = Some(current);
                }
            }
            node = current.parent();
        }
        best.or(first)
    }
}

/// Go: `jsxTagNameAt`: normalizes a node selected from a complete JSX name
/// span to the complete compiler tag-name expression. A deliberately narrower
/// demand, such as `Runtime` within `Runtime.Component`, keeps the identifier
/// node so callers can ask whether the member root itself resolves.
#[must_use]
pub fn jsx_tag_name_at(node: P<Node>, start: u64, end: u64) -> P<Node> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        match candidate.kind() {
            Kind::JsxOpeningElement | Kind::JsxSelfClosingElement | Kind::JsxClosingElement => {
                let tag_name = candidate.tag_name();
                // `location.StartByte != tagName.Pos()`: the raw position, not
                // the trivia-skipped one, exactly as Go compares it.
                let same = tag_name.pos().source_offset().map(u64::from) == Some(start)
                    && tag_name.end().source_offset().map(u64::from) == Some(end);
                return if same { tag_name } else { node };
            }
            _ => current = candidate.parent(),
        }
    }
    node
}

/// Go: `symbolSpanCovers`: whether `node` is the whole demanded span, with
/// the trivia-free start the producer's locations use. The cursor answers the
/// node at the demand's first byte, which for `f(x)` is the callee `f` and for
/// `o.p` is `o`: a symbol taken from such a node names a token inside the
/// demand, not the value the demand describes. Identifiers that are the whole
/// span (specifiers, bindings, JSX tag names) cover it like any other node.
#[must_use]
pub fn symbol_span_covers(file: P<SourceFile>, node: P<Node>, start: u64, end: u64) -> bool {
    node.end().source_offset().map(u64::from) == Some(end)
        && token_start(file, node).map(u64::from) == Some(start)
}
