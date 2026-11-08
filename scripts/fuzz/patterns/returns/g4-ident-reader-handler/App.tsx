import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { run: () => void }) { return <button onClick={() => p.run()} />; }
export function App() { const [n] = createSignal(1); const handle = () => { log(n()); }; return <C run={handle} />; }
