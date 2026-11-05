import { createSignal, createEffect, createStore } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const [c2] = createSignal(1);
export function App() { const [st, setSt] = createStore({ a: 1 }); const [b] = createStore({ y: 1 }); createEffect(() => c2(), () => { setSt((d) => { log("ran"); d.a = b.y; }); }); return <i>{st.a}</i>; }
