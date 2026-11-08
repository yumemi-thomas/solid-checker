import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
const [c] = createSignal(1);
const pair = () => [c] as const;
export function App() {
  console.log(pair()[0]());
  return <div />;
}
