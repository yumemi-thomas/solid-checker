# host-dead-return-server

`host-dead-return`'s package and components under a Node string-SSR host (ADR 0288): `@solidjs/web`'s `node` entries export `isServer = true`, so the fold keeps each guarded server return, removes the signal factory after it, and nothing is reported for any case: a browser-host proof never leaks into a host where the guard is live. Stubs and `factories.ts` copy `host-dead-return` unchanged.
