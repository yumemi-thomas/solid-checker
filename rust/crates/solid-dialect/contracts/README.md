# Historical dialect contract artifacts

These are review-location copies of the historical stable-v1 main documents
under `pkg/contracts/bundled/`. Both bundle indexes are empty. The documents
remain proposals and audit material; they do not carry active authority into
ordinary analysis.

The documents serve two related checks:

- dialect tests cross-check reviewed vocabulary and callback timing against
  historical conformance material;
- bundle gates keep the two inventories consistent. An empty bundle index
  cannot establish that the analyzer consumes receipts.

`solid-v1` covers published `solid-js@1.9.14` and exact scheduled, debounce,
and rootless packages. `solid-v2` covers published RC.3 `solid-js`,
`@solidjs/web`, and `@solidjs/signals`. Browser/node and
development/production cases remain separate artifact cases; no consumer may
choose one by export spelling alone.

Each `rust/dialects/<id>/dialect.json` names both bundle indexes. Preserve the
artifact pins and conformance checks. Core behavior is supplied by the built-in
runtime model (ADR 0027); it is not independently certified by restating the
same dialect tables in a package contract.
