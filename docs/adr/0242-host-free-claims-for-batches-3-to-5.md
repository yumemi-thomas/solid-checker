# ADR 0242: Host-free claims for primitives batches 3 to 5

- Status: accepted and implemented (2026-10-08).
- Owners: the `hostFree` blocks and `identity.host-free.json` files of
  fifteen specs under `pkg/contracts/authored/specs/`.
- Relation: applies ADR 0230 to the exports admitted by ADRs 0236, 0238 and
  0240. Drafted read-only (`rust/target/research/host-free-batch/`), each
  safe export with its own server-path citation.

## Decision

- 25 exports receive a host-free claim, the ADR 0230 weakening of their
  probed browser claim. Fourteen specs gain an `identity.host-free.json`.
  The utils identity is regenerated unchanged.
- The rest are withheld under ADR 0230 decision 4, because their server path
  differs in what the claim states. Examples: a returned accessor becomes a
  plain getter (`createUserTheme`, `createWSState`, `createScheduled`), a
  returned setter is inert (`createReducer`), the server memo sync path does
  not observe (`createAggregated`, `createSegment`), or the settling effect
  never applies (`until`).

## Consequences

- None host: seven module-scope cases (`createMicrotask`,
  `createPointerListeners`, `createMarker`, `createReconnectingWS`,
  `createPageLeaveBlocker`, `repeat`, `tap`) move from SC9005 notices on both
  twins to `missing-owner` uncertifiable. Five of them now have a clean
  correct twin. They still do not report correctly, because ADR 0241 keeps a
  none expectation at browser strength (a violation), while a host-free claim
  deliberately proves only what holds on the server build too. That is an
  accepted under-report.
- Browser unchanged (66 of 114). rc.13 sweep unchanged. No correct twin on
  any host has a violation.
