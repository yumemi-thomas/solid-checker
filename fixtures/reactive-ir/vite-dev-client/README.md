# vite-dev-client

**Claim (ADR 1144).** The package named `vite` is a page's host, not a writer of its built-ins' prototypes, where no program file loads it at run time and the dev client its server puts in the page is a release whose bytes were read. The walk reads Vite's own closure from its Node entry (`dist/node`: code it generates, packages it loads) and finds a writer of any name; that code runs in the dev server's process. What runs in the page is `dist/client/client.mjs` and `env.mjs`, which write no prototype: `env.mjs` writes the config's `define` keys on the global object (here none), and Vite 8's console forwarding replaces `console.error` and `console.warn`. So a site whose only prototype writer is that root ends in the typed silence `package_action_calls.package_action_calls.reviewed-host`, named in the proof log. A global the client writes stays a site: the package keeps its global writes, and the premise requires that they cover the client's (`console`, every define).

- **Cleared (a typed silence, no row)**: the SC2002 sites of `CalledDirectly`, `CalledOnAny`, `TemplateSubstitution` and `Mapped` in `App.tsx`. Dropping the silence (`SOLID_CHECKER_IR_ABLATE_SILENCE=package_action_calls.package_action_calls.reviewed-host`) gives back the rows the package's own writes gave them.
- **Kept (an uncertifiable SC2002 row)**: `Logged`'s `console.error(…)`: the package writes globals under any name, and the client writes `console`'s members.

No `index.html`: the product realm is unknown, and the premise holds all the same (it is the root's, not the realm's).

Variants (`variants/<name>`, each a project of its own on this fixture's `node_modules`):
- `type-import` (cleared): a `import type … from "vite"` loads nothing.
- `solid-plugin` (cleared): the config's one plugin is the Solid plugin, the stand-in of `fixtures/reactive-ir/node_modules/@solidjs/vite-plugin` (`0.0.0-fixture`, which adds no `define`), whose package content is a row of `SOLID_PLUGIN_READS`.
- `solid-plugin-patched` (kept): its own copy of that stand-in, one line longer: no reading matches the package's content.
- `vite-7` (cleared): its own `node_modules/vite` with Vite 7.3.1's published manifest and client (no console forwarding).
- `runtime-import` (kept): `setup.ts` loads `vite` at run time.
- `plugin` (kept): a plugin that is not Solid's; its `define` keys are not read.
- `define` (kept): a config `define`; the closed grammar does not read it (a prototype path, here).
- `production` (kept): a production build; the dev client is not what Vite puts in that page.
- `mismatch` (kept): its own `node_modules/vite` whose `env.mjs` differs by one line from the published bytes.

`node_modules/vite/package.json` and `dist/client/{client.mjs,env.mjs,bundledDevClient.mjs}` are the published bytes of `vite@8.3.0` (integrity `sha512-lhZBVvEHefgE+HQZC9O7EBJgCU/nVzFNl7vkS4RE0APtWLP02/8QVIkQtzBxPquh7lq5/78NHipTj7ODQ6XuyQ==`); `variants/vite-7/node_modules/vite/package.json` and `dist/client/{client.mjs,env.mjs}` those of `vite@7.3.1` (`sha512-w+N7Hifpc3gRjZ63vYBXA56dvvRlNWRczTdmCBBa+CotUzAPf5b7YMdMR/8CQoeYE5LX3W4wj6RYTgonm1b9DA==`). Everything else under `node_modules/vite` (`dist/node`, `client.d.ts`, `misc`) is **not** published bytes: a stand-in written for this fixture, whose Node entry writes a prototype and a global under keys nothing decides and reaches a Solid action. `node_modules/solid-js` and `@solidjs/*` are the manifests of `product-realm-dev-client`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
