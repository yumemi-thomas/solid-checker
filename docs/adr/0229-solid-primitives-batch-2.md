# ADR 0229: Solid Primitives batch 2

- Status: accepted and implemented (2026-10-08).
- Owners: `pkg/contracts/authored/specs/@solid-primitives+*` (8 new version
  specs); `scripts/author-contracts.mjs`;
  `fixtures/primitives-misuse/cases.json`.
- Relation: the second batch after ADR 0227, under the same rules (ADR 0198,
  ADR 0226): a citation for every closed domain, a probe pair for every
  positive claim. The first batch with no certified tier to start from (ADR
  0228): every identity comes from a browser proposal (ADR 0207).

## Decision

1. **Eight packages enter the authored tier**, each at its Solid 2 release
   in the primitives checkpoint corpus: `cursor` 1.0.0-next.2, `date`
   3.0.0-next.3, `signal-builders` 1.0.0-next.4, `pointer` 1.0.0-next.2,
   `event-bus` 3.0.0-next.3, `set` 1.0.0-next.2, `static-store`
   1.0.0-next.2 and `lifecycle` 1.0.0-next.2.
   - 21 export rows: 20 positive claims and one closure-only pure function
     (`getCountdown`).
   - All 52 probe pairs pass in Chrome on rc.13.
   - Nine rows close all four domains `SC9005` demands: both cursor
     constructors, six signal builders and `getCountdown`. `event-bus` and
     `createDerivedStaticStore` close nothing.
2. **The tool refuses a created owner its own productions do not name.** The
   decoder rejects such an operation, and one undecodable authored document
   fails every project. The first build of this batch did exactly that.
3. **The timer module-scope case is split by host.** In the browser build
   `isServer` is false, so `createTimer` always registers its cleanup on the
   ambient owner. The authored contract states that with `min: 1`, and Chrome
   raises the missing-owner warning, so the browser case now expects a
   violation. The none and node hosts have no contract and stay uncertifiable.

## Consequences

- Primitives ledger, browser host: 16 cases report correctly, up from 10. The
  6 new ones are `createTimer` at module scope, both cursor constructors at
  module scope, `createDateNow` at module scope, and the `capitalize` and
  `ceil` top-level reads. No correct twin on any host has a violation.
- Seven more browser cases now prove the misuse, but their correct twin is
  not certified clean: it keeps a `package-contract-incomplete` notice for a
  domain left open (`createPointerPosition`, `createEventStack`, `toEffect`,
  `union`, `createDate`, `createDateNow`, `createIsMounted`).
- rc.13 corpus sweep: 555 violations and 3,424 uncertifiable, both unchanged.
  The corpus barely calls these packages.
- Not changed, pending runtime evidence: the source reading suggests three
  ledger expectations are wrong. They are `set-ReactiveSet-top-level-read` (an
  unobserved `has` creates no signal), `static-store-createStaticStore-top-level-read`
  (a fresh property read returns the copy) and
  `lifecycle-createIsMounted-module-scope` (an ownerless `onSettled` returning
  nothing raises no warning).
- The none and node hosts still have no contract for any primitive, so every
  case there stays a wrong finding.
