# Dependency source: a package-returned accessor (ADR 0284)

Claim: an accessor a selected dependency source returns is a proven reactive source when the package's runtime source creates it with `createSignal`, so its read in a component body is an `SC1001` violation; a function of the same declared type whose source reads nothing stays clean (ADR 0294: the type alone proves nothing).

This recreates the `Bad` case of the deleted `package-return-consumer` fixture (removed with package contracts, ADR 0312), which declared `reactive-package` by `.d.ts` alone and came out clean once contracts were gone. With the runtime source visible it is reported.

- `Bad` (positive): `count()` in the body, `count` created at module level by `createCount()`. `SC1001` violation, evidence located at `createSignal`'s getter in `node_modules/reactive-package/index.js`.
- `BadAlias`, `BadNamespace`, `BadLocal` (positive): the same export through an import alias, a namespace member, and a call in the component body. `SC1001` each.
- `Good` (negative): the read is in a tracked JSX expression.
- `Constant` (negative): `createConstant()` has the same declared `Accessor<number>`, but its source returns `() => 0`. Clean.

Each positive site also carries an `SC9012` `type-only-source` uncertifiable at the callee span: the typed-candidate obligation from the declared `Accessor<number>` is not withdrawn when the source proves the read. The same pairing exists in other fixtures (for example `accessor-value-origin`, `literal-argument-member-dispatch`); uncertifiable results are internal (ADR 0328).

`reactive-package` is synthetic, not a published package: `index.js`, `index.d.ts` and `package.json` are written for this fixture in published layout (`exports` with `types` and `default`, a `solid-js` peer). `solid-js.d.ts` is `dependency-source`'s stub, whose claim-bearing signatures are copied from published 2.0.0-rc.13. `.solid-checker/runtime.json` declares a browser development host and selects `reactive-package` as a dependency source. `App.tsx` passes `tsc --noEmit` (TypeScript 5.9.3), so no finding duplicates a type error.
