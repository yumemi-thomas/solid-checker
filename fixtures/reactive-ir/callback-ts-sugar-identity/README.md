A TypeScript wrapper keeps a callback's identity. `h as X`, `h!` and
`h satisfies X` evaluate to the function `h`, so the callback is `h` (ADR 0346):
the checker reads the identity of the identifier the wrapper holds, through the
wrapper. The positive forms are the sugared callbacks of `createReaction`, an
`onClick` handler and a `ref`, each creating an effect or a cleanup with no
owner, and an identity wrapper under `as` (`wrapReaction(…) as any`). Each is an
SC4001 violation, as the bare identifier is. The negatives stay silent: a
wrapper around a call (`makeHandler(0) as any`) names the call's value, which
is not known, and a wrapper around a member (`handlers.onTap as any`) names the
member's value, which the bare member never names either.
