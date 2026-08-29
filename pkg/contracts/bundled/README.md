# Receipt-issued first-party contracts

Each dialect directory contains a `bundle-index.json` plus normalized stable-v1
main documents and their proof-issued receipts. The same
bytes are generated under `rust/crates/solid-dialect/contracts/` so the runtime
and review locations cannot drift.

`solid-v1` is generated from the checked published-artifact authority under
`benchmarks/package-contract-v2/phase14/solid-v1-authority/`. It covers exact
`solid-js@1.9.14`, `@solid-primitives/scheduled@1.5.3`,
`@solid-primitives/debounce@1.3.0`, and
`@solid-primitives/rootless@1.5.4` artifact cases. Two JSX subpaths have no
common runtime/declaration value bindings; they remain in the package census
without an accepted semantic case.

`solid-v2` is generated from the checked RC.3 conformance authority. It covers
exact `solid-js@2.0.0-rc.3`, `@solidjs/web@2.0.0-rc.3`, and
`@solidjs/signals@2.0.0-rc.3` cases. Environment selection is explicit; the
consumer never guesses a browser, node, development, or production branch.

Run `make contracts` to reissue both physical bundle sets and `make
contract-conformance` to check deterministic bytes, receipt roots, dialect
inventories, and live registry pins. `runtime-lock.json` records the exact
published package closure used by the runtime authority.
