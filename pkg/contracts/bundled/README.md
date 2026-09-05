# Historical first-party contract artifacts

Both dialect bundle indexes are currently empty. The retained stable-v1 main
documents are proposals and conformance history, not active receipt-issued
inputs to ordinary analysis. The matching review location is
`rust/crates/solid-dialect/contracts/`.

The historical `solid-v1` material comes from the published-artifact audit under
`benchmarks/package-contract-v2/phase14/solid-v1-authority/`. It covers exact
`solid-js@1.9.14`, `@solid-primitives/scheduled@1.5.3`,
`@solid-primitives/debounce@1.3.0`, and
`@solid-primitives/rootless@1.5.4` artifact cases. Two JSX subpaths have no
common runtime/declaration value bindings; they remain in the package census
without an accepted semantic case.

The historical `solid-v2` material comes from the RC.3 conformance audit. It covers
exact `solid-js@2.0.0-rc.3`, `@solidjs/web@2.0.0-rc.3`, and
`@solidjs/signals@2.0.0-rc.3` cases. Environment selection is explicit; the
consumer never guesses a browser, node, development, or production branch.

Ordinary analysis uses the built-in Solid dialect for `solid-js`,
`@solidjs/signals`, and `@solidjs/web`; core contracts cannot override that
model. External packages still require independently accepted contracts.
See ADR 0027 for the migration and the distinction between reviewed runtime
premises and independent package certification.

Bundle and conformance gates retain the historical inventories and artifact
pins. `runtime-lock.json` records the published closure used by the audit;
its existence alone is not evidence that an installed runtime was validated.
