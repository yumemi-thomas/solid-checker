import { createSignal, createUniqueId } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function useThing(o: { a: number }) { const start = o.a; return { start }; }
function Child(props: { a: number }) { const t = useThing(props); log(t.start); return <i>x</i>; }
export function App() { const [s] = createSignal(1); return <Child a={s()} />; }
