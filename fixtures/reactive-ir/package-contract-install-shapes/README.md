# Which install shapes the attested identity comparison accepts

A package contract is applied only to an import that resolves into the
installed package the contract was classified against. That comparison has to
survive the install shapes real projects have, or a rare false certification
would be traded for a routine false uncertifiable result — which is why the
earlier attempt at this check (containment on *declaration* paths) was rejected
in `docs/precision-backlog.md`. This fixture pins the three shapes that
decision named, in one file, so none of them can be silently reversed.

- `ambientlyTypedInstall` — `ambient-package` is installed with a reviewed
  contract and ships no typings; the project types it with an ambient
  `declare module`. The compiler answers `unresolved` for the specifier. Nothing
  resolved means nothing *else* claimed the specifier, so no shadowing package
  can be what the contract describes, and the contract **applies**: the
  unbindable callback-argument claim raises its one `SC9005` obligation. This is
  the untyped-JavaScript case the backlog calls "precisely where a contract
  matters most".
- `typesRedirectedInstall` — `redirected-package` is installed with a reviewed
  contract, and the specifier resolves into
  `node_modules/@types/redirected-package`, which is a different installed
  package. The contract is **refused** and raises nothing. This is a deliberate
  fail-closed outcome, not an oversight: deriving "`@types/x` describes `x`"
  from the two names is the name-only reasoning the identity gate exists to
  remove, and a Solid-aware package that both ships a reactivity contract and
  is typed only through DefinitelyTyped is a shape nothing in the corpus has.
  It is recorded as a named residue in `docs/precision-backlog.md`.
- `subpathUnderUnnamedManifest` — `subpath-package/deep` resolves to a file
  under `node_modules/subpath-package/deep/package.json`, which declares
  `{"type": "module"}` and no name. The nearest manifest therefore names no
  package and the resolver recorded no package identity either, so a comparison
  that required a matching *name* would refuse a perfectly ordinary published
  layout. The resolved file is inside the installed directory, so the contract
  **applies**.

`tsc --noEmit` is silent on this project. None of the three outcomes is a typing
question: the ambient declaration satisfies the compiler, the `@types` package
satisfies it, and the subpath's declaration satisfies it. Which *installed
package* a specifier resolves to is a resolution fact, and the contract that
may describe it is not something the type system models.

Each contract here is minimal on purpose — one export with one callback claim
whose argument descriptor a by-name callback cannot bind — because the subject
is which contracts bind, not what they say. The consumer side of a rich claim
set is `fixtures/reactive-ir/package-callback-arguments-consumer`; the shadowing
direction is `fixtures/reactive-ir/package-contract-paths-shadow`.
