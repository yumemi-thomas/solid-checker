import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(o: { a: { b: () => number } }): { read: Accessor<number> } { return { read: () => o.a.b() }; }
export function App() { const [n] = createSignal(1); const x = useT({ a: { b: n } }); log("r=" + x.read()); return <div />; }
