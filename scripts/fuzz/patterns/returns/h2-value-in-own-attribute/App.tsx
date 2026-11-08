import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { a?: number; other?: any }) { log("a=" + p.a); return <i>{p.other ? <Dynamic component={p.other} a={7} /> : null}</i>; }
export function App() { const [n] = createSignal(1); const Other = (q: any) => <C a={n()} />; return <C a={1} other={Other} />; }
