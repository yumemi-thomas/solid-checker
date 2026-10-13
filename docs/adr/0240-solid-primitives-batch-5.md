# ADR 0240: Solid Primitives batch 5

- Status: accepted and implemented (2026-10-08), in two slices (5a, 5b).
- Owners: `pkg/contracts/authored/specs/@solid-primitives+*` (the changed
  version specs), their probe pairs, and three ledger cases.
- Relation: follows the open-domain census
  (`rust/target/research/open-domain-census/CENSUS.md`). That census found
  that most failing browser cases were claims nobody had drafted, not limits
  of the format. Drafted read-only by research agents and admitted only
  through the authoring tool and Chrome probes on rc.13.

## Slice 5a: closing reads and creates in existing specs

- Specs changed: `active-element`, `connectivity`, `fullscreen`,
  `intersection-observer`, `lifecycle`, `media` (`createMediaQuery`) and
  `page-utilities`. All 41 probe pairs pass.
- The optional hydration or settle child computation of connectivity, media,
  page visibility, active element and lifecycle is stated with count `0..1`
  and no required owner, citing the rc.13 `onSettled` implementation. A branch
  probe witnesses the returned value on that path, but not the child
  production by itself. A `min: 0` registration can only become an
  uncertifiable obligation (ADR 0231), never a violation.
- `createPageLeaveBlocker` gained its claim, but its ledger case did not flip.

## Slice 5b: five unclaimed exports

- Exports claimed: `createEventSignal` (event-listener `3.0.0-next.5`),
  `createPrefersDark` (media), `createReducer` (memo), `createMicrotask`
  (utils) and `createPointerListeners` (pointer). All 44 probe pairs pass.
- `createPointerListeners` leaves `callbacks` open, because its config keys
  are installed as handlers whatever their names (`pointer/dist/index.js:30-39`).
  `createMicrotask` leaves `returns` open: it returns a stateful function.
- event-listener's `_pairs` directory is shared with `3.0.0-next.3`, whose
  claim owns the main `createEventSignal` pair (a leaf-owner test). The
  `3.0.0-next.5` claim reuses that pair under the same rule. Its strict-read
  misuse is the `read` probe.

## Ledger corrections

`connectivity`, `media` `createMediaQuery` and `active-element` at module
scope expected `missing-owner` on every host. In the browser build their
listeners register through `tryOnCleanup` (`utils/dist/index.js:149`), which
adds a cleanup only when an owner exists. So module scope leaks nothing Solid
tracks, and Chrome on rc.13 is silent for all three. With 5a's claims, the
checker is silent too. The browser host is removed from those cases; none
and node keep theirs.

## Consequences

- Primitives ledger, browser: 63 report correctly out of 129 cases (was 50
  of 132, before 5a and the three corrections). No correct twin on any host
  has a violation.
- rc.13 corpus sweep: no violation added or lost (557); uncertifiable
  3471 to 3467.
