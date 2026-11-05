import { createSignal, createStore, createMemo, merge } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
function C(raw: { run?: () => number; on?: boolean }) { const p = merge({ on: true }, raw); if (p.on) log("r=" + p.run!()); return <i />; }
export function App() { const [n] = createSignal(1); return <C run={() => n()} />; }
