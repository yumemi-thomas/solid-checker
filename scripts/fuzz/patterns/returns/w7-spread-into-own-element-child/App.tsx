import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { run?: () => void; children?: any; me?: any }) { p.run?.(); return <i>{p.children}</i>; }
export function App() { const [n, setN] = createSignal(1); const o = { run: () => setN(2) }; return <C run={() => {}}><C {...o} /></C>; }
