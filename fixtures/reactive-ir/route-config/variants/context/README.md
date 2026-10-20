# The route-definitions premise is withheld: context (ADR 0335)

`Read.tsx` imports `RouterContext`, whose `matches` hold route records with `component`. A project that can obtain a route definition can call its `component` itself, so the premise is not applied: every route component in `Routes.tsx` (a copy of `route-config`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
