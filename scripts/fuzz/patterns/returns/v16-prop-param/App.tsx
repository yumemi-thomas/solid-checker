import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(n: () => number) { return { read: (): number => n() }; }
function C(p: { n: () => number }) { const x = useT(p.n); log("r=" + x.read()); return <i />; }
export function App() { const [n] = createSignal(1); return <C n={n} />; }
