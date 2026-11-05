import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { run?: (v: number) => void; children?: any }) { p.run?.(2); return <i>{p.children}</i>; }
export function App() { const [n, setN] = createSignal(1); return <C run={() => {}}><Dynamic component={C} run={setN} /></C>; }
