// The seams (ADR 0935): the places where one check of the checker hands a case to another.
//
// The pattern behind the ADR 0803 false clean: the census treated a mutating builtin as the visible-write scan's question, the scan decided only where the
// target or the key is spelled at the mutation, and a helper handed `Array.prototype` that wrote its parameter under an unspelled key belonged to neither.
// Each side's comment says the other decides; no test asked whether the other decides the same case with the same inputs (a parameter, an alias, a named
// source). So every comment in the checker's Rust that hands a case to another check (boundary-inventory.mjs `deferrals`) is classified here: a SEAM
// (the two checks, the case that falls between them if each trusts the other, and the evidence: pairs, fixtures, or a stated gap) or NOT_SEAMS (it names no
// second check, with the reason). boundary.test.mjs fails on a deferral comment in neither list, on a stale entry, and on a seam with no evidence and no gap.
//
// `sites` match the comment line (`file` is repository-relative; `match` a regular expression on the line's text).
export const SEAMS = [
  {
    id: "iteration-protocol",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/accessor_provenance/mutation_sites.rs", match: /A write is the visible side's/ }],
    between: ["the visible-write scan (`visible_iteration_write`, ADR 0803)", "the census (`handed_to_unseen_code`, `handed_on`, ADR 0781)"],
    case: "a project helper handed `Array.prototype` that writes its parameter under a key or a source no spelling at the call names (`Object.assign(p, {[k]: f})`, `Reflect.set(p, k, f)`)",
    gap: "the shapes are phase B's, on its own slice and not merged yet: fixtures `iteration-written-slots/variants/handed-assign-open`, `handed-assign-source`, `handed-reflect-set` and the negative `assign-computed-intact`, and the `iter-swap` variant of REACTIVE_WRITE_IN_OWNED_SCOPE",
  },
  {
    id: "value-only-row",
    sites: [],
    between: ["the table of value-only standard-library rows (`timing_behavior` ValueOnly, `hand_off_at` Value)", "the census (`handed_to_unseen_code`)"],
    case: "`Number.isFinite` replaced through a helper that writes its parameter under a computed key and never reads the original: the row says the API never runs its argument",
    gap: "the shapes are the fixer's: contexts `bseam-isfinite-*` and the pair `seam-isfinite` (branch claude/intrinsic-patch-seam, ADR 0939); a false clean Chrome confirmed on a value-only row, `Number.isFinite` replaced through a helper that writes its parameter under a computed key",
  },
  {
    id: "thrown-value-routing",
    sites: [{ file: "rust/crates/solid-facts/src/ast/types.rs", match: /where a thrown value goes is the census's question/ }],
    between: ["the facts (`throws`: every `throw` with its operand)", "the throw census (stored_throw.rs `handled_above`)"],
    case: "a thrown value that reaches a boundary through an alias, a parameter or a rethrow, where the fact records the operand and the census follows only the spelled routes",
    fixtures: ["reactive-ir/stored-throw-observation", "reactive-ir/stored-throw-nested-component", "reactive-ir/stored-throw-value-mount", "reactive-ir/stored-throw-class-member-wrapper"],
  },
  {
    id: "callback-passthrough",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/callback_return.rs", match: /Runtime passthrough belongs to the dialect/ }],
    between: ["callback-result identity (`callback_return.rs`)", "the dialect (which primitive returns or forwards its callback's result)"],
    case: "a primitive whose return value is its callback's result, called where the result is read",
    gap: "no pair yet: the case needs a primitive per release that forwards its callback's result, and the generator's contexts place the callback, not the result's reader",
  },
  {
    id: "component-identity",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/execution_role/flow_memos.rs", match: /component identity is the census's per-file memo/ }],
    between: ["the flow memos (`execution_role/flow_memos.rs`)", "the JSX census (`execution_role/jsx_census.rs`, its per-file memo)"],
    case: "a function that returns JSX and is also handed to code that may call it as a plain function",
    fixtures: ["reactive-ir/undecided-flush-callback", "reactive-ir/undecided-flush-closure"],
  },
  {
    id: "intrinsic-handler-write",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/execution_role/invocation_flow.rs", match: /intrinsic handler is the write walk's own business/ }],
    between: ["invocation flow (`execution_role/invocation_flow.rs`, the read walk)", "the write walk (`execution_role/write_roles.rs`)"],
    case: "a function written as an intrinsic element's handler that the read walk reaches by name",
    gap: "no pair yet: the generator's `handler` contexts write the handler inline; the case needs the handler named and reached by a read through another route",
  },
  {
    id: "split-symbol-protocol",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/indexes/primitive_proof.rs", match: /`split` defers to its separator's `Symbol.split`/ }],
    between: ["the primitive proof for string methods (`indexes/primitive_proof.rs`)", "the project's own `Symbol.split` objects (user code)"],
    case: "`text.split(separator)` where the separator is a project object with a `[Symbol.split]` method that runs a function",
    gap: "no pair yet: the table family has no string-method row, and the case needs a project object with a well-known-symbol method as the argument",
  },
  {
    id: "promise-flight-and-stack",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/static_rules/after_await.rs", match: /handled by the promise-flight walk, never by this stack walk/ }],
    between: ["the stack walk (`report_prefix`, an async function's prefix)", "the promise-flight walk (its tail past the first await)"],
    case: "an async function whose prefix runs immediately and whose tail follows an await",
    fixtures: ["reactive-ir/after-await-handoff"],
  },
  {
    id: "unplaced-functions",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/static_rules/after_await.rs", match: /is left to the arms that place it/ }],
    between: ["the obligation for unplaced functions (`report_unplaced_functions`)", "the arms that place a function (execution_role/callee_roles.rs)"],
    case: "a function only called directly, scheduled on a fresh stack, kept by a host, returned by the member or never referenced",
    fixtures: ["reactive-ir/after-await-exact-callbacks"],
  },
  {
    id: "possible-reads",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/static_rules.rs", match: /what it may read is the possible-read scan's/ }],
    between: ["the check that proves a read (`static_rules.rs`)", "the possible-read scan"],
    case: "a value that is not proven to read but may",
    gap: "no pair yet: the case is a read the proof cannot place and the scan can, and no generator context isolates the proof's refusal from the scan's answer",
  },
  {
    id: "result-access-and-unknown-callback",
    sites: [{ file: "rust/crates/solid-reactive-ir/src/static_rules.rs", match: /Two shapes are left to other passes and say so/ }],
    between: ["the result-access check (`RESULT_ACCESS_*`)", "the unknown-callback obligation"],
    case: "a parameter forwarded into a result-access position, whose body is the caller's, and a spread argument",
    gap: "no pair yet: the case needs a helper whose parameter is forwarded into `createMemo`'s result-access position by one caller and a visible body by another",
  },
];

/** Deferral comments that name no second check, each with the reason. Grouped by file where the reason is the same. */
export const NOT_SEAMS = [
  { file: "rust/crates/solid-dialect/src/dialect.rs", match: /window belongs to the \*call position\*|the answer belongs to the release that implements the guard/, reason: "the dialect interface's own documentation: which call position or release an answer is for; one table, not two checks" },
  { file: "rust/crates/solid-dialect/src/semantics.rs", match: /only one of them is answered by this word alone/, reason: "wording of two dialect questions" },
  { file: "rust/crates/solid-dialect/src/solid_2.rs", match: /`createEffect` defers to the attribution answer/, reason: "one dialect table: one primitive takes another row's answer" },
  { file: "rust/crates/solid-facts/src/ast/", match: /decided by the bytes alone|Property completeness belongs to the runtime object|belongs to the function whose list it|are left to the reader/, reason: "facts: how the producer records a span, a flavor or a pattern; the consumers read the fact and nothing is deferred" },
  { file: "rust/crates/solid-facts-backend/src/", match: /answered by a|answered by the ordinary analysis|answered by its|are left to the daemon's own overlay|left to a later pass/, reason: "process and source selection (the daemon, the demand plan, the package requirements): not a role or a silence" },
  { file: "rust/crates/solid-reactive-ir/src/execution_paths.rs", match: /the rest of the function is the other side/, reason: "a control-flow narrative: the two sides of an `if`" },
  { file: "rust/crates/solid-reactive-ir/src/execution_role/pending_sources.rs", match: /belongs to the root's owner/, reason: "ownership of a created computation, not a split between checks" },
  { file: "rust/crates/solid-reactive-ir/src/indexes/initialization.rs", match: /decided by its clause alone/, reason: "internal to one index: a `switch` case clause" },
  { file: "rust/crates/solid-reactive-ir/src/", match: /belongs to the node of the function whose list|belongs to the caller-provided callback|belongs to the helper's own summary|that list belongs to the nested function|belongs to the function around the list's function|belongs to the arrow, and only the conditional|a function literal stored there is decided by its own body/, reason: "attribution of a site to the function whose parameter list, summary or body holds it: one structure, no second check" },
  { file: "rust/crates/solid-reactive-ir/src/static_rules.rs", match: /"reactive condition" is answered by the draft's reads/, reason: "one draft pass answers its own condition" },
  { file: "rust/crates/typefacts/src/", match: /belongs to the producer that minted it|decided by the frame's content/, reason: "the producer protocol (symbol ids, frame compatibility)" },
];
