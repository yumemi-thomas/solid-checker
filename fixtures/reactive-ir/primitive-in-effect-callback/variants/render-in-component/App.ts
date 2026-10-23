// `render()` called inside a function body (here a component that mounts a
// subtree) flushes on that function's stack, whatever owner is current there:
// the exemption does not apply, so SC4005 cannot prove the apply is unowned.
import { createEffect, createMemo, createSignal } from "solid-js";
import { render } from "@solidjs/web";

declare const root: HTMLElement;

function Inner() {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  return null;
}

function Panel() {
  render(() => Inner(), root);
  return null;
}

Panel();
