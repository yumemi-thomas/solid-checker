# jsx-void-child-divergence-solid-2

**Claim.** A reactive read inside a JSX child region the pinned compiler fork
*lowered* and the compiler Solid actually ships does not lower is an
**uncertifiable** SC1001 — because the two disagree about whether that child
exists at runtime, and nothing available to the checker says which one will
build the project.

Two element classes reach this today, and they are two named conditions in one
predicate, not one condition over a merged tag list:

- **HTML void elements** — the fork's divergence 1;
- **`<noscript>`** — the fork's divergence 3.

The first of those is itself two lists, because "void" is a property of a
*compiler*: the 14 tags both producers and both parity targets agree on, plus the
tags void in only this dialect's parity target (`keygen`, `menuitem` under 1.x;
none under 2.0). See "The two void lists" below.

The 1.x sibling — `fixtures/reactive-ir/jsx-void-child-divergence-solid-1x` —
holds a **byte-identical** `App.tsx`, and that is the point: the mitigation
policy is shared while its tag set is dialect-supplied, so the same source
must pin each producer's answer. Their snapshots differ in exactly three
rows — one reworded message and two findings absent under 2.0 — for reasons
the fixture names below.

## The divergences

The pinned fork's own `docs/execution-contract.md` (revision
`c6008f01df199ff0f0d072093e2393ed3d67f0c4`, "The trace describes this compiler,
not the parity target") names both and states each as a **binding consumer
rule**: *"A consumer must treat a `jsx-child` site inside a void native element
as uncertifiable"* and *"a consumer must treat a `jsx-child` site inside a
`<noscript>` as uncertifiable."*

**Divergence 1 — void element children.** `<div><br>{count()}</br></div>`:

| compiler | emitted code |
| --- | --- |
| this fork (`lower_dynamic_native_child` walks into `lower_dom_children` unconditionally) | `_$insert(_el$2, count)` — a real reactive insert into the `<br>` |
| Babel (`babel-plugin-jsx-dom-expressions`, the compiler Solid ships) | nothing; the child list is discarded in every position |

**Divergence 3 — `<noscript>` children.** `<noscript>{a()}</noscript>`:

| compiler | emitted code |
| --- | --- |
| this fork (drops them only on the static-template fast path) | `_$insert(_el$, a)` at template root, and wherever attributes force the element off that fast path |
| Babel (the compiler Solid ships) | nothing; `<noscript>` children are dropped in every position |

The census follows the emission in both cases, so at this pin each site **is**
reported, as `jsx-child` / `reactive-rerun`. That is the fork being truthful
about itself. It is not evidence about Babel, and the checker cannot certify
either reading:

- calling the read **tracked** believes only the fork;
- calling it a **stale untracked read** believes only Babel.

Uncertifiable is the only claim the facts support. The two get **different
sentences**, because they are true for different reasons: Babel *deletes* a void
element's child list, while it never *lowers* a `<noscript>` subtree at all.
Saying "deletes it" of `<noscript>` would describe a compiler step that does not
happen.

## Why detection is positive, from the AST

The mitigation is `divergent_lowered_child` in
`rust/crates/solid-reactive-ir/src/execution_role.rs`. It identifies the element
from **this checker's own AST** — solid-facts owns JSX element-name syntax — and
`VOID_ELEMENTS` is the one copy in the repository, byte-checked against
`void_elements` in `packages/compiler/src/shared/constants.rs` at *both* pinned
producer revisions (the two lists are identical: `area base br col embed hr img
input link meta param source track wbr`).

**`<noscript>` is deliberately not a member of that list.** The list's whole
value is that it matches the compiler's `void_elements` exactly, and
`<noscript>` is not in it: it has an ordinary content model and diverges for an
unrelated reason (inert markup the shipped compiler declines to lower, not a
childless content model). It is a separate named condition — `INERT_MARKUP_ELEMENT`
— in the same predicate. A unit test pins the two apart, including that
`<template>`, which is also inert-feeling and which the compiler does *not*
treat specially, reports nothing.

Where the two nest (`<noscript><br>{x()}</br></noscript>`) the **narrowest**
enclosing divergent element decides the wording, the same rule
`narrowest_jsx_region_containing` uses: both answers are uncertifiable, but only
the nearer one is the accurate reason.

Detecting the divergence from **census absence** would be wrong twice over.
`census_touches`'s deliberate overlap rule lets a wider censused region shadow a
narrower hole, and — decisively — after this pin the divergent child is not a
hole at all: it carries an entry claiming `ReactiveRerun`, which is exactly the
claim that must not be believed.

The one compiler fact the mitigation *does* consult is positive: a
`jsx-expression` operation inside the void element's child region — that is the
`ExecutionMap` operation kind the predicate matches, not the fork contract's
`jsx-child` *site* vocabulary the sections above quote. That is what separates the
divergence from an ordinary census gap, and it is what makes the differential
case below come out right.

## Cases

| function | 2.0 | 1.x |
| --- | --- | --- |
| `NestedVoidChild` | SC1001 **uncertifiable**, void wording | identical |
| `RootVoidChildDependsOnTheProducer` | SC1001 **uncertifiable**, *census-gap* wording | SC1001 **uncertifiable**, *void divergence* wording |
| `VoidAttributeStaysCertified` | **silent** | identical |
| `AdjacentTrackedChildStaysCertified` | **silent** | identical |
| `RootNoscriptChild` | SC1001 **uncertifiable**, noscript wording | identical |
| `NestedNoscriptOffTheFastPath` | SC1001 **uncertifiable**, noscript wording; the attribute read stays **silent** | identical |
| `NestedKeygenChild` | **silent** — certified, both compilers lower it | SC1001 **uncertifiable**, void wording |
| `RootMenuitemChild` | **silent** — certified | SC1001 **uncertifiable**, void wording |
| `CleanupInsideADivergentChild` | SC4001 **uncertifiable**, divergence wording | identical |
| `CleanupInsideACertifiedChild` | **silent** | identical |
| module-scope `onCleanup(() => {})` | SC4001 **violation** | identical |

`RootVoidChildDependsOnTheProducer` is the fixture's differential, probed
rather than assumed. At template root 2.0 gates child lowering on
`!is_void_element` and emits nothing — agreeing with Babel, so the hole is a
genuine hole and `missing_jsx_census` owns it with its own wording. 1.x lowers
the children in that position too, so the site is reported and this is the same
divergence as the nested case. Both answer *uncertifiable*; only the reason
differs. A mitigation keyed on the tag alone would give both the divergence
wording and lie about 2.0; one keyed on census silence would give both the gap
wording and lie about 1.x.

`VoidAttributeStaysCertified` pins the boundary the fork's contract draws:
attributes are not children. Both compilers lower an attribute, an event
handler and a `ref` on a void element in every position and keep their sites,
so the read is certified tracked. `NestedNoscriptOffTheFastPath` asserts the
same thing for `<noscript>` in passing — its `id={tag()}` read stays silent
while its child does not — and its dynamic attribute is also what forces the
element off the static-template fast path. A *static* attribute does not; that
was probed, not assumed.

**The retracting `<noscript>` position is pinned elsewhere, and deliberately.**
On the static-template fast path this producer emits the tag and returns without
visiting the children, retracting their sites — there it agrees with Babel, so
the result is an ordinary census gap, not a divergence.
`fixtures/reactive-ir/jsx-census-gap-solid-2` holds that arm
(`RetractedInertNoscriptChild`) and is the mechanical guard against this
mitigation keying on the tag alone: if it did, that arm would flip to the
divergence wording, and that fixture is in `KEEPS_WORDING`, so the flip cannot
pass silently. It cannot live in **this** pair because the 1.x producer does not
retract that subtree at all — it fails reconciliation and rejects the file (see
`docs/precision-backlog.md`), and a fixture would pin an exit code rather than a
semantic claim.

`Root` gives every component above an enumerable call site. It is load-bearing
here: without it each read would be uncertifiable for the unrelated reason that
the component's callers are unknown, and the fixture would pass while proving
nothing.

## The two void lists

`NestedKeygenChild` and `RootMenuitemChild` are the arms that make the void tag
set dialect-aware, and their verdicts are **opposite** in the two dialects while
their source is byte-shared.

| | Rust producer | parity target |
| --- | --- | --- |
| 1.x | 14 tags — `void_elements`, `packages/compiler/src/shared/constants.rs` @ `b66c3e34` | **16** — `packages/babel-plugin-jsx-dom-expressions/src/VoidElements.ts` @ `b66c3e34`, adding `keygen` and `menuitem` |
| 2.0 | 14 tags — same file @ `c6008f01` | 14 — `VoidElements` in `packages/runtime/src/constants.js` @ `c6008f01`, imported by `babel-plugin-jsx` |

A divergence is a producer disagreeing with **its own** parity target, so under
1.x these two tags diverge exactly as `<br>` does — the plugin computes
`voidTag = VoidElements.indexOf(tagName) > -1` and gates the whole child pass on
`if (!voidTag)`, so it deletes the child in every position while the producer
lowers it and censuses `reactive-rerun`. Before this was dialect-aware the
checker **certified those reads by silence**, which is the worst of the three
possible answers.

Under 2.0 the same source is certified on purpose: that parity target dropped
both tags deliberately (`packages/babel-plugin-jsx/CHANGELOG.md`, `1cc342c`,
unifying the plugin's former `src/VoidElements.ts` with the runtime set), so both
compilers lower the child and the read really is tracked. A single union list
would report here and withhold a certification the facts support; a shared list
with no dialect seam certifies the 1.x case by silence. Hence two lists: the
shared, byte-checked `VOID_ELEMENTS`, and
`Dialect::parity_target_only_void_elements`, which each dialect answers from its
own parity target's file. They are joined only inside
`divergent_candidate_child`.

Both arms keep the shared `VoidElementChild` wording, because the reason is the
same one. And the positive lowered-site fact still gates them — a `<keygen>`
whose children the producer discarded would be an ordinary census gap, which is
pinned as a unit test rather than here (no producer position discards them).

**Producer stderr, not a finding.** Both producers print `The HTML provided is
malformed … Browser HTML: <keygen>` to stderr for these arms: their template
round-trip validator follows the HTML standard's legacy void list while their
lowering does not. No gate reads stderr and the analysis result is unaffected; it
is noise around the very disagreement these arms pin.

## The ownership arms

The divergence is not only about whether a read is tracked. The pinned fork wraps
the insert it emits for a divergent child in its default effect wrapper, so the
producer reports an `Owned` ownership region there — and `owners.rs` must not
read that as proof of an owner, because the parity target emits neither the insert
nor the wrapper.

Dropping the region is only half the answer, and the missing half was a real
defect: where the surrounding context is *proven unowned* (module scope), the
requirement stands and SC4001 fired as a **proven violation** about an operation
neither compiler leaves unowned — under the fork it runs under the insert's
owner, under the parity target it sits in deleted code and never runs at all.
Three arms pin the fix:

- `CleanupInsideADivergentChild` — SC4001 **uncertifiable**, worded as the
  disagreement (`whether this call runs at all, and under which owner, depends on
  which compiler builds this project`) rather than as a missing owner;
- `CleanupInsideACertifiedChild` — the identical call one tag over, inside a
  `<span>` both compilers lower: **silent**, because the region really is
  evidence there. This is what makes the escalation positional and narrow;
- the bare module-scope `onCleanup(() => {})` — still a proven **violation**. The
  divergence removes a proof; it does not excuse an operation that never had one.

The escalation lives in `push_owner_requirement`, the single funnel both owner
passes use, and rides `OwnerRequirement::divergent_lowering`. `createEffect`
reaches it identically (verified by probe under 1.x); this fixture uses
`onCleanup` because 2.0's two-argument `createEffect` would add an unrelated
signature violation to a byte-shared source.

## What is *not* mitigated

Other consumers of the same divergent facts still read them, deliberately and
recorded in `docs/precision-backlog.md` (2026-08-24): the destructure freshness
discharge in `static_rules.rs`, `resolve_tracked_scope` in `static_api.rs`, the
post-flush server rules, and SC1003 no-destructure — which keeps its
**violation** on a divergent case while refusing its autofix, since the reactive
read that would satisfy the rewrite exists only under the fork. Each would need its own
uncertifiable path; forcing the *role* away from `TrackedJsx` to reach them
would manufacture proven disallowed-write findings inside a subtree that may not
exist, which is a worse claim than the one it fixes.

The fork's divergence 2 (nested dynamic `textContent`) needs no mitigation here
— the producer retracts, so the pre-existing census-gap path already answers
uncertifiable. Divergence 4 (a `children` attribute on a nested void element
with no source children) stays a hard reconciliation failure in the producer by
design. Both are recorded in the backlog.

## Dialect

`node_modules/solid-js/package.json` pins `2.0.0-rc.0` (the sibling pins
`1.9.14`). 2.0 is also the fallback default, so the stub is not what makes this
fixture run the v2 catalog — it is here so the fixture states its dialect rather
than inheriting it, and so a stub appearing above the fixture tree cannot
silently re-dialect it. Its `.gitignore` exception lines are
`!fixtures/reactive-ir/jsx-void-child-divergence-*/node_modules/` and its `/**`
twin.

## Stub faithfulness

`solid-js.d.ts` declares only `createSignal`, `onCleanup`, and the intrinsic
elements used here. `children?: unknown` is wider than solid-js's own
`children?: JSX.Element` — the shape the sibling dialect fixtures use — but
every child written here is one the published typing accepts, so the width
manufactures no finding. `br`'s `id` and the elements' other attributes stay
narrowed as the published `HTMLAttributes` declares them, because the negative
case's proof rests on the attribute being a *legal dynamic attribute*.

Giving `br` and `hr` children is not a type error against the real typings:
Solid types them as `HTMLAttributes<HTMLBRElement>` / `HTMLAttributes<HTMLHRElement>`,
and `DOMAttributes` carries `children` for every element, void or not. `keygen`
and `menuitem` are likewise ordinary intrinsic elements in both published
typings — the divergence is about lowering, not about types.

`onCleanup` is declared `(fn: () => void): void`, **narrower** than either
published signature (1.x returns the callback `T`; 2.0 returns `Disposable`).
Narrower is the safe direction — a stub must never be *looser* than the real
package — and nothing here reads the return value, so it cannot hide a finding
either. It also lets one declaration stay byte-shared across the pair where the
two real signatures differ.

**Why the ownership arms use a comma expression.** `{onCleanup(() => {})}` on its
own is a `tsc` **error** against both real packages, because neither return type
is a `JSX.Element`: TS2322 `Type '() => void' is not assignable to type 'Element'`
under `solid-js@1.9.14`, and `Type 'Disposable' is not assignable to type
'Element'` under `2.0.0-rc.0`, in `strict` and `loose` alike. Writing that arm
would have required a stub looser than the real package — exactly the trap
AGENTS.md names — so the arm is `{(onCleanup(() => {}), null)}`, which every real
typing accepts and which keeps the call inside the divergent child region.

Checked, not assumed: this `App.tsx` was compiled by
`node scripts/tsc-oracle.mjs check` against the real audited typings —
`solid-js@2.0.0-rc.0`, `@solidjs/web@2.0.0-rc.0`, `@solidjs/signals@2.0.0-rc.0`
under v2, and `solid-js@1.9.14` under v1 — with the fixture stub excluded.
**Zero diagnostics in both dialects, under `strict` and `loose`.** So a reactive
read inside a void element's children, a `<keygen>` with children, and a cleanup
registered inside one are all shapes a real project can write, and nothing here
duplicates a `tsc` claim.
