import { createSignal, createUniqueId } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
import { useThing } from "./h";
export function Child(props: { a: number; on?: () => void }) { const t = useThing(props); log(t.p); return <i>x</i>; }
export function App() { const [s] = createSignal(1); return <Child a={s()} />; }
