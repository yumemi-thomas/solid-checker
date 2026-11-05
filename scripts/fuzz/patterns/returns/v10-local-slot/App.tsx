import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() { const [n] = createSignal(1); const o = { n }; log("r=" + o.n()); return <div />; }
