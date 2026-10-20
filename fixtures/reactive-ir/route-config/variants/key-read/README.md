# The route-definitions premise is withheld: key-read (ADR 0335)

`Read.tsx` reads `route.component` off `useRouteMatches()`, which hands out route records. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-config`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
