import { createSignal } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const [c] = createSignal(1);
const h = () => c();
const o = { toString() { return String(h()); } };
export function App() { const v = `${o}`; log(v); return <i>{v}</i>; }
