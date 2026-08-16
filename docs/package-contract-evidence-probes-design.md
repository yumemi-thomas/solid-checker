# Package-contract evidence and probe design

This note records the schema-v1-compatible design for per-claim evidence,
runtime probes, and probe-driven discovery. It is intentionally short and is
the decision record for contract-series slices 1–3.

## Claim evidence

The existing contract-level `evidence` object remains the compatibility and
certification default. Claim-bearing rows may additionally carry an optional
`evidence` object. The rows are the export summary, each reactive-read row,
each callback row, and every recursive return node. The object has one of four
claims:

```json
{ "kind": "inferred" }
{ "kind": "probed", "modes": ["browser"], "calls": 2 }
{ "kind": "reviewed" }
{ "kind": "inherited-from", "package": "solid-js", "version": "2.0.0-rc.0" }
```

`probed` requires at least one mode and one call. `inherited-from` requires an
exact package and version so inherited knowledge cannot be mistaken for a
local observation. The field is additive to schema version 1. A document with
no row evidence keeps today’s contract-level certification behavior. When row
evidence is present, certification rejects an `inferred` row; it accepts only
probed, reviewed, or exact inherited evidence and still applies the existing
contract-level gate.

The package generator emits `inferred` rows. The conformance suite may replace
or validate those rows only through an explicit checked-in promotion; observed
behavior is never written into a contract automatically.

## Runtime probes

Conformance derives the conditional runtime leaves from each package export
map. It runs the same behavioral claim in every applicable mode (`client`,
`server`, `development`, and `production`, plus package-specific conditions)
and records the mode names and call count. A callback probe invokes the
callback once during the initial operation and again after the relevant
reactive or scheduling trigger. A claim passes only if every applicable mode
and both call phases pass. Surface inspection remains independent of behavior
and continues to check every materialized ESM leaf.

## Differential discovery

The probe worker produces two outputs: the existing confirmation results and a
normalized observed surface/claim set. Conformance compares that observed set
with the expanded contract. An observed export or behavior absent from the
contract is an incompleteness failure; a contract claim that disagrees with an
observation is a conformance failure. The worker never edits a contract. A
human promotion or a reviewed generator change is required before an
observation becomes a claim.

When conditional runtime leaves disagree, the generator preserves the complete
per-leaf summaries in additive export `variants` entries instead of merging an
environment-specific claim into an unconditional one. Until analysis has an
explicit runtime-condition selector, the Rust reader refuses that export as an
uncertifiable environment-dependent boundary. This keeps SSR/client skew
visible and prevents a browser summary from certifying server code.

## Migration

Existing bundled, published, and local contracts remain valid without row
evidence. They continue to use contract-level evidence until a maintainer
promotes individual rows. Generated contracts begin carrying row-level
`inferred` evidence, which makes their uncertainty visible without changing
the current default refusal to certify inferred contracts. Bundled contracts
can be promoted incrementally: first add exact row evidence, then keep the
existing package version/integrity and behavioral conformance gates unchanged.
