# component-identity-renders

ADR 0331 (amended): beside a tag, a reference withholds component identity only if a plain call can come of it. Two holders allow only renders and are read through: a closed object literal (unexported `const`, static unduplicated data keys, no spread, getter, method or `__proto__`) whose every reference is a member read `Dynamic` renders (`<Dynamic component={icons[k]}/>`, rc.17 `Dynamic` is `createComponent(dynamic(() => props.component), …)`), and an unexported `const` alias whose every reference is a plain tag (`<Badge/>` is `createComponent(Badge, {})`). Runtime-probed: `runtime-oracle/component-identity-dynamic-map-tagged` and `component-identity-alias-tag` warn `STRICT_READ_UNTRACKED` and are `covered-violation`.

- `Icons.tsx` (violations): `Archive` and `Trash`, each tagged and stored in `icons` (a key and a shorthand, `as const`), read by computed and static members in `Dynamic`.
- `Alias.tsx`: `Aliased` with `const Badge = Aliased` used only as `<Badge></Badge>` (violation); `AliasCalled`, whose alias is also called (`Called()`), and `AliasExported`, exported (uncertifiable).
- `Escapes.tsx` (uncertifiable): a map also read as `called.run()`, an exported map, a map spread into another object, a getter-held value, a stored value handed to a project callee.
- `DynamicOnly.tsx`: `Panel`, rendered only by `Dynamic`, is a component with no strict-read window (`dynamicCore`'s unlabeled `untrack`), so its read, and `Helped`'s read through a helper, are uncertifiable (`runtime-oracle/component-identity-dynamic-only`: quiet). `Both`, also tagged, is a violation, and so is a read in `Listed`'s `<For>` row, which `mapArray` gives its own window (`component-identity-dynamic-for-row`). The map in `Icons.tsx` keeps its violations through the tags (`component-identity-dynamic-map-tagged`).
- `NoTag.tsx` (uncertifiable): a function no tag renders keeps ADR 0331's census; a map only `Dynamic` reads is still an escape for it.
- `variants/prototype` (uncertifiable): `Object.defineProperty(Object.prototype, …)` installs an accessor a member read can fall through to, with the map as `this` (ADR 0292's veto). Replacing the target by `globalThis` makes it a violation.
- Route options beside a tag: `route-components` and `route-config` (`Tagged`, violations).

Stubs: `@solidjs/web@2.0.0-rc.17` `Dynamic` and its prop types (`types/index.d.ts:1, 46-48, 50-54, 177`, published lines); `solid-js` and `@solidjs/signals` manifests pin rc.17.
