# Package callback consumer

This fixture is a contract-consumer regression. The reviewed contract carries
callback execution plus an explicit `leaf` owner for `runLeaf`, and an exported
`effect` owner requirement for `runOwnedEffect`.

- `Leaf` proves a contract-provided leaf owner reaches the callback body; its
  nested `createEffect` must not produce `SC4001`.
- The module-level `runOwnedEffect()` call must produce `SC4001` from the
  contract's exported-call owner requirement.
- `Bad` retains the timing-only mixed callback and must produce its existing
  `SC1001` finding.

The contract is fixture-reviewed evidence, not a claim about a published
package. Missing callback-owner rows remain fail-closed elsewhere.
