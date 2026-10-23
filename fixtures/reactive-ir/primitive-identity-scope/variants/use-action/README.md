# primitive-identity-scope / use-action

Variant of `primitive-identity-scope` (shares its `node_modules`). The fake package's `useAction(action)` invokes its parameter with `action.apply(null, args)`, the shape of `@solidjs/router@2.0.0-next.41`'s `dist/data/action.js:232-236`. The parameter is not Solid's `action` (ADR 0365), so it is no flush site and the `createMemo` in the `createEffect` apply stays an SC4005 violation. Before ADR 0365 the parameter was named Solid's `action` by its path, and the ADR 0362 veto made it uncertifiable.
