import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const Ctx = createContext<() => number>();
function useT() { const c = useContext(Ctx)!; return { read: (): number => c() }; }
function Child() { const x = useT(); log("r=" + x.read()); return <i />; }
export function App() { const [n] = createSignal(1); return <Ctx value={n}><Child /></Ctx>; }
