# ADR 0269: a browser-target config for client applications

- Status: accepted and implemented (2026-10-09).
- Owners: `packages/cli/eslint.cjs` (the `browser-<dialect>` configs), its
  types and the CLI README.

The primary integration is Oxlint through the ESLint adapter
(`jsPlugins: ["solid-checker/eslint"]`). The runtime target comes only from
`settings.solidChecker.runtime.target`; unset, the analysis has no target,
and package contracts are read with their host-free claims. For the
primitives ledger that is the difference between 99 of 111 cases reported
correctly (browser) and 7 of 112 (no target). On the 94 rows measured under
both, a misuse gets a violation in 89 under browser and 9 without a target,
and no correct twin gets a violation under either.

## Decision

Ship an explicit, opt-in browser target, not an inferred one:

- Each dialect config gains a `browser-<dialect>` twin (`browser-v2`): the same
  rules, plus `settings.solidChecker.runtime.target: "browser"`. A later
  config that sets `runtime` overrides it.
- The README shows the equivalent Oxlint setting. It names the Solid 2 setups
  it fits, client start mode (`solid({ start: true })` in
  `@solidjs/vite-plugin`) and plain Vite SPAs, and when to leave it unset:
  projects whose files include server-only code, and libraries.

The default is unchanged. The user chooses the target, so no inference can
analyze server-only code as browser code behind their back.

## Rejected: inferring browser

An adapter-only inference that admitted only projects provably all-browser
admitted 0 of the 38 rc.13 corpus apps. The native checker takes one runtime
target per project, so the adapter cannot give server-only files a different
host. A sound per-file host from application reachability is designed in
`rust/target/research/per-file-host/DESIGN.md`. It finds candidate browser
roots in 36 of 38 corpus apps, but needs execution-context-scoped contract
admission. It also needs an ADR reconciling ADR 0241, under which no-target
core rules already have browser strength. That is estimated at 6 to 11 days
for client start mode and plain SPAs, more for routing, `"use server"` bodies
and SSR. It is deferred until users ask.

## Consequences

- No analysis change: the native checker, contracts and findings are
  untouched. Unit tests check that each browser config equals its dialect
  config plus the setting.
- A user who sets the browser target on a project that also contains
  server-only code takes on the risk the README states. The checker does not
  detect it.
