import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(o: { key: () => number }) { return { read: (): number => o.key() }; }
export function App() { const [n] = createSignal(1); const x = useT({ key: () => n() }); log("r=" + x.read()); return <div />; }
