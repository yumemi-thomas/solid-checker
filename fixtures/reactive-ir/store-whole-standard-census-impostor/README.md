# SC9012: a replaced standard member keeps a whole-store form open (ADR 1178)

The twin of `store-whole-standard-census`: the program writes `Object.values` and `Array.prototype[Symbol.iterator]`, so
`Object.values(row)` is withheld (ADR 0840) and the iteration's premise (ADR 1129) fails. The census keeps its
`value-in-carrier` rows at both references.
