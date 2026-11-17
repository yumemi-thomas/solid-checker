# projection-draft-assign

ADR 1053 positive: the program's only open write is `Object.assign(draft, …)` on the first parameter of a callback passed to
`createProjection` (@tanstack/solid-query 6.0.0-rc.5 `build/dev.js`, `metaProjection`). rc.21 calls that callback with a
draft wrapper whose traps write the projection's store, or a clone of its value, so no standard root is written and
`Number.isFinite` handed on is certified (it was `standard-member-impostor`, `OpenMemberWrite(*)`).
