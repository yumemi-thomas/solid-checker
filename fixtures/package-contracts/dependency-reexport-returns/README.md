# dependency-reexport-returns

ADR 0170's tracer, certified in the published graph by
`contract_certification::tests::a_reexport_restates_its_dependencys_closed_returns_end_to_end`.
Not a contract-corpus fixture: the test builds both archives from these files
and certifies the root with `leaf.js` as its one dependency node. The root's
proposal is the generator's own for a re-export: the projection of the leaf's
export, which restates the leaf's closed `returns` operations.

| root export | the leaf | `returns` in the root |
| --- | --- | --- |
| `count` | closes one plain return | closes the same plain return, discharged against the leaf's claim |
| `reset` | closes `returns: []` | closes `[]` (ADR 0143, unchanged) |
| `widened` | withholds its plain return (a reassigned `let`) | stays open: nothing is restated that the leaf did not certify |

No `solid-js` stub: nothing here imports Solid, and the dialect is not what the
test measures. `leaf.js` and `leaf.d.ts` are byte copies of
`dependency-plain-return`'s.

A local wrapper is pinned beside it by the generator-level test
`scripts/contract-dependency-reexport.test.mjs`, where a re-export and a wrapper of
the same dependency name share one package.
