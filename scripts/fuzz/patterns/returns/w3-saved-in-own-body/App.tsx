import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
let saved: any;
function C(p: { run?: () => void }) { saved = C; p.run?.(); return <i />; }
function H() { const [n, setN] = createSignal(1); return <Dynamic component={saved} run={() => setN(2)} />; }
export function App() { return <><C run={() => {}} /><H /></>; }
