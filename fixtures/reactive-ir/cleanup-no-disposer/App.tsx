// ADR 0695: a cleanup's read is silent only while nothing in the project can
// dispose an owner on a stack of its own. Here nothing can: no `getOwner`, no
// `createRoot` that takes `dispose`, and `render`'s result is dropped. Chrome is
// quiet. The twin holding a disposer is fixture `walk-bounds`.
import { createRoot, createSignal, onCleanup, onSettled, untrack } from "solid-js";
import { render } from "@solidjs/web";

const [c] = createSignal(1);

export function PlainCleanup() {
  onCleanup(() => {
    void c();
  });
  return <i />;
}

export function RootWithoutHandle() {
  createRoot(() => {
    onCleanup(() => {
      void c();
    });
  });
  return <i />;
}

// ADR 0695: the primitive runs these with the window closed.
export function Closed() {
  const first = untrack(() => c());
  onSettled(() => {
    void c();
  });
  return <i>{first}</i>;
}

render(() => <PlainCleanup />, document.body);
