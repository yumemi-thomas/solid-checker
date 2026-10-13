# Dependency source: a package context another package re-provides (ADR 0289, review round 1)

Claim: a context created by a package analyzed as source has no closed
provider census. Packages that are not analyzed can import it, provide it or
write its value.

- `dep-ctx` (selected, `.solid-checker/runtime.json`) creates `Ctx`. It
  exports `useCtx` and a `Provider` whose value holds two signal accessors.
- `dep-wrap` (not selected: types and compiled JavaScript only) imports `Ctx`
  and provides a static value through `StaticWrap`.
- `A`, rendered under `StaticWrap`, reads `api.orientation()`. At run time
  `useContext` returns the static value, so nothing reactive is read.

Expected: nothing reported. Before the fix, the selected package's single
visible provider was trusted, which reported a false SC1001 at `A`. With only
`Provider` the read would be real, but it is not proven either: no context a
package creates transports a member to the project.

`solid-js.d.ts` is `context-member-identity`'s stub. `App.tsx` passes
`tsc --noEmit`.
