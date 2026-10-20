# runtime-oracle

Cases for `scripts/runtime-oracle.mjs`, which runs each in Chrome on the Solid 2 dev build and compares the runtime's diagnostics with the checker's verdict at the same site; a dev diagnostic where the checker reports nothing is an unproven clean claim. Cases run in `../solid-checker-rc13-corpus/apps/helge-dev` (rc.13; override with `--install`), against the real packages: `*.d.ts` stubs and `node_modules` are not copied, and a case `tsc` rejects is `type-error`.

A case is a directory with an `App.tsx` (no `tsconfig.json`); every exported component is rendered and events are dispatched on every element. Optional `oracle.json`: `note`, `steps`, `install` (corpus app with the same Solid version), `open` (known unproven clean sites `{ code, site, reason }`; a site that stops firing is `stale-open`).

Verdicts: `covered-violation`/`covered-uncertifiable` (a finding on the diagnostic's line or an application stack frame), `unproven-clean` (exit 1), `known-open`, `no-rule`; `static-only` (checker violation, no runtime diagnostic: inspect before calling it a false positive, e.g. a store silently drops a direct write); collector failures are `harness-error`. Quiet cases are controls, not proof every branch executed.

The `c*`, `r*`, `review-r*` and `oracle-*` cases are reviewer counterexamples and minimal probes; adaptations (mount wrappers, no-op sink witnesses, `on:click` to `onClick`) are in each `note`; r10 cases use `install: "helge-dev"` (`@solidjs/router@2.0.0-next.26`). Archive cases needing package metadata, CommonJS exports or declaration-only values are not represented.
