# ADR 0236: Solid Primitives batch 3

- Status: accepted and implemented (2026-10-08), in slices (3a, 3b, 3c).
- Owners: `pkg/contracts/authored/specs/@solid-primitives+*` (the new version
  specs), their probe pairs.
- Relation: the third batch under ADRs 0198, 0226 and 0227, drafted read-only
  by research agents and admitted only through the authoring tool and Chrome
  probes on rc.13.

## Slice 3a

- Specs: `a11y`, `active-element`, `clipboard`, `connectivity`,
  `fullscreen`, `page-utilities`, `pagination`, `upload` and `websocket`.
  `storage` gets no claim: its forwarded getter's identity cannot be
  projected yet.
- 29 probe pairs pass in Chrome. Two claims do not ship, because a claim
  ships only when all its pairs pass: `createPagination` (its constructor
  read warns unattributed) and `upload`'s `createDropzone` (harness errors).
- Primitives ledger, browser: 26 report correctly (was 21). Seven more prove
  the misuse while their correct twin keeps an open domain. No correct twin
  on any host has a violation.

## Slice 3b

- Specs: `devices`, `intersection-observer`, `notification`,
  `orientation`, `permission`, `range`, `selection`, `sensors`, `spring` and
  `tween`. 43 probe pairs pass in Chrome; `mapRange` and `createTween` do not
  ship (one pair each warned unattributed).
- Three ledger cases are split by host, as the timer case was (ADR 0229):
  `devices`, `permission` and `sensors` `createBattery` at module scope.
  Their `uncertifiable` expectation came from certified summaries whose
  `isServer` early return made every owner count `min: 0`. In the browser
  build the registration always happens, and Chrome raises NO_OWNER_CLEANUP
  (and NO_OWNER_EFFECT for `createPermission`). The browser half expects
  the violation; none and node stay uncertifiable.
- Primitives ledger, browser: 36 report correctly (was 26), out of 127 cases.
  No correct twin on any host has a violation.

## Slice 3c

- Specs: `deep`, `history`, `i18n`, `map`, `marker`, `mutation-observer`,
  `props`, `rootless`, `scheduled` and `video`. All 29 probe pairs pass in
  Chrome.
- `mutation-observer-createMutationObserver-module-scope` is split by host
  like the 3b cases: in the browser build `isSupported` is true, and Chrome
  raises NO_OWNER_CLEANUP. Its browser half proves the misuse; its correct
  twin keeps an open domain.
- Primitives ledger, browser: 40 report correctly (was 36), out of 128 cases.
  No correct twin on any host has a violation.
- Corpus sweep over slices 3a-3c: two violations added, both true positives
  (`createRAF`'s `start()` in a component body, `app-game`), and one lost:
  `...overlays()` passed to `combineProps` in
  `solid-props-proxy/example-3.tsx`. The loss is a pre-existing consumer bug,
  not the new claims. An eager read inside an argument of a contracted export
  whose `callbacks` domain is open is dropped in favour of a
  `package-contract-incomplete` notice (`chain(count())` from utils does the
  same). Before this slice `props` had no contract, so the path never applied
  there. `combineProps`'s own claim gained nothing on the ledger and is
  withdrawn. The consumer fix is ADR 0237, which restores the finding.
