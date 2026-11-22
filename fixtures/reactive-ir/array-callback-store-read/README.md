# array-callback-store-read

**Claim (ADR 1181).** A Get `x.v` through the parameter of an inline callback (first parameter a plain identifier, no default,
never written; not `async`, not a generator) that is the whole first argument of a default-library `Array.prototype` `map`,
`forEach`, `filter`, `some`, `every` or `find` call (Type Facts; no impostor or patch veto, ADR 0840) on an array literal of exactly
one element naming a store is that store's Get: the read row `s.v` written there gets. Violations: `MapInBody`, `ForEachInBody`,
`FindInBody` (the body window). Clean: `MapInMemo` (the memo tracks). As before (no row, or the census's obligation): two
elements, a written parameter, a non-literal receiver. Chrome: `runtime-oracle/array-callback-store-read`. Stub:
`named-callback-reporting-run`'s.
