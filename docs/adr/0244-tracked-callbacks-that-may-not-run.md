# ADR 0244: Tracked callbacks that may not run, and mandatory first computes

- Status: accepted and implemented (2026-10-08).
- Owners: `SemanticLookup::contract_callback_reads_are_tracked`
  (`solid-reactive-ir/src/indexes.rs`), `contract_tracked_accessor_read_role`
  (`execution_role.rs`), its use for accessor reads in `local_access.rs`; the
  `cursor` and `signal-builders` specs.
- Fixtures: `fixtures/reactive-ir/package-tracked-optional-callback-consumer`,
  `fixtures/reactive-ir/package-mandatory-first-compute-consumer`.
- Investigation: `rust/target/research/consumer-fixes-2/`.

## Context

A read inside a callback handed to an accepted export got a tracked role only
through ADR 0183's guaranteed-callback slots: a callback that runs on every
call, during it, with `min >= 1`. `createLazyMemo`'s calc runs tracked, under
the memo it creates, but lazily: possibly never, and otherwise when its
accessor is read (`memo/dist/index.js:95-117`). So its reads stayed
uncertifiable timing obligations, and the ledger's correct twin was not clean.

Separately, the claims of `createBodyCursor` and `capitalize` stated their
compute `min: 0` although it provably runs once at construction:
`cursor/dist/index.js:99` passes it straight to `createEffect`, whose compute
runs at once (`@solidjs/signals/dist/dev.js:1723-1733`), and
`signal-builders/dist/string.js:16` invokes the input first thing in a
non-lazy memo.

## Decision

1. **A read in a callback that can only ever run tracked is tracked.** An
   accessor read directly in the body of a synchronous arrow, passed as the
   whole argument to an accepted export, gets the tracked role when:
   - the export's `callbacks` domain is closed;
   - every row for that parameter invokes the argument, `tracked`, under a
     `created` owner;
   - the export returns a closed plain accessor, so the callback cannot escape
     to another invoker.

   A count of zero changes whether the read happens, not its context. Writes,
   ownership and execution are not affected: nothing here proves the callback
   runs.
2. **The two compute claims say `min: 1`**, with the citations above. For
   `createBodyCursor`, the operation is also added to `hostFree.minZero`,
   because the server build returns first (`cursor/dist/index.js:98`). New
   `first-run` probe pairs pass in Chrome on rc.13.

## Consequences

- Browser ledger: 71 of 114 report correctly (was 68). `createLazyMemo`,
  `createBodyCursor` and `capitalize` report correctly; `capitalize` also
  reports correctly on none (5 of 115). No correct twin has a violation.
- rc.13 sweep: no violation moves.
- The slot identity check does not see through `(...) satisfies T`, as in ADR
  0183's helper, so such a read stays uncertifiable. Nested functions, async,
  generator and named callbacks, and an export that returns the callback
  itself stay uncertifiable too.
