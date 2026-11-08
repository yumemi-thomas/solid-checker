import { createSignal, createUniqueId } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useThing(o: { a?: number }) { const p = o.a ?? 7; return { p }; }
function Child(props: { a: number }) { const t = useThing(props); log(t.p); return <i>x</i>; }
export function App() { const [s] = createSignal(1); return <Child a={s()} />; }
