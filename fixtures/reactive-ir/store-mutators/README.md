# SC2003: an in-place array method on a store path

`store.items.push(x)` and the eight other `Array.prototype` mutators written through a `createStore` read proxy are dropped outside the setter (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:5691-5700`, the `set` trap returns without storing; `STORE_WRITE_OUTSIDE_SETTER`). The rule reports a call whose callee resolves, through Type Facts, to the standard library's `Array` method on a receiver rooted at the store's bare identifier.

Silent: a plain array, a copy (`[...store.items]`, `store.items.slice()`), a project class method named `push`, a typed array's `fill` (`Uint8Array.fill` is not `Array.fill`), a mutator in the store's own setter callback (commits, rc.17 probe), on the setter's draft, and through a shadowing parameter. `delete store.path` is not reported either: tsc says TS2790 for a required property and no Type Fact says whether a property is optional (docs/open-work.md).
