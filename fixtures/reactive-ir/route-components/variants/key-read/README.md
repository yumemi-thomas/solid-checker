# The route-components premise is withheld: key-read (ADR 0334)

`Read.tsx` reads `props.component`. A value read back from a component key can be called by the project, which the census does not see. The premise is not applied, so every route component in `Routes.tsx` (a copy of `route-components`'s; packages resolve from its `node_modules` by Node's lookup) stays undecided and its body read is uncertifiable, not a violation.
