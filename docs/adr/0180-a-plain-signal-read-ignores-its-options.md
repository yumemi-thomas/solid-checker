# ADR 0180: A plain signal's read ignores its options

- Status: accepted and implemented (2026-10-05). Fourth lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners: the dialect (`Dialect::inert_read_ignores_options`,
  `unambiguous_inert_read_ignores_options`; Solid 2 answers for
  `createSignal`) and the inert-signal argument census
  (`inert_signal_arguments_discharged`).
- Relation: narrows ADR 0146's argument condition for the inert accessor row,
  for the options slot only. The computed (memo) row of ADR 0162 keeps its
  condition: a memo's read can re-run its computation, which calls `equals`.

## Context

ADR 0146 states an inert `createSignal` accessor read only when every argument
is a primitive by grammar, a non-function first argument, or an options
object literal of primitives. The audited body takes the memo path only for
`typeof first === "function"`, and reads callbacks off the options object.

The second half was stricter than the claim needs. The claim describes what
*invoking the accessor* does. The rc.9 `signal(v, options)` copies `equals`,
`ownedWrite` and `name` onto the node and stores `unobserved`, all at
creation:

- `equals` is called by the setter and by a pending write's commit;
- `unobserved` is called when the last subscriber unlinks;
- `read` consults neither.

Packages routinely pass a module constant (`INTERNAL_OPTIONS`,
`OWNED_WRITE`), which the census cannot trace. Its returned accessor was
therefore undescribed, and a top-level read of it went unreported although
Solid warns `STRICT_READ_UNTRACKED`. Examples:

- `createVideoFrameCallback`: `createSignal(false, INTERNAL_OPTIONS)`;
- `createBattery`: `createSignal(void 0, OWNED_WRITE)`;
- `createFullscreen`: `createSignal(<boolean>, INTERNAL_OPTIONS)`.

## Decision

1. **The dialect answers whether an inert read ignores options.** Solid 2
   answers `true` for `createSignal`; every other primitive, and every
   dialect by default, answers `false`.
2. **The census discharges the options slot by position** where the dialect
   says so. That requires an options slot the dialect names, and no spread at
   or before it, so that no spread can move a function into the first slot.
   The first argument still decides the path, exactly as before.

## Consequences

- The inert read claim still says the read runs no code, and that remains
  true. It never claimed anything about the setter or about unlinking.
- A caller-supplied `unobserved` can still run caller code when a reader is
  disposed. No rule read the inert row as excluding that, so at most a
  disposal-time finding is hidden; none is invented.

## Evidence

- Source: `@solidjs/signals@2.0.0-rc.9` `dist/dev-shared.js` `signal` and
  `read`.
- Runtime probe, rc.9 dev and prod under Node 24: a `createSignal(false,
  options)` whose `equals` and `unobserved` record each call was read
  untracked, in a `createMemo` and in a `createEffect`, with flushes and no
  write. Neither callback ran. `unobserved` ran once, on the root's disposal.

- Unit tests: the dialect's `only_a_plain_signal_read_ignores_its_options`.
  The inert witness census accepts a primitive first argument beside a
  non-spread options binding, and refuses a spread options argument and a
  spread first argument.
- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 68 to 70 static violations of 123:
    `createBattery` and `createVideoFrameCallback`;
  - both are runtime-detected (`STRICT_READ_UNTRACKED`), and no correct twin
    is flagged;
  - no checkpoint row changes status;
  - per environment, two exports gain a returned-accessor claim
    (`createVideoFrameCallback`, spring's `makeSpring`) and none loses one.

  `createFullscreen` is unchanged: its accessor sits behind a different wall.
