import { createSignal, createStore, createMemo, merge } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
export function App() { const [s] = createStore({ a: 0 }); if (Math.random() > 2) s.a = 1; return <i />; }
