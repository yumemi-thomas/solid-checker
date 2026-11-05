import { createSignal, createStore, createContext, useContext, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const bump = (o: { n: number }) => { o.n = 5; };
export function App() { const [st] = createStore({ items: [{ n: 1 }] }); return <For each={st.items}>{(x) => <button onClick={() => { bump(x); }}>go</button>}</For>; }
