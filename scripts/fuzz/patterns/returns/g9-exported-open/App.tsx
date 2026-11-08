import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
export function C(p: { run: () => void }) { p.run(); return <i />; }
export function App() { const handle = () => {}; return <C run={handle} />; }
