# Solid 1.x vocabulary inside a package artifact

The package declares a `solid-js@^1.8.6` peer dependency and installs a 1.9.14
stub, so the analyzed artifact runs the v1 catalog rather than the 2.0 default.
The claim is that the dialect's vocabulary is actually reached from contract
generation -- not only from project analysis -- and that it survives the
indirections a published package uses.

The facts that only the v1 catalog can produce, and where to see them:

- `indirect` and `indirectResource` carry a **`tracked`** callback operation:
  the caller's accessor is invoked inside `createMemo` / a `createResource`
  source, so the invocation happens under a tracking scope. `observe`
  (`on(source, …)`) is `untracked` by contrast.
- `effectThroughMaybeAccessor` **creates an owner requirement**: `createEffect`
  cannot run without an owner, and that obligation reaches the public export
  through a wrapper.
- `returnedAccessor`, `returnedResource`, `assignedResource`, `tupleResult`,
  `objectResult`, the `projected*` family, `identityResult`,
  `memoThroughConditionalAdapter` and the two `context*` readers all prove a
  **return**: a memo, resource or store accessor escapes to the caller through
  a returned closure, a destructuring assignment, a tuple, an object literal, a
  projection, a transparent identity wrapper, and a context read.
- `conditionalInlineCallback` invokes the caller's callback same-stack through
  a conditional `runWithOwner` adapter.
- `conditionalIdentity`, `guardedIdentity`, `constructionCandidates`,
  `isObject`, `mountRouter` prove nothing, and `t` is `plain`. Those are the
  controls: one identity-looking branch is not an identity contract, and a
  minified namespace-export call must not inherit its callee's summary.

If the `node_modules/solid-js` stub is lost or its version stops parsing,
dialect selection silently falls back to 2.0 and every `tracked` and owner
fact above disappears -- which is why the stub is tracked and why coverage's
`checkDialectStubs` enumerates this directory.

The stub is reduced (`createResource` returns a one-element accessor tuple
rather than the published `[Resource<T>, { mutate, refetch }]`), but it is not
*looser* in any direction a claim above depends on: every signature the
accessor and tracking facts rest on is the published shape.
