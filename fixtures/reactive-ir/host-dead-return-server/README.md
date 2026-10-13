# host-dead-return-server

**Claim (ADR 0288).** `host-dead-return`'s package and components under a
Node string-SSR host. `@solidjs/web`'s `node` entries export `isServer =
true`, so the fold keeps each guarded server return, removes the signal
factory after it, and nothing is reported. A browser-host proof never leaks
into a host where the guard is live.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop`, `DefaultLoop`, `BlockLoop` | none | the live return is `[() => false, noop, noop]`; slot 1 is a module binding, which ADR 0282 does not resolve |
| `IdleLoop` | none | the live return's slot 1 is `idle`, a closure the factory declares: it resolves, and reads nothing |
| `ClientLoop` | none | `if (!isServer) { ... }` is dead; the server return is the one left |
| `EitherLoop` | none | `isServer \|\| flag` is decided `true` here |
| `FlaggedLoop`, `NamespacedLoop`, `ProjectLoop` | none | refused as under the browser host |

Stubs and `tsc --noEmit -p` as in `host-dead-return`, whose `node_modules`
and `factories.ts` this copies unchanged.
