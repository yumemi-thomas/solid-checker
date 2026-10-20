# The route-definitions premise is withheld: patched (ADR 0335)

`package.json` records a `patchedDependencies` entry for the router. The premise is not applied, so every route component in `Routes.tsx` (the sources and packages of `route-config`) stays undecided and its body read is uncertifiable, not a violation.
