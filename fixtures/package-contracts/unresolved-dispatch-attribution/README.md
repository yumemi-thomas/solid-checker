# Attributing an unresolved dispatch to the exports that own it

`channelFor` invokes `.getThing` on its own parameter. In a published
JavaScript runtime artifact that parameter is `any`, so the member callee
resolves to no implementation and every call site of `channelFor` receives one
`ReactiveDispatchUnresolved` obligation.

The fixture pins two things the generator used to get wrong at once.

## Which claim domains the obligation invalidates

The obligation proves that the possible runtime implementations of `.getThing`
do not share one reactive-read summary. That makes `reactiveReads` unknown, and
`returns` with it: the returned object's property is described from the local
accessor index, which knows nothing about the dispatch result, so a
possibly-reactive property would otherwise be published as a certified-negative
omission. See `unresolved-dispatch-domains-control`, whose resolved dispatch
makes the generator claim that property outright.

It proves nothing about the callback the export invokes, so every host keeps
its `callbacks` row. Erasing `callbacks`, `ownerRequirements` and
`asyncBehavior` here was discarding four independently established claims to
record one.

## Which exports the obligation belongs to

`Direct` holds the unresolved call in its own body -- the innermost enclosing
function *is* the export, and attribution joins directly.

`Arrow` holds it in an anonymous arrow and `Helper` in a named local function.
Neither of those is an export, and reading only the innermost enclosing
function sent both obligations to the mark-everything fallback: the whole
entrypoint, including `inert`, collapsed to one summary with all five domains
unknown. Attribution now walks the enclosing chain outward and stops at the
exported function that lexically contains the obligation.

`inert` is the negative control and the point of the fixture: it invokes no
unresolved member and must keep its complete, proven summary. An export that
cannot reach the obligation is not made uncertain by it.

The generator records which rung of that ladder answered on each
`unknown-sentinel` item of `<contract>.review.json`, under `because`: `joined`
for `Direct`, `enclosing-chain` for `Arrow` and `Helper`.

## Stub faithfulness

`node_modules/solid-js` pins the 1.x dialect and is otherwise unused by these
exports; `index.d.ts` types the published surface exactly as the runtime
artifact behaves. Nothing here is loosened to manufacture the obligation --
`client.getThing` is unresolvable because a structural `Client` interface has
no runtime implementation, which is the shape the ecosystem benchmark found in
real packages.
