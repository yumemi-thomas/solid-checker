import { createSignal, createMemo, createEffect } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const [c] = createSignal(1);
async function h(f: boolean) { if (f) await wait(1); return c(); }
const o = { get x() { return h(false); } };
export function App() { void o.x; return <i/>; }
