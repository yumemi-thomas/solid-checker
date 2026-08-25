# Nested children-attribute coverage — Solid 2

This fixture pins nested `children={...}` lowering at
`dom-expressions#next`
`c7e83a1bb0fc8e8f7fad37a7523db9fcce568820`.

A dynamic `children` value on an otherwise childless nested native element is
elided by Ryan's current `next` static-template fast path, and the semantic
trace reports that output truthfully. This is intentionally not forced to
Babel output parity. A confidently folded value stays an ordinary attribute.
When source children shadow the attribute, including a JSX-valued attribute,
the discarded subtree reconciles without an exit-2 refusal; TypeScript owns
the duplicate `children` diagnostic.

The former `<noscript children={...}>` divergence arm is intentionally gone:
the current producer no longer exposes that as a compiler disagreement. The
fixture's remaining findings are live controls for prop destructuring,
leaf-owner cleanup, and missing-effect API shape. Their identical deleted
counterparts stay silent through the common discarded-region projection gate.
