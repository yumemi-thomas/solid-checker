# reactive-source-uncaptured

`SC9011` · **warning** · uncertifiable

A reactive source flows into a position the analysis cannot see into, so whether
it is ever read in a tracking scope cannot be certified.

## What it does

Flags reactive sources — signal accessors, stores, derived functions — that
are passed to a *package-imported* function whose reactive behavior the
analysis does not know: no source in the project, no package contract entry,
no primitive semantics. Shared with the 1.x catalog as
[v1/reactive-source-uncaptured](v1/reactive-source-uncaptured.md) under the
same code, so a suppression comment survives a migration.

Only callees imported from a package are reported, because those are the
callees the fix applies to. An ambient global (`setTimeout`, `console.log`,
an array method) comes from no package, so no contract could ever describe
it; reads flowing through one stay uncertified without a finding demanding a
fix nobody can write.

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

See [package-contracts.md](../package-contracts.md) for the contract format.

## Related

- [package-contract-export-missing](package-contract-export-missing.md) — a contract exists but misses an export
- [strict-read-untracked](strict-read-untracked.md) — reads proven untracked
