# The route-components premise is withheld: patched (ADR 0334)

`package.json` records a `patchedDependencies` entry for the router. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-components`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
