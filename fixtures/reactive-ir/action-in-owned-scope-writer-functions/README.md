# action-in-owned-scope-writer-functions

**Claim (ADR 0974, the reviews of #231).** A function of the writer family (`Object.assign`, `Object.defineProperty`, `Object.defineProperties`, `Object.setPrototypeOf`, `Reflect.set`, `Reflect.defineProperty`, `Reflect.setPrototypeOf`, `Object.prototype.__defineGetter__`) is followed to its calls wherever the file uses it other than by naming the global object: `Reflect.apply(Object.assign, null, […])`, `.call`, `.apply` and `.bind` of it, a name it is bound to (`const assign = Object.assign`, `const { defineProperty } = Object`, `Object.assign || fallback`). Each call is judged like a direct call: a target that is a fresh object (an object or array literal, `new`, a function, `exports`, a `const` bound to one, `Object.create(…)`) writes no global, any other target may be the global object, so the package writes any global (and, for the writers that define accessors, defines an accessor under any name). A writer that escapes (exported, passed on, stored where the walk does not follow it, read as a value) may be called with the global object anywhere: the same.

- **Uncertifiable** (`package-may-call-action`): `CallsFetch`, `CallsTimer` (a call of a global the library declares) and `ReadsGlobal` (a read of one), because `fz-wreflect`, `fz-walias` and `fz-wbound` are packages that may reach a Solid action and may write any global. Each package alone would do: the first writer by name answers.
- The twin where every target is fresh is `action-in-owned-scope-writer-functions-fresh`: silent.

Chrome (rc.21): `pkgaction/global-write-reflect-apply` throws `ACTION_CALLED_IN_OWNED_SCOPE` at `btoa("x")`.

Not followed (`docs/open-work.md`): `Object` or `Reflect` themselves bound to a name (`const O = Object; O.assign(…)`) or reached through a member (`globalThis.Object.assign`).

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-unread-packages`; `.solid-checker/runtime.json` closes the program boundary.
