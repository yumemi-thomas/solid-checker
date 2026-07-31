# v1/reactive-source-uncaptured

`SC9011` · **warning** · uncertifiable

A reactive source flows into a position the analysis cannot see into, so whether
it is ever read in a tracking scope cannot be certified.

## What it does

Flags reactive sources — signal accessors, stores, derived functions — that are
passed to functions whose reactive behavior the analysis does not know: no
source in the project, no package contract entry, no primitive semantics. Part
of the fine-grained decomposition of eslint-plugin-solid's monolithic
`reactivity` rule.

## Why is this analysis-limiting?

A reactive value handed to an unknown function may be read immediately (severing
reactivity), called inside a tracking scope (correct), or stored for later. The
analysis can prove none of these, so every read that flows through the unknown
function becomes uncertifiable rather than certified or proven wrong.

## How to fix

Give the analysis a way to see the call:

- If the receiving function is yours, keep it in the project so its body is
  analyzed directly.
- If it comes from a package, describe the export in the package's
  `solid-reactivity.json` contract — which arguments it tracks and what it
  returns.

See [package-contracts.md](../../package-contracts.md) for the contract format.

## Related

- [v1/package-contract-export-missing](./package-contract-export-missing.md) — a contract exists but misses an export
- [v1/strict-read-untracked](./strict-read-untracked.md) — reads proven untracked
