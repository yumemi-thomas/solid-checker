# impostor-census-key-proved

**Claim (ADR 0903, casts stage 3).** A computed key the census proves names no census member, `prototype`, `__proto__` or host key puts no function at a member: a numeric literal or `const`, a numeric operator, `+` of proved numbers, a module `let` counter whose every write is a number, `Math.floor` (trusted while unpatched), a template or concatenation whose literal head or tail no listed name has. So does a copy of a closed literal's own members (`Object.keys` / `Object.entries`), a `new Array` or an inert object literal under a key no fact names, and a `new Map` under a named key. No site is `*`, so `forEach` and `then` on the signals' values stay standard: no `standard-member-impostor` row.

The twin `impostor-census-key-open` adds one classic-script counter, which keeps `*`.
