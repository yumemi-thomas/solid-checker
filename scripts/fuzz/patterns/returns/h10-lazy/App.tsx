import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { a?: number; children?: any; other?: any }) { log("a=" + p.a); return <i>{p.children}</i>; }
const L = lazy(async () => ({ default: C }));
export function App() { const [n] = createSignal(1); return <><C a={1} /><Loading fallback={<b />}><L a={n()} /></Loading></>; }
