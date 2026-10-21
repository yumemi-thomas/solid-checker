import { createEffect, createSignal, onCleanup, onSettled } from "solid-js";

const [count] = createSignal(0);

// Exported helpers in an open program: an unseen caller may supply no owner, so whether
// the leaf is owned is a caller obligation. A violation needs a proven unowned path.
export function helperCleanup() {
  onSettled(() => { onCleanup(() => {}); });
}

export function helperEffect() {
  onSettled(() => { createEffect(count, () => {}); });
}

export function helperNested() {
  // The inner onSettled is queued by whatever owner the outer leaf has, so its
  // callback is proven unowned, as a timer callback inside this helper would be.
  onSettled(() => { onSettled(() => { onCleanup(() => {}); }); });
}

export function helperShared() {
  onSettled(() => { createEffect(count, () => {}); });
}

// Control: an owned caller makes helperShared's leaf owned on that path, and the unseen
// caller keeps the other path open. Both claims stay uncertain.
export function Comp() {
  helperShared();
  return <i />;
}

function setup() {
  onSettled(() => { onCleanup(() => {}); });
}

// Control: an event handler is a proven unowned invocation, so setup's leaf cleanup is a
// violation on that path; the owned call keeps the owned path uncertain.
export function Mixed() {
  setup();
  return <div onClick={() => setup()} />;
}
