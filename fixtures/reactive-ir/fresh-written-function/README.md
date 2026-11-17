# fresh-written-function

ADR 1041 positive: `Number` is handed on, and every member write under a key no fact names writes a function object
this file makes, in the shapes of `@solidjs/router` next.41's dist (`data/action.js:406-435`, `data/query.js:368-395`,
`routers/factory.jsx:301`): a `const` bound to a function expression, a `const` bound to an arrow, a function declaration
nothing rebinds (exported: an importer cannot rebind it), and `Object.assign` onto a nested function declaration. Each
evaluation of a function makes a new object, so none of them is a standard root, and `Number.isFinite` is certified (it
was `standard-member-impostor` before).

Variants, each keeping the row:
- `-parameter`: the written object is a parameter (`fn[k] = v`, `Object.assign(fn, src)`): the caller's object, which may
  be a standard root.
- `-reassigned`: a `let` bound to a function expression, then assigned a parameter before the write.
- `-shadowed`: a function declaration whose name a `let` in an inner scope shadows, and the write goes through the `let`.
