import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { a: any }) { const f = () => p.a; return <i />; }
export function App() { const a = createMemo(async () => { await sleep(10); return 1; }); return <Loading fallback={<b />}><div><C a={a()} /></div></Loading>; }
