# action-in-owned-scope-global-writes-any

**Claim (ADR 0973).** A package that writes a global under a name the syntax does not fix (`Object.assign(globalThis, source)`, `window[key] = v`, `Object.defineProperty(globalThis, key, d)`) may have written any global, so while its closure may reach a Solid action every call of a global the standard library declares is a site (`AnyGlobal`: `btoa`, and `atob`, which `source` does not hold). A computed key matches every name, as for getters.

`node_modules/fz-wany` is **not** published bytes. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-unread-packages`.
