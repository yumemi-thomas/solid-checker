import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, type Accessor } from "solid-js";
import { createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
let impl: () => number = () => 0;
function useT() { return { read: (): number => impl() }; }
export function App() { const [n] = createSignal(1); impl = n; const x = useT(); log("r=" + x.read()); return <div />; }
