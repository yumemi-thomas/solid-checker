import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
const [c] = createSignal(1);
export function App() {
  const a: number[] = [2, 1];
  a.sort();
  return <div>{c()}</div>;
}
