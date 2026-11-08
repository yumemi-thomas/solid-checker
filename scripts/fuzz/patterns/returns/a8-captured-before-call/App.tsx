import { createSignal, createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const R: typeof Reflect.set = Reflect.set;
const k = String("read");
const [n] = createSignal(1);
function useThing() { return { read: (): number => n() }; }
export function App() { const t = { read: (): number => n() }; const keep = [t]; A(t, { read: () => 0 }); log("r=" + keep[0].read()); return <div />; }
