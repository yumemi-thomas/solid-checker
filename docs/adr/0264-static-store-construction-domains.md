# ADR 0264: createStaticStore's construction reads and creates

- Status: accepted and implemented (2026-10-09).
- Owner: the `@solid-primitives/static-store@1.0.0-next.2` spec (browser row).

`createStaticStore` closed only `returns`, so every import was
`package-contract-incomplete`, even where it was used correctly. A
construction audit of the dist (`index.js:27-54`, for all inputs) shows that
the call copies its input with two spreads. It allocates plain objects and
closures, installs getters with `Object.defineProperty`, and returns the
tuple. It invokes no input function, reads no reactive accessor and registers
no computation. The row now closes its own `reads` and `creates` as empty.

Caller getters and Proxy traps hit by the spreads and the for-in stay caller
behavior: the existing positive initial-copy rows, ambient, min zero.
`callbacks` stays open. The returned getter and setter bodies are separate,
later scopes and are not closed by this ADR. Host-free claims are unchanged.

Two new pairs pass in Chrome on rc.13 with every existing static-store pair:
`construction-read` shows caller getters are not silently untracked, and
`construction-owner` checks ownerless construction.

## The misuse stays uncertifiable

On rc.13 an untracked Get warns only after an observer-present Get has primed
that key's signal (`index.js:30-36`). The ledger misuse primes `state.count`
in an eager memo, then reads it in the component body. Proving that would need
the consumer to show the priming Get certainly ran first: the tracked Get is
reached and succeeds on every path, with the same receiver and key, and with no
throw, early return or cache change in between. The IR has call-reachability
facts but no evaluation-order or successful-completion relation for a property
Get, so that proof is withheld (as ADR 0255 already withheld it). The misuse
keeps one `reactive-dispatch-unresolved` uncertifiable instead of a violation.

## Measured consequences

- Primitives ledger, browser: the primed-read correct twin is now clean; the
  misuse has only the dispatch obligation (was also incomplete). 97 of 111
  still report correctly. No correct twin has a violation.
- rc.13 corpus: one SC9005 import site (app-game) is gone; no violation or
  uncertifiable site was added.
