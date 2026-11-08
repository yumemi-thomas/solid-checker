import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { run?: (v: number) => void; me?: any }) { p.run?.(2); const [n, setN] = createSignal(1); return <i>{p.me ? <Dynamic component={p.me} run={setN} /> : null}</i>; }
export function App() { return <C run={() => {}} me={C} />; }
