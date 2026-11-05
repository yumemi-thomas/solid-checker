import { createSignal, createTrackedEffect, createMemo, createStore } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const [c] = createSignal(1);
export function App() { const [n, setN] = createSignal(0); createTrackedEffect(() => { setN(() => { log("ran"); return c(); }); }); return <i>{n()}</i>; }
