# namespace-then-escape

**Claim (ADR 1071, twin of `namespace-then-named`).** The `.then` callback hands the namespace on (`keep(m)`): every export of the barrel may
be called by code that receives it, `createLoader` included, so `useIsMounted`'s callers are not enumerable: SC4001
`owner-callers-unenumerated` at `holder.ts:6`, as before ADR 1071.
