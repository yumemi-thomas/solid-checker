import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
class Late extends Array<number> { sort(): this { console.log(c()); return this; } }
const [c] = createSignal(1);
export function App() {
  const a: number[] = new Late();
  a.sort();
  return <div />;
}
