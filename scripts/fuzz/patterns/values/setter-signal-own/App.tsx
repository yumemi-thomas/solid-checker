import { createSignal, createEffect, createStore } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const [c2] = createSignal(1);
export function App() { const [n, setN] = createSignal(0); createEffect(() => c2(), () => { setN((p) => { log("ran"); return p + n(); }); }); return <i>{n()}</i>; }
