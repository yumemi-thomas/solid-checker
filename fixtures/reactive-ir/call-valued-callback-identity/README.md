A call-valued callback names the value a call returns, not the callee. The
checker resolves a callback's identity from an entity at the exact span of the
argument, and a call's entity answers with the callee's symbol. So
`onClick={makeHandler(count)}`, `createEffect(count, makeApply(count))`, and an
IIFE or `untrack` handler must not make the factory or the IIFE body the
callback. Each case here is owned or unowned in a way the checker cannot name
exactly. Every case is silent for SC4001. The one finding is an SC7001
uncertifiable on H2's call-valued effect argument, an existing rule's claim.
