import { createSignal, createStore, createMemo, merge } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
export function App() { const [n] = createSignal(1); if (Math.random() > 2) n(); return <i />; }
