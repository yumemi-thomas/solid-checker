import { createSignal } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const [c] = createSignal(1);
const h = () => c();
const o = { *[Symbol.iterator]() { yield h(); } };
export function App() { const v = [...o]; log(v[0]); return <i>{v[0]}</i>; }
