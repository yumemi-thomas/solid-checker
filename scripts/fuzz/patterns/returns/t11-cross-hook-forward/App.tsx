import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useA(n: () => number) { return { read: (): number => n() }; }
function useB(n: () => number) { const a = useA(n); return { read: (): number => a.read() }; }
export function App() { const [n] = createSignal(1); const x = useB(n); log("r=" + x.read()); return <div />; }
