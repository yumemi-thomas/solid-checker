# Solid compiler semantic-facts bootstrap conformance

Date: 2026-08-27

Phase 0A's compiler transition is complete. The checker now consumes
`solidjs-compiler` from `yumemi-thomas/solid@1d81e67fd393d12c74b13aa7d3fb492f3d85353b`,
based exactly on `solidjs/solid#next@a10cf1a147209d8da50697896742d2b1d4afad75`.
The branch is `solid-checker/compiler-facts-v2`. It is fork-only; no upstream
Solid pull request was opened or will be opened for this semantic trace.

## Scope ruling

The fork commit changes only `packages/compiler`. Its source delta consists of
the trace model, census and output-neutral recorder calls at existing lowering
decisions, reconciliation/serialization, and the host-independent trace
option/result. The remaining changes are trace dependencies, facts tests, the
independent transform baseline, and the semantic-facts boundary document.

No runtime, lowering, generated-output, diagnostic, compiler feature,
optimization, or unrelated dependency change is carried. Historical DOM
Expressions behavior fixes were excluded; the complete rulings are in the
[port ledger](2026-08-27-port-ledger.md).

## Output and trace proof

- 358 fixture/probe entries were compiled independently from the unmodified
  exact upstream base and from the fork with tracing disabled.
- Both baselines are 955,562 bytes and have SHA-256
  `2f7f2b9a9d8a8cf3eb1d60a0cc35ee4a97142a9ff027fdcb2079f741ed00b92b`.
- `cmp` reported byte identity.
- With tracing enabled and disabled, every corpus entry produced identical
  JavaScript and source maps; rejected inputs produced identical diagnostics.
- Census mutation tests reject unresolved, uncensused, and conflicting terminal
  decisions. Unsupported modes fail closed.

## Verification results

| Gate | Result |
| --- | --- |
| Compiler Rust tests, default features | 15 passed |
| Compiler Rust tests, no default features | 8 unit + 5 census + 17 regression + 7 interface passed; 1 deliberate baseline-writer ignored |
| Compiler JavaScript/Vitest suite | 28 files, 3,933 tests passed |
| Solid 2 adapter | 9 passed |
| Solid 1 adapter and distinct-compiler selection | 7 passed |
| Contract process | 57 passed |
| Dialect process | 37 passed |
| Coverage | 94 fixture projects, 557 findings matched |
| Ownership gate | 289 cases, 465 ledger rows, 0 pending |
| Full checker `make verify` | passed in 42.83 seconds |

The compiler's strict all-target Clippy command still reports one
`clippy::collapsible-if` in `dom/element.rs`. The same code is present in the
exact upstream base. The semantic fork does not carry an unrelated formatting
patch; Clippy with that inherited lint allowed reports no facts-delta warnings.
The checker's full workspace Clippy gate passes.

## Finding delta

The frozen checker baseline contained 558 findings. The migrated checker has
557. Exactly one stale uncertifiable SC1001 was removed in
`jsx-census-gap-solid-2`: current Solid positively lowers `body()` beside a
dynamic `textContent` attribute as a tracked child insert. The independent
SC8003 authoring violation remains, and all other findings are unchanged.

This is an accuracy correction caused by adopting the current authoritative
compiler behavior. Preserving the old count would require discarding a positive
fact in the adapter, so no compatibility shim was added.

## Remaining Phase 0A work

This section records the state at the compiler-only handoff. It is superseded
by the completed
[Type Facts repatriation conformance](../typefacts-repatriation/2026-08-27-conformance.md).

The compiler half (C1-C18) is complete. Type Facts repatriation is the remaining
half of Phase 0A and has not started in this change. The external producer and
client remain pinned and coherent; no mixed local/external Type Facts state was
introduced.
