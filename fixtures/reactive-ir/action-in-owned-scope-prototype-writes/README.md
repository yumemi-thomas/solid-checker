# action-in-owned-scope-prototype-writes

**Claim (ADR 1108, the named miss of ADR 0974 "a builtin prototype a package extends").** A resolved standard-library signature proves the type of a method, not that no installed package replaced it on the built-in's prototype where it loads. `module_reach.rs` records the members each runtime file writes on a built-in's prototype (`GlobalWrites::prototype_names`, `prototype_any_name`); a call of a method the standard library declares, where a package the program loads writes a member of that name on a built-in's prototype and its closure may reach a Solid action, is an uncertifiable site (`package-may-call-action:reaches-solid`, provider `prototype-write:<name>:<root>`). Never a violation.

- **Uncertifiable**: `PatchedByAssignment` (`new Float32Array(1).reverse()`, `fz-proto`: `Float32Array.prototype.reverse = …`), `PatchedThroughAlias` (`new Uint8Array(2).fill(1)`, `fz-protoalias`: `const UP = Uint8Array.prototype; UP.fill = …`), `PatchedByDefine` (`new Int16Array(2).join("-")`, `fz-protodefine`: `Object.defineProperty(Int16Array.prototype, "join", …)`).
- **Uncertifiable, unchanged (ADR 0974)**: `GlobalWrittenByName` (`atob`, which `fz-proto` writes by name on the global object).
- **Silent**: `PatchedByClearPackage` (`fz-protoclear` writes `Array.prototype.sort` and names no Solid package: it cannot call an action), `Unpatched` (`map`, `trim`, `slice`: no package writes them) and `PatchedInHandler` (the replaced `reverse` called in a click handler, where no owner is active).

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-prototype-writes`, and the pattern lane's `pkgaction/patch-f32-undeclared`): `ACTION_CALLED_IN_OWNED_SCOPE` at each uncertifiable call; the silent components are quiet.

Not followed (`docs/open-work.md`): a prototype reached through a value (`Object.getPrototypeOf(x)`, `x.__proto__`, `x.constructor.prototype`) handed on (a member written through it is recorded), a built-in bound to a name (`const A = Array; A.prototype.at = …`), an accessor a package defines on a prototype read for its value, and an implicit call (an iterator, `toString`).

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-global-writes`; `.solid-checker/runtime.json` closes the program boundary.
