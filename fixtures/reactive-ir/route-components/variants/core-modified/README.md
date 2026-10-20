# The route-components premise is withheld: core-modified (ADR 0334)

The router is the published one (its own copy, so that router-core is looked up beside it), but the router-core it loads is a partial copy whose bytes are not the published release's, and the premise reads router-core too. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-components`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
