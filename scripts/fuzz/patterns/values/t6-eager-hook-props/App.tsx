import { createSignal, createStore, createContext, useContext, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useThing(props: { a: number }) { const start = props.a; return { start }; }
function Child(props: { a: number }) { const t = useThing(props); log(t.start); return <i>x</i>; }
export function App() { const [s] = createSignal(1); return <Child a={s()} />; }
