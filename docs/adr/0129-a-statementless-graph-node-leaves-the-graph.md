# ADR 0129: A statementless graph node leaves the graph

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the published dependency graph lane
  (`packages/cli/scripts/certify-contract.mjs`: `graphProposalStatesNothing`,
  `reachableGraphStatesWithoutStatementlessNodes`, the per-node generation loop)
- Relation: narrows the node-refusal cascade (`cascadeGraphNodeRefusals`) for
  one class of node. Follows ADR 0128. ADR 0027 (core edges are the dialect's)
  is unchanged and is what a pruned core edge falls back to.

## Context

After ADR 0128, rc.9 census head rows still got 0 closure candidates. The
graph node `@solidjs/web@2.0.0-rc.9 . [import]` refused its generation with
`normalized operation graph is invalid: acceptance receipt has no locally
closed semantic claim`.

The refusal is not about `@solidjs/web`. It comes from a dependency's
proposal that could not be projected for web's generation
(`project_untrusted_proposal_for_generation` → `derive_closed_claims_root`).
The dependency is `solid-js@2.0.0-rc.9 ./internal`:

- rc.9 web's `dist/web.js` imports `sourceKeys`, `viewOf`, … from
  `solid-js/internal`.
- rc.3's web does not import it, and rc.3's `solid-js` has no `./internal`
  entrypoint. That is why earlier head rows never met this node.
- Its `dist/internal.js` declares nine `const x = core.x` aliases, where
  `core` is `import * as core from 'solid-js'`. The names are not on the
  declaration surface of `.`, so `.`'s contract does not describe them. The
  generator then states `{ call: {}, shape: "unknown" }` for all nine.
- Every domain is declined by hazards in `dist/solid.js`: the global writes
  `Promise = MockPromise` at 4737–4758 and `Promise = ogPromise` at 5064–5083,
  and runtime accessor installations at 8671, 23003 and 23595.
- The 17 re-exports from `@solidjs/signals` are core and leave its surface.

The proposal states nothing and proposes nothing, so no receipt can ever close
a claim in it. That part is correct and stays. The cost was wrong: under the
cascade, one unacceptable dependency refused `@solidjs/web`, then
`@solid-primitives/utils`, then every root above them.

## Decision

1. **A generated non-root node whose proposal states nothing and proposes
   nothing is pruned, not refused.** It qualifies when it has one artifact
   case, no initialization claim, no closure candidate, and every export
   summary is `{ call: <only empty lists>, shape: "unknown" }`. Its
   dependents generate without it in their proposal-dependency catalog. It
   is not certified with the case, and neither is anything that only it
   reaches. The audit lists it under `graphPreparation.statesNothingNodes`.
2. **Anything else keeps the cascade.** That includes a node that failed to
   generate, and any document the recognizer does not read as statementless.

## Soundness

A pruned dependency is simply an unaccepted dependency of its dependents,
which is what its contract could only ever have told them:

- A dependent that re-exports from it finds no accepted binding and refuses.
  rc.9 web's server builds, which re-export `ssrScope as scope`, stay refused
  for that reason.
- A dependent whose closure reaches an ordinary unaccepted package gets
  `unaccepted-external-dependency` declines.
- A core edge takes the dialect's answer, as in the default lane (ADR 0027).
  Its unmodelled callees stay open obligations.

A misclassification in either direction costs precision only, never
soundness. Pruning a node that could have been accepted leaves its dependents
generating against an unaccepted dependency. Keeping a statementless node
keeps the old refusal.

## Consequences

Measured on 2026-09-27 with the release binary on the memo head project
alone: `@solid-primitives/memo@2.0.0-next.2` head certified with 145 closure
candidates and 37 certified closures (the floor row has 145). The only pruned
node was `solid-js@2.0.0-rc.9 ./internal [import]`.
