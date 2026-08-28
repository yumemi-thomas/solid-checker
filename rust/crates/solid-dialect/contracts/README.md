# Receipt-issued dialect contracts

These are the review-location copies of the exact temporary-v2 bundles each
dialect contributes. Every dialect directory contains a `bundle-index.json`,
normalized main documents, and proof-issued receipts. Their bytes must match
the runtime copies under `pkg/contracts/bundled/`; the analyzer never consumes
an older flat vocabulary contract or a schema-1 compatibility form.

The documents serve two related checks:

- dialect tests cross-check hand-written vocabulary and callback timing against
  exact accepted exports;
- bundle gates prove that the same receipted artifact cases are embedded for
  ordinary analysis.

`solid-v1` covers published `solid-js@1.9.14` and exact scheduled, debounce,
and rootless packages. `solid-v2` covers published RC.3 `solid-js`,
`@solidjs/web`, and `@solidjs/signals`. Browser/node and
development/production cases remain separate artifact cases; no consumer may
choose one by export spelling alone.

Each `rust/dialects/<id>/dialect.json` names the review and runtime bundle
indexes. Run `make contracts` to regenerate both physical locations and `make
contract-conformance` to verify deterministic document bytes, receipts,
manifests, runtime locks, exact package pins, and dialect export indexes. Do not
hand-copy or independently edit either bundle set.
