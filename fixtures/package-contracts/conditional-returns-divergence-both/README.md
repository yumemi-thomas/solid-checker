# Two conditional branches that prove different returns

The control for
[`conditional-returns-divergence`](../conditional-returns-divergence/README.md).
Here *both* branches prove a reactive return and they disagree about which
property carries it: `{ view }` in the `browser` branch, `{ other }` in the
`node` branch.

Temporary-v2 generation keeps each exact artifact case independent. There is
no environment-unaware base summary: unresolved selection joins possible
recursive shapes without turning either into a guarantee. The synthetic export
map also admits the contradictory `browser,node` condition set, so generation
refuses that case rather than inventing precedence; `expected-refusal.txt` pins
the refusal.

`Steady` is the equivalent-return control in both fixtures.
