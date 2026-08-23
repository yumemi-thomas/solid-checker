# Two conditional branches that prove different returns

The control for
[`conditional-returns-divergence`](../conditional-returns-divergence/README.md).
Here *both* branches prove a reactive return and they disagree about which
property carries it: `{ view }` in the `browser` branch, `{ other }` in the
`node` branch.

This path already produced the sentinel before the one-sided case was fixed,
and the pair is what pins that both shapes reach the same answer — the
environment-unaware base is `{"status":"unknown"}` and the exact per-branch
summaries survive as `variants`. Keeping both in the corpus is what makes a
future "simplification" of `mergeSummaries` visible: collapsing either case
back to `left.returns ?? right.returns` breaks exactly one of the two.

The `unknown-sentinel` review item carries the divergence under
`because.divergences`, with the shape spelled out — here "the branches prove
different values" rather than the one-sided fixture's "one branch proves it and
another proves none".

`Steady` is the negative control in both fixtures.
