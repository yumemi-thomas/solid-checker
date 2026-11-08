//! Visible writes to the built-ins a promise or aggregate proof relies on
//! (ADR 0349).
//!
//! A program keeps the Promise premise while no analyzed file spells a write to
//! the global `Promise`, to its prototype's `then`, `catch`, `finally` or
//! `constructor`, or to `Symbol.species`. It keeps the array-iteration premise
//! while none spells a write to `Array`, `ReadonlyArray`, `Symbol.iterator`,
//! `ArrayIteratorPrototype`, a `prototype` or `__proto__`, and no `setPrototypeOf`
//! runs. Each spelling is a tripwire. An alias written through is the residual
//! premise, as for ADR 0335's `Object.prototype` callables.

use super::*;

/// Whether a visible write, class heritage or reflective target spells the
/// Promise intrinsics.
const PROMISE_WORDS: &[&str] = &[
    "Promise",
    "species",
    "then",
    "catch",
    "finally",
    "constructor",
];
/// The member names an assignment writes a Promise intrinsic through.
const PROMISE_MEMBERS: &[&str] = &["then", "catch", "finally", "constructor", "species"];
/// Words a write or reflective target spells the array intrinsics by.
const ARRAY_WORDS: &[&str] = &[
    "Array",
    "ReadonlyArray",
    "ArrayIteratorPrototype",
    "prototype",
    "__proto__",
    "iterator",
];

/// Which premises the program keeps: none of their tripwires is present.
#[derive(Clone, Copy)]
pub(crate) struct Intrinsics {
    /// The global `Promise`, its prototype's handlers and its statics are the
    /// built-ins.
    pub(crate) promise: bool,
    /// `Array.prototype[Symbol.iterator]` and the array iterator are the
    /// built-ins.
    pub(crate) array_iteration: bool,
    /// ADR 0803: no visible write can reach the array iteration protocol
    /// ([`crate::interproc::accessor_provenance::visible_iteration_write`]).
    /// Read only for a primitive's array-pattern slots.
    pub(crate) visible_iteration: bool,
}

impl<'a> SemanticLookup<'a> {
    /// ADR 0349: no analyzed file visibly writes to a Promise intrinsic.
    pub(crate) fn promise_intrinsics_intact(&self) -> bool {
        self.intrinsics_view().promise
    }

    /// ADR 0349: no analyzed file visibly writes to the array iterator.
    pub(crate) fn array_iteration_intact(&self) -> bool {
        // Review a9fc #2: ADR 0349's spelling misses a write through
        // `Object.getPrototypeOf([])` under a key no literal names; whatever
        // withdraws a tuple's slots (ADR 0803) breaks this premise too.
        self.intrinsics_view().array_iteration && !self.array_slots_withdrawn()
    }

    /// ADR 0803: whether a primitive's array pattern may not hold the
    /// primitive's own slots: a visible write that can reach iteration, or
    /// ADR 0801's census (code from a string, an intrinsic obtained as a
    /// value). An unresolved call alone is no evidence of either.
    pub(crate) fn array_slots_withdrawn(&self) -> bool {
        !self.intrinsics_view().visible_iteration || self.tuple_slot_iteration_may_be_mutated()
    }
}

impl SemanticLookup<'_> {
    /// The premises, a view of the [`Intrinsics`](super::project_memos::IntrinsicsTripwires)
    /// memo built once per build.
    fn intrinsics_view(&self) -> &Intrinsics {
        self.intrinsics.get_or_init(|| {
            let (promise, array_iteration, visible_iteration) =
                self.whole_program_answer::<super::project_memos::IntrinsicsTripwires>();
            Intrinsics {
                promise,
                array_iteration,
                visible_iteration,
            }
        })
    }
}

pub(super) fn tripwires(lookup: &SemanticLookup<'_>) -> Intrinsics {
    // Each file clears premises of its own; a premise holds where no file
    // clears it, so the files are read on the workers and the answers met.
    let per_file = crate::pipeline::parallel_balanced_results(lookup.files(), |file| {
        let mut found = Intrinsics {
            promise: true,
            array_iteration: true,
            visible_iteration: true,
        };
        if crate::interproc::accessor_provenance::visible_iteration_write(file, lookup) {
            found.visible_iteration = false;
        }
        for target in file
            .ast
            .write_targets
            .iter()
            .chain(&file.ast.deleted_targets)
        {
            let text = file.source_text(*target).unwrap_or_default();
            if promise_spelled(text)
                || member_name(file, *target).is_some_and(|name| PROMISE_MEMBERS.contains(&name))
            {
                found.promise = false;
            }
            if array_spelled(text) {
                found.array_iteration = false;
            }
        }
        for class in &file.ast.classes {
            let heritage = class
                .heritage
                .and_then(|heritage| file.source_text(heritage))
                .unwrap_or_default();
            if promise_spelled(heritage) {
                found.promise = false;
            }
        }
        for call in &file.ast.calls {
            let name = member_name(file, file.ast.peel_ts_sugar_span(call.callee));
            let reflective =
                crate::interproc::accessor_provenance::mutation_builtin(file, call, lookup)
                    .is_some();
            if !reflective && name != Some("setPrototypeOf") {
                continue;
            }
            let target = call
                .arguments
                .first()
                .and_then(|argument| file.source_text(argument.span))
                .unwrap_or_default();
            if promise_spelled(target) {
                found.promise = false;
            }
            if array_spelled(target) || name == Some("setPrototypeOf") {
                found.array_iteration = false;
            }
        }
        found
    });
    let mut found = Intrinsics {
        promise: true,
        array_iteration: true,
        visible_iteration: true,
    };
    for file in per_file {
        found.promise &= file.promise;
        found.array_iteration &= file.array_iteration;
        found.visible_iteration &= file.visible_iteration;
    }
    found
}

/// The property name a non-computed member expression names.
fn member_name(file: &FileFacts, span: Span) -> Option<&str> {
    let member = file.ast.member_at(span)?;
    if file.ast.computed_members.contains(&member.span) {
        return None;
    }
    file.source_text(member.property)
}

fn promise_spelled(text: &str) -> bool {
    PROMISE_WORDS.iter().any(|word| mentions_word(text, word))
}

fn array_spelled(text: &str) -> bool {
    text.contains("Symbol.iterator") || ARRAY_WORDS.iter().any(|word| mentions_word(text, word))
}

/// Whether `word` occurs in `text` as a whole identifier.
pub(crate) fn mentions_word(text: &str, word: &str) -> bool {
    let identifier = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(index, _)| {
        !text[..index].chars().next_back().is_some_and(identifier)
            && !text[index + word.len()..]
                .chars()
                .next()
                .is_some_and(identifier)
    })
}
