import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function D(q: { b: any }) { return <i>{q.b}</i>; }
function C(p: { a: any }) { return <D b={p.a} />; }
export function App() { const a = createMemo(async () => { await sleep(10); return 1; }); return <Loading fallback={<b />}><div><C a={a()} /></div></Loading>; }
