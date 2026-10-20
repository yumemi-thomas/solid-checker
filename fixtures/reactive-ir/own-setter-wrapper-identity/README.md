# Own-setter write through a TypeScript-wrapped setter

A wrapped callee (`setStore!`, `as typeof setStore`, `satisfies`, `<typeof setStore>setStore`, nestings) is the setter since transparent wrappers preserve the value, so a write through the store's own proxy in its own setter's draft callback is silent while a write in another store's setter is SC2003; an own-first composite callee (`setStore.bind(null)`) keeps the leading-token exemption. `createStore` copied from published `@solidjs/signals@2.0.0-rc.13`; strict `tsc` against installed rc.13.
