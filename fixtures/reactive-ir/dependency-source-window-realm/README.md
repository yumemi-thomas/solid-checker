# dependency-source-window-realm

**Claim (ADR 0295 under ADR 0284 and 0287).** Under a declared Window realm (`.solid-checker/runtime.json` `"realm": "window"`, the CLI's `--runtime-realm window`, with `--runtime-target browser` and `dep-query` as dependency source), a selected package's strict comparison of `typeof window` or `typeof document` with `"undefined"` is decided when the identifier is the unbound global, and a module-level `var` holding exactly that comparison carries it when every read is proved to run after the initializer. The decided `if` drops the forwarding proxy trap's server branch, so the trap returns the store's member and the project's read is SC1001.

- `WindowQuery` (violation): `var isServer2 = typeof window === "undefined"` read inside the trap, the guard `@tanstack/solid-query` 6.0.0-rc.0 publishes. The statements before it in `src/index.js` are the kinds `build/dev.js` runs before its own `var isServer2`: imports and `export *`, function declarations, `createContext(null)` and `createContext(() => false)` (an imported callee with closed arguments), another Window-realm comparison, and arrow functions. None can call into the module, and no module imports `index.js` back.
- `ClientQuery` (violation): `var isClient = typeof window !== "undefined"` and `if (!isClient)`.
- `DocumentQuery` (violation): `if ("undefined" === typeof document)` directly, operands reversed.
- `WrittenQuery` (silent): the alias is written by an exported `forceServer()`.
- `ShadowedQuery` (silent): `window` is the package function's parameter; the project passes none, so at run time the server branch is the one taken.
- `GlobalThisQuery` (silent): `typeof globalThis.window` reads a member of a replaceable global.
- `EarlyQuery` (silent): `src/early.js` calls its own hoisted function at module level before `var isClient`, whose trap then reads `undefined` and takes the server branch.
- `CyclicQuery` (silent): `src/cyclic.js` imports `cycle-back.js`, which imports it back and runs first; the alias is refused.

Each silent case reports SC1001 once its one defect is undone (checked by hand on a copy). `dependency-source-window-undeclared` is this project without the realm, byte-identical except `runtime.json` (coverage's `IDENTICAL_SOURCES`): nothing is reported there.

`solid-js.d.ts`, `tsconfig.json` and the `@solidjs/signals` and `@solidjs/web` stubs are copied from `dependency-source-proxy-store`; `createContext` and `useContext` are added, byte-faithful to the published 2.0.0-rc.13 `types/client/core.d.ts`, over the reduced `Context` of `dependency-source-context-getter`. The `solid-js` stub adds `"module": "./dist/solid.js"`, an empty module: the cycle proof follows every static load of a dependency source, and the published `dist/solid.js` and `dist/solid.dev.js` load only `@solidjs/signals`. TypeScript does not read `module`, and nothing analyzes that file. `App.tsx` passes `tsc --noEmit` (5.9.3); the package's JavaScript is not type-checked (`checkJs: false`).
