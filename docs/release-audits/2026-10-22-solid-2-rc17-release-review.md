# Review: moving the audited Solid 2 triple from `2.0.0-rc.13` to `2.0.0-rc.17`

Date: 2026-10-22. Owner request: follow the latest release candidate.
`solid-js`, `@solidjs/signals` and `@solidjs/web` `2.0.0-rc.17` are `next` on
npm (published 2026-10-22). rc.14, rc.15 and rc.16 were published and not read.

Integrities (each equal to the sha512 of the registry tarball):

- `solid-js` `sha512-ym3x2kzC+vzgD9VNqhqExn3qWzkrpJco3vane0wTFHCVPS9HBXI3GgSp+ieZ1h72zcb2z27PXFSbMsi8Ot+Hxw==`
- `@solidjs/signals` `sha512-GXXWBlJsiZeBCTw3a941Y/HVBYfwupCfctPnwNjFJV+u3tZnRe/SL+NeStI1247zfxlvkH4p9PWzdZK/yX1I1Q==`
- `@solidjs/web` `sha512-cJ+8GzVYWBi+/glbgWt6cVgRTxvVa4TG+rShaEuQnPHjUZQTsHh8hvR89KVdFB+5XQJnAajWDNpxz7LkD4MkDg==`

Evidence, tagged **[M]** measured (Node probes on the dev and prod builds,
happy-dom for the web client) or **[R]** read in the bytes; every rc.13
citation was re-checked on rc.13's bytes. The probe scratch was not retained.

| Section | Covers |
|---|---|
| [solid-js.md](2026-10-22-solid-2-rc17/solid-js.md) | `lazy`, components, control flow, context, getters, `dynamicCore`, new exports |
| [web.md](2026-10-22-solid-2-rc17/web.md) | `render`/`hydrate`, `dynamic`, spread listeners, content props, host constants, new exports, compiler helpers |
| [signals.md](2026-10-22-solid-2-rc17/signals.md) | write guards, pending reads, strict reads, timing, diagnostics |

The `tsc` oracle's 108 rule cases give identical verdicts on rc.13 and rc.17.

## Decision and changes (ADR 0194)

rc.17 becomes the audited triple, with rc.13's answers except:

- **Spread listeners** (web W6): only `on` + an ASCII uppercase letter;
  `onclick` is an attribute (`SpreadEventKeys::OnUppercase`). The pinned
  compiler uses the same grammar for JSX attributes (ADR 0196).
- **`createOptimistic` setter** (signals C1): meets no owned-scope guard on
  rc.17, so SC2001 is withheld unless the setter is proven `createSignal`'s.
- **`latest(fn)`** (signals C2): absorbs a pending read on rc.17, so SC5001
  is not reported there.
- **`hydrate`** (web W2): may run `code` after it returns, so no inline
  execution is stated; the root claim holds.
- **`dynamic`'s static form** (web W3): now `solid-js`'s `dynamicCore`; under
  web rc.17 it is modelled only with `solid-js` rc.17.
- **New callback-taking exports** (`createDeferred`, `dynamicComponent`,
  `lazyModule`, `onHydrationEnd`; web `dispatchAsInteraction`,
  `dynamicComponent`, `setLinkClaim`): neither modelled nor excluded, so an
  import-scoped gap names them.

Recorded, not modelled: the new dev throw `PRIMITIVE_IN_EFFECT_CALLBACK`
(signals C3), writes allowed during disposal (C4), the removed derived-store
pending throw (C5), seedless `createStore(fn)` (C6), the delegated-handler
narrowing (web W13), the unowned subpaths `@solidjs/web/frames/{assets,
regions,trace,wire}` and `solid-js/internal/container-trace`. The `isServer`
host fold gains an rc.17 entry (web W9).
