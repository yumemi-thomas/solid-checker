import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function h(n: () => number) { return n(); }
export function App() { const [n] = createSignal(1); log("r=" + h(n)); return <div />; }
