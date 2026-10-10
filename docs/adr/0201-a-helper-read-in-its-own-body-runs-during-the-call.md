# ADR 0201: A helper's read in its own body runs during the call

- Status: accepted (2026-10-05). Track A, recall, of
  `docs/2026-10-05-package-direction.md`.
- Owner: `read_is_direct` in `interproc::interprocedural_result_reads_for_file`
  (`solid-reactive-ir/src/interproc.rs`).
- Fixture: `fixtures/reactive-ir/summary-direct-read`; recall ledger
  `fixtures/app-patterns-misuse`.

## Context

`e101c6989` made every strict read attributed through another function's
summary uncertifiable. On 48 projects, 73 of 96 such findings were false: a
helper returning an accessor, `merge` getters, reads in timers or listeners,
default parameters, `untrack`, reads after `await`, lazy props, and reads in
a returned closure. The fix withheld the true ones too.

The app-pattern recall ledger (31 plain-Solid misuse twins, run in Chrome on
rc.13) measured the cost. Each of these misuses raises
`STRICT_READ_UNTRACKED`, and each stayed uncertifiable:

- a local helper called in the body;
- a hook reading its accessor argument;
- a hook reading its own signal;
- a helper called from an effect's apply callback.

## Decision

At a call site, an attributed read keeps the call's own role (a proven read
where that role reports one) when all of the following hold:

1. **The call** has exactly one target, a synchronous project function (not
   async, not a generator), and sits outside any JSX: a call in a prop runs
   in a getter when the consumer reads it.
2. **The read** is one of these:
   - discovered in that callee itself (its summary row's owner is the callee
     node), as a call written directly in the callee's body: not in a
     nested function (a returned closure, a timer, a listener, a getter, an
     `untrack` callback) and not in a default parameter; or
   - the accessor argument the callee's own body calls, the row
     `invokes_parameter_during_call` already proved.

A read the summary propagated from a deeper callee stays attributed and
uncertifiable. One call level is proven, not a chain.

## Consequences

- The false-positive shapes `e101c6989` found stay uncertifiable or silent;
  the `fp-summary-*` fixtures hold their negative cases. Each of those shapes
  is a nested function, an async callee, a default parameter, a JSX prop, or
  a deeper callee.
- Still not proven:
  - two-level chains (`ChainCalledInBody`, `CallsReadThroughHelper`);
  - a callback prop the child calls in its body
    (`callback-prop-called-in-render`).
- A default-parameter read is not attributed at all, a missed finding
  (`CallsReadDefault`).

## Evidence

- Recall ledger: all five helper shapes are runtime-detected, now proven, and
  their correct twins are clean. Over the ledger, 28 of 29 runtime-detected
  misuses are proven (26 under their own rule, 2 under another), 1 is
  uncertifiable, and no correct twin is flagged.
- Coverage: 13 strict reads in 11 fixtures move from uncertifiable to
  violation, and nothing else moves. Each is a statement-level call in a
  component body, and the READMEs of `fp-summary-nested-helper` and
  `fp-summary-returned-closure` list their moved cases as runtime-confirmed.
  The tsc-oracle SC1006 keystone case returns to `checkerKind: "violation"`.
- rc.13 corpus: violations go from 267 to 285, 18 findings on 15 sites. All
  were reviewed and fall into two shapes:
  - a hook called in a component body that reads its own signal or its
    accessor argument in its body (`createHealth`'s
    `let current = connectivity()`, `createDragSensor(target)`);
  - a helper called from an effect's apply callback (beacon-web's
    `syncFromValue`, probus-hk's `centreOn` and `place`).

  Both shapes are runtime-confirmed by the ledger.
