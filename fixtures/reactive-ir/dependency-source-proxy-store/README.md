# dependency-source-proxy-store

**Claim (ADR 0287 under ADR 0284).** A branch of a forwarding proxy trap that returns another object's member is excluded only when a host-constant fold proves it dead for the declared host. `.solid-checker/runtime.json` declares a browser development host and selects `dep-query` as dependency source.

- `FoldedQuery` (violation): `useBaseQuery`'s server branch is guarded by `isServer` imported from `@solidjs/web`. Every file the `browser` condition selects (`dist/web.js`, `dist/web.dev.js`) exports `const isServer = false`, so the fold (ADR 0166) removes the branch, and the trap returns only `Reflect.get(target, …)`. `useQuery` forwards to `useBaseQuery` in the same file. SC1001 at `query.search`.
- `WindowQuery` (negative): the same trap guarded by `var isServer2 = typeof window === "undefined"`, the guard `@tanstack/solid-query` 6.0.0-rc.0 publishes. `typeof window` is not a host constant, so the branch stays live and the proof refuses: silent. Only a declared Window realm decides it (ADR 0295, `dependency-source-window-realm`); this project declares none.

Under `--runtime-target node`, `dist/server.js` exports `const isServer = true`; the server branch is live and `FoldedQuery` is silent too. Without the flag, both hooks resolve to `dist/index.d.ts` only and nothing is reported.

`solid-js.d.ts`, `tsconfig.json`, and the `solid-js` and `@solidjs/signals` stubs are copied from `returned-store-nested-read` (claim-bearing signatures byte-faithful to the published 2.0.0-rc.13 typings). The `@solidjs/web` stub keeps only its `exports` shape for `.` and the `isServer` export of each target. `App.tsx` passes `tsc --noEmit`; the package's JavaScript is not type-checked (`checkJs: false`).
