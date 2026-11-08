import { createSignal, createMemo, createEffect } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const [c] = createSignal(1);
function* g() { yield c(); }
const o = { get x() { return [...g()]; } };
export function App() { void o.x; return <i/>; }
