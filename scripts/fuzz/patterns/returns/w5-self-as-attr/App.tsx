import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, lazy, Show, Loading, type Accessor, type JSX } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
const sleep = (ms = 0) => new Promise((r) => setTimeout(r, ms));
function C(p: { run?: () => void; me?: any }) { p.run?.(); const [n, setN] = createSignal(1); return <i>{p.me ? <Dynamic component={p.me} run={() => setN(2)} /> : null}</i>; }
export function App() { return <C run={() => {}} me={C} />; }
