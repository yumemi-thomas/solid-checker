# impostor-census-absent

**Claim (ADR 0840), the negative.** None of these sites puts a function at a census member: a data literal (`{ then: 1 }`), a computed key a `const` names (`"other"`), other names (`thenable`, `forEachOf`), an interface member, and class fields with no function in them. `forEach` and `then` on values typed as an array and a promise keep their proofs (the write in the `forEach` callback stays a violation).
