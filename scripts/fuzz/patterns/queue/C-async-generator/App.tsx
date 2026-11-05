import { createEffect, createMemo, createSignal, createStore, onCleanup, untrack } from "solid-js";
import { render } from "@solidjs/web";
const [c] = createSignal(1);
async function* gen() { yield c(); }
export function App() {
  gen().next();
  return <div />;
}
