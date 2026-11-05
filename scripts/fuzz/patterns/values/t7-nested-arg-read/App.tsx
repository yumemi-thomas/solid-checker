import { createSignal, createUniqueId } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useThing(o: { a: number }) { const c = Array.from({ length: o.a }, (_, i) => i); return { c }; }
function Child(props: { a: number }) { const t = useThing(props); log(t.c.length); return <i>x</i>; }
export function App() { const [s] = createSignal(1); return <Child a={s()} />; }
