import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { run: () => void }) { p.run(); return <i />; }
function useT() { const [n, setN] = createSignal(1); return { save: () => { setN(2); } }; }
export function App() { const { save } = useT(); return <C run={save} />; }
