# jsx-void-child-divergence-solid-1x

The 1.x arm of the lowering-divergence fixtures — void-element children (the
fork's divergence 1) and `<noscript>` children (divergence 3). Everything about
the claims, the mitigation, the emitted-code evidence and the stub is stated
once, in the 2.0 sibling:
**`fixtures/reactive-ir/jsx-void-child-divergence-solid-2/README.md`**. Read
that first.

This directory exists to hold two things the sibling cannot:

**A byte-identical `App.tsx`.** The mitigation is dialect-free consumer policy,
so the same source must reach the same verdict under both producers. `coverage`
enforces the identity through its `IDENTICAL_SOURCES` list, which is what keeps
the snapshot diff between the two meaning "the *producer* changed the answer"
rather than "someone edited one file".

**The 1.x half of the differential.** Probed against the pinned
`solid1-dom-expressions-compiler` (`b66c3e34ba2a0b74238726eb2b83f767eacf94f2`)
with the fresh debug binary, not assumed:

| function | 1.x verdict |
| --- | --- |
| `NestedVoidChild` | SC1001 **uncertifiable**, void wording — same as 2.0 |
| `RootVoidChildDependsOnTheProducer` | SC1001 **uncertifiable**, **void** wording — *2.0 says census gap here* |
| `VoidAttributeStaysCertified` | **silent** — same as 2.0 |
| `AdjacentTrackedChildStaysCertified` | **silent** — same as 2.0 |
| `RootNoscriptChild` | SC1001 **uncertifiable**, noscript wording — same as 2.0 |
| `NestedNoscriptOffTheFastPath` | SC1001 **uncertifiable**, noscript wording; attribute read silent — same as 2.0 |
| `NestedKeygenChild` | SC1001 **uncertifiable**, void wording — *2.0 is silent* |
| `RootMenuitemChild` | SC1001 **uncertifiable**, void wording — *2.0 is silent* |
| `CleanupInsideADivergentChild` | SC4001 **uncertifiable**, divergence wording — same as 2.0 |
| `CleanupInsideACertifiedChild` | **silent** — same as 2.0 |
| module-scope `onCleanup(() => {})` | SC4001 **violation** — same as 2.0 |

Three rows differ from 2.0, and they differ for two different reasons.

**The producer difference** is `RootVoidChildDependsOnTheProducer`, and it is
what made this a pair in the first place. 2.0's template-root path gates child
lowering on `!is_void_element` and emits nothing, agreeing with Babel — so under
2.0 that read is an ordinary census hole and `missing_jsx_census` words it. 1.x
lowers a void element's children in the template-root position too, so the site
is reported and the divergence mitigation words it instead. Same source, same
verdict, different reason, and the mitigation has to get the *reason* right in
both.

**The parity-target difference** is `NestedKeygenChild` and `RootMenuitemChild`,
and here the *verdicts* differ, not just the reasons. This dialect's parity
target — `packages/babel-plugin-jsx-dom-expressions/src/VoidElements.ts` at the
pinned rev — lists 16 tags where the producer's `void_elements` lists 14, so
`<keygen>` and `<menuitem>` children are deleted by the compiler Solid 1.x ships
and lowered by the producer: the divergence, reached through tags the shared list
does not name. 2.0's parity target dropped both on purpose, so under 2.0 both
compilers lower them and the read is certified. The sibling README's "The two
void lists" states the whole comparison; the extras reach the predicate through
`Dialect::parity_target_only_void_elements`, answered here by `Solid1x`.

Both producers also print an HTML round-trip warning to **stderr** for the
`<keygen>` arms (their template validator follows the standard's legacy void
list while their lowering does not). No gate reads stderr and no finding depends
on it.

The `<noscript>` rows agree exactly: this producer keeps a `<noscript>`'s
children in the same two positions the 2.0 fork does (template root, and off the
static-template fast path). Probed, not inherited from the 2.0 contract.

**What this fixture cannot hold.** The *retracting* `<noscript>` position — the
static-template fast path — is where the two producers stop agreeing in the
other direction: 2.0 retracts the discarded sites, while this producer does not
retract them at all and **fails reconciliation** (`semantic trace has unresolved
execution sites: JsxChild@<span>`, exit 2). Adding that arm here would break the
whole fixture, so it lives 2.0-only in
`fixtures/reactive-ir/jsx-census-gap-solid-2` and the 1.x gap is recorded in
`docs/precision-backlog.md` rather than pinned as an exit code — the same
treatment as that fixture's `textContent` retraction arm, which this producer
also rejects.

## Dialect

`node_modules/solid-js/package.json` pins `1.9.14`, and here the stub **is**
load-bearing: dialect selection follows the nearest installed `solid-js`, and
without a 1.x stub this fixture would silently fall back to the 2.0 default and
pin nothing. Its `.gitignore` exception lines are
`!fixtures/reactive-ir/jsx-void-child-divergence-*/node_modules/` and its `/**`
twin.

## Stub faithfulness

`solid-js.d.ts` is byte-identical to the sibling's and faithful in the same
respects — including why the ownership arms are written
`{(onCleanup(() => {}), null)}` rather than `{onCleanup(() => {})}`, which is a
`tsc` error against both real packages; see that README. Checked, not assumed:
this `App.tsx` compiles with **zero diagnostics** against the real audited
`solid-js@1.9.14` typings under both `strict` and `loose`
(`node scripts/tsc-oracle.mjs check --dialect v1`), with the fixture stub
excluded — `<keygen>` and `<menuitem>` with children included, so the 1.x-only
divergence arms are shapes a real project can write and the checker is not
duplicating a type error there.
