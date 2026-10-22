import { createEffect, createRoot, createSignal, runWithOwner, Show } from "solid-js";

type El = HTMLDivElement;
const noop = (element: El) => { void element; };
const [count] = createSignal(0);
const on: boolean = Math.random() < 2;

// Positive control: the right operand of `&&` is a value the ref receives.
export function RightOperand() {
  return <div ref={(on && ((element: El) => { createEffect(() => count(), () => { void element; }); })) || undefined} />;
}

// Uncertifiable: the left operand of `&&` is tested and never applied, since a function is always truthy.
export function LeftOperand() {
  return <div ref={(on ? (element: El) => { createEffect(() => count(), () => { void element; }); } : undefined) && noop} />;
}

// Show's children run inside Show's own memo. The arrow element is applied with
// no owner whatever renders it (a violation); the IIFE's call runs inside that
// memo, so its effect is uncertifiable, never a violation.
export function ShowChildren(props: { when: boolean }) {
  return (
    <Show when={props.when}>
      <div ref={(element: El) => { createEffect(() => count(), () => { void element; }); }} />
      <div ref={(() => { createEffect(() => count(), () => {}); return noop; })()} />
    </Show>
  );
}

// Positive: a runWithOwner(null) callback runs with no owner, so its effect does too.
export function DetachedEffect() {
  runWithOwner(null, () => { createEffect(() => count(), () => {}); });
  return <i />;
}

// Negative control: under createRoot's owner, the IIFE's effect is owned by the root.
export const rooted = createRoot(() => <div ref={(() => { createEffect(() => count(), () => {}); return noop; })()} />);

// Positive: exported and open, this callback runs with no owner, so the IIFE a
// ref inside it calls runs with none.
export function DetachedRef() {
  return runWithOwner(null, () => <div ref={(() => { createEffect(() => count(), () => {}); return noop; })()} />);
}

// Positive: at module initialization no owner is current, so the IIFE a
// module-scope wrapper calls, for a ref, runs with none.
export const moduleWrapped = (() => <div ref={(() => { createEffect(() => count(), () => {}); return noop; })()} />)();

// Uncertifiable: the ref's IIFE sits in a method whose call this graph does not
// resolve (`new Builder().build()` names no owner edge), so the owner of the code
// the ref runs is unknown, not proven.
class Builder {
  build() {
    return <div ref={(() => { createEffect(() => count(), () => {}); return noop; })()} />;
  }
}
export function Built() {
  return new Builder().build();
}
