import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(s: { v: number }): { read: Accessor<number> } { return { read: () => s.v }; }
export function App() { const [s] = createStore({ v: 1 }); const x = useT(s); log("r=" + x.read()); return <div />; }
