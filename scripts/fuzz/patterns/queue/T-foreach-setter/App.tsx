import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
export function App() {
  const [c, setC] = createSignal(0);
  [1].forEach(setC);
  return <div>{c()}</div>;
}
