# Dependency source: a package context another package re-provides (ADR 0289, review round 1)

Claim: every visible provider still vetoes a selected package's context proof
when it supplies an inert value. ADR 0300 excludes unseen providers only.

- `dep-ctx` (selected, `.solid-checker/runtime.json`) creates `Ctx`. It
  exports `useCtx` and a `Provider` whose value holds two signal accessors.
- `dep-wrap` (selected explicitly since ADR 0300) imports `Ctx`
  and provides a static value through `StaticWrap`.
- `A`, rendered under `StaticWrap`, reads `api.orientation()`. At run time
  `useContext` returns the static value, so nothing reactive is read.

Expected: nothing reported. Before the fix, the selected package's single
visible provider was trusted, which reported a false SC1001 at `A`. Both
providers must remain visible in this regression: the original declaration-only
wrapper's runtime lies outside ADR 0300's premise. The new integrity fixtures
separately distinguish a non-selected source provider from an unseen one.

`solid-js.d.ts` is `context-member-identity`'s stub. `App.tsx` passes
`tsc --noEmit`.
