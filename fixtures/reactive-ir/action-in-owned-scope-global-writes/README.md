# action-in-owned-scope-global-writes

**Claim (ADR 0973, the second re-check of #231).** A resolved standard-library signature proves the TYPE of a global, not that no package replaced it at run time. The scan of each installed package's closure records the globals it writes (`globalThis.x = …`, `window.x = …`, `Object.defineProperty(globalThis, "x", …)`, `Object.assign(globalThis, { x })`, `Reflect.set`, an unbound `x = …`); a call of a global some package writes, where that package's closure may reach a Solid action, is a site whatever type the library declares it with. The premise named in the ADR: types are sound for declarations, and runtime writes come from the scan.

- **Uncertifiable** (`package-may-call-action:reaches-solid`): `WrittenTimer` (`setTimeout`, written by `fz-wtimer` with no type declaration), `WrittenThroughWindow` (`window.fetch`, a member of the global object), `WrittenByDefine` (`queueMicrotask`, `Object.defineProperty`), `WrittenByAssign` (`structuredClone`, `Object.assign` with a literal).
- **Silent**: `WrittenByClearPackage` (`fz-wclear` writes `btoa` and names no Solid package: it cannot call an action) and `Unwritten` (`atob`, `JSON.parse`, `Math.max`: no installed package writes them).

Chrome (rc.21): the pattern lane's `pkgaction/global-write-btoa`, `global-write-window`, `global-write-define` and `global-write-assign` throw `ACTION_CALLED_IN_OWNED_SCOPE` at the call (`btoa` and `atob` stand for the global: the lane's own harness uses timers); `global-write-clear` is quiet.

Not followed (`docs/open-work.md`): the global object aliased or handed to a function that writes it (`const g = window; g.x = …`, `install(window)`), a global written through a prototype or a member of a host object (`console.log = …`, `Array.prototype.map = …`), a script's top-level `var` or function declaration, a sloppy `this`.

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-unread-packages`; `.solid-checker/runtime.json` closes the program boundary.
