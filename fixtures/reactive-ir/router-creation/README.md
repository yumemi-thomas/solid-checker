# router-creation

**Claim (ADR 1066, a decision not to build).** `const Router = createRouter({ … })` of a package read as dependency source (`mini-router`, the shape
of `@solidjs/router` 2.0.0-next.41's `createRouter`: a component declared in the body, returned as `Object.assign(RouterComponent, { … })`).
The children function of `<Router>` runs where the package's component runs it, and the creations in it and in `Layout` stay uncertifiable
(`unread-package-may-run`, SC3001/SC6002): the hand-off reads the tag's `const` as a value nothing here follows. Taking the package's code as
`Known` would clean them, which is wrong for a package that runs the children in a leaf callback (twin `router-creation-settled`, Chrome:
PRIMITIVE_IN_FORBIDDEN_SCOPE); following the call to the returned component and reading its body exactly answers as main does here.
