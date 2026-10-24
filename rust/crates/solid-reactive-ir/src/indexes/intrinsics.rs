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
}

impl<'a> SemanticLookup<'a> {
    /// ADR 0349: no analyzed file visibly writes to a Promise intrinsic.
    pub(crate) fn promise_intrinsics_intact(&self) -> bool {
        self.intrinsics_view().promise
    }

    /// ADR 0349: no analyzed file visibly writes to the array iterator.
    pub(crate) fn array_iteration_intact(&self) -> bool {
        self.intrinsics_view().array_iteration
    }
}

impl SemanticLookup<'_> {
    /// The premises, a view of the [`Intrinsics`](super::project_memos::IntrinsicsTripwires)
    /// memo built once per build.
    fn intrinsics_view(&self) -> &Intrinsics {
        self.intrinsics.get_or_init(|| {
            let (promise, array_iteration) =
                self.whole_program_answer::<super::project_memos::IntrinsicsTripwires>();
            Intrinsics {
                promise,
                array_iteration,
            }
        })
    }
}

pub(super) fn tripwires(lookup: &SemanticLookup<'_>) -> Intrinsics {
    let mut found = Intrinsics {
        promise: true,
        array_iteration: true,
    };
    for file in lookup.files() {
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
