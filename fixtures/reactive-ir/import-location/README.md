# import-location

SC7005, `primitive-imported-from-wrong-module`, on Solid 1.x — the dialect
with four subpaths, and so the only one where "which module exports this?" has
an interesting answer.

The snapshot keeps message text (`KEEPS_WORDING` in `scripts/coverage.mjs`).
The wording is the behaviour under test: the message names the module the
checker believes the name comes from, and a rule that reported the right line
with the wrong module would otherwise pass.

What each case is for is written beside it in `App.tsx`. The two that would
silently pass under a weaker rule are the ones that report **nothing**:
`Show` from `solid-js/web`, which both modules export, and `createStore` from
`@my/ui`, which is not Solid's.
