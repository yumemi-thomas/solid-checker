import { createSignal, createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const R: typeof Reflect.set = Reflect.set;
const k = String("read");
const [n] = createSignal(1);
function useThing() { return { read: (): number => n() }; }
export function App() { const x = useThing(); R({}, "read", () => 0, x); log("r=" + x.read()); return <div />; }
