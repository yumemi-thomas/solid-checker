# ADR 0246: A package's own read stays the package's through wrappers

- Status: accepted and implemented (2026-10-08).
- Owner: the `package_internal` test in the result-read emission of
  `solid-reactive-ir/src/interproc.rs`.
- Fixture: `fixtures/reactive-ir/package-own-tracked-read-consumer`
  (`PeekedThroughWrapper`, `PeekedThroughModule`).
- Investigation: `rust/target/research/merge-getter-fp/FINDINGS.md`.

## Context

A contract may state that an export reads reactive state of its own while it
runs. Called directly, such a read is reported as the package's own read
(`SC1001`, uncertifiable): it is the package's implementation, not misuse at
the call. The test for that required the read's symbol to equal the callee
being emitted. When the export was called from a project function that a
component then called, the read reached the component as a row of the
wrapper's summary. The symbols no longer matched, so the read became a proven
violation at the wrapper call.

This surfaced in ADR 0245: closing `createScrollPosition`'s reads produced six
false violations in app-game's `solid-virtual`. The real read runs inside an
unlabelled `untrack`, where rc.13 does not warn
(`@solidjs/signals/dist/dev-shared.js:5863-5881`).

## Decision

A read whose declaration is contract-declared state of an export
(`contract_declared_state`), and whose symbol is a contracted export
(`contract_export_identity`), is the package's own read wherever it
propagates. It is uncertifiable at a wrapper's call exactly as at a direct
one.

## Consequences

- No proven violation is built from a package's own read through a wrapper.
  This only turns violations into uncertifiable results.
- Still open: a contract read's own execution context. A read the package
  performs inside an explicit `untrack` is still projected as a plain
  untracked read at the call. The `scroll` claim stays withdrawn until the
  contract can state that clearing (the read-tracking design in
  `rust/target/research/read-tracking/DESIGN.md` covers it).
