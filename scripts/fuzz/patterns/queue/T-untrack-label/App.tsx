import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
export function App() {
  const [c] = createSignal(1);
  console.log((untrack as any)(c, "x"));
  return <div />;
}
