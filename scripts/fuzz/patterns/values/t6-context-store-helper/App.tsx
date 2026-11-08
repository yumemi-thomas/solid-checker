import { createSignal, createStore, createContext, useContext, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const bump = (o: { n: number }) => { o.n = 5; };
const Ctx = createContext<{ s: { n: number } }>();
function Child() { const c = useContext(Ctx)!; return <button onClick={() => { bump(c.s); }}>go</button>; }
export function App() { const [st] = createStore({ n: 1 }); return <Ctx value={{ s: st }}><Child /></Ctx>; }
