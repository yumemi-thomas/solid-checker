import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(set: (v: number) => void) { return { bump: (): void => { set(1); } }; }
export function App() { const [n, setN] = createSignal(1); const x = useT(setN); createMemo(() => { x.bump(); return 1; }); return <div />; }
