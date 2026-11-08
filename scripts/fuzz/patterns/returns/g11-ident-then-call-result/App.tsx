import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { run: () => void }) { p.run(); return <i />; }
export function App() { const [n, setN] = createSignal(1); const mk = () => () => { setN(2); }; const handle = mk(); return <C run={handle} />; }
