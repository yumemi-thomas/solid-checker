# Nested children-attribute coverage — Solid 2

This fixture pins nested `children={...}` lowering at
`dom-expressions#next`
`ead46d12da34db2ae366e1c02183a87f7479f05c`.

A dynamic `children` value on an otherwise childless nested native element is
promoted and censused as an ordinary tracked JSX child. A confidently folded
value stays an ordinary attribute. When source children shadow the attribute,
the discarded attribute read remains silent; TypeScript owns the duplicate
`children` diagnostic.

The former `<noscript children={...}>` divergence arm is intentionally gone:
the current producer no longer exposes that as a compiler disagreement. The
fixture's remaining findings are unrelated controls for prop destructuring and
leaf-owner cleanup.
