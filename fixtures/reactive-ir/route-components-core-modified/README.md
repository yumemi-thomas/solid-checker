# The route-components premise is withheld: core-modified (ADR 0334)

The router is the published one, but a byte of router-core's `dist/esm/route.js` differs, and the premise reads router-core too. The premise is not applied, so every route component in `Routes.tsx` (the sources and packages of `route-components`) stays undecided and its body read is uncertifiable, not a violation.
