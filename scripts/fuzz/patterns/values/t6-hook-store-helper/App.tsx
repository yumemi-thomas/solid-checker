import { createSignal, createStore, createContext, useContext, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const bump = (o: { n: number }) => { o.n = 5; };
function useT() { const [s, set] = createStore({ n: 1 }); return { s, set }; }
export function App() { const t = useT(); return <button onClick={() => { bump(t.s); }}>go</button>; }
