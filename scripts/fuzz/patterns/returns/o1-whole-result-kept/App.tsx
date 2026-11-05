import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function keep(f: () => number) { (window as any).__k = f; }
function make() { return (): number => 0; }
export function App() { const task = make(); log("r=" + task()); keep(task); return <div />; }
