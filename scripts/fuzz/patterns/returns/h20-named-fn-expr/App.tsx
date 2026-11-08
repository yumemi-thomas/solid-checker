import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
const C = function Inner(p: { a?: number }) { log("a=" + p.a); return <i />; };
export function App() { const [n] = createSignal(1); return <C a={n()} />; }
