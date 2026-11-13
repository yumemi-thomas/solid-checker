# action-in-owned-scope-lazy

**Claim (ADR 0973, the review of #231, B5).** The component `lazy(() => import("pkg"))` makes is the code of the module its loader imports: a package's, which the build does not read, and which runs its body under an owner where the project renders it. The tag's alias ended at the dialect's `lazy`, whose runtime is the catalog's, so the render had no site. `lazy-render` in the pattern lane (`patterns/pkgaction/lazy-render`) is Chrome-confirmed: the module is preloaded and rendered later, and `ACTION_CALLED_IN_OWNED_SCOPE` is thrown at the element; its twin `lazy-render-clear` throws nothing.

- **Uncertifiable** (`package-may-call-action`): `Action` (`import("fz-lazy")`, its closure loads solid-js: `reaches-solid`), `Named` (`import("fz-lazy-named").then((m) => ({ default: m.View }))`: the loader's own code is the project's, the module the package's) `Vendor` (`import("./view.vendor.js")`, JavaScript behind an ambient shim that the program does not analyze: `unanalyzed-project-js`), and `Computed` (`import(moduleName)`: a module named by anything but a literal may be any package's, `dynamic-load`).
- **Silent**: `Clear` (the package is clear), `Local` (`import("./Local")` is the project's module: analyzed) and `Preloaded` (`LazyAction.preload()` is the wrapper's own member, not the package's component).

Not followed (`docs/open-work.md`): a loader that is not a function literal (`lazy(loader)`), and one that gets its module another way (`pkg.load()`).

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-unread-packages`; `.solid-checker/runtime.json` closes the program boundary.
