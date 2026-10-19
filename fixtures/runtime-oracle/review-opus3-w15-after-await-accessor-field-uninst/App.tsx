import { createSignal, createStore, untrack, children, Show, For, createMemo, onSettled, onCleanup, action, refresh, resolve, createEffect, Loading } from "solid-js";
import type { JSX } from "@solidjs/web";
import { mark } from "./mark";

function Sub(props: { v: number; children?: JSX.Element }) {
  const [a] = createSignal(0);
  const m = createMemo(async () => { await Promise.resolve(); class K { x = a(); } return 1; });
  return <Loading fallback="l"><p>{m()}</p></Loading>;
}
export function App() {
  const [s, set] = createSignal(1);
  return <><Sub v={s()}>{String(s())}</Sub><button id="bump" onClick={() => set(s() + 1)}>b</button><p id="ran-a">pending</p></>;
}
