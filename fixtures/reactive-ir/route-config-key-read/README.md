# The route-definitions premise is withheld: key-read (ADR 0335)

`Read.tsx` reads `route.component` off `useRouteMatches()`: a value read back can be called by the project, which the census does not see. The premise is not applied, so every route component in `Routes.tsx` (the sources and packages of `route-config`) stays undecided and its body read is uncertifiable, not a violation.
