# context-package-lazy

**Claim (ADR 0949).** A package loaded only through a literal `import()` (or `require()`) has no import binding to attribute through, and source analysis does not redirect a dynamic load: the package stays off source (ADR 0300), so nothing analyzed says what `lazy(() => import("pkg"))` renders. The load is an SC9005 obligation (`package-source-unavailable`, uncertifiable) at the `import(…)`, named for the exports its consumer reads (`lazy` reads `default`; a destructuring names its members; an escaped result is `*`). Before ADR 0949 `discover_boundary_defects` read only import declarations, so the program was certified clean while the loaded component throws ContextNotFoundError.

`App.tsx`: `FromPackage` lazy-loads `dep-ui-context` (obligation at the `import()`); `FromProject` lazy-loads `./Local`, a project module (silent: it is analyzed). The two `lazy` elements also carry SC5003 `async-provenance-unproven` (a lazy result's pending state), as before.

Package: `node_modules/dep-ui-context` as in `context-package-component`, selected by `.solid-checker/runtime.json`: the selector notes that it stays off source.

Chrome (rc.21 development build, the runtime oracle): the lazy package component throws ContextNotFoundError once loaded.
