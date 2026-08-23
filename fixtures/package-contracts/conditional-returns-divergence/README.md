# A conditional branch that proves a return where the other proves none

`Show` returns `{ view: <accessor> }` in the `browser` branch and a plain
`{ view: props.when }` in the `node` branch. Both branches were analyzed, so
the node branch is not silence: it is a *proven negative*, the certified claim
that this export returns nothing reactive there.

`mergeSummaries` used to read only the both-present case as a divergence, so
`left.returns ?? right.returns` handed the browser branch's accessor to the
environment-unaware base. A server consumer then read a certified positive
claim about a value that is not reactive in its environment. One-sided
presence is a divergence too: the base is the unknown sentinel, and the exact
per-branch claims stay in `variants` so an environment-aware consumer loses
nothing.

The review plan's `unknown-sentinel` item for `.:Show: returns` carries the
reason under `because.divergences` — which branches disagreed and how. A merge
is the second emitter of the sentinel and used to be the silent one.

`Steady` is the negative control: identical in both branches, so it stays
unconditional and gains neither a variant nor a sentinel.

The `node_modules/solid-js` stub exists only so `createSignal` resolves to a
1.x dialect; nothing here depends on its body.
