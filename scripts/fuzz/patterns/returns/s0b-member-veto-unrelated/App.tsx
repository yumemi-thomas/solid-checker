import { createSignal, createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const R: typeof Reflect.set = Reflect.set;
const k = String("read");
const [n] = createSignal(1);
function useThing() { return { read: (): number => n() }; }
function useT() { return { read: (): number => n() }; }
const other = { read: () => 1 }; other.read = () => 2;
export function App() { const x = useT(); log("r=" + x.read()); return <div />; }
