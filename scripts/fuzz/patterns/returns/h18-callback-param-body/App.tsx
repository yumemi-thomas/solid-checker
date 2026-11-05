import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { run: () => number }) { log("r=" + p.run()); return <i />; }
function Mid(q: { get: () => number }) { return <C run={() => q.get()} />; }
export function App() { const [n] = createSignal(1); return <Mid get={n} />; }
