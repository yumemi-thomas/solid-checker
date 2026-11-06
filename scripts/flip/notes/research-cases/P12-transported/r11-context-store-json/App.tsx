import { createSignal, createMemo, createStore, createEffect, createContext, useContext, For, Show } from "solid-js";
const [c] = createSignal(1);
const [st] = createStore({ a: 1, o: { y: 2 }, list: [{ id: 1 }] });
const useC = () => c;
const useS = () => st;
const id = <T,>(x: T): T => x;
const Ctx = createContext<() => number>(() => 0);
const SCtx = createContext<typeof st>(st);
function Ch() { const g = useContext(SCtx); const v = JSON.stringify(g); return <i>{v}</i>; }
export function App() { return <SCtx value={st}><Ch /></SCtx>; }
