# The route-components premise is withheld: patched (ADR 0334)

`package.json` records a `patchedDependencies` entry for the router. The premise is not applied, so every route component in `Routes.tsx` (the sources and packages of `route-components`) stays undecided and its body read is uncertifiable, not a violation.
