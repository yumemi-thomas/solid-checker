import { createSignal, createStore, createMemo, merge } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
function C(p: { run?: () => number; on?: boolean }) { if (p.on) log("r=" + p.run!()); return <i />; }
export function App() { const [n] = createSignal(1); return <C on run={() => n()} />; }
