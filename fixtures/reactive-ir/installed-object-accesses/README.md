# Each access of an installed object (ADR 0643)

With a store prototype, each access this file writes of the object (literal,
binding, a call handed it, placed at the call) is `installed-member`; a handed
`__proto__` key marks every non-literal-key write. Plain: none.
