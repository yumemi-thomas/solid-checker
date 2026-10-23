// The usual entry: `render(() => App(), root)` as a module statement. Web's
// `render` ends with `flush()` on the caller's stack (`@solidjs/web` rc.17
// `dist/web.dev.js:567`), but a module statement has no owner, so the
// exemption keeps the claim. Positive: the memo in the apply, inside App.
import { createEffect, createMemo, createSignal } from "solid-js";
import { render } from "@solidjs/web";

declare const root: HTMLElement;

function App() {
  const [v] = createSignal(1);
  createEffect(v, () => {
    createMemo(() => 1);
  });
  return null;
}

render(() => App(), root);
