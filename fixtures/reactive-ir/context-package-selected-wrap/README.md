# context-package-selected-wrap

The selected twin of `dependency-source-integrity-r2-visible-js` (ADR 0949, ADR 0290).

There, `--dependency-source dep-router` selects only the router; `dep-wrap` is visible to the program (`allowJs`, `maxNodeModuleJsDepth` 1) but not selected, so it is analyzed as the program's own code and its SC1001 and SC4002 rows stand in `node_modules/dep-wrap/index.jsx`: its call to the router's `useLocation()` is a project call (`dependency_source_root` is `None`), reported where it is written, like the pinned SC1001 at its `props.children`. Not a duplicate and not misplaced: the file is analyzed as project code, and its row is at the call that runs the router's read.

Here `.solid-checker/runtime.json` selects both packages. `dep-wrap` is package source then, its findings are not reported (ADR 0284), and the requirement that its `StaticWrap` reaches the router's read through `useLocation()` has to stand at the project site that runs `StaticWrap`: the `<StaticWrap>` element in `App.tsx`. Before ADR 0949 that element got no row (a render is no call), so `StaticWrap`'s read was silent and the program was certified clean for a `<StaticWrap>` rendered with no `<Router>` above it (probed: Chrome throws ContextNotFoundError).

Expected rows: SC4002 at `App.tsx`'s `useLocation()` (the `Read` component's own call), SC9012 `dependency-source-getter` (unchanged), and SC4002 at `<StaticWrap>` (new). None in `node_modules`.
