import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT(n: () => number) { return { read: (): number => n() }; }
export function App() { const [n] = createSignal(1); log("r=" + useT(n).read()); return <div />; }
