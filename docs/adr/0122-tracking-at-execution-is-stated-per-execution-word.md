# ADR 0122: Tracking at execution is stated per execution word

- Status: accepted and implemented (2026-09-25); written with the implementation
- Date: 2026-09-25
- Owners: the generator's callback rows (`interproc.rs`, `runtime_semantics.rs`,
  `inferred_contract.rs`), `ContractCallback::clears_tracking` (`lib.rs`), and
  the consumer's read-back (`contracts.rs`)
- Relation: finishes what d15aaa69 (2026-09-18) began for `inline` rows, and
  applies ADR 0118's reason for `ambient-at-execution` to every execution word.
  Step 7 part 1 of ways-to-improve § 4. Nothing here changes what the census
  or the veto certifies: tracking is a statement of the generator.

## Context

d15aaa69 made an `inline` row say `untracked` only through a proven clearing
wrapper and `ambient-at-execution` otherwise. Three places still stated a
clear nobody proved, or dropped one that was proven:

- **G1.** Every `deferred` row was written `(queued, untracked)`, on the
  reasoning that a deferred callback runs after the export returns, so no
  scope of the caller is open. That is false for a callback reached through a
  closure the export returns (`@solid-primitives/utils` `safe`, `pipe`): the
  consumer calls the closure, possibly inside its own memo. The same is true of
  `addEventListener` (a synchronous `dispatchEvent` or `click()` runs the
  listener on the dispatcher's stack) and `Function.prototype.bind`.
- **G3.** A proven clear was lost in two places. A local helper wrapping
  `untrack` under `createMemo(() => runUntracked(cb))` published `tracked`,
  because forwarding replaced the callee's clearing with the chain's
  (backlog, "affirmative wrong claim"). And the consumer's wrapper fold read a
  package row that states a clear as transparent.
- **G4.** The consumer read `untracked` back as a clear whatever the schedule.

## Decision

**`ContractCallback::clears_tracking` is defined per execution word, and
`untracked` is written only where that definition holds:**

- `inline`: a clearing wrapper stands between the export and the callback.
- `deferred`: the deferral is proven to clear. That is a reviewed fresh-stack
  host queue, a dialect slot the dialect states untracked, a clearing wrapper
  inside the deferral, or a dependency row that states one.
- `tracked`: never.

Every other row says `ambient-at-execution`.

The bit stays a bool. The meaningful states are the word and whether a clear is
proven; no consumer needs to know why it was proven. The invalid pair
(`tracked` with a clear) cannot be built by chain composition or by
`clears_tracking_from`, the read-back, which is the exact inverse of the
generator's mapping.

### The fresh-stack list

`FRESH_STACK_SCHEDULERS` (`runtime_semantics.rs`) is the only host evidence
for a clearing `deferred` row: `queueMicrotask`, `setTimeout`, `setInterval`,
`requestAnimationFrame`, `requestIdleCallback`, `Promise.then`/`catch`/`finally`,
`Scheduler.postTask`, and the Intersection, Resize, Mutation, Performance and
Reporting observer constructors. Each one invokes its callback from a task or
microtask, on an empty execution-context stack.

It is kept apart from the timing table on purpose: a scheduler added to the
table later is `deferred` and ambient until its stack is reviewed. Left off,
each with its reason in the code: `addEventListener`, `bind`, `PromiseLike.then`
(any thenable may call back synchronously) and Geolocation (it calls back with
an error synchronously when the document is not fully active).

### Composition

A clearing wrapper counts only while the callback still runs inline in the
chain. So `[deferred, detaching]` no longer sets the bit, and
`[tracked, detaching, tracked]` no longer composes back to `inline`.
Forwarding prepends the callee's own row to the chain instead of replacing
it, and a chain that then refuses opens the parameter's sentinel.

The consumer's wrapper fold reads a package slot as clearing only when every
row for that slot states the clear under the same word.

### What no rule may do

**No violation rule reads the tracking word.** Neither the census nor any veto
observes a listener, so `untracked` and `ambient-at-execution` are both
unwitnessed statements. A rule that promoted an `untracked` read to an
untracked callback would fire on correct code wherever a stated clear is
wrong. This holds until a veto runs the callback under a memo that reads a
recipe-owned signal and treats a re-run as the contradiction.

`read` operations keep `untracked` (G2). Changing them would reopen ADR 0101's
`reads` census, which accepts only that word, and no consumer reads a read
operation's tracking.

## Consequences

Measured on the generator corpus (regenerated with a fresh debug binary): 9
rows go from `untracked` to `ambient-at-execution` in six fixtures, and no
closure or claim changes. The rows are:

- returned closures: `decorated`, `throughIdentity`, `Direct`, `returned`;
- `onCleanup` callbacks: `deferredWrapper`, `cleanUp` (no dialect row states
  cleanup runs untracked);
- the Geolocation pair.

`debounce` (`setTimeout`) stays `untracked`. The new fixture
`forwarded-local-untrack-wrapper` pins the local `untrack` helper
(`inline` + `untracked`), `addEventListener` and `bind` (ambient), `setTimeout`
(untracked) and an unclassified wrapper (open). No findings snapshot moves.

Still open:

- **The G3 fold does nothing yet.** No generated clearing row has a closed
  `callbacks` domain, and the only rows it could apply to are hand-audited
  bundled ones written before the split (`applyRef`, `renderToString`,
  `renderToStream`, `createServerReference`). Their `untracked` should be
  reviewed before anything relies on it.
- **Effect apply, `onSettled` and `onCleanup` are now ambient.** If 2.0 does
  clear the listener there, that needs an audited dialect row.
- **Forwarding still skips the enclosing wrappers in two places:** a package
  row forwarded directly (`pkgInline(cb)` under `createMemo`) and the
  local-scheduler callee path both copy the package's word as it is.
