import { createSignal, createMemo, Show, Loading } from "solid-js";
import { Dynamic } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function C(p: { a?: number; children?: any }) { log("a=" + p.a); return <i>{p.children}</i>; }
export function App() { const [n] = createSignal(1); return <C a={1}>{C({ get a() { return n(); } })}</C>; }
