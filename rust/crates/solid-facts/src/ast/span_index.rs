//! Lazy per-file containment index over the sorted fact tables.
//!
//! Rule discovery asks span-containment questions per call, member, and
//! spread; scanning a fact array per question is quadratic within a file.
//! This index answers "which elements' key spans contain this span" in
//! O(log n + answer) via sorted starts and a prefix maximum of ends, and
//! yields candidates in original array order so callers can run the exact
//! filter/tie-break expressions they always ran, just over fewer elements.
//!
//! The index is derived data: it is skipped by serde, compares equal to any
//! other index, resets on clone, and is built at most once per `AstFacts`
//! instance on first use, so warm builds that never ask never pay.

use std::sync::OnceLock;

use crate::core::Span;

use crate::ast::{
    AstFacts, BindingFact, CallFact, ExportFact, FunctionFact, IdentifierFact, JsxElementFact,
    ReturnFact,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CallId(u32);

/// Lazily-built [`AstSpanIndex`] slot embedded in [`AstFacts`]. Transparent
/// to the derived trait implementations: clones start empty and any two
/// slots compare equal, because the index is a pure function of the tables.
#[derive(Default)]
pub struct LazySpanIndex(OnceLock<Box<AstSpanIndex>>);

impl Clone for LazySpanIndex {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for LazySpanIndex {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for LazySpanIndex {}

impl std::fmt::Debug for LazySpanIndex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LazySpanIndex")
    }
}

/// One key-span table sorted by (start, end) with a prefix maximum of ends.
#[derive(Debug, Default)]
struct SpanTable {
    starts: Vec<u32>,
    ends: Vec<u32>,
    prefix_max_end: Vec<u32>,
    payloads: Vec<u32>,
}

impl SpanTable {
    fn build(mut entries: Vec<(Span, u32)>) -> Self {
        entries.sort_by_key(|(span, payload)| (span.start, span.end, *payload));
        let mut table = Self {
            starts: Vec::with_capacity(entries.len()),
            ends: Vec::with_capacity(entries.len()),
            prefix_max_end: Vec::with_capacity(entries.len()),
            payloads: Vec::with_capacity(entries.len()),
        };
        let mut max_end = 0;
        for (span, payload) in entries {
            max_end = max_end.max(span.end);
            table.starts.push(span.start);
            table.ends.push(span.end);
            table.prefix_max_end.push(max_end);
            table.payloads.push(payload);
        }
        table
    }

    /// Payloads of entries whose key span contains `span`, in ascending
    /// payload order (== original array order for index payloads).
    fn containing(&self, span: Span) -> Vec<u32> {
        let mut out = Vec::new();
        let mut position = self.starts.partition_point(|start| *start <= span.start);
        while position > 0 {
            position -= 1;
            if self.prefix_max_end[position] < span.end {
                break;
            }
            if self.ends[position] >= span.end {
                out.push(self.payloads[position]);
            }
        }
        out.sort_unstable();
        out
    }

    fn contains_any(&self, span: Span) -> bool {
        let mut position = self.starts.partition_point(|start| *start <= span.start);
        while position > 0 {
            position -= 1;
            if self.prefix_max_end[position] < span.end {
                return false;
            }
            if self.ends[position] >= span.end {
                return true;
            }
        }
        false
    }
}

/// Per-file containment index over the span-sorted fact tables.
#[derive(Debug)]
pub struct AstSpanIndex {
    functions_by_body: SpanTable,
    jsx_by_span: SpanTable,
    exports_by_span: SpanTable,
    conditional_tests: SpanTable,
    /// Key = argument span; payload indexes `argument_slots`.
    arguments_by_span: SpanTable,
    argument_slots: Vec<(u32, u32)>,
    /// Key = binding initializer span (bindings without one are absent).
    bindings_by_initializer: SpanTable,
}

impl AstSpanIndex {
    fn build(facts: &AstFacts) -> Self {
        let spans = |iter: &mut dyn Iterator<Item = Span>| -> Vec<(Span, u32)> {
            iter.enumerate()
                .map(|(index, span)| (span, index as u32))
                .collect()
        };
        let mut argument_slots = Vec::new();
        let mut argument_entries = Vec::new();
        for (call_index, call) in facts.calls.iter().enumerate() {
            for (argument_index, argument) in call.arguments.iter().enumerate() {
                argument_entries.push((argument.span, argument_slots.len() as u32));
                argument_slots.push((call_index as u32, argument_index as u32));
            }
        }
        Self {
            functions_by_body: SpanTable::build(spans(
                &mut facts.functions.iter().map(|function| function.body),
            )),
            jsx_by_span: SpanTable::build(spans(
                &mut facts.jsx_elements.iter().map(|element| element.span),
            )),
            exports_by_span: SpanTable::build(spans(
                &mut facts.exports.iter().map(|export| export.span),
            )),
            conditional_tests: SpanTable::build(spans(
                &mut facts.conditional_tests.iter().copied(),
            )),
            arguments_by_span: SpanTable::build(argument_entries),
            argument_slots,
            bindings_by_initializer: SpanTable::build(
                facts
                    .bindings
                    .iter()
                    .enumerate()
                    .filter_map(|(index, binding)| {
                        binding
                            .initializer
                            .map(|initializer| (initializer, index as u32))
                    })
                    .collect(),
            ),
        }
    }
}

fn within<T>(elements: &[T], region: Span, span_of: fn(&T) -> Span) -> impl Iterator<Item = &T> {
    let start = elements.partition_point(move |element| span_of(element).start < region.start);
    elements[start..]
        .iter()
        .take_while(move |element| span_of(element).start <= region.end)
        .filter(move |element| region.contains(span_of(element)))
}

impl AstFacts {
    pub fn span_index(&self) -> &AstSpanIndex {
        self.span_index
            .0
            .get_or_init(|| Box::new(AstSpanIndex::build(self)))
    }

    /// Typed handle for the call with exactly this span.
    pub fn call_id_at(&self, span: Span) -> Option<CallId> {
        self.calls
            .binary_search_by_key(&span, |call| call.span)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(CallId)
    }

    pub fn call(&self, id: CallId) -> &CallFact {
        &self.calls[id.0 as usize]
    }

    pub fn call_at(&self, span: Span) -> Option<&CallFact> {
        self.call_id_at(span).map(|id| self.call(id))
    }

    /// Functions whose body contains `span`, in original array order.
    pub fn functions_body_containing(&self, span: Span) -> impl Iterator<Item = &FunctionFact> {
        self.span_index()
            .functions_by_body
            .containing(span)
            .into_iter()
            .map(|index| &self.functions[index as usize])
    }

    /// Whether any function body contains `span`.
    pub fn any_function_body_containing(&self, span: Span) -> bool {
        self.span_index().functions_by_body.contains_any(span)
    }

    /// JSX elements whose span contains `span`, in original array order.
    pub fn jsx_containing(&self, span: Span) -> impl Iterator<Item = &JsxElementFact> {
        self.span_index()
            .jsx_by_span
            .containing(span)
            .into_iter()
            .map(|index| &self.jsx_elements[index as usize])
    }

    /// Exports whose span contains `span`, in original array order.
    pub fn exports_containing(&self, span: Span) -> impl Iterator<Item = &ExportFact> {
        self.span_index()
            .exports_by_span
            .containing(span)
            .into_iter()
            .map(|index| &self.exports[index as usize])
    }

    /// Whether any conditional test span contains `span`.
    pub fn any_conditional_test_containing(&self, span: Span) -> bool {
        self.span_index().conditional_tests.contains_any(span)
    }

    /// `(call, argument index)` pairs whose argument span contains `span`,
    /// ordered by (call, argument) — the order a nested array scan visits.
    pub fn arguments_containing(&self, span: Span) -> impl Iterator<Item = (&CallFact, usize)> {
        let index = self.span_index();
        let mut slots = index
            .arguments_by_span
            .containing(span)
            .into_iter()
            .map(|slot| index.argument_slots[slot as usize])
            .collect::<Vec<_>>();
        slots.sort_unstable();
        slots
            .into_iter()
            .map(|(call, argument)| (&self.calls[call as usize], argument as usize))
    }

    /// Whether any JSX element span contains `span`.
    pub fn any_jsx_containing(&self, span: Span) -> bool {
        self.span_index().jsx_by_span.contains_any(span)
    }

    /// Bindings whose initializer span contains `span`, in original order.
    pub fn bindings_initializer_containing(
        &self,
        span: Span,
    ) -> impl Iterator<Item = &BindingFact> {
        self.span_index()
            .bindings_by_initializer
            .containing(span)
            .into_iter()
            .map(|index| &self.bindings[index as usize])
    }

    /// Functions whose span lies inside `region`, in original array order.
    pub fn functions_within(&self, region: Span) -> impl Iterator<Item = &FunctionFact> {
        within(&self.functions, region, |function| function.span)
    }

    /// Calls whose span lies inside `region`, in original array order.
    pub fn calls_within(&self, region: Span) -> impl Iterator<Item = &CallFact> {
        within(&self.calls, region, |call| call.span)
    }

    /// JSX elements whose span lies inside `region`, in original array order.
    pub fn jsx_within(&self, region: Span) -> impl Iterator<Item = &JsxElementFact> {
        within(&self.jsx_elements, region, |element| element.span)
    }

    /// Returns whose span lies inside `region`, in original array order.
    pub fn returns_within(&self, region: Span) -> impl Iterator<Item = &ReturnFact> {
        within(&self.returns, region, |fact| fact.span)
    }

    /// Identifiers whose span lies inside `region`, in original array order.
    pub fn identifiers_within(&self, region: Span) -> impl Iterator<Item = &IdentifierFact> {
        within(&self.identifiers, region, |identifier| identifier.span)
    }
}
