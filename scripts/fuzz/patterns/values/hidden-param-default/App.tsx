import { createSignal, createMemo, createEffect } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const [c] = createSignal(1);
function h(x = c()) { return x; }
const o = { get x() { return h(); } };
export function App() { void o.x; return <i/>; }
