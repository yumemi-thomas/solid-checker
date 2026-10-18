# Route parameters are URL segments (ADR 0324)

`useParams()` of the audited `@solidjs/router@2.0.0-next.26` hands out route
parameters as URL path-segment strings, or `undefined` for an absent optional
one (`createMatcher` assigns `match.params[key] = locSegment` and a splat's
remainder string, `dist/index.js:76,85`, `utils.js:81,91`; `mergeParams`
`Object.assign`s those, `index.js:1040-1046`; `useParams` is
`useRoute().params`, `index.js:700-702`, `routing.js:163-165`; the object is
`createMemoObject` of the merge unless `utils.paramsWrapper` replaces it,
`index.js:1306`, `routing.js:725-728`, `954`; declared type
`Record<string, string | undefined>`, `types.d.ts:27`). A `<Show when=
{params.k}>` callback therefore receives a string: the accessor form's
`id()`, or the value itself under `keyed` (`solid-js@2.0.0-rc.13`
`dist/solid.dev.js:1462-1486`). That value settles the unmodeled use a callee
makes of the prop it feeds (ADR 0320's primitive proofs).

`App.tsx`. Settled (no `SC9012` at the callee's argument): `RouteAccessor`,
`RouteKeyed`. Refused, each the premise's own boundary, with the same type
domain and no proof that the value is the router's: a computed key
(`ComputedKey`), a name of `Object.prototype` (`PrototypeName`), an alias of
the params object (`AliasedParams`), a wrapped `useParams` (`WrappedUseParams`),
a namespace member (`NamespaceUseParams`), a reassignable binding
(`ReassignedParams`), `useSearchParams` (`SearchParams`), a dynamic `keyed`
flag (`DynamicKeyed`), a key some assignment writes (`WrittenName`) and a key
defined reflectively (`DefinedName`). `SC1001` on `props.clusterId` is a
separate claim (the prop's reactive backing) and is untouched.

The router's declarations are the installed package's, byte for byte
(`package.json` and every `dist/**/*.d.ts`); `@solidjs/web` is a stand-in
carrying only what they name, and `solid-js` and `@solidjs/signals` are the
published rc.13 declarations. Pins: router `dist/index.js` sha256
`b657bc8d…baa35`, `routing.js` `639f16fd…153ea`, `utils.js` `562b280a…7e2a`.
The tree passes `tsc --noEmit`. Siblings: `router-params-replaced-wrapper`,
`router-params-replaced-integration`, `router-params-unaudited-release`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
