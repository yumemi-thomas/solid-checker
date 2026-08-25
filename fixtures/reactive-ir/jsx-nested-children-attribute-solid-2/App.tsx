import { createSignal, onCleanup, onSettled } from "solid-js";

// The pin this fixture exists for: `dom-expressions-compiler` moved from
// c6008f01 to fea62adb (upstream PR #3, "nested children attribute
// promotion"). Before that move, `<div><span children={x()} /></div>` made
// the fork's own census name a `jsx-child` site that nested lowering never
// resolved -- a hard reconciliation failure, so `solid-checker-rust` exited 2
// and the file was never analyzed at all (docs/precision-backlog.md,
// "Divergence 4 stays a hard reconciliation failure by design"). There was no
// checker-side mitigation to test, because there was nothing to certify or
// mark uncertain -- the process never got that far.
//
// At the new pin `lower_dynamic_native_child` performs the same promotion
// `lower_dom_element` already did at a template root, so the file compiles,
// the site resolves as an ordinary `jsx-child` / `reactive-rerun`, and this
// checker's existing tracked-JSX machinery certifies it exactly as it would
// any other nested child -- no new rule, no new mitigation, because the
// divergence this checker used to have nothing to say about no longer exists.
//
// Positive: the promoted value is read exactly like an ordinary tracked JSX
// child. Silent is the correct verdict here, and it is a meaningful one: had
// this shape still been treated as divergent or otherwise uncertain, it would
// have surfaced as an SC1001 uncertainty finding the way the void-element and
// `<noscript>` divergences do
// (fixtures/reactive-ir/jsx-void-child-divergence-solid-2). It does not,
// because resolving upstream PR #3 removed the divergence rather than adding
// a case to it.
function NestedChildrenAttributePromoted() {
  const [content] = createSignal(0);
  return (
    <div>
      <span children={content()} />
    </div>
  );
}

// Negative: a `children` attribute whose value the constant fold resolves
// confidently is never promoted -- Babel's own capture only accepts a
// non-literal expression, and this fork's `captured_child` filter mirrors
// that (`evaluate_confident(...).is_none()`). The attribute stays where it
// always was, an ordinary constant-folded property write, and carries no
// reactive read for any rule to reach.
//
// The spelling is `{"a" + "b"}`, not a bare `{"a literal string"}`, and that
// is a parity constraint rather than a style choice: a *literal-spelled*
// nested `children` attribute crashes the parity-target Babel plugin outright
// (recorded in docs/precision-backlog.md as a pre-existing fork-repo
// divergence note), so there is no parity-verified verdict to pin for it. A
// concatenation of two literals is non-literal in spelling and confidently
// foldable in value, which is exactly the case both compilers agree on.
function LiteralChildrenAttributeStaysSilent() {
  return (
    <div>
      <span children={"a" + "b"} />
    </div>
  );
}

// The with-source-children shape: unaffected by this pin, and the fixture
// pins that explicitly rather than assuming it. `captured_child` is gated on
// `child.children.is_empty()`, a condition this shape never satisfies either
// side of PR #3, so the attribute never reaches the promotion logic the diff
// added -- it falls through to the ordinary attribute-lowering path exactly
// as it did at the old pin, reported as a `native-attribute` site resolved
// `elided` (docs/execution-contract.md, divergence 4: "with source
// children... both compilers insert only `y` and report the attribute as
// `native-attribute`/elided").
//
// "Elided" is a real fact, and what it says is that the value is **deleted**:
// `plan_attributes` drops the shadowed attribute and nothing is ever lowered
// for it, in this fork or in Babel (both insert only the source child). So
// `ignored()` is **silent**, and that is the whole claim of this arm. It used
// to be pinned here as an SC1001 proven violation, which was wrong in every
// clause -- "the read sees the current value once and never updates" asserts
// a read that happens, and this one does not happen in either compiler's
// output. Deleted code is projected as a *discarded* region, distinct from an
// untracked one (`ExecutionRole::DiscardedRendering`); see
// docs/precision-backlog.md, 2026-08-24.
//
// Silence here is not a certification either. Nothing about this span is
// proven safe -- it is proven absent, which is a different thing and licenses
// no positive claim (no rerun, no owner, no satisfied reader).
//
// The real defect at this span belongs to TypeScript, and the absolute rule
// says leave it there: `<span children={ignored()}>{visible()}</span>` is
// TS2710 ("'children' are specified twice. The attribute named 'children'
// will be overwritten") against the real published typings -- checked with
// scripts/tsc-oracle.mjs, not assumed; see the README. `visible()`, the real
// JSX child, stays an ordinary certified tracked read.
function SourceChildrenShadowChildrenAttribute() {
  const [ignored] = createSignal(0);
  const [visible] = createSignal(0);
  return (
    <div>
      <span children={ignored()}>{visible()}</span>
    </div>
  );
}

// Current-next control: this shape is no longer maintained as a checker-side
// transform divergence. The producer trace decides whether the promoted value
// is live; missing facts use the ordinary census-gap path rather than a
// tag-specific mitigation.
function NoscriptPromotedChildrenAttribute() {
  const [note] = createSignal(0);
  return <noscript children={note()} />;
}

// The ownership half of the same claim. The fork wraps the insert it emits
// for the promoted value in its default effect wrapper exactly as it does for
// any other nested child, so a cleanup registered inside the promoted
// expression is owned by that insert -- not left standing the way an
// unowned-context requirement would. Compare
// `CleanupInsideACertifiedChild` in the sibling void-element fixture, which
// pins the same shape of claim (owned exactly like an ordinary tracked
// child) for a divergence that *is* still open; this is the same proof for
// the one that just closed.
//
// The comma expression is not decoration: `onCleanup` returns `Disposable`
// under this dialect's real published typings, which is not a `JSX.Element`,
// so `children={onCleanup(() => {})}` alone is a `tsc` error (TS2322) against
// the real `solid-js@2.0.0-rc.0` typings. `(onCleanup(() => {}), null)`
// evaluates to `null`, which the real `Element` union accepts, and keeps the
// call inside the promoted child's expression.
export const CleanupOwnedByPromotedChild = (
  <div>
    <span children={(onCleanup(() => {}), null)} />
  </div>
);

// The boundary of the divergence arm above, and the shape that showed the arm
// had to have one: a spread means the attribute was never promoted, so nothing
// about it diverges.
//
// The census here is genuinely a *child* entry claiming a rerun --
// `semantic_trace.rs` gates the `children` census kind on
// `(has_spread || element.children.is_empty())`, so the spread member is
// censused `JsxChild`/`ReactiveRerun` -- and that is truthful: at runtime
// `spread()` assigns it as the element's children through a `mergeProps`
// getter. But Babel's `processSpreads` consumes the attribute into the merged
// props before its promotion capture ever runs, and the fork's own contract
// lists "a spread keeps `children` in the merged props" among the shapes the
// two compilers already agree on (its docs/execution-contract.md, divergence
// 4, "the shapes that already agreed still do"). Both compilers keep the
// value, both execute it deferred, so there is nothing to be uncertain about
// and the read is certified.
//
// The gate is written into `promoted_children_attribute_value` rather than
// left to the positive-lowering test because this is the one promotion
// condition the census cannot express: every *other* one -- a void element, a
// shadowing source child list, a confidently foldable value, a name-first
// dedup loser -- makes the producer census or resolve the value as something
// other than a lowered child, which the positive-lowering test already
// refuses. Without the spread gate both spellings below claimed SC1001
// uncertifiable with divergence-3 wording.
function SpreadKeepsChildrenInMergedProps(props: { id: string }) {
  const [note] = createSignal(0);
  return <noscript {...props} children={note()} />;
}

// The nested spelling of the same thing. Position is what decides the
// no-spread case (promoted-and-lowered at a template root, capture discarded
// when nested), and it decides nothing here: with a spread neither position
// promotes, so both are certified.
function NestedSpreadKeepsChildrenInMergedProps(props: { id: string }) {
  const [note] = createSignal(0);
  return (
    <div>
      <noscript {...props} children={note()} />
    </div>
  );
}

// Deleted code reaches rules that never consult execution facts, and these two
// arms pin the two funnels that were fixed for it. Both use the nested
// `<noscript children={...}/>` spelling because it is the cleanest discarded
// region available: the capture is dropped rather than promoted (promoting
// would emit an insert Babel does not), both compilers emit nothing, and --
// unlike the shadowed-`children` spelling above -- it draws no `tsc`
// diagnostic of its own, so the checker's silence here is its own claim rather
// than deference to TS2710.
//
// SC1003's claim is that a destructure's bindings are read once, outside
// tracking, and then never update. Inside a deleted value the read does not
// happen, so the claim is false in every clause -- the same defect the
// projection fix corrected for SC1001, in a rule that reaches the role
// allowlist instead of the read table. `DiscardedRendering` is *not* in that
// allowlist and must not be added to it: the list means "fresh at call time,
// therefore legal", which is a different claim about a different situation.
function DestructureInsideADeletedChildrenValue(props: { hidden: number }) {
  return (
    <div>
      <noscript
        children={(() => {
          const { hidden } = props;
          return hidden;
        })()}
      />
    </div>
  );
}

// The live control for it, in the same file so the arm above cannot pass by
// the rule being off: the identical destructure at component-body scope is
// still reported.
function DestructureAtComponentBodyScope(props: { shown: number }) {
  const { shown } = props;
  return <div>{shown}</div>;
}

// The leaf-owner funnel. `onCleanup` inside `onSettled` is SC3001 -- "these
// nested primitives are never disposed" -- and inside a deleted value there is
// no leaf scope to open and no disposal to come due. The pass is entirely
// lexical (`leaf_owner_operations_for_file` reads no execution facts at all),
// which is why the gate is a positive discarded-region lookup on the owner
// call rather than a role: deletion travels down from the owner call, so one
// check covers every operation the pass would otherwise record, including a
// leaf callback resolved in another file.
function LeafOwnerInsideADeletedChildrenValue() {
  return (
    <div>
      <noscript
        children={
          (onSettled(() => {
            onCleanup(() => {});
          }),
          null)
        }
      />
    </div>
  );
}

// Its live control, and the pair that isolates deletion as the only
// difference: the same call in a *promoted* `children` value -- lowered by
// `lower_dom_element` into a real insert -- is an SC3001 proven violation.
function LeafOwnerInsideAPromotedChildrenValue() {
  return (
    <span
      children={
        (onSettled(() => {
          onCleanup(() => {});
        }),
        null)
      }
    />
  );
}

// One exported root giving every component above an enumerable call site.
// Without it each read would be uncertifiable for the unrelated reason that
// the component's callers are unknown, and the fixture would pass while
// proving nothing about the pin.
//
// The two destructure arms are passed a *reactive* prop deliberately. With
// every caller enumerable and every caller passing a literal, SC1003's
// caller-proven gate answers `PropUse::Static` and the rule returns before it
// ever reaches the execution role -- so both arms would be silent for a reason
// that has nothing to do with discarded regions, and the deleted-value arm
// would pin nothing. `shown={tick()}` makes the prop provably reactive, which
// is what puts the role test on the path.
export function Root() {
  const [tick] = createSignal(0);
  return (
    <div>
      <NestedChildrenAttributePromoted />
      <LiteralChildrenAttributeStaysSilent />
      <SourceChildrenShadowChildrenAttribute />
      <NoscriptPromotedChildrenAttribute />
      <SpreadKeepsChildrenInMergedProps id="a" />
      <NestedSpreadKeepsChildrenInMergedProps id="b" />
      <DestructureInsideADeletedChildrenValue hidden={tick()} />
      <DestructureAtComponentBodyScope shown={tick()} />
      <LeafOwnerInsideADeletedChildrenValue />
      <LeafOwnerInsideAPromotedChildrenValue />
    </div>
  );
}
