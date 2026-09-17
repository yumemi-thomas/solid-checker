---
status: accepted
---

# Built-in Solid runtime foundation

## Decision

Ordinary analysis obtains the behavior of `solid-js`, `@solidjs/signals`, and
`@solidjs/web` from the selected, versioned Solid dialect. These three package
identities form the built-in runtime foundation. Package contracts describe
external packages. A core contract must not override, supplement, or serve as
a prerequisite for built-in runtime semantics. This decision is about ordinary
analysis, not permission for the package certifier to prove core from itself.

The dialect supplies reviewed premises. The engine combines them with exact
symbols, Type Facts, control flow and compiler execution facts. It does not
claim that those premises are independent implementation certificates. Missing
native behavior stays unknown; deleting a contract requirement does not prove
that an unmodelled export has no effects.

## Current-state audit before implementation (2026-09-05)

Both `EMBEDDED_BUNDLES` and `EMBEDDED_SOLID1_BUNDLES` in
`first_party_bundles.rs` are empty. Both checked bundle indexes contain no
contracts. The loader nevertheless decodes historical conformance and Solid 1
contract documents before returning an index. The earlier documentation that
describes these as active receipt-issued core bundles is stale.

There are still active paths for a caller-supplied core contract:

- `AcceptedContractIndex` can contain an exact core package binding.
- `contracts.rs` projects its export facts and overlays callback timing even
  for a primitive already owned by the dialect.
- `accepted_bundled_returns` builds a name-keyed return table from those
  bindings; source discovery and interprocedural inference consume that table.
- package status reporting asks users to generate core receipts.
- native facts cite `bundled://` paths even when they came from Rust tables.

The current dialect selection is major-version based, and primitive discovery
also has a declaration-path bootstrap. Those are not authenticated runtime
identity checks. Historical bundle closure verification is a separate path and
cannot be cited as if it guarded all built-in facts. The refactor must preserve
the existing audited artifact/conformance material and make applicability and
its limitations explicit rather than claiming that removing empty bundles
establishes stronger runtime authentication.

## Authority and migration boundary

The normalized accepted-contract index remains a general certification data
structure: independent core certification experiments may still construct it.
At the ordinary-analysis boundary, core entries are excluded by their
authenticated package identity, including entries reached through an alias.
Core specifier refusal markers are excluded too: obsolete/missing receipts are
not a requirement of this foundation. The same boundary must apply to native,
daemon, WASM and direct IR entry points, to analysis metrics, and to retained
cache identities.

External packages retain exact artifact selection and receipt validation.
Names similar to the three core names are external. No new receipt issuance,
signature policy, implementation-census disposition, or probe authority is
introduced. ADRs 0005 and 0007's prohibition on circular core certification
remains in force. The controlled type-erasure profile and its security pins
are independent and unchanged.

Runtime support reporting names the versioned dialect model identity
(`solid-v1/model-1` or `solid-v2/model-1`) instead of a contract filename.
Native findings retain their actual program source locations; the obsolete
`bundled://` provenance constructor and contract-return table are removed.
Runtime support reporting must distinguish built-in model
support from independent package certification and must not recommend
generating a contract as the remedy for unsupported core behavior.

## Completion criteria

1. No ordinary analysis entry point needs or consumes a core receipt/document.
2. Caller-supplied core contracts cannot change findings, callback timing,
   return classification, metrics, or cache identity. External contracts still
   contribute their proven facts.
3. Native model facts identify their actual source. Unknown exports and
   unsupported applicability cannot obtain fabricated negative knowledge.
4. Historical artifact identities and conformance checks remain available;
   the documentation accurately distinguishes them from active authority.
5. Focused named/namespace/alias, core/external, and mismatch controls pass,
   followed by armed process tests, coverage, ownership, CLI/WASM and relevant
   contract gates. Snapshot changes are reviewed before any update.

## Implementation and verification

The migration is implemented at the shared IR boundary, backend session,
incremental cache, native and daemon catalog loaders, and WASM host loader.
Core catalog objects are withheld before their documents or receipts are
opened. The old core return table and callback overlay are removed. External
missing-contract discovery has a separate owner and retains its prior behavior.

Contract status reports `builtin` with the selected model identity, or
`unsupported-runtime` when a core package is outside that dialect. Installed
aliases require consistent resolved package facts to receive the built-in
status; similarly named external packages remain external.

Focused controls and the broader verification passed without snapshot moves.
The dated [implementation report](../2026-09-05-built-in-solid-runtime-foundation.md)
records the checks and remaining applicability limits. No sandbox policy,
receipt schema, certification pin, or core certification premise changed.
