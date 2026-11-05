import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function pick(f: () => number) { return f(); }
function useT(n: Accessor<number>): { read: Accessor<number> } { return { read: () => pick(n) }; }
export function App() { const [n] = createSignal(1); const x = useT(n); log("r=" + x.read()); return <div />; }
