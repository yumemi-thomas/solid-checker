# An export name present in only one conditional branch

`observe` exists in the `browser` branch and not in the `node` one. The
temporary-v2 model keeps those selections as separate exact artifact cases, so
the node case omits `observe` without turning that local absence into a global
negative. A node consumer therefore cannot inherit the browser branch's
callback semantics.

`shared` is the negative control: it exists in both branches with the same
semantics. The browser and node cases are retained in `expected.json`; the
empty and contradictory condition selections stay local in
`expected-refusals.json` rather than refusing the whole package.
