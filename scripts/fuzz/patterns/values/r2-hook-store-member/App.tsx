const log = (...a: any[]) => (window as any).__log(...a);
import { createStore } from "solid-js";
const [st] = createStore({ v: 1 });
const use = () => st;
export function App() { const r = use().v; log(r); return <p>{r}</p>; }
