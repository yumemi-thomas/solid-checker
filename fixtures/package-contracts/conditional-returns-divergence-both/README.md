# Two conditional branches that prove different returns

The control for
[`conditional-returns-divergence`](../conditional-returns-divergence/README.md).
Here *both* branches prove a reactive return and they disagree about which
property carries it: `{ view }` in the `browser` branch, `{ other }` in the
`node` branch.

Stable-v1 generation keeps each exact artifact case independent. There is no
environment-unaware base summary: unresolved selection joins possible recursive
shapes without turning either into a guarantee. The condition census treats
`browser` and `node` as mutually exclusive runtime-target values rather than
inventing a combined selection. The default and browser cases remain in
`expected.json`; the independently conflicting node merge stays pinned in
`expected-refusals.json`.

`Steady` is the equivalent-return control in both fixtures.
