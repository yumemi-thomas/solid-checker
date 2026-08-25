# jsx-census-gap-solid-2

**Claim.** A reactive read inside a source-level JSX expression container that
the Solid 2.0 compiler's execution census never recorded is an **uncertifiable**
SC1001, not a proven violation and not silence.

The 1.x sibling — `fixtures/reactive-ir/jsx-census-gap-solid-1x` — pins the
same claim over the shape *that* producer drops. The two are not a differential
pair: the rule is identical in both dialects, and only the JSX each compiler
declines to census differs. That is why the seam
(`missing_jsx_census` in `rust/crates/solid-reactive-ir/src/execution_role.rs`)
is shared and dialect-free, while the shapes that reach it are per-producer.

## Why these shapes

Two ways a source-level JSX expression can reach the checker with no census
entry, arrived at from opposite directions.

**Never censused.** A void element at its own template root: the 2.0 compiler
gates child lowering on `!is_void_element` there, so it emits no execution site
for the child and the generated code never reads the accessor. `{count()}`
reaches the checker as a hole in the `ExecutionMap`: no tracked region, no
untracked region, no callback role, no JSX operation. That hole used to fall
through to "inside a component body, classified by nothing" and SC1001 fired as
a **proven violation** about an expression the compiler declined to report on.

**Censused, then retracted.** A `<noscript>` on the static-template fast path
has its tag emitted with the children never visited at all. The recorder
retracts those censused sites during lowering. What arrives is a hole in exactly
the same shape as a never-censused one, so it takes the same wording and the same
verdict. The mitigation cannot key on *why* the hole exists.

The former second retraction arm is now a negative control. At producer
`0ce01d74`, a nested element with dynamic `textContent` and real children
matches Babel's `!hasChildren` gate: the children lower normally, their sites
remain tracked, and only an empty child list receives a synthesized placeholder.

The `<br>` stays at the root of its own component deliberately. **Nested**
(`<div><br>{count()}</br></div>`) the producer really lowers the child and
reports a `reactive-rerun` site the compiler Solid ships would never emit —
which is a different claim, needing a different mitigation and a different
message, and `fixtures/reactive-ir/jsx-void-child-divergence-solid-2` pins it.
(Before the pin at `c6008f01`, that nested shape failed the compile outright
with `semantic decision targets an uncensused JsxChild site`; the fork's PR #2
censuses it instead, so it is now analyzable — and the reason this fixture no
longer claims otherwise.)

`RetractedInertNoscriptChild` earns its place twice. It pins the retraction, and
it is the mechanical guard on the divergence mitigation next door: `<noscript>`
children *are* a divergence in the two positions this producer keeps them
(template root, and off the fast path via a dynamic attribute), so a mitigation
that keyed on the `<noscript>` tag alone rather than on a lowered site would flip
this arm to the divergence wording. This fixture is in coverage's
`KEEPS_WORDING` set, so that flip fails the gate instead of passing quietly.

The static-`<noscript>` retraction is shared: the 1.x pin at `d1e08958` added
it, and the 1.x census-gap fixture pins that arm. The 1.x producer already had
the correct dynamic-`textContent` child-list gate before this 2.0 pin move.

## Cases

| function | expected |
| --- | --- |
| `ReadInsideVoidElementChild` | SC1001 **uncertifiable** — template-root void-element child, never censused |
| `TextContentChildNowCertified` | SC1001 **silent** — the real child is an explicit tracked site; SC8003 remains a **violation** on the element for the independent children-and-`textContent` conflict |
| `RetractedInertNoscriptChild` | SC1001 **uncertifiable** — censused, then retracted by the inert-`<noscript>` fast path; **and** the guard that the void/`<noscript>` divergence mitigation stays keyed on a lowered site rather than on a tag |
| `TrackedChildStaysCertified` | **silent** — an ordinary censused tracked child; the escalation must not start reporting these |
| `ReadOutsideJsxStaysAViolation` | SC1001 **violation** — the read is in no JSX expression at all, so its untracked-rendering proof owes nothing to the census |

`Root` gives each component above an enumerable call site. It was **not**
load-bearing when this fixture held only the never-censused arm — measured, not
assumed: deleting it then left the findings byte-identical. It is kept because
it *is* load-bearing in the 1.x sibling, where the same deletion flips
`ReadOutsideJsxStaysAViolation` from a violation to an uncertifiable finding —
same message, different verdict, for a reason unrelated to the census — and so
destroys that fixture's second negative. With both roots present the pair
differs only in the shape its producer declines to census. The measurement was
not redone for the retraction arm added at the `c6008f01` pin; treat the root as
required rather than as proven redundant.

## Dialect

`node_modules/solid-js/package.json` pins `2.0.0-rc.0`. 2.0 is also the
fallback default, so the stub is not what makes this fixture run the v2
catalog — it is here so the fixture states its dialect rather than inheriting
it, and so a stub appearing above the fixture tree cannot silently re-dialect
it. Its `.gitignore` exception lines are
`!fixtures/reactive-ir/jsx-census-gap-*/node_modules/` and its `/**` twin.

## Stub faithfulness

`solid-js.d.ts` declares only `createSignal` and the intrinsic elements used
here. `children?: unknown` is wider than solid-js's own `children?: JSX.Element`
— the shape the sibling dialect fixtures use — but every child written here is
one the published typing accepts, so the width manufactures no finding. Giving
`br` children is likewise not a type error against the real typings: Solid
types `br` as `HTMLAttributes<HTMLBRElement>`, and `DOMAttributes` carries
`children` for every element, void or not. `span`'s `textContent` stays
narrowed to `string` exactly as the published `HTMLAttributes` declares it,
because the retraction arm's proof rests on it being a legal *dynamic
attribute* rather than an unknown one.

Checked, not assumed: `App.tsx` was compiled against the real
`@solidjs/web@2.0.0-rc.0` and `solid-js@2.0.0-rc.0` typings provisioned in
`rust/target/tsc-oracle/v2`, with the fixture stub excluded — re-run with
`node scripts/tsc-oracle.mjs check --dialect v2 --file
fixtures/reactive-ir/jsx-census-gap-solid-2/App.tsx` after adding the
retraction arm. Zero diagnostics under both `strict` and `loose` — so `<br>`
with a reactive child, and `textContent` alongside children, are shapes a real
project can write, and SC1001 here duplicates no `tsc` claim. (SC8003 is a
different claim about the same element: a JSX authoring conflict `tsc` does not
report either, since both attributes are legally typed.)
