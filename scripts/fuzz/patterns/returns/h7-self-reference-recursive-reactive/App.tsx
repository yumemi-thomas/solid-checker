import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { a?: number; d?: number }) { const [n] = createSignal(1); log("a=" + p.a); return <i>{(p.d ?? 0) < 1 ? <C a={n()} d={1} /> : null}</i>; }
export function App() { return <C a={1} />; }
