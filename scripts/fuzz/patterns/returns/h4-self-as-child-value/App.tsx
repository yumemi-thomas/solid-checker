import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { a?: number; children?: any }) { log("a=" + p.a); const [n] = createSignal(1); return <i>{p.children ? <Dynamic component={p.children} a={n()} /> : null}</i>; }
export function App() { return <C a={1}>{C}</C>; }
