# leaf-scope-skippable-await

ADR 0357: a skippable await leaves both leaf and unowned continuations open
(SC3001 and SC4001 obligations); a definite await is SC4001's proof.
MaybeOwner pins namespace, ordinary/callback forwarding, dormant/shadowed
negatives and a fresh root. `untrack` copies signals rc.13 core/core.d.ts:136;
other signatures are the existing fixture's published stubs.
