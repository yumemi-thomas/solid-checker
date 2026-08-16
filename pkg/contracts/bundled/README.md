# Bundled runtime contracts

These are the normalized, entrypoint-aware package contracts compiled into the
backend and used during project analysis. Paths are grouped by the same stable
dialect id used by the checker:

- `solid-v1/solid-js.json` models `solid-js@1.9.14`;
- `solid-v1/solid-primitives-scheduled.json` is the reviewed callback-timing
  overlay for `@solid-primitives/scheduled@1.5.3`;
- `solid-v2/solid-js.json` models `solid-js@2.0.0-rc.0`;
- `solid-v2/solidjs-web.json` models `@solidjs/web@2.0.0-rc.0`;
- `runtime-lock.json` pins the resolved dependency closure used by the Solid 2
  runtime probes, including `@solidjs/signals`, with version and npm integrity.

The per-dialect assembly files at `rust/dialects/<id>/dialect.json` own these
paths. `node scripts/check-bundled-contracts.mjs` enumerates contracts marked
`probeRuntime`, installs their exact releases, checks their export surfaces and
integrity, checks every edge in `runtime-lock.json`, and executes every
declared behavior probe in client, server, development, and production Node
condition modes. A lock or probe mismatch fails conformance; it is not repaired
by `--write`.

The Solid 1.x artifact is composed by
`scripts/generate-bundled-solid1-contract.mjs` from the adjacent
`solid-v1/solid-js-census.json` and the reviewed vocabulary contract. Run
`node scripts/dialect-manifests.mjs check-composed-contracts` to detect drift.
`make contract-conformance` runs both forms of verification.

The scheduled-primitives overlay is intentionally Solid 1.x-only because that
release's peer range is `solid-js@^1.6.12`. Its exact npm version and integrity
are pinned; its deferred/inline claims were reviewed against the published
implementation and the contract generator's returned-wrapper regression suite.

The similarly named files below `rust/crates/solid-dialect/contracts/` are a
different artifact set: flat review inputs used to test the hand-written
vocabulary and generate Rust export indexes. See that directory's README and
use `make contracts` to regenerate them.

The Solid 2 contracts retain conditional variants for browser/development and
server/worker builds. The backend does not guess which variant an unconfigured
consumer runs: it reports an uncertifiable environment-dependent export until
the runtime condition is explicitly selected.
