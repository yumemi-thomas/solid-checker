# action-in-owned-scope-global-writes-any

**Decision (ADR 0973; the coordinator's ruling after the first measurement).** A package that writes a global under a key nothing in its file decides (`Object.assign(globalThis, loadPatches())`, `window[key] = v` with a key from a call or a parameter, `Object.defineProperty(globalThis, key, d)`) may have written any global, and the build follows it: every call of a global the library declares, in a program that loads the package, is a site (`package-may-call-action:reaches-solid`, "may write … under a key the scan does not decide"). "Unproven means uncertifiable": leaving the writer out was the quiet side, not the conservative one.

It was measured first. A key the writer's own constants decide is a name (`action-in-owned-scope-global-writes`: a `const`, a loop over a literal array, a symbol that names none), so only a key nothing decides reaches every global. Before that resolution, `storybook-solidjs-vite` and storybook's `var KEY = "…"` made a row of every `Math.max`, `JSON.parse` and `new Map` of a twelve-file project (kui `feature-alerts`, 318 rows); after it only vitest's `stubGlobal(name, value)` and `for (const key in config.defines) globalThis[key] = …` remain, and those are honestly unknown.

All five calls of `AnyGlobal` are uncertifiable. The writer's own verdict stands (`reaches-solid`: a call into `fz-wany` is a site).

`node_modules/fz-wany` is **not** published bytes. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-unread-packages`.
