import { createSignal, createStore, untrack, children, Show, For, createMemo, onSettled, onCleanup, action, refresh, resolve, createEffect, Loading } from "solid-js";
import type { JSX } from "@solidjs/web";
import { mark } from "./mark";
const reg: Array<new () => unknown> = []; function register(k: new () => unknown) { reg.push(k); }
function Sub(props: { v: number; children?: JSX.Element }) {
  register(class { x = (mark("a"), props.v); });
  return <button onClick={() => new reg[0]()}>k</button>;
}
export function App() {
  const [s, set] = createSignal(1);
  return <><Sub v={s()}>{String(s())}</Sub><button id="bump" onClick={() => set(s() + 1)}>b</button><p id="ran-a">pending</p></>;
}
