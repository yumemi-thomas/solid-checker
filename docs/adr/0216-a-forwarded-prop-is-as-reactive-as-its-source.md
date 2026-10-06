# ADR 0216: A forwarded prop is as reactive as its source

- Status: accepted and implemented (2026-10-06).
- Owners: `classify_passed_expression`, `classify_one_component`,
  `resolve_forwarded_props` and `classification_use`
  (`solid-reactive-ir/src/source_discovery.rs`).
- Relation: corrects the caller-witness proof of component props that the
  strict-read (`SC1001`), frozen-handler (`SC1007`) and destructure (`SC1003`)
  rules read through `PropsReactivityIndex::prop_use`.

## Context

A prop read untracked in a component body is a proven strict read only when
some call of the component passes a reactive value. The caller-witness proof
classifies each JSX attribute expression. It treated any expression rooted at
the caller's own `props` object as reactive:

```tsx
function Inner(props: { value: Item }) {
  const item = props.value; // was: proven SC1001
  return <main>{item.text}</main>;
}
function Outer(props: { value: Item }) {
  return <Inner value={props.value} />;
}
<Outer value={{ text: "plain" }} />;
```

The compiler writes a getter for `value={props.value}`, and that getter reads
`Outer`'s `value`, a plain object. Solid 2.0.0-rc.13 raises no
`STRICT_READ_UNTRACKED` here; the runtime check was run in Chrome on the
probus-hk install. The checker reported both reads in such a pair as proven
violations: false positives.

The finding was surfaced by a review of a recall patch
(`rust/target/research/g1g2/review-g1.md`). That patch would have reported
more reads through the same shortcut.

## Decision

1. **A forwarded prop is decided by the prop it forwards.** A member chain in
   an attribute expression rooted at the caller's props records the caller's
   props declaration and the chain head's prop name (a literal computed key is
   cooked; any other computed head is unknown). The expression itself is then
   classified from its other parts.
2. **The forwards are resolved once every component is classified.** The
   receiving prop:
   - is reactive when the forwarded prop is reactive;
   - is unresolved when that prop is unresolved, the parent escapes
     enumeration, or the parent has no classification;
   - otherwise adds nothing.
   This is a least fixpoint from "adds nothing". A prop is therefore proven
   static only when no chain of forwards reaches a reactive or unresolved
   value. Cycles add nothing beyond what their entries bring.
3. **A props object passed whole is unknown.** Which of its props the
   receiver reads, and whether they are live, is not followed.
4. **Only the props parameter itself forwards.** A `merge` result or a
   destructured binding attached to it is a view whose keys may come from
   another source (`merge(props, { get value() … })`), so a chain rooted at
   one is unknown.
5. **A static head certifies only an exact one-link forward.** `props.a` is
   static when the parent's `a` is. `props.a.b` reads `.b` of whatever `a`
   holds, which may be a getter or a store, so a static head makes it
   unknown.
6. **A store passed whole is unknown, not reactive.** Reading the prop hands
   over the proxy and reads no key.
7. **Several children are unresolved, never reactive.** The compiler builds
   an array whose dynamic entries are memo accessors, so reading
   `props.children` reads none of them. A single child is the getter's own
   expression and is classified as before.

## Consequences

- A prop forwarded from a plain value is no longer a proven strict read or
  frozen handler.
- A prop forwarded from a live value at a visible call site stays proven, at
  any depth.
- A prop forwarded from a parent whose own callers are not all visible, or
  whose call site passes something the proof cannot classify (a call through
  a context object), is uncertifiable. Some of these are real defects whose
  previous "proof" rested on the shortcut. Recovering them needs a witness
  for the upstream value, for example through context values.
- Still open: forwarding through `merge`, `omit` and `splitProps` results is
  decided by the same declaration as the props object they wrap.

## Evidence

- **Review:** an adversarial review of the first version
  (`rust/target/research/review-forwarded-prop.md`) found two new wrong clean
  results (a `merge` view, a multi-link chain) and two retained false
  positives (a whole store, several children). Decisions 4 to 7 close them.
  Kept for later: forwarded reads on a branch that never runs, a parent
  instance that never renders the child, and a forwarded accessor called in
  the child (uncertifiable, not proven).
- **Fixture** `fixtures/reactive-ir/forwarded-prop-backing`:
  - clean: a plain object and a string forwarded once;
  - proven violation: a signal read forwarded once;
  - uncertifiable: a forward from an exported parent, a props object passed
    whole, a `merge` view with a getter override, a multi-link chain through
    an object with a getter, a store passed whole, and several children.
- **Moved fixtures** (violation to uncertifiable): `fp-exec-true-defects`
  (`StopRow`'s `onAlight` and `onBoard`, forwarded from an exported `Route`;
  its `canAlight` condition stays a proven violation), `forwarded-event-prop`
  (`Direct`'s `onPress` through an exported `Chained`; every caller passes a
  function literal), and `prop-pass-through-dispatch` (`List` and `Mixed`).
- **Coverage:** 185 fixture projects. Only these and the new snapshot move.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-x-browser.json`: violations 285 to 195, with 86 distinct sites
  removed and none added. Uncertifiable rose 3,370 to 3,422, where the
  removed ones remain as proof obligations.
- **The removed violations, traced by hand**
  (`rust/target/research/removed-violations.md`, for the first version's 82):
  - 65 were false positives: plain values and stable callbacks forwarded
    through wrappers, including every frozen-handler and destructure case;
  - 14 distinct sites are real strict reads now left uncertifiable. Their
    live source is a router memo, a query hook's state, a context-held
    memo (`settings.lang()`), a `For` index accessor forwarded through four
    components, or a store reached through a returned object, and the proof
    cannot classify any of these. The shortcut had reported them without
    proving them;
  - 1 is unclear (a Storybook control).
