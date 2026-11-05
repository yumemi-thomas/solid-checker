import { createSignal } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const [c] = createSignal(1);
const h = () => c();
class K { f = h(); }
export function App() { const v = new K().f; log(v); return <i>{v}</i>; }
