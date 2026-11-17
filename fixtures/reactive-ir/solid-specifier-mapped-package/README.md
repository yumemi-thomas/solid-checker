# solid-specifier-mapped-package

**Claim (ADR 1060, positive twin of `solid-specifier-mapped`).** `paths` maps `solid-js` to `./node_modules/solid-js`: the
resolver lands inside a `node_modules` tree whose owning manifest names `solid-js`, so the specifier is the package and each import
names its primitive: SC4001, SC7002 ×3, SC7001 are violations (the same claims Chrome rc.21 confirms in
`unresolved-namespace-slots-resolved`). The stub is the published rc.21 declarations reduced to the forms used.
