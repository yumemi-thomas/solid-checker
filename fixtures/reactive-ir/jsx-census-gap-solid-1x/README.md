# jsx-census-gap-solid-1x

**Claim.** A reactive read inside a source-level JSX expression container that
the Solid 1.x compiler's execution census never recorded is an **uncertifiable**
SC1001, not a proven violation and not silence.

## Why this shape

Each dialect's compiler censuses the JSX *it* lowers. The 1.x compiler drops a
nested, non-hydratable `<head>` — for
`ReadInsideDroppedHead` it warns that the browser will read the template as
`<div><title></title></div>` — before it is censused. That is the remaining
census hole in this fixture. The static-template `<noscript>` path is now a
positive deletion control: the producer records its discarded child list as an
`Elided` range and the checker stays silent because it has that fact.

Before this fixture existed, that hole fell through to "inside a component
body, classified by nothing" and SC1001 fired as a **proven violation** about
an expression the compiler declined to report on. Absence of a compiler fact is
not a fact — in either direction. The finding stays (the read may well be
stale), but as a proof obligation; and nothing here certifies the opposite
claim that the expression was deleted and is therefore safe, which would need
its own evidence.

The seam is `missing_jsx_census` in
`rust/crates/solid-reactive-ir/src/execution_role.rs`: it consults solid-facts'
source-level JSX syntax (attribute expression containers, spread containers,
children) rather than the census, because the whole question is what the source
has that the census does not.

## Cases

| function | expected |
| --- | --- |
| `ReadInsideDroppedHead` | SC1001 **uncertifiable** — child expression container, no census entry |
| `AttributeReadInsideDroppedHead` | SC1001 **uncertifiable** — same gap reached through an attribute expression container |
| `ReadInsideDiscardedNoscript` | **silent** — the producer positively reports the discarded child list as `Elided` |
| `TrackedChildStaysCertified` | **silent** — an ordinary censused tracked child; the escalation must not start reporting these |
| `ReadOutsideJsxStaysAViolation` | SC1001 **violation** — the read is in no JSX expression at all, so its untracked-rendering proof owes nothing to the census |

`Root` exists so each component above has an enumerable call site. Without it
every read would be uncertifiable for the unrelated reason that the callers of
an exported component are unknown, and the fixture would pass while proving
nothing about the census.

## Dialect

`node_modules/solid-js/package.json` pins `1.9.14`, which is what selects the
v1 catalog (`v1/strict-read-untracked`). Its `.gitignore` exception lines are
`!fixtures/reactive-ir/jsx-census-gap-*/node_modules/` and its `/**` twin.

## Stub faithfulness

`solid-js.d.ts` declares only `createSignal` and the intrinsic elements used
here. `children?: unknown` is wider than solid-js's own `children?: JSX.Element`
— the shape the sibling dialect fixtures use — but every child written here is
one the published typing accepts, so the width manufactures no finding. Nesting
`<head>` inside `<div>` is not a type error against the real typings either:
Solid's JSX namespace types attributes and children, never document structure.

Checked, not assumed: `App.tsx` was compiled with `tsc --noEmit` under
`"jsxImportSource": "solid-js"` against the real `solid-js@1.9.14` typings
provisioned in `rust/target/tsc-oracle/v1`, with the stub removed and
`"types": []`. Zero diagnostics — so neither the stub's width nor the nesting is
manufacturing the defect, and SC1001 here is not duplicating a `tsc` claim.
