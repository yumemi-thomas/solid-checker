# Nested children-attribute coverage (Solid 2)

Pins nested `children={...}` lowering at `dom-expressions#next` `c7e83a1bb0fc8e8f7fad37a7523db9fcce568820`: a dynamic `children` on an otherwise childless nested native element is elided by the static-template fast path and reported truthfully (not forced to Babel parity); a folded value stays an attribute; source children shadowing the attribute reconcile the discarded subtree without exit-2 refusal (TypeScript owns the duplicate `children` diagnostic). Remaining findings are live controls; deleted counterparts stay silent.
