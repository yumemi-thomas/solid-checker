import { createSignal, createTrackedEffect, createMemo, createStore } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const [c] = createSignal(1);
export function App() { const [st, setSt] = createStore<number[]>([]); createTrackedEffect(() => { setSt(() => { log("ran"); return [c()]; }); }); return <i>{st.length}</i>; }
