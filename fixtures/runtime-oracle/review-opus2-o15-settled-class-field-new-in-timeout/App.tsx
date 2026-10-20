import { createSignal, createStore, createMemo, onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
export function Leaf() {
  onSettled(() => {
    class K { x = onCleanup(() => {}); y = mark("a"); }
    setTimeout(() => new K(), 0);
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
