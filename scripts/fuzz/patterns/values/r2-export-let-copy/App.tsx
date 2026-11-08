const log = (...a: any[]) => (window as any).__log(...a);
import { read } from "./lib";
export function App() { const r = read(); log(r); return <p>{r}</p>; }
