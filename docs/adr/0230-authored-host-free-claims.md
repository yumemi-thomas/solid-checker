# ADR 0230: An authored contract may state a host-free claim

- Status: accepted and implemented (2026-10-08). Pilot: `createTimer`.
- Owners: `scripts/author-contracts.mjs` (`hostFreeClaim`, `hostFreeCases`,
  `identity --host-free`); each spec's `identity.host-free.json`.
- Relation: extends ADR 0198 decision 6, which authored browser cases only.
  ADR 0140 still decides which case a run receives: a run that declares no
  host gets only cases with no host condition.

## Context

A run that declares no host (ESLint, the plain CLI) received no authored
contract: every authored case carries `browser`. The primitives ledger's
none host therefore read every primitive as `package-contract-incomplete`.

The bytes differ by host only through their dependencies. `@solid-primitives/timer`
ships one `dist/index.js`, but it imports `isServer` from `@solidjs/web`, which
is `true` in `server.js` and `false` in `web.dev.js`. On the server build
`createTimer` returns before doing anything. A run with no declared host may
be either, so a claim for it must hold on both.

## Decision

1. **A spec may name `hostFreeIdentity`**: the version's host-free artifact
   cases, written by `identity --host-free` from a proposal generated with no
   `--host` (conditions `import` only). A case naming a host condition is
   refused.
2. **An export's `hostFree` is a weakening of its probed browser claim**,
   derived by the tool, never written out:
   - `minZero` lists the operations that may not run (the server early
     return); each gets `count.min: 0`. Nothing else about any operation
     changes.
   - A domain is closed only where `hostFree.closed` names it, with its own
     citation in `hostFree.closures` covering the server path, and only if
     the browser claim closes it too. Open domains lose empty lists.
   - `why` is required.
3. **It ships only when the browser claim's pairs passed.** No probe runs on
   the server build; the host-free claim adds no operation the probes did not
   exercise.
4. **An export whose server path returns something else gets no host-free
   claim.** `makeTimer` returns a no-op function on the server, so its
   return claim would not hold there.

## Consequences

- Timer ledger, none host: `timer-createTimer-module-scope` and
  `timer-createTimer-effect-apply` now report correctly (`missing-owner`,
  uncertifiable), and their correct twins are clean. The leaf-owner case
  reports `reactive-dispatch-unresolved` (uncertifiable) rather than a
  violation, which is right: on the server build the call does nothing.
  Browser results are unchanged.
- The node host is not covered: a declared node host receives only cases
  certified for it (ADR 0140), and its correct answer is often "nothing
  happens", which the ledger cannot express yet.
