# exact-copy-aliases

ADR 0515: two more initializers are exact copies of a `const` variable (`LocalAliasBindingFact`): a conditional both of whose arms are that variable (`c ? r : r`), and an inline identity arrow called on it (`((x: T) => x)(r)`). Each yields the variable's very value, so the binding is its alias for every rule that follows local aliases (ADR 0463). Before, both were not aliases and the use was not the source's: a strict read through them was uncertifiable here and silent in the fuzzer's rc.17 runs (recall grid, `value-binding:cond-alias`, `value-binding:identity`, 2026-10-31), as were store writes and signal writes.

Violations (rc.17 reports STRICT_READ_UNTRACKED, Chrome-witnessed by the recall grid):

- `ConditionalReadInBody` (line 6): a conditional copy of an accessor, read in the body.
- `IdentityReadInBody` (line 52): an identity-call copy, read in the body.

No SC1001 or SC2001 violation:

- `ConditionalReadInMemo` (line 23): the copy read in a memo, where a read tracks.
- `IdentityWriteInHandler` (line 30): an identity-call copy of a setter written in a handler.
- `ConditionalOfTwoSignals` (line 37): arms naming two different signals are no copy; the read stays uncertifiable (`reactive-dispatch-unresolved`).

Uncertifiable here, not because of the alias: `IdentityWriteInMemo` (line 14) and `IdentityWriteInBody` (line 44). With no rendering entry this fixture's stubs place no component body for a write; in the fuzzer's rc.17 project (an entry rendering the component) the body write through an identity copy is a violation.

Declaration stubs: `callback-containment`'s `solid-js.d.ts` (published 2.0.0-rc.9 and rc.13 typings, as recorded there).
