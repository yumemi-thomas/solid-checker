# action-in-owned-scope-prototype-iteration

**Claim (ADR 1114 family 3, ADR 1108 part 2).** `fz-piter` writes `Float32Array.prototype[Symbol.iterator]` (and `Array.prototype[Symbol.iterator]`), a key the scan does not decide, so it may write any member of a built-in's prototype (`prototype_any_name`). An iteration runs the iterator's `Symbol.iterator` and `next`: each iterated operand (a spread, `for…of`, an array pattern, `yield*`) and each collection constructor given a value (`new Set(it)`, `new Map(it)`) is an uncertifiable site (`prototype-write-any`). Never a violation.

- **Uncertifiable**: `Spread`, `ForOf`, `ArrayPattern`, `SetConstructor`, `MapConstructor`.
- **Uncertifiable, unchanged (part 1)**: `LibraryCall` (`new Float32Array(1).slice()`, a method the any-name writer may have written).
- **Silent**: `SpreadInHandler` (no owner is active). A program that loads an any-name writer has no owned iteration that is provably safe.

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-prototype-iteration`): on main (e637f50f2) 5 unproven-clean sites; here every site covered-uncertifiable.

`node_modules/fz-piter` is **not** published bytes. The Solid stubs and the closed boundary are those of `action-in-owned-scope-prototype-writes`.

**ADR 1114 family 3 slice E.** Where a loaded package writes a built-in's prototype and may reach a Solid action, every operation that may run a written member is uncertifiable (`package-may-call-action:<reach>:prototype-<kind>`): a member call whatever its declaration, a call, constructor or tag the library declares or nothing resolves, a converting operator, a template substitution, an array pattern, `await`, `using` and a JSX value. A twin above called silent that makes such an operation in an owned scope under such a writer is uncertifiable now; a twin in a handler stays silent.
