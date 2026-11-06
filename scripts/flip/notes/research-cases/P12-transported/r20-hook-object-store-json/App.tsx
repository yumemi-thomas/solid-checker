import { createSignal, createMemo, createStore, createEffect, createContext, useContext, For, Show } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1, o: { y: 2 }, list: [{ id: 1 }] });
const useC = () => c;
const useS = () => st;
const id = <T,>(x: T): T => x;
const Ctx = createContext<() => number>(() => 0);
const SCtx = createContext<typeof st>(st);
const useH = () => ({ s: st });
export function App() {
  const h = useH();
  const v = JSON.stringify(h.s);
  return <p>{v}</p>;
}
