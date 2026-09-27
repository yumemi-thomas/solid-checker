# A declaration re-export of a name its module does not declare

`types/index.d.ts` re-exports `GAP` from `./core.js`, and `types/core.d.ts`
declares no `GAP`; the runtime exports it (`dist/core.js`). This is the shape of
`solid-js@2.0.0-rc.9`, whose typings re-export `$DEVCOMP`,
`createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder` and
`sharedConfig` from modules that no longer declare them (an upstream typing
defect; `docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`).

`GAP` has no declaration identity for any consumer, so it cannot be described.
Before ADR 0128 the emitter published it anyway and the whole artifact case
refused at `bind_exports` with `resolved artifact has no exact
runtime/declaration binding for export "GAP"`, costing `own` too. Now:

- the resolver names `GAP` in `unboundDeclarationExports`;
- the emitter leaves it off the surface, so the contract publishes `own` alone;
- certification replays the same census from the archive bytes and refuses a
  resolver that disagrees in either direction.

What must hold: the contract has exactly one export, `own`, and no refusal.
A consumer that imports `GAP` from this package reaches a name the accepted
contract does not have, which stays uncertifiable (`SC9005`).

`namespace-export-surface` is the unchanged negative control: an exported
namespace is declared by its own module, so it is not this gap, and it still
refuses the case.

No stub is involved: these are the fixture's own bytes. `tsc` reports TS2305 on
`types/index.d.ts` without `skipLibCheck` (as it does on the published rc.9);
that is a diagnostic about the package's typings, and nothing here reports it.
