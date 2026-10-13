# Argument-scoped `reads` rows (ADR 0168)

Source for `the_reads_walk_admits_untrack_of_a_literal_and_refuses_create_signal_of_a_function`
in `rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`. There is
no `expected.json` and the fixture is not in `corpus.json`: the native tracer harness
cannot stand up an audited `solid-js` archive (ADR 0146), so the census is driven over
transcripts synthesized from this file's exact spans, with the audited rc.9 archives
supplied as authenticated dependency snapshots. The producer half is pinned over real
transcripts by `TestCallStatesItsArgumentsPrimitiveSyntax`
(`apps/solid-typefacts/internal/typefacts/tsgo/primitive_syntax_test.go`).

| Export | Verdict | Why |
| --- | --- | --- |
| `untrackLiteral` | closes | `untrack`'s row holds for a literal in the frame; its call of the caller's `sig` is the caller's |
| `untrackParameter` | closes | slot 0 is rooted at the export's own parameter |
| `createSignalOfPrimitive` | closes | `solid-js`' row holds for a primitive first argument (producer `argumentsPrimitiveSyntax`) |
| `createSignalOfFunction` | refuses | the row declines by name: argument 0 is not a primitive by its grammar |
| `ownerOnly` | closes | a flat row: `getOwner` reads nothing |

Stub note: nothing here is typed against a published package by a rule, so the tsc rule
has nothing to duplicate; the test names no stub.
