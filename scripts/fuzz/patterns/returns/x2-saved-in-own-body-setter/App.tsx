import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
let saved: any;
function C(p: { run?: (v: number) => void }) { saved = C; p.run?.(2); return <i />; }
function H() { const [n, setN] = createSignal(1); return <Dynamic component={saved} run={setN} />; }
export function App() { return <><C run={() => {}} /><H /></>; }
