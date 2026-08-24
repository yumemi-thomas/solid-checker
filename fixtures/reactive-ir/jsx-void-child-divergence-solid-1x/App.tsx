import { createSignal, onCleanup } from "solid-js";

// The divergence. `<br>` cannot hold children in HTML, and the two compilers
// disagree about what that means for the expression written inside one:
//
//   this checker's pinned compiler fork -- nested native-child lowering walks
//   into the children unconditionally, so it emits a real reactive insert and
//   its census reports a `jsx-child` site resolved as `reactive-rerun`;
//
//   the compiler Solid ships (Babel) -- discards a void element's child list
//   in every position, so it emits nothing at all and the accessor is never
//   read.
//
// Both traces are truthful about their own compiler; neither is evidence about
// the other, and nothing available here says which one will build this
// project. So the read is uncertifiable in both directions: certifying it
// tracked would believe only the fork, and reporting it as a stale untracked
// read would believe only Babel.
//
// This is NOT the census gap its sibling fixture pins. There the compiler was
// silent; here the census entry is present and claims a reactive rerun -- which
// is exactly the claim that must not be believed. Detection is therefore
// positive, from this checker's own AST (`divergent_lowered_child` in
// rust/crates/solid-reactive-ir/src/execution_role.rs), never from the
// compiler's silence.
function NestedVoidChild() {
  const [count] = createSignal(0);
  return <div><br>{count()}</br></div>;
}

// The same void element at its own *template root*, and this is where the two
// producers stop agreeing -- probed, not assumed:
//
//   2.0 gates template-root child lowering on `!is_void_element`, emits
//   nothing, and agrees with Babel. The hole is a genuine hole, and the
//   ordinary census gap owns it (its own fixture is
//   fixtures/reactive-ir/jsx-census-gap-solid-2);
//
//   1.x lowers the children in this position too, so the site is reported and
//   this is the same divergence as above.
//
// Both dialects therefore answer *uncertifiable* and only the reason differs,
// which is exactly why this pair keeps byte-identical sources: the snapshots
// differ in three rows (this message, plus the keygen/menuitem findings that
// exist only under 1.x), and those differences are the whole claim. A mitigation
// that keyed on the tag alone would give both the divergence wording and lie
// about 2.0; one that keyed on census silence would give both the gap wording
// and lie about 1.x.
function RootVoidChildDependsOnTheProducer() {
  const [total] = createSignal(0);
  return <br>{total()}</br>;
}

// Negative: a void element's *attribute*. Attributes are not children -- both
// compilers lower an attribute, an event handler and a `ref` on a void element
// in every position and keep their sites -- so this read is certified tracked
// and the checker stays silent.
function VoidAttributeStaysCertified() {
  const [id] = createSignal("i");
  return <div><br id={id()} /></div>;
}

// Negative: an ordinary censused tracked child sitting between two void
// elements. Nothing about being adjacent to a void element makes a read
// uncertifiable; only being *inside one's children* does. This is what the
// mitigation must not start reporting.
function AdjacentTrackedChildStaysCertified() {
  const [name] = createSignal("n");
  return <div><br /><span>{name()}</span><hr /></div>;
}

// The `<noscript>` arm: the fork's parity divergence 3, and the same claim by a
// different route. `<noscript>`'s markup is inert, so the compiler Solid ships
// never lowers its children in any position. This fork drops them only on the
// static-template fast path; where the `<noscript>` is its own template root it
// emits `_$insert(_el$, a)` and censuses a `reactive-rerun` site.
//
// Note what is *not* said: "the shipped compiler deletes it". It does not
// delete a child it kept -- it never lowers the subtree at all. Same verdict,
// different reason, so the message says the different reason.
//
// `<noscript>` is deliberately NOT a member of the checker's void-element list:
// that list is byte-checked against the *producers'* own `void_elements` and
// `<noscript>` is not in it. It has an ordinary content model and diverges for
// an unrelated reason. Two named conditions, one predicate. (The third input to
// that predicate is the dialect's parity-target-only extras -- see the
// `<keygen>` arm below -- which is a separate list for a separate reason.)
function RootNoscriptChild() {
  const [a] = createSignal(0);
  return <noscript>{a()}</noscript>;
}

// The other position where this fork keeps a `<noscript>`'s children: nested,
// but with attributes that force it off the static-template fast path. A
// *dynamic* attribute does that; a static one does not, which is why this case
// uses `id={tag()}` -- probed, not assumed. On the fast path the recorder
// retracts the discarded sites instead, and that shape is an ordinary census
// gap; `fixtures/reactive-ir/jsx-census-gap-solid-2` holds it, because the 1.x
// producer fails reconciliation outright there and cannot host it.
//
// `tag()` in the attribute is a second, silent assertion: a `<noscript>`'s
// attributes are not its children. Both compilers lower an attribute on it in
// every position and keep the site, so that read stays certified tracked.
function NestedNoscriptOffTheFastPath() {
  const [body] = createSignal(0);
  const [tag] = createSignal("t");
  return <div><noscript id={tag()}>{body()}</noscript></div>;
}

// The void list is not one list, and this is the pair of tags that proves it
// cannot be. `<keygen>` and `<menuitem>` are void in the Babel plugin Solid
// *1.x* ships -- its `VoidElements.ts` holds 16 tags and gates the whole child
// pass on `if (!voidTag)` -- and not void in either Rust producer, whose
// `void_elements` holds 14. So under 1.x the producer lowers this child while
// the parity target deletes it: the same divergence as `<br>` above, reached
// through a tag the shared list does not name.
//
// Under 2.0 the same source is *certified*, and that is not an omission. 2.0's
// parity target imports the runtime's 14-tag `VoidElements` set, having dropped
// these two deliberately (its babel-plugin-jsx CHANGELOG records the removal),
// so both compilers lower the child and the read really is tracked. A single
// union list would report here under 2.0 and withhold a certification the facts
// support; a shared list with no dialect seam certifies the 1.x case by silence.
// The extras are dialect vocabulary --
// `Dialect::parity_target_only_void_elements`, answered by each dialect from its
// own parity target's file.
function NestedKeygenChild() {
  const [keyed] = createSignal(0);
  return <div><keygen>{keyed()}</keygen></div>;
}

// The other tag, in the other position. 1.x lowers a void element's children at
// template root as well, and the parity target's gate is position-independent,
// so this is the divergence under 1.x too -- and still certified under 2.0,
// where nothing treats `<menuitem>` as void.
function RootMenuitemChild() {
  const [item] = createSignal(0);
  return <menuitem>{item()}</menuitem>;
}

// The ownership consumer of the same divergence, and the reason it needs its own
// arms: the escalation is not only about whether a read is tracked.
//
// The pinned fork wraps the insert it emits for a divergent child in its default
// effect wrapper, so the producer reports an `Owned` ownership region there.
// `owners.rs` must not read that region as proof of an owner -- the parity target
// emits neither the insert nor its wrapper -- and it does not. But dropping the
// region is only half the answer: where the surrounding context is *proven
// unowned* (module scope), dropping it leaves the requirement standing, and
// SC4001 then reports a **proven violation** about an operation neither compiler
// leaves unowned. Under the fork the call runs under the insert's owner; under
// the parity target it sits in deleted code and never runs at all.
//
// So the requirement is uncertifiable here, worded as the disagreement it is.
// The same funnel carries every owner operation, `createEffect` included; this
// arm uses `onCleanup` because 2.0's two-argument `createEffect` would report an
// unrelated signature violation and this source is byte-shared with the 1.x
// sibling.
//
// The comma expression is not decoration. `onCleanup` returns `() => void` under
// 1.x and `Disposable` under 2.0, and neither is a `JSX.Element`, so
// `{onCleanup(...)}` alone is a *type error* against the real published typings
// -- checked, not assumed. A fixture stub loose enough to accept it would
// manufacture a shape no real project can write.
export const CleanupInsideADivergentChild = (
  <div><br>{(onCleanup(() => {}), null)}</br></div>
);

// Negative, and the one that shows the escalation is positional and narrow: the
// identical call one tag over, inside a `<span>` both compilers lower. The
// producer's `Owned` region there is evidence in both compilers, so the call is
// owned and the checker stays silent.
export const CleanupInsideACertifiedChild = (
  <div><span>{(onCleanup(() => {}), null)}</span></div>
);

// Negative, and the one that proves nothing was weakened: an unowned cleanup at
// module scope with no JSX anywhere near it stays a **proven violation**. The
// divergence removes a proof; it does not excuse an operation that never had one.
onCleanup(() => {});

// One exported root giving every component above an enumerable call site.
// Without it each read would be uncertifiable for the unrelated reason that
// the component's callers are unknown, and the fixture would pass while
// proving nothing about the divergence.
export function Root() {
  return (
    <div>
      <NestedVoidChild />
      <RootVoidChildDependsOnTheProducer />
      <VoidAttributeStaysCertified />
      <AdjacentTrackedChildStaysCertified />
      <RootNoscriptChild />
      <NestedNoscriptOffTheFastPath />
      <NestedKeygenChild />
      <RootMenuitemChild />
    </div>
  );
}
