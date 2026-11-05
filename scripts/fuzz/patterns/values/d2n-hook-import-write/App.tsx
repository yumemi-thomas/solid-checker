import { useThing } from "./s";
import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const t = useThing();
  return <div><button onClick={() => { t.state.n = 5; }}>go</button></div>;
}
