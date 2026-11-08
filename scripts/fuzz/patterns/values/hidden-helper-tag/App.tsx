import { createSignal } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const [c] = createSignal(1);
const h = () => c();
const t = (_: TemplateStringsArray) => h();
export function App() { const v = t`x`; log(v); return <i>{v}</i>; }
