import { state } from "./s";
import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  
  return <div><button onClick={() => { state.n = 5; }}>go</button></div>;
}
