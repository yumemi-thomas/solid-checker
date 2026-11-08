import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
let saved: any;
function C(p: { a?: number }) { saved = C; log("a=" + p.a); return <i />; }
function H() { const [n] = createSignal(1); return <Dynamic component={saved} a={n()} />; }
export function App() { return <><C a={1} /><H /></>; }
