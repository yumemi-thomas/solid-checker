import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useT() { const [n] = createSignal(1); return { read: (): number => n() }; }
export function App() { log("r=" + useT().read()); return <div />; }
