# fresh-target-definer

ADR 1036 positive: `Number` is handed on, the program's only definer is `Object.defineProperties({}, d)` (solid-query's
`createInfiniteQuery`, probus-hk's getters) and its only writes under a computed key use a key that is a number by its
syntax (a loop counter, `length++`, `items.length - 1`). Neither defines nor writes a member of a root, so `Number.isFinite`
is certified (it was `standard-member-impostor`: `ReflectiveDefinition(*)`, `OpenMemberWrite(*)`).
