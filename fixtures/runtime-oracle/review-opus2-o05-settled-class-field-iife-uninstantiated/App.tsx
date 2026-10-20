import { createSignal, createStore, createMemo, onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
export function Leaf() {
  onSettled(() => {
    mark("a");
    class K { x = (() => onCleanup(() => {}))(); }
    void K;
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
