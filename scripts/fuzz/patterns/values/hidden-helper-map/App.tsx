import { createSignal } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const [c] = createSignal(1);
const h = () => c();
const m = new Map([["a", () => h()]]);
export function App() { const v = m.get("a")!(); log(v); return <i>{v}</i>; }
