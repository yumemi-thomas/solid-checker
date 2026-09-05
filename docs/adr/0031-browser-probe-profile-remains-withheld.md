# ADR 0031: Browser probe profile remains withheld

- Status: superseded in part by ADR 0033 (2026-09-05), which admits a pinned
  headless-shell boundary over a checker-owned CDP pipe as a controlled profile;
  ADR 0032 measured that boundary and named the premises. What this ADR forbids
  — Vitest browser mode, ambient Playwright caches and fake globals as
  *authority* — remains in force
- Date: 2026-09-05
- Owners: package-contract certifier and probe harness

## Context

Six of the original TypeScript feasibility samples reached source loading and
then needed a browser API. A fresh native census narrows the useful population:
`getScrollParent` in Kobalte 0.9.2 and 2.0.0-alpha.0 and
`runAfterTransition` in 0.9.2 reach a mandatory gate. `focusWithoutScrolling`
refuses on `instanceof`; both `debugPolygon` cases refuse on reachable property
getters. A browser executor could therefore affect at most three current
closures, and it cannot waive the three census refusals.

Vitest browser mode is available as a development-test concept, and local
Playwright browser binaries happen to exist on this machine. Neither fact is a
production trust boundary. Vitest inserts Vite transformation and a test runner
protocol whose emitted modules, resolver, browser executable and page bootstrap
are not compiled pins in this checker. The existing Node harness's descriptor-3
startup/run framing, process identity, exact package-resolution echo, primordial
capture before recipe import, prototype freezing and watched private filesystem
do not automatically hold in a browser page. A DOM shim would test the shim,
not browser behavior.

## Decision

Retain an explicit browser-profile refusal. Do not synthesize `document`,
`window`, `HTMLElement`, layout, computed style, transition events, or animation
frames in the Node worker. Do not treat Vitest's browser mode or an ambient
Playwright cache as certification authority.

A future browser profile may be admitted only after it binds all of the
following in one versioned policy:

- the browser executable bytes and version, driver and transport protocol, and
  launch flags;
- the authenticated source graph, every derived browser-loadable module, the
  transform and import-map algorithm, and exact module URLs;
- a checker-owned page bootstrap that captures report primitives and freezes
  the relevant prototypes before any recipe or package module loads;
- an authenticated startup identity and exactly one bounded run report per
  repeat, with the browser process group killed on every exit path;
- the browser profile's origin, permissions, storage, service workers, network,
  filesystem and child-process dispositions;
- exact artifact-case resolution and a consumer capability that names this
  browser profile. A digest alone is not compatibility with an application's
  browser, bundler, transforms, polyfills or layout engine.

## Consequences

The three census-ready browser candidates remain incomplete rather than gaining
a weaker certificate. This is a concrete harness/profile blocker, not evidence
that their `creates` domains are closed. The other three browser-feasibility
cases remain blocked earlier by the native census, so implementing a browser
driver would not certify them.

The decision can be revisited with an independently reviewed browser harness.
Doing so will require a new ADR, worker protocol, sandbox scheme and controlled
receipt identity; it cannot reuse the Node-strip profile receipts. ADR 0032
records the measured design such a harness would take and the ordered
decisions — a provenance-bearing browser build input, a sandbox-scheme decision
on the browser's own profile writes, a replacement for the resolution
independence premise, and a service-surface disposition table — that must
precede it.
