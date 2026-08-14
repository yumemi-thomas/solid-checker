# Bundled runtime contracts

These are the normalized, entrypoint-aware package contracts compiled into the
backend and used during project analysis. Paths are grouped by the same stable
dialect id used by the checker:

- `solid-v1/solid-js.json` models `solid-js@1.9.14`;
- `solid-v2/solid-js.json` models `solid-js@2.0.0-rc.0`;
- `solid-v2/solidjs-web.json` models `@solidjs/web@2.0.0-rc.0`.

The per-dialect assembly files at `rust/dialects/<id>/dialect.json` own these
paths. `node scripts/check-bundled-contracts.mjs` enumerates contracts marked
`probeRuntime`, installs their exact releases, checks their export surfaces and
integrity, and executes every declared behavior probe.

The Solid 1.x artifact is composed by
`scripts/generate-bundled-solid1-contract.mjs` from the adjacent
`solid-v1/solid-js-census.json` and the reviewed vocabulary contract. Run
`node scripts/dialect-manifests.mjs check-composed-contracts` to detect drift.
`make contract-conformance` runs both forms of verification.

The similarly named files below `rust/crates/solid-dialect/contracts/` are a
different artifact set: flat review inputs used to test the hand-written
vocabulary and generate Rust export indexes. See that directory's README and
use `make contracts` to regenerate them.
