import { createSignal, createStore, untrack, children, Show, For, createMemo, onSettled, onCleanup } from "solid-js";
import type { JSX } from "@solidjs/web";
import { mark } from "./mark";

function Sub(props: { v: number; children?: JSX.Element }) {
  class K { x = (() => (mark("a"), props.v))(); }
  const k = new K();
  return <p>{k.x}</p>;
}
export function App() {
  const [s, set] = createSignal(1);
  return <><Sub v={s()}>{String(s())}</Sub><button id="bump" onClick={() => set(s() + 1)}>b</button><p id="ran-a">pending</p></>;
}
