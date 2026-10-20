import { createSignal, createStore, untrack, children, Show, For, createMemo, onSettled } from "solid-js";
import type { JSX } from "@solidjs/web";
import { mark } from "./mark";

function Sub(props: { v: number; children?: JSX.Element }) {
  const [st] = createStore({ a: 1 });
  const g = (function* () { mark("a"); yield st.a; })();
  return <p>{[...g].join()}</p>;
}
export function App() {
  const [s, set] = createSignal(1);
  return <><Sub v={s()}>{String(s())}</Sub><button id="bump" onClick={() => set(s() + 1)}>b</button><p id="ran-a">pending</p></>;
}
