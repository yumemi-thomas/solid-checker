# The route-definitions premise is withheld: dynamic-load (ADR 0335)

`Read.tsx` loads a module whose specifier is not a literal, which can be the router and its `useRouteMatches`. A project that can obtain a route definition can call its `component` itself, so the premise is not applied: every route component in `Routes.tsx` (the sources and packages of `route-config`) stays undecided and its body read is uncertifiable, not a violation.
