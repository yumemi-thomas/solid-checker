import { createSignal, createStore, untrack, children, Show, For, createMemo, onSettled, onCleanup, action, refresh, resolve, createEffect, Loading } from "solid-js";
import type { JSX } from "@solidjs/web";
import { mark } from "./mark";

function Sub(props: { v: number; children?: JSX.Element }) {
  const o = { *g() { mark("a"); yield props.v; } };
  o.g();
  return <button onClick={() => o.g().next()}>k</button>;
}
export function App() {
  const [s, set] = createSignal(1);
  return <><Sub v={s()}>{String(s())}</Sub><button id="bump" onClick={() => set(s() + 1)}>b</button><p id="ran-a">pending</p></>;
}
