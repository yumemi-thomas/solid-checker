import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(n: () => number) { const d = createMemo(() => n() + 1); return { read: (): number => d() }; }
export function App() { const [n] = createSignal(1); const x = useT(n); log("r=" + x.read()); return <div />; }
