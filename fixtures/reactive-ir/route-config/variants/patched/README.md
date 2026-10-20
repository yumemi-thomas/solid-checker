# The route-definitions premise is withheld: patched (ADR 0335)

`package.json` records a `patchedDependencies` entry for the router. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-config`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
