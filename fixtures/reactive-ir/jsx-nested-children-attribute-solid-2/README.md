# jsx-nested-children-attribute-solid-2

**Claim.** Moving `dom-expressions-compiler`'s pinned revision from
`c6008f01df199ff0f0d072093e2393ed3d67f0c4` to
`fea62adb5d0332a4a3cb5088e97283673c40b540` (upstream PR #3, "nested children
attribute promotion") resolves the fork's declared divergence 4 — a `children`
attribute on a nested element with no source children used to make the
producer's own census name a `jsx-child` site that nested lowering never
resolved, a **hard reconciliation failure** the producer treats as its only
detection signal: `solid-checker-rust` exited 2 and the file was never
analyzed at all
(`docs/precision-backlog.md`, "Divergence 4 stays a hard reconciliation
failure by design"). There is no checker-side mitigation this fixture removes
or exercises for divergence 4 — it never reached one, because the failure mode
was the whole file being rejected before analysis, not a fact this checker read
and had to distrust the way it does for divergences 1 and 3
(`fixtures/reactive-ir/jsx-void-child-divergence-solid-2`).

At the new pin, `lower_dynamic_native_child` performs the same promotion
`lower_dom_element` already performed at a template root: the value is pushed
onto the (empty) child list as an ordinary expression container, so it lowers
and censuses exactly like any other nested child.

The current `0ce01d7476367dab2f4d067f4771d5010e347c75` pin retains this work from
`main` onto `next`; the historical `c6008f01` → `fea62adb` transition above is
the semantic change this fixture exists to pin.

Promotion is also what makes this fixture the natural home for the second
claim it pins: **what the promoted-or-dropped attribute means when the
compiler deletes it**, and where deletion stops being the whole story because
a named divergence touches the same span.

1. **The promoted value is read exactly like an ordinary tracked JSX
   child.** `NestedChildrenAttributePromoted` — silent, the same verdict as
   `AdjacentTrackedChildStaysCertified` in the void-element sibling fixture.
2. **A confidently-foldable `children` value is never promoted.**
   `LiteralChildrenAttributeStaysSilent` — `captured_child` accepts only a
   non-literal expression (`evaluate_confident(...).is_none()`, mirroring
   Babel's own capture), so the attribute stays an ordinary constant-folded
   property write and carries no reactive read at all. Silent for a reason
   unrelated to the rest of the fixture.
3. **A `children` attribute the compiler *deletes* is silent, not a proven
   stale read.** `SourceChildrenShadowChildrenAttribute` — `captured_child` is
   gated on `child.children.is_empty()`, which this shape never satisfies on
   either side of PR #3, so the attribute never reaches the promotion logic and
   is dropped during attribute planning instead (the fork's
   docs/execution-contract.md, divergence 4: *"the shapes that already agreed
   still do: with source children... both compilers insert only `y` and report
   the attribute as `native-attribute`/elided"*). Both compilers emit nothing
   for it. See "What `elided` means" below — this arm is the fixture's pin for
   the projection fix landed 2026-08-24.
4. **Where the deletion sits on a divergent element, the divergence wins.**
   `NoscriptPromotedChildrenAttribute` — a *template-root*
   `<noscript children={note()}/>` is promoted **and lowered**
   (`_$insert(_el$, note)`), while Babel promotes the attribute into a child
   list its `transformElement` never visits and emits nothing. SC1001
   **uncertifiable**, the same verdict `<noscript>{x()}</noscript>` gets in the
   sibling fixture. The fork names this route itself: *"The root-level
   `children`-attribute-promoted variant is the same divergence by another
   route ... Still divergent"* (its docs/execution-contract.md, divergence 3).

5. **A spread means the attribute was never promoted, so nothing about it
   diverges.** `SpreadKeepsChildrenInMergedProps` and
   `NestedSpreadKeepsChildrenInMergedProps` — both **silent**. This is arm 4's
   boundary, and the shape that proved the arm needed one; see "The
   promoted-attribute arm's boundary" below.
6. **Deleted code reaches rules that consult no execution facts, and two of
   them are gated for it.** `DestructureInsideADeletedChildrenValue` (SC1003)
   and `LeafOwnerInsideADeletedChildrenValue` (SC3001) — both **silent**, each
   with its live control in the same file
   (`DestructureAtComponentBodyScope`, `LeafOwnerInsideAPromotedChildrenValue`)
   so neither arm can pass by its rule being off. See "Two more funnels"
   below.

An ownership arm, `CleanupOwnedByPromotedChild`, pins the same claim for
attribution: the fork wraps the promoted value's insert in its default effect
wrapper exactly as it does for any other nested child, so a cleanup registered
inside the promoted expression is owned by that insert, not left standing
against an unowned surrounding context the way the void-element divergence's
`CleanupInsideADivergentChild` arm is.

## What `elided` means, and what this fixture used to claim it meant

`Value(Elided)` is a real fact and not a census hole — but the fact is that
the value is **deleted**, not that it runs once. This fixture originally pinned
`ignored()` as an SC1001 **proven violation** on the reasoning that an elided
attribute is "read once at setup and then discarded", and that reasoning was
wrong at the emitted-code level: `plan_attributes` drops the shadowed attribute
before anything is lowered for it, so no read happens in either compiler's
output. Every clause of the rule's sentence — *"the read sees the current value
once and never updates"* — is false of code that does not exist. All nine of
the 2.0 fork's `Elided` emission sites (and all eight of the 1.x fork's) are
either constants folded into the template or values discarded unlowered; none
of them evaluates at runtime.

Both dialect adapters now project `Value(Elided)` to a **discarded region**
(`ExecutionMap::discarded_regions`), distinct from an untracked region, and the
IR classifies a span inside one as `ExecutionRole::DiscardedRendering`. That
role reports nothing — and certifies nothing either: silence there means "both
compilers deleted this", never "this was proven safe". `Value(EagerOnce)` is
unchanged and stays an untracked region, because it genuinely does execute,
once. The full account is in `docs/precision-backlog.md` (2026-08-24) and
`docs/compiler-facts.md`.

The exception is arm 4 above. Where a deletion decision and a named
producer/parity-target divergence touch the same span, the divergence wins:
`divergent_lowered_child` gained an arm for the promoted-`children`-attribute
span on the divergent elements, because `children={…}` *is* the child list
(Babel promotes it to a real child before `transformElement` runs) and the
predicate's child-region containment deliberately excluded attribute spans.
Before that arm, a template-root `<noscript children={c()}/>` was silently
**certified** — the census reports the promoted site as an ordinary tracked
child — while the same divergence written as `<noscript>{c()}</noscript>` was
correctly uncertifiable.

The arm's *conditions* are arm 5's subject and are described under "The
promoted-attribute arm's boundary" below.

The no-spread nested spelling is the other side of that arm and is deliberately
*not* here: `<div><noscript children={c()}/></div>` discards the capture instead of
promoting it — promoting would emit an insert Babel does not — so both
compilers agree and no divergence may be claimed. That case is pinned as a unit
test in `rust/crates/solid-reactive-ir/src/execution_role.rs`
(`a_promoted_children_attribute_diverges_only_where_the_producer_lowered_it`
and `without_a_spread_the_same_two_positions_keep_their_verdicts`),
with the two producer facts it depends on pinned in the adapter
(`rust/dialects/solid-v2/compiler/src/lib.rs`:
`a_template_root_noscript_children_attribute_is_lowered_not_deleted` and
`a_nested_noscript_children_attribute_is_deleted_not_lowered`), because writing
the nested shape here would need a second wrapper element and would pin the
position rather than the promotion this fixture is about.

## The promoted-attribute arm's boundary: a spread

Arm 4 fires on a *promoted* value, and promotion has conditions. Both
compilers gate their capture the same way — the fork on
`!is_void_element && !has_spread && element.children.is_empty()` plus a
non-literal, non-confidently-foldable expression container, Babel through the
matching preprocessing — and where promotion does not happen the value is an
ordinary `children` prop or a deleted value in *both* compilers, so there is
nothing for a divergence to be about.

Only the spread condition needs to be written into the predicate, and that is a
property of the census rather than an oversight. With a spread the producer
still censuses the `children` member as a **child** claiming `ReactiveRerun`
(`semantic_trace.rs` gates the child kind on
`has_spread || element.children.is_empty()`), because at runtime `spread()`
really does assign it as the element's children through a `mergeProps` getter —
so by every signal the positive-lowering test can read it looks exactly like a
promotion. But Babel's `processSpreads` consumes the attribute into the merged
props before its promotion capture runs, and the fork lists "a spread keeps
`children` in the merged props" among the shapes the two compilers already
agree on (its docs/execution-contract.md, divergence 4). Without the gate, both
`<noscript {...props} children={note()}/>` and its nested spelling claimed
SC1001 uncertifiable with divergence-3 wording over a value that executes,
deferred, in either compiler's output.

Every *other* promotion condition is already refused by the positive-lowering
test, because each one makes the producer census or resolve the value as
something other than a lowered child: a void element's `children` attribute is
`native-attribute`/`elided` (the census gates the child kind on
`!is_void_element` for the same reason lowering does), a shadowed one is too
(arm 3), a confidently foldable one is resolved `elided` by the attribute
planner (arm 2), and a duplicate the name-first dedup discards is an elided
value span. The *parity-target-only* void tags stay divergent through this arm
and correctly so: the 1.x fork does not treat `<keygen>`/`<menuitem>` as void,
so it promotes and lowers where 1.x's Babel skips `transformChildren` entirely.

## Two more funnels: deleted code reaching rules that read no execution facts

The discarded-region projection covers the channels a read, write, action,
async read, contract callback or owner requirement flows through. Two rules
reached their verdict without consulting execution facts at all, and both
reported **proven violations about deleted code** until they were gated:

- **SC1003 (`static_rules.rs`)** — the destructure-freshness rule tests the
  role against an allowlist of *fresh-at-call-time* contexts, and
  `DiscardedRendering` is not one of them, so a destructure inside a deleted
  value fell through to "this component never updates". It is gated by a
  positive discarded-region lookup and an early return, deliberately **not** by
  adding the role to the allowlist: that list means "legal because fresh", and
  a deleted destructure is not legal-because-fresh, it is absent.
- **SC3001 (`cleanup.rs`)** — `leaf_owner_operations_for_file` is entirely
  lexical, so `onCleanup` inside an `onSettled` inside a deleted value was
  reported as "these nested primitives are never disposed" about a disposal
  that never comes due. Gated on the owner call, this pass's single entry:
  deletion travels down from there, so one check covers every operation the
  pass would record, including a leaf callback resolved in another file.

Both arms use the *nested* `<noscript children={…}/>` spelling because it is
the cleanest discarded region available — the capture is dropped rather than
promoted, both compilers emit nothing, and unlike arm 3's shadowed spelling it
draws no `tsc` diagnostic, so the checker's silence there is its own claim
rather than deference to TS2710.

The async-read selection in `projection.rs` was gated in the same pass, with
its `DiscardedRendering` exclusion ordered *ahead* of the
`read.leaf_owner.is_some()` short-circuit; that one is pinned by a unit test
rather than an arm here, and is defensive as much as reachable (see
`docs/precision-backlog.md`).

## Cases

| function / binding | verdict |
| --- | --- |
| `NestedChildrenAttributePromoted` | **silent** — certified, ordinary tracked child |
| `LiteralChildrenAttributeStaysSilent` | **silent** — never promoted, no reactive read |
| `SourceChildrenShadowChildrenAttribute`, `ignored()` | **silent** — a discarded region: both compilers delete the shadowed attribute |
| `SourceChildrenShadowChildrenAttribute`, `visible()` | **silent** — ordinary certified tracked read |
| `NoscriptPromotedChildrenAttribute`, `note()` | SC1001 **uncertifiable** — promoted, lowered here, emitted nowhere by Babel |
| `CleanupOwnedByPromotedChild` | **silent** — owned by the promoted insert |
| `SpreadKeepsChildrenInMergedProps`, `note()` | **silent** — a spread promotes nothing; both compilers keep it in the merged props |
| `NestedSpreadKeepsChildrenInMergedProps`, `note()` | **silent** — same, and position decides nothing once a spread is present |
| `DestructureInsideADeletedChildrenValue` | **silent** — SC1003's funnel gated on the discarded region |
| `DestructureAtComponentBodyScope` | SC1003 **violation** — the live control for the arm above |
| `LeafOwnerInsideADeletedChildrenValue` | **silent** — SC3001's funnel gated on the discarded region |
| `LeafOwnerInsideAPromotedChildrenValue` | SC3001 **violation** — the live control: the same call in a promoted, lowered value |

`Root` gives every component above an enumerable call site. Without it each
read would be uncertifiable for the unrelated reason that the component's
callers are unknown, and the fixture would pass while proving nothing.

The two destructure arms are passed `tick()` rather than a literal, and that is
load-bearing: with every caller enumerable and every caller passing a literal,
SC1003's caller-proven gate answers `PropUse::Static` and the rule returns
*before* the execution role is consulted, so both arms would be silent for a
reason unrelated to discarded regions and the deleted-value arm would pin
nothing. Verified by disabling each gate and confirming the arm fires: gates
off, this fixture reports seven findings (the three below plus SC1001 on both
spread spellings, SC1003 on the deleted destructure, SC3001 on the deleted leaf
owner); gates on, three.

## `SourceChildrenShadowChildrenAttribute` draws a real `tsc` diagnostic, and that is the point

`<span children={ignored()}>{visible()}</span>` is TS2710 — *"'children' are
specified twice. The attribute named 'children' will be overwritten"* —
against the real published `solid-js@2.0.0-rc.0` / `@solidjs/web@2.0.0-rc.0`
typings, in both `strict` and `loose` (checked with `scripts/tsc-oracle.mjs`,
not assumed; see "Oracle check" below). That diagnostic is the *whole* defect
at this span, and it is TypeScript's. The absolute rule in AGENTS.md is what
makes this arm's silence mandatory rather than merely acceptable: the checker
has nothing of its own to add about a shadowed `children` attribute, so it must
not add anything. It is not a case
`jsx-no-duplicate-props` (SC8003) needs to avoid duplicating by accident
either: its shared implementation
(`rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_syntax.rs`,
`only_the_children_pair`) already narrows away exactly this pair —
"the `children`-prop plus JSX-children pair alone is left to TS2710"
(`docs/rules/jsx-no-duplicate-props.md`).

## Dialect

`node_modules/solid-js/package.json` pins `2.0.0-rc.0`; there is no 1.x
sibling because the pin this fixture is about —
`dom-expressions-compiler` — is 2.0-only. Solid 1.x is built by its own fork,
`solid1-dom-expressions-compiler`, pinned separately in `rust/Cargo.toml` and
untouched by this move. Its `.gitignore` exception lines are
`!fixtures/reactive-ir/jsx-nested-children-attribute-*/node_modules/` and its
`/**` twin. (The discarded-region projection is *not* 2.0-only — both adapters
changed together — and the 1.x half is pinned by the 1.x adapter's own unit
test, `deleted_values_are_discarded_regions_rather_than_untracked_ones`.)

## Stub faithfulness

`solid-js.d.ts` declares only `createSignal`, `onCleanup`, `onSettled`, and the
three intrinsic elements this fixture uses. `children?: unknown` is wider than the
real `children?: Element | undefined`
(`Element = RenderedElement | ArrayElement | (string & {}) | number | boolean
| null | undefined` in `solid-js@2.0.0-rc.0`), but every value written to it
here — number-returning accessor calls, a concatenation of two string
literals, and a comma expression ending in `null` — is one the real, narrower
type already accepts, so the width cannot manufacture a finding a real project
could not also produce.

`onCleanup`'s return type is declared `void`, narrower than the real
package's `Disposable`. A stub must never be looser than the real package in a
way a rule's proof depends on; nothing here reads the return value, so
narrowing it cannot hide a finding either.

`onSettled` is byte-faithful to `@solidjs/signals@2.0.0-rc.0`'s
`onSettled(callback: () => void | (() => void))`, the same signature
`fixtures/reactive-ir/leaf-owner/solid-js.d.ts` carries and for the same
reason: the leaf-owner arms' whole claim is which calls inside that callback
are reported, and a callback typed `() => unknown` would manufacture
cleanup-return defects no real project can produce. `noscript`'s `id?: string`
exists only so the spread arms have something real to spread; the published
typings accept it and everything else through `HTMLAttributes<HTMLElement>`, so
one optional string is strictly narrower.

**Why the ownership arm uses a comma expression.** `Disposable` is not
assignable to `Element`, so `children={onCleanup(() => {})}` alone is a `tsc`
error (TS2322) against the real typings — checked, not assumed. Writing that
arm directly would have required a stub looser than the real package, exactly
the trap AGENTS.md names, so the arm is
`children={(onCleanup(() => {}), null)}`, which the real typings accept and
which keeps the call inside the promoted child's own expression.

**Why the literal arm spells its value `{"a" + "b"}`.** A *literal-spelled*
nested `children` attribute is reported to crash the parity-target Babel plugin
outright in the fork's own parity harness, so there is no parity-verified
verdict to pin for that spelling. The observation and its provenance — an
adversarial review of this change, not a run in this repository, which has no
Babel harness — are recorded in `docs/precision-backlog.md` as a pre-existing
fork-repo divergence note. `"a" + "b"` is non-literal in spelling — so it reaches the
same code path a dynamic value would — and confidently foldable in value, and
both compilers agree on it.

## Oracle check

Checked, not assumed: this `App.tsx` was compiled by
`node scripts/tsc-oracle.mjs check --dialect v2 --file App.tsx` against the
real audited typings (`solid-js@2.0.0-rc.0`, `@solidjs/web@2.0.0-rc.0`,
`@solidjs/signals@2.0.0-rc.0`), with the fixture stub excluded. Exactly one
diagnostic, identical in `strict` and `loose`:

```
App.tsx TS2710: 'children' are specified twice. The attribute named 'children' will be overwritten.
```

at `SourceChildrenShadowChildrenAttribute`'s `<span children={ignored()}>{visible()}</span>`,
addressed above. No other line in this fixture — the promoted value, the
folded concatenation, the `<noscript>` arm, the ownership arm's comma
expression, the two spread arms, or the four deleted/live funnel arms — draws
any diagnostic in either pass, so nothing this checker says or stays silent
about here duplicates a `tsc` claim. That matters most for the two
deleted-value arms: their silence is the checker's own claim about deleted
code, not deference to a `tsc` diagnostic that happens to cover the same span.
