# The route-definitions premise is withheld: namespace (ADR 0335)

`Read.tsx` imports the router as a namespace, which reaches `useRouteMatches` and `RouterContext`. A project that can obtain a route definition can call its `component` itself, so the premise is not applied: every route component in `Routes.tsx` (the sources and packages of `route-config`) stays undecided and its body read is uncertifiable, not a violation.
