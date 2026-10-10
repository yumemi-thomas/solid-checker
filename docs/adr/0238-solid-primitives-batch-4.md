# ADR 0238: Solid Primitives batch 4

- Status: accepted and implemented (2026-10-08).
- Owners: `pkg/contracts/authored/specs/@solid-primitives+*` (the new version
  specs), their probe pairs.
- Relation: the fourth batch under ADRs 0198, 0226 and 0227, drafted read-only
  by research agents and admitted only through the authoring tool and Chrome
  probes on rc.13. It is the last batch of Solid 2 primitive packages with a
  ledger case.

## Decision

- Specs: `async`, `audio`, `broadcast-channel`, `cookies`, `focus`,
  `gestures`, `keyed`, `promise`, `script-loader`, `transition-group`,
  `trigger`, `vibrate` and `workers`. All 47 probe pairs pass in Chrome, so
  every drafted claim ships.
- `keyed`'s `dist` imports `@solid-primitives/utils` but declares it only as
  a devDependency. Its probe install adds `utils@7.0.0-next.4` as an app
  dependency, which is what any app using keyed must do. Without it the page
  fails to load, and the pairs were harness errors.
- Four ledger cases are split by host, as in ADR 0236:
  `broadcast-channel` `makeBroadcastChannel`, `script-loader`, `workers` and
  `keyed`, each at module scope. In the browser build the cleanup
  registration always happens, and Chrome raises NO_OWNER_CLEANUP. The browser
  half expects the violation; none and node keep their old expectation.
- What stays open:
  - `vibrate`: its owner registration depends on `navigator.vibrate`, so the
    browser misuse stays uncertifiable.
  - `keyArray`, `tap`, `until` and `createAggregated` return or take functions
    the format cannot describe, so their correct twins keep an open domain.
  - The returned controls of `trigger`, `async`, `audio` and
    `broadcast-channel` stay `callable` obligations (ADR 0234).

## Consequences

- Primitives ledger, browser: 47 report correctly (was 40), out of 132 cases.
  No correct twin on any host has a violation.
- rc.13 corpus sweep: no violation added or lost (557); uncertifiable
  3473 to 3471.
