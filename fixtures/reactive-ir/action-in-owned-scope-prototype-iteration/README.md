# action-in-owned-scope-prototype-iteration

**Claim (ADR 1114 family 3, ADR 1108 part 2).** `fz-piter` writes `Float32Array.prototype[Symbol.iterator]` (and `Array.prototype[Symbol.iterator]`), a key the scan does not decide, so it may write any member of a built-in's prototype (`prototype_any_name`). An iteration runs the iterator's `Symbol.iterator` and `next`: each collection constructor given a value (`new Set(it)`, `new Map(it)`) is an uncertifiable site (`prototype-write-any`). Never a violation. An iterated operand (a spread, `for…of`, an array pattern, `yield*`) is the held kind `iteration` (slice C, after its writers are cut; `scripts/fuzz/open-defects.json`).

- **Uncertifiable**: `SetConstructor`, `MapConstructor`.
- **Uncertifiable, unchanged (part 1)**: `LibraryCall` (`new Float32Array(1).slice()`, a method the any-name writer may have written).
- **Silent**: `SpreadInHandler` (no owner is active). A program that loads an any-name writer has no owned iteration that is provably safe.

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-prototype-iteration`): on main 2 unproven-clean sites; here both covered-uncertifiable.

`node_modules/fz-piter` is **not** published bytes. The Solid stubs and the closed boundary are those of `action-in-owned-scope-prototype-writes`.
