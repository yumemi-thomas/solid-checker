import { createSignal, createStore, createContext, useContext, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const bump = (o: { n: number }) => { o.n = 5; };
function Child(props: { s: { n: number } }) { return <button onClick={() => { bump(props.s); }}>go</button>; }
export function App() { const [st] = createStore({ n: 1 }); return <Child s={st} />; }
