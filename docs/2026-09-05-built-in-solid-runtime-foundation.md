# Built-in Solid runtime foundation — 2026-09-05

Ordinary analysis no longer needs or consumes package contracts for `solid-js`,
`@solidjs/signals`, or `@solidjs/web`. The selected Solid dialect supplies their
reviewed runtime model. External package contracts retain their existing
artifact and receipt boundary. [ADR 0027](adr/0027-built-in-solid-runtime-foundation.md)
was written before implementation.

## Initial finding

Both embedded core bundle lists and both bundle indexes were already empty.
Ordinary loading still decoded historical core documents, and caller-supplied
contracts could still overlay native callback and return behavior. Earlier
descriptions of active receipt-backed core bundles were incorrect. This change
removes those dormant dependencies and competing semantic authority.

## Changes and completion evidence

| Boundary | Implementation and control |
| --- | --- |
| Native and daemon loading | External-only catalog discovery skips core objects before opening documents or receipts. A catalog referencing nonexistent core objects leaves findings unchanged. |
| WASM and direct IR | Host loading and the shared normalized-index boundary withhold core facts. General independent certification APIs remain available. |
| Sessions and caches | Core entries are removed before metrics and semantic cache identity. Direct and aliased core bindings cannot change the analysis fingerprint. |
| Native semantics | The name-keyed contract return table, callback overlay, and obsolete bundled provenance constructor are removed. Findings retain actual program source locations. |
| Status | Core support is `builtin`, with a model identity and explicit authentication limitation. A core package outside the dialect is `unsupported-runtime`, with a dialect remedy rather than a receipt remedy. |
| Aliases and external packages | Consistent exact resolver facts identify installed core aliases for status. Similar names remain external; invalid external documents still refuse. |
| Historical audits | Artifact/conformance material and independent certifier identity checks remain intact. No sandbox or receipt policy changes. |

The existing Solid 1 and Solid 2 fixture READMEs document their native premise.
Focused tests cover missing core files, all three identities, aliases, external
lookalikes, cache identity, and incompatible dialects. No new fixture main was
added; the stable document count stays 185.

## Verification

- Backend library: 379 passed, including probe-harness controls.
- IR: 236; dialect: 63; WASM: 2 passed.
- Armed process suites: contracts 11, diagnostics 15, dialects 37 passed.
- Scripts: 154 tests in 25 files; CLI: 175 tests and TypeScript checking passed.
- Coverage: 94 projects, 547 findings, no moves.
- Ownership: 289 cases, 465 ledger rows, zero pending.
- Contract corpus: 94 fixtures, no moves.
- Schema parsing, dialect manifests, composed contracts and bundled-contract
  conformance passed.
- Formatting and workspace Clippy with warnings denied passed. The debug
  checker was rebuilt through the Makefile after Clippy to restore its compiled
  certification pins. `git diff --check` passed.

Logs are under `/private/tmp/core-foundation-*.log`. The composed-contract
command unexpectedly invoked Cargo internally and waited on an active build;
it completed successfully. Subsequent Cargo checks ran sequentially through
the Makefile, and the final pinned rebuild completed.

No snapshots were updated for this refactor. Existing dirty-worktree changes
were preserved. No ecosystem baseline or phase ledger was refreshed, no commit
or push was made, and `make verify` remains for the lead.

## Remaining limits

The existing major-version dialect selection and declaration bootstrap are
model applicability mechanisms, not authentication of installed runtime bytes.
This refactor does not strengthen them or independently certify every core
version. Unmodelled behavior remains unknown. Independent core certification
must not use its own built-in premises circularly.

The prior Solid 1 scheduled/debounce/rootless discovery exception is preserved;
those helper packages are not promoted into the built-in foundation. Its
separate migration remains open. TypeScript execution-profile restrictions and
the independent accessor-census blocker are unchanged by this refactor.
