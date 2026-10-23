// `hydrate()` reaches `render()` (`@solidjs/web` rc.17 `dist/web.dev.js:1644`,
// `:1732`) and so `flush()`; called inside a function body it withholds the
// ownerless-apply proof as `render()` does.
import { createEffect, createMemo, createSignal } from "solid-js";
import { hydrate } from "@solidjs/web";

declare const root: HTMLElement;

function Inner() {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  return null;
}

function Panel() {
  hydrate(() => Inner(), root);
  return null;
}

Panel();
