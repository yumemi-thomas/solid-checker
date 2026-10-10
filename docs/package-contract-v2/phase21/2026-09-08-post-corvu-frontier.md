# Next executable-entrypoint investigations after Corvu

This is investigation, not certification or a coverage estimate. The baseline
is `2026-09-08-verified-retained-floor-full.json`, SHA-256
`15c947a5de23f032c8bc7d6a73e2e15010669aa2371e9966bc7be222dd423bae`.
The new full corpus completed with 326 complete rows and 22 new artifact
cases; see the [completed comparison](2026-09-08-defined-input-recovery.md).
Package paths below come from that baseline's named retained projects unless
explicitly identified as coming from the completed run.

## Fractional Indexing is not an undefined default

`@tanstack/solid-db@0.2.40|solid1|only` stops at
`fractional-indexing@3.4.0:generateKeyBetween`. Its retained source is
`solid-checker-ecosystem-Ct3v7y/node_modules/fractional-indexing/src/index.js`
under the report's temporary project root; SHA-256
`b82f2275d6ea2e6c28b346b89c859a349034350c4541fc9b485b2a2d02c4e1f1`.
It conditionally exchanges `a` and `b` when `a > b`, then reads their `slice`
members. Its other parameters also have default initializers. Neither an
unwritten binding nor ADR 0080's sole local-array store applies. Recovering it
requires a branch-relative origin proof, including the swap and exact read
occurrence; applying the Corvu rule by resemblance would be wrong.

## Flux Store needs returned-value provenance

`@solid-primitives/flux-store@1.0.0-next.2` floor/head refuse a generic returned
value path in `createFluxStore`. The head project's named retained root ends
in `solid-checker-ecosystem-XeJyvb`. Runtime `dist/index.js` has SHA-256
`e1868167aec93d2017e992fe76049dea0546f6e494802cae42ad1d626f9c8b6a`; declarations
`dist/index.d.ts` have SHA-256
`91a07dfce46e02dbea3bc8f2a2f02e79523d0703fbe1238c3a24a4e1063da594`.

The retained proposal returns an object with one stated property, `state`,
whose value is a store. The source destructures slot 0 of `createStore` and
returns it as that property. The declaration instead exposes generic `TState`.
This is not a missing `getters` member or permission to discard a generic
reason. `returnValueSourcesLocked` currently traces array literals and exact
unwritten tuple bindings, but not object-literal return properties. Even after
adding that trace, a consumer would still need an exact accepted dependency
result-slot premise to justify the state shape; a generic constraint alone
does not prove non-callability.

Two diagnostic snippets in `/private/tmp/flux-generic-origin` pass the real
published typings with strict `tsc --noEmit` (TypeScript from the CLI's local
installation). They pass `() => 42` and `() => () => 42` as initial states.
Both runtime attempts throw `TypeError: Cannot create proxy with a non-object
as target or handler`; neither supplies a completed callable-state
counterexample or a new certificate. RC.3's installed signals implementation
routes function arguments to `createStoreDerivedNext`, so ordinary object-store
behavior must not be inferred for that overload. No new checker diagnostic or
weaker generic proof was added.

## Local Store has a smaller concrete runtime shape

`@solid-primitives/local-store@1.1.4|solid1|only` also refuses a generic returned
value path. Its named retained root ends in `solid-checker-ecosystem-c4m4ur`.
Runtime `dist/index.js` has SHA-256
`12358d36efd6b0af4f492d2a2cdae566d57752e3dc0d494cc818cdef0e2ad6ef`; declarations
`dist/index.d.ts` have SHA-256
`b7af771793a86eb4b5d71f91665ed7db9f5dfe6a74fc8a2b67677cdc4d7c7c59`.
The declaration's tuple slot 0 is unconstrained `T`, while the source returns
`new Proxy({}, { get(...) { ... } })` at that slot.

A bounded candidate is a positive non-callable result premise for the exact
intrinsic Proxy constructor applied to an object-literal target. It would
require reviewed runtime authority and exact constructor identity, not the
spelling `Proxy`, a generic type, or an absent callable signature. Shadowed or
imported constructors, callable targets and uncertain constructor resolution
must remain refused. This candidate has not been implemented or measured.

A follow-up diagnostic in `/private/tmp/local-store-proxy-authority` exposes
the runtime-authority requirement. Before calling the unchanged published
Local Store implementation, it uses `Reflect.set(globalThis, "Proxy",
replacement)` where the replacement constructor returns a function. Strict
`tsc --noEmit`, with the real published declarations and Bundler resolution,
exits 0. The Bun runtime completes and reports
`{"completed":true,"resultKind":"function","value":42}`. The process
restores the original global in `finally`. An initial NodeNext type-check
failed on this legacy package's default-import interpretation, so that failed
attempt is not used as the type-correct observation.

The constructor declaration in the package is unchanged by this replacement.
Consequently, exact default-library symbol identity alone cannot prove the
result non-callable. No semantic permission or new finding was added. A later
intrinsic rule must bind the runtime authority as well as the returned target.

The completed report's published `@solidjs/signals@2.0.0-rc.3` root catalog
also limits the Flux Store candidate: the inspected `./dist/prod/index.js`
case describes `createStore` as `{"call":{},"shape":"callable"}`. It does
not contain a returned-store claim. This is a different importer context and
is not reusable authority for Flux Store; even as evidence of available proof
content, it supplies no positive result-slot premise. The bundled dialect
model is not a substitute for a context-bound dependency receipt. Adding
object-property tracing alone therefore cannot justify claiming two new
complete rows.

Other retained refusals still include unexported Solid 1 `solid-js/web` imports
in Solid 2 packages, CJS-only dependency exports, and Node runtime-library
policy requirements. None is cleared by the defined-input work. The full
coverage ceiling remains unproven.

## Callback frontier and the current refused-row census

The completed report has 20 refused rows: eight incompatible published
`solid-js/web` imports; three generic-result proofs (Flux Store floor/head and
Local Store); three callback-flow proofs (Intersection Observer floor/head and
Until); two CommonJS export cases; Fractional Indexing's swapped-parameter
origin proof; and three `node:stream` runtime-policy cases.

Until's exact retained project in that report ends in
`solid-checker-ecosystem-jTHc8r`. Its source passes a closure to the imported
`createBranch`, and the closure passes the condition to `createMemo`.
Installed `@solid-primitives/rootless@1.5.4` positively binds `createBranch`
to `createSubRoot`; that function invokes its `fn` inside a `createRoot`
callback. This establishes the source chain to investigate, not permission
to cross the dependency boundary without a contract.

The report's separately certified Rootless row publishes its catalog at
`solid-checker-ecosystem-out-E8tXAo/` under the exact probe filename. Following
that catalog's document reference shows `createBranch` and `createSubRoot`
both as `{"call":{},"shape":"callable"}`. Neither states a callback
invocation claim. That catalog belongs to another importer and is not
reusable authority for Until. A recovery must first certify the positive
callback premise and compose it in Until's exact dependency context.

Investigating Flux Store's needed return-path tracing exposed an independent
unsound suffix index after an array spread. ADR 0081 corrects that existing
tracer before any expansion of its proof surface. It makes no coverage-gain
claim and does not change the 326-row measured baseline. The full corpus has
not been rerun for that subsequent narrowing.
