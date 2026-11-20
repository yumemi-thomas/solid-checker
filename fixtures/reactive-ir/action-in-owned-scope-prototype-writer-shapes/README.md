# action-in-owned-scope-prototype-writer-shapes

**Claim (ADR 1108 part 2, the writer half of the review of part 1).** `module_reach.rs` records a member written on a built-in's prototype reached by each shape below (`GlobalWrites::prototype_names`); the call of a library method of that name is an uncertifiable site (`prototype-write:<name>:<root>`). Each package writes its own method, so each row names one shape. Never a violation.

- **Uncertifiable**: `GetPrototypeOfBuiltin` (`Object.getPrototypeOf(Int8Array).prototype.copyWithin`), `GlobalByConstantKey` (`const K = "Float32Array"; globalThis[K].prototype.lastIndexOf`), `PrototypeByConstantKey` (`const P = "prototype"; Float64Array[P].indexOf`), `DestructuredPrototype` (`const { prototype } = Uint8Array; prototype.includes`), `ConstructorList` (`[Int16Array, Int32Array].forEach((C) => { C.prototype.every = … })`: a parameter's prototype may be a built-in's), `ThroughEventView` (`event.view.Uint32Array.prototype.some`), `BoundBuiltin` (`const F = Uint8ClampedArray; F.prototype.subarray`), `PrototypeOfValue` (`const p = Object.getPrototypeOf(new BigInt64Array(1)); Object.assign(p, { fill })`).
- **Uncertifiable, unchanged (part 1)**: `WrittenDirectly` (`Float32Array.prototype.reverse = …`).
- **Silent**: `OwnPrototype` (`fz-wown` writes `map` on its own function's prototype by a constant key: no built-in's), `WrittenByClearPackage` (`fz-wclear` destructures `Uint16Array`'s prototype and writes `at`, naming no Solid package), `Unwritten`, `CalledInHandler`.

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-prototype-writer-shapes`): on the part 1 build 8 unproven-clean sites; here every site covered-uncertifiable, the silent components quiet.

`node_modules/fz-*` are **not** published bytes. The Solid stubs and the closed boundary are those of `action-in-owned-scope-prototype-writes`.

**ADR 1114 family 3 slice E.** Where a loaded package writes a built-in's prototype and may reach a Solid action, every operation that may run a written member is uncertifiable (`package-may-call-action:<reach>:prototype-<kind>`): a member call whatever its declaration, a call, constructor or tag the library declares or nothing resolves, a converting operator, a template substitution, an array pattern, `await`, `using` and a JSX value. A twin above called silent that makes such an operation in an owned scope under such a writer is uncertifiable now; a twin in a handler stays silent.
