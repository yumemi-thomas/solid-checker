# prototype-write-object-host

**Claim (ADR 0314), refusal.** `prototype-write-narrowed` with one more call:
`OriginHelper.store(Object, "toString")`. `Object` is declared in
`lib.es5.d.ts`, not `lib.dom.d.ts`, and `Object.prototype` is in the chain of
every class instance and of every built-in value, so the write vetoes every
exact-member proof: every case, `ExactClass` and `BuiltinInstances`
included, keeps its `SC9012` uncertifiable finding.
