# ADR 0195: A source tracked before the await is a dependency

- Status: accepted and implemented (2026-10-05). Item D4 of the rc.13 review
  (`docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-vocabulary-review.md`).
- Owners:
  - `reactive_read_after_await` (SC1002) and `tracked_before_first_suspension`
    in `solid-reactive-ir/src/static_rules.rs`;
  - `AstFacts::straight_line_calls` in `solid-facts/src/ast/mod.rs`.
- Relation: narrows SC1002. No wire change.

## Context

rc.13 adds a dev warning, `UNTRACKED_READ_AFTER_AWAIT`, the runtime
counterpart of SC1002 (`reactive-read-after-await`). The rc.13 review ran
both on the cases the runtime stays silent for. SC1002 agreed on all but one:

```ts
createMemo(async () => {
  s();
  await 0;
  return s();
});
```

SC1002 reported the second `s()`, and the runtime does not warn. The first
call ran inside the tracking window, so `s` is a dependency and the memo
re-runs when it changes; the read after the `await` loses nothing. The
runtime's check skips a source already among the computation's dependencies
(`checkPostAwaitRead`, `dev-shared.js:968-1037`). The finding was a false
positive.

## Decision

1. **A new syntax fact, `straight_line_calls`.** These are the calls written
   in their innermost function's straight-line flow, as `unconditional_awaits`
   are, and outside any optional chain (`a?.b(c())` evaluates `c()` only when
   `a` is not nullish). Optional chains get their own counter, so the
   existing answer for awaits is unchanged.
2. **SC1002 skips a tracked source.** An after-await accessor call (and the
   same in an inline `filter` callback) is not reported when the async
   function itself has a straight-line call of the **same symbol** before its
   first suspension of any kind: any `await`, conditional ones included, or
   an implicit suspension (`for await`, `await using`). The symbol is resolved
   as SC1002 resolves the after-await call: a member call by its non-computed
   property, any other call by its callee.
3. **Member reads are unchanged.** Store-path and props member reads keep the
   rule as before; the review did not probe an already-tracked store path.

## Consequences

- A mistake in the exemption can only hide a finding, never add one. Each way
  it could mistake a read for a tracked one is pinned as a positive control:
  a conditional earlier read, a conditional `await` first, another accessor,
  an optional chain, a nested function.
- Not modelled: an earlier read that only *some* branch makes still reports,
  even if every branch makes one. That is conservative.

## Evidence

- **Probe** over the rc.13 install (`rust/target/audit-rc13/sc1002`), the
  eight cases of runtime § 3.4.2. Before, SC1002 reported the positive and
  `alreadyTracked`. After, it reports the positive only, matching the runtime
  on every case.
- **Fixture** `fixtures/reactive-ir/read-after-await-already-tracked`: the two
  already-tracked reads become clean, and the five controls stay SC1002.
  `tsc` reports nothing.
- **Unit test** `straight_line_calls_exclude_every_conditional_construct`:
  `if`, ternary, `&&`, loop, `try`, `switch` and an optional chain keep a call
  out. A call in a nested arrow is straight-line for that arrow only.
- **Coverage:** 167 fixture projects. Only the new snapshot is new.
- **38-app browser sweep** (against ADR 0193's `c0193-browser.json`): the
  violations are unchanged at 265, so the pattern does not occur in the
  corpus. Uncertifiable +8, all of them ADR 0194's notice on the apps that
  are still on rc.9.
- `solid-facts` (141) and `solid-reactive-ir` (304) library tests pass, and
  workspace clippy is clean.
