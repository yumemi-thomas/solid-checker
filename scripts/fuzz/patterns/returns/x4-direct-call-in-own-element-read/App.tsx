import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { get?: () => number; children?: any }) { log("g=" + p.get?.()); return <i>{p.children}</i>; }
export function App() { const [n] = createSignal(1); return <C get={() => 0}>{C({ get: () => n() })}</C>; }
