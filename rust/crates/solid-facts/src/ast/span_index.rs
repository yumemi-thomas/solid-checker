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

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::core::Span;

use crate::ast::{
    AstFacts, BindingFact, CallFact, ExportFact, FunctionFact, IdentifierFact, IdentifierRole,
    JsxElementFact, MemberFact, ObjectPropertyFact, ReturnFact,
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

/// One key-span table sorted by (start, end), with, for each entry, the
/// nearest earlier entry that ends strictly later. "Which earlier entries end
/// at or after `x`" then skips a whole run of entries that all end before
/// `x` in one step. A running prefix maximum cannot do that: once one early
/// entry spans the file (a function wrapping the module), it never prunes and
/// every query walks the whole prefix.
#[derive(Debug, Default)]
struct SpanTable {
    starts: Vec<u32>,
    ends: Vec<u32>,
    payloads: Vec<u32>,
    /// `previous_longer[i]`: the nearest `j < i` with `ends[j] > ends[i]`, or
    /// `NONE`. Every entry between `j` and `i` ends at or before `ends[i]`.
    previous_longer: Vec<u32>,
}

const NONE: u32 = u32::MAX;

impl SpanTable {
    fn build(mut entries: Vec<(Span, u32)>) -> Self {
        entries.sort_by_key(|(span, payload)| (span.start, span.end, *payload));
        let mut table = Self {
            starts: Vec::with_capacity(entries.len()),
            ends: Vec::with_capacity(entries.len()),
            payloads: Vec::with_capacity(entries.len()),
            previous_longer: Vec::with_capacity(entries.len()),
        };
        // Indexes whose ends strictly decrease from bottom to top.
        let mut chain: Vec<u32> = Vec::new();
        for (index, (span, payload)) in entries.into_iter().enumerate() {
            while chain
                .last()
                .is_some_and(|top| table.ends[*top as usize] <= span.end)
            {
                chain.pop();
            }
            table
                .previous_longer
                .push(chain.last().copied().unwrap_or(NONE));
            chain.push(index as u32);
            table.starts.push(span.start);
            table.ends.push(span.end);
            table.payloads.push(payload);
        }
        table
    }

    /// Calls `visit` with the index of every entry before `bound` whose end is
    /// at least `end`; stops early when `visit` returns `true`.
    fn visit_ending_at_or_after(
        &self,
        bound: usize,
        end: u32,
        mut visit: impl FnMut(usize) -> bool,
    ) {
        let mut position = bound;
        while position > 0 {
            position -= 1;
            if self.ends[position] >= end {
                if visit(position) {
                    return;
                }
            } else {
                // Everything between the nearest longer entry and this one
                // ends at or before this one, so before `end` as well.
                match self.previous_longer[position] {
                    NONE => return,
                    longer => position = longer as usize + 1,
                }
            }
        }
    }

    /// Payloads of entries whose key span contains `span`, in ascending
    /// payload order (== original array order for index payloads).
    fn containing(&self, span: Span) -> Vec<u32> {
        let mut out = Vec::new();
        let bound = self.starts.partition_point(|start| *start <= span.start);
        self.visit_ending_at_or_after(bound, span.end, |index| {
            out.push(self.payloads[index]);
            false
        });
        out.sort_unstable();
        out
    }

    /// The payload of the containing entry with the least `key`, the lowest
    /// payload among equals: what `min_by_key` returns over [`Self::containing`].
    fn containing_least_by<K: Ord>(&self, span: Span, key: impl Fn(u32) -> K) -> Option<u32> {
        let bound = self.starts.partition_point(|start| *start <= span.start);
        let mut best: Option<(K, u32)> = None;
        self.visit_ending_at_or_after(bound, span.end, |index| {
            let payload = self.payloads[index];
            let candidate = (key(payload), payload);
            if best.as_ref().is_none_or(|best| candidate < *best) {
                best = Some(candidate);
            }
            false
        });
        best.map(|(_, payload)| payload)
    }

    fn contains_any(&self, span: Span) -> bool {
        let bound = self.starts.partition_point(|start| *start <= span.start);
        let mut found = false;
        self.visit_ending_at_or_after(bound, span.end, |_| {
            found = true;
            true
        });
        found
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
    /// Key = call callee span; payload indexes `calls`.
    calls_by_callee: SpanTable,
    /// Key = member object span; payload indexes `members`.
    members_by_object: SpanTable,
    /// Key = assignment target span; payload indexes `assignments`.
    assignments_by_target: SpanTable,
    /// Key = object property span; payload indexes `object_properties`.
    object_properties_by_span: SpanTable,
    /// The spans of the calls whose callee is spelled `eval`; the spelling
    /// needs the source text, so the file supplies it on first use.
    eval_calls: OnceLock<Vec<Span>>,
    /// Key = function span; payload indexes `functions`.
    functions_by_span: SpanTable,
    /// Key = loop-head target span; payload indexes `iteration_targets`.
    iteration_targets_by_span: SpanTable,
    /// The reference spans the binder resolved to each declaration span.
    references_by_declaration: OnceLock<HashMap<Span, Vec<Span>>>,
    /// Answers to per-declaration questions, by (question, declaration).
    flags: RwLock<HashMap<(u8, Span), bool>>,
}

/// What the queries below assume of the fact tables. They hold by
/// construction (`extract` sorts each table, a property spans its value);
/// debug builds, which every gate and test runs, check them whenever an index
/// is built, so a table that stopped being sorted fails loudly instead of
/// answering from the wrong place.
#[cfg(debug_assertions)]
fn verify_table_invariants(facts: &AstFacts) {
    fn sorted<T>(table: &[T], key: impl Fn(&T) -> Span) -> bool {
        table.windows(2).all(|pair| key(&pair[0]) <= key(&pair[1]))
    }
    assert!(sorted(&facts.calls, |call| call.span), "calls are sorted");
    assert!(
        sorted(&facts.members, |member| member.span),
        "members are sorted"
    );
    assert!(
        sorted(&facts.identifiers, |identifier| identifier.span),
        "identifiers are sorted"
    );
    assert!(
        sorted(&facts.returns, |fact| fact.span),
        "returns are sorted"
    );
    assert!(
        sorted(&facts.functions, |function| function.span),
        "functions are sorted"
    );
    assert!(
        sorted(&facts.bindings, |binding| binding.declaration),
        "bindings are sorted by declaration"
    );
    assert!(
        sorted(&facts.object_properties, |property| property.span),
        "object properties are sorted"
    );
    assert!(
        facts
            .object_properties
            .iter()
            .all(|property| property.span.contains(property.value)),
        "a property spans its value"
    );
}

impl AstSpanIndex {
    /// `compute()` for `(question, declaration)`, remembered. Concurrent
    /// first askers may each compute; the answers are equal.
    pub fn flag(&self, question: u8, declaration: Span, compute: impl FnOnce() -> bool) -> bool {
        let key = (question, declaration);
        let known = self
            .flags
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .copied();
        if let Some(known) = known {
            return known;
        }
        let answer = compute();
        self.flags
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, answer);
        answer
    }

    /// The spans of the file's `eval(...)` calls, computed by `compute` once.
    pub fn eval_calls_with(&self, compute: impl FnOnce() -> Vec<Span>) -> &[Span] {
        self.eval_calls.get_or_init(compute)
    }

    fn build(facts: &AstFacts) -> Self {
        #[cfg(debug_assertions)]
        verify_table_invariants(facts);
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
            calls_by_callee: SpanTable::build(spans(
                &mut facts.calls.iter().map(|call| call.callee),
            )),
            members_by_object: SpanTable::build(spans(
                &mut facts.members.iter().map(|member| member.object),
            )),
            assignments_by_target: SpanTable::build(spans(
                &mut facts.assignments.iter().map(|assignment| assignment.target),
            )),
            object_properties_by_span: SpanTable::build(spans(
                &mut facts.object_properties.iter().map(|property| property.span),
            )),
            eval_calls: OnceLock::new(),
            functions_by_span: SpanTable::build(spans(
                &mut facts.functions.iter().map(|function| function.span),
            )),
            iteration_targets_by_span: SpanTable::build(spans(
                &mut facts.iteration_targets.iter().copied(),
            )),
            references_by_declaration: OnceLock::new(),
            flags: RwLock::new(HashMap::new()),
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

/// Whether `function` owns `span`: the body holds it, or it lies in the
/// parameter list ([`FunctionFact::parameter_list_start`] to the body). The
/// function's name and type parameters are bound in the scope around it and
/// run nothing, so they are not its list.
fn function_owns(function: &FunctionFact, span: Span) -> bool {
    function.body.contains(span)
        || (span.end <= function.body.start
            && function
                .parameter_list_start()
                .is_some_and(|start| start <= span.start))
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

    /// Functions that own `span` (ADR 0454): the body holds it, or it is
    /// written in the function's parameter list (a default value, a pattern, a
    /// function written inside either), which runs only when the function is
    /// called without that argument. The list ends where the body begins.
    pub fn functions_owning(&self, span: Span) -> impl Iterator<Item = &FunctionFact> {
        self.span_index()
            .functions_by_span
            .containing(span)
            .into_iter()
            .map(|index| &self.functions[index as usize])
            .filter(move |function| function_owns(function, span))
    }

    /// The innermost function that owns `span` (ADR 0454, [`Self::functions_owning`]):
    /// the shortest span among the owners, the first in array order among
    /// equals. A site in a parameter list belongs to the function whose list it
    /// is, not to the function around that function.
    pub fn innermost_function_owning(&self, span: Span) -> Option<&FunctionFact> {
        let functions = &self.functions;
        self.span_index()
            .functions_by_span
            .containing_least_by(span, |index| {
                let function = &functions[index as usize];
                let length = function.span.end - function.span.start;
                // Owners sort before non-owners; no owner is no answer.
                (!function_owns(function, span), length)
            })
            .map(|index| &self.functions[index as usize])
            .filter(|function| function_owns(function, span))
    }

    /// The function with the shortest body containing `span` (the first in
    /// array order among equals): `functions_body_containing(span)
    /// .min_by_key(body length)` without collecting the candidates.
    ///
    /// A site in a parameter list is in no body, so this names the function
    /// around the function whose list holds it: ask
    /// [`Self::innermost_function_owning`] for the function a site belongs to.
    pub fn innermost_function_body_containing(&self, span: Span) -> Option<&FunctionFact> {
        self.span_index()
            .functions_by_body
            .containing_least_by(span, |index| {
                let body = self.functions[index as usize].body;
                body.end - body.start
            })
            .map(|index| &self.functions[index as usize])
    }

    /// Whether any function body contains `span`.
    pub fn any_function_body_containing(&self, span: Span) -> bool {
        self.span_index().functions_by_body.contains_any(span)
    }

    /// The return facts `function`'s *own* body writes, in the order the table
    /// holds them (source order for extracted facts). This is a syntactic
    /// containment query over these facts' functions and returns; it does not
    /// check that `function` belongs to this table.
    pub fn own_returns<'a>(
        &'a self,
        function: &'a FunctionFact,
    ) -> impl Iterator<Item = &'a ReturnFact> + 'a {
        let body = function.body;
        self.returns.iter().filter(move |returned| {
            if returned.span.start < body.start || returned.span.end > body.end {
                return false;
            }
            // Owned by a nested callable: some *other* function whose span lies
            // inside this body and *strictly* contains the return fact. Strict,
            // because a return fact's span is its argument's span when it has one:
            // `return () => …` yields a fact whose span *is* the arrow's, and that
            // arrow does not own the return — it is the value returned.
            !self.functions.iter().any(|other| {
                other.span != function.span
                    && other.span.start >= body.start
                    && other.span.end <= body.end
                    && (other.span.start < returned.span.start
                        || returned.span.end < other.span.end)
                    && other.span.start <= returned.span.start
                    && returned.span.end <= other.span.end
            })
        })
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

    /// Calls whose callee span contains `span`, in original array order.
    pub fn calls_with_callee_containing(&self, span: Span) -> impl Iterator<Item = &CallFact> {
        self.span_index()
            .calls_by_callee
            .containing(span)
            .into_iter()
            .map(|index| &self.calls[index as usize])
    }

    /// The first call (in array order) whose callee peels to `span`.
    /// A callee that peels to `span` is `span` or a wrapper around it, so it
    /// contains `span`.
    pub fn call_with_peeled_callee(&self, span: Span) -> Option<&CallFact> {
        self.calls_with_callee_containing(span)
            .find(|call| self.peel_ts_sugar_span(call.callee) == span)
    }

    /// Whether some call's callee peels to `span`.
    pub fn any_call_with_peeled_callee(&self, span: Span) -> bool {
        self.call_with_peeled_callee(span).is_some()
    }

    /// The first member (in array order) whose object peels to `span`.
    pub fn member_with_peeled_object(&self, span: Span) -> Option<&MemberFact> {
        self.span_index()
            .members_by_object
            .containing(span)
            .into_iter()
            .map(|index| &self.members[index as usize])
            .find(|member| self.peel_ts_sugar_span(member.object) == span)
    }

    /// Whether some assignment target contains `span`.
    pub fn any_assignment_target_containing(&self, span: Span) -> bool {
        self.span_index().assignments_by_target.contains_any(span)
    }

    /// The first object property (in array order) whose value is exactly
    /// `span`. A property contains its value.
    pub fn object_property_with_value(&self, span: Span) -> Option<&ObjectPropertyFact> {
        self.span_index()
            .object_properties_by_span
            .containing(span)
            .into_iter()
            .map(|index| &self.object_properties[index as usize])
            .find(|property| property.value == span)
    }

    /// Object properties whose span contains `span`, in original array order.
    pub fn object_properties_containing(
        &self,
        span: Span,
    ) -> impl Iterator<Item = &ObjectPropertyFact> {
        self.span_index()
            .object_properties_by_span
            .containing(span)
            .into_iter()
            .map(|index| &self.object_properties[index as usize])
    }

    /// Object properties whose span lies inside `region`, in original array
    /// order.
    pub fn object_properties_within(
        &self,
        region: Span,
    ) -> impl Iterator<Item = &ObjectPropertyFact> {
        within(&self.object_properties, region, |property| property.span)
    }

    /// Whether a reference-role identifier is written at exactly `span`.
    /// `identifiers` is sorted by span.
    pub fn has_reference_identifier_at(&self, span: Span) -> bool {
        let first = self
            .identifiers
            .partition_point(|identifier| identifier.span < span);
        self.identifiers[first..]
            .iter()
            .take_while(|identifier| identifier.span == span)
            .any(|identifier| identifier.role == IdentifierRole::Reference)
    }

    /// The function with the shortest span containing `span` (the first in
    /// array order among equals).
    pub fn innermost_function_containing(&self, span: Span) -> Option<&FunctionFact> {
        self.span_index()
            .functions_by_span
            .containing_least_by(span, |index| {
                let function = self.functions[index as usize].span;
                function.end - function.start
            })
            .map(|index| &self.functions[index as usize])
    }

    /// Functions whose span contains `span`, in original array order.
    pub fn functions_containing(&self, span: Span) -> impl Iterator<Item = &FunctionFact> {
        self.span_index()
            .functions_by_span
            .containing(span)
            .into_iter()
            .map(|index| &self.functions[index as usize])
    }

    /// Whether an assignment target or a loop-head target contains `span`.
    pub fn any_write_target_containing(&self, span: Span) -> bool {
        let index = self.span_index();
        index.assignments_by_target.contains_any(span)
            || index.iteration_targets_by_span.contains_any(span)
    }

    /// The reference spans the binder resolved to the declaration at
    /// `declaration`, in `reference_declarations` order.
    pub fn references_to(&self, declaration: Span) -> &[Span] {
        self.span_index()
            .references_by_declaration
            .get_or_init(|| {
                let mut by_declaration: HashMap<Span, Vec<Span>> = HashMap::new();
                for (reference, declared) in &self.reference_declarations {
                    by_declaration
                        .entry(*declared)
                        .or_default()
                        .push(*reference);
                }
                by_declaration
            })
            .get(&declaration)
            .map_or(&[], Vec::as_slice)
    }

    /// Whether some member access has exactly `span` as its object.
    pub fn any_member_with_object(&self, span: Span) -> bool {
        self.span_index()
            .members_by_object
            .containing(span)
            .into_iter()
            .any(|index| self.members[index as usize].object == span)
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

    /// Member accesses whose span lies inside `region`, in original array order.
    pub fn members_within(&self, region: Span) -> impl Iterator<Item = &MemberFact> {
        within(&self.members, region, |member| member.span)
    }

    /// Identifiers whose span lies inside `region`, in original array order.
    pub fn identifiers_within(&self, region: Span) -> impl Iterator<Item = &IdentifierFact> {
        within(&self.identifiers, region, |identifier| identifier.span)
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;

    /// The table answers exactly what a scan of every entry would, for
    /// arbitrary (including overlapping and wrapping) spans.
    #[test]
    fn containing_matches_a_full_scan() {
        let mut seed = 0x2545_f491_u32;
        let mut next = move |modulus: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % modulus
        };
        for round in 0..40 {
            let count = 1 + next(60) as usize;
            let entries = (0..count)
                .map(|index| {
                    let start = next(100);
                    let end = if round % 3 == 0 && index == 0 {
                        1_000
                    } else {
                        start + next(30)
                    };
                    (Span::new(start, end), index as u32)
                })
                .collect::<Vec<_>>();
            let table = SpanTable::build(entries.clone());
            for _ in 0..200 {
                let start = next(110);
                let query = Span::new(start, start + next(20));
                let mut expected = entries
                    .iter()
                    .filter(|(span, _)| span.contains(query))
                    .map(|(_, payload)| *payload)
                    .collect::<Vec<_>>();
                expected.sort_unstable();
                assert_eq!(table.containing(query), expected);
                assert_eq!(table.contains_any(query), !expected.is_empty());
            }
        }
    }

    /// The reference: what `min_by_key` returns over the payload-ordered
    /// `containing` list (the first, so the lowest payload, among equal keys).
    fn reference_least(
        entries: &[(Span, u32)],
        query: Span,
        key: impl Fn(u32) -> u32,
    ) -> Option<u32> {
        let mut containing = entries
            .iter()
            .filter(|(span, _)| span.contains(query))
            .map(|(_, payload)| *payload)
            .collect::<Vec<_>>();
        containing.sort_unstable();
        containing.into_iter().min_by_key(|payload| key(*payload))
    }

    #[test]
    fn containing_least_by_matches_min_by_key_with_the_lowest_payload_on_ties() {
        // Explicit cases. The key ignores the span, so equal keys are common.
        let spans = [
            (Span::new(0, 10), 3),
            (Span::new(0, 10), 1), // duplicate span, lower payload
            (Span::new(2, 8), 2),
            (Span::new(5, 5), 0), // zero-length entry
        ];
        let table = SpanTable::build(spans.to_vec());
        let same_key = |_: u32| 7;
        // Payloads 1, 2 and 3 contain (4, 6) with equal keys: the lowest wins.
        assert_eq!(
            table.containing_least_by(Span::new(4, 6), same_key),
            Some(1)
        );
        // A zero-length query at an entry's edge and inside the zero-length one.
        assert_eq!(
            table.containing_least_by(Span::new(5, 5), same_key),
            Some(0)
        );
        assert_eq!(
            table.containing_least_by(Span::new(0, 0), same_key),
            Some(1)
        );
        // The key decides before the payload does.
        assert_eq!(
            table.containing_least_by(Span::new(4, 6), |payload| 10 - payload),
            Some(3)
        );
        // Nothing contains the query.
        assert_eq!(table.containing_least_by(Span::new(9, 11), same_key), None);
        // An empty table.
        let empty = SpanTable::build(Vec::new());
        assert_eq!(empty.containing_least_by(Span::new(0, 0), same_key), None);
        assert_eq!(empty.containing_least_by(Span::new(3, 4), same_key), None);
        assert!(!empty.contains_any(Span::new(0, 0)));
    }

    #[test]
    fn containing_least_by_matches_a_full_scan() {
        let mut seed = 0x9e37_79b9_u32;
        let mut next = move |modulus: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % modulus
        };
        for round in 0..60 {
            let count = round % 7 * 9 + next(12) as usize; // includes 0
            let entries = (0..count)
                .map(|index| {
                    // Narrow ranges force duplicate spans.
                    let start = next(25);
                    let end = start + next(12);
                    (Span::new(start, end), index as u32)
                })
                .collect::<Vec<_>>();
            let table = SpanTable::build(entries.clone());
            // Few distinct keys: most queries tie.
            let key = |payload: u32| payload % 3;
            for _ in 0..200 {
                let start = next(30);
                let length = if next(3) == 0 { 0 } else { next(8) };
                let query = Span::new(start, start + length);
                assert_eq!(
                    table.containing_least_by(query, key),
                    reference_least(&entries, query, key)
                );
                assert_eq!(
                    table.containing_least_by(query, |_| 0),
                    reference_least(&entries, query, |_| 0)
                );
            }
        }
    }
}
