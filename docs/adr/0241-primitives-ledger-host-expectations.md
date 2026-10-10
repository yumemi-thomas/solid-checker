# ADR 0241: Primitives ledger host expectations

- Status: accepted and implemented (2026-10-08).
- Owner: `fixtures/primitives-misuse/cases.json`.
- Investigation: `rust/target/research/ledger-expectations/PROPOSAL.md`
  (a read-only source audit of every none and node row, with citations).

## Context

The ledger's expectations were written per primitive, before the hosts were
told apart. Many `node` rows expected a defect that the server build cannot
produce. On the server build, `onCleanup` without an owner does nothing
(`solid-js/dist/server.dev.js:213`), `createTrackedEffect` never runs its
callback (`:1179`), and signal and memo getters return plain values without a
strict-read check (`:581`, `:676`).

## Decision

1. **A host where the misuse does nothing is removed from the case.** Node
   is removed from 79 cases. Eight cases have no host left and are deleted.
   Five node rows remain: four explicit owned writes that reach the setter on
   the server (`SERVER_WRITE`, `:551`), and a live websocket.
2. **No declared host means browser strength.** A run that declares no host
   (the plain CLI, ESLint) may be code that runs in the browser, so a defect
   of the browser build is still a defect there. The checker's own rules on
   user code already work this way. The audit proposed the opposite (a none
   proof must hold on both builds), which would have made the checker's core
   findings overclaims on none; the owner chose browser strength. None
   expectations are unchanged. Host-free package claims stay the sound
   weakening of ADR 0230: they may under-report on none, never overclaim.
3. **Eight browser expectations change, each confirmed in Chrome on rc.13.**
   The module-scope or effect-apply misuses of `createTimer`,
   `createTimeoutLoop`, `createPolled`, `createPureReaction` and `createTween`
   become violations, because the registration is mandatory for the supplied
   arguments and Chrome raises NO_OWNER_CLEANUP or NO_OWNER_EFFECT. The
   `createNotification` (two cases) and `createVibrate` misuses become
   uncertifiable: Chrome warns, but browser support for the capability is
   not provable statically.

## Consequences

- Browser: 66 of 114 report correctly (was 63 of 119). None: 4 of 115. Node:
  0 of 5. No correct twin on any host has a violation.
- Earlier reports gave the total case count (132, 129) as the browser
  denominator. The browser case count was 119 before this ADR.
