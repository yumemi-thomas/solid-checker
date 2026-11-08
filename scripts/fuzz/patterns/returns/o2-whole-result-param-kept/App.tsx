import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function keep(f: () => number) { (window as any).__k = f; }
function make(n: () => number) { return (): number => n(); }
export function App() { const [n] = createSignal(1); const task = make(n); log("r=" + task()); keep(task); return <div />; }
