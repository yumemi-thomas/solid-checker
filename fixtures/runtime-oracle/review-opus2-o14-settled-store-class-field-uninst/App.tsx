import { createSignal, createStore, createMemo, onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
export function Leaf() {
  const [st] = createStore({ a: 1 });
  class K { x = [1].map(() => st.a); }
  mark("a");
  return <button onClick={() => new K()}>{st.a}</button>;
}
export function App() { return <p id="ran-a">pending</p>; }
