import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, onCleanup, type Accessor } from "solid-js";
import { render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
function mount(c: () => any) { const el = document.createElement("div"); document.body.appendChild(el); return render(c, el); }
function useS() { const [rows] = createSignal(0); return { read: () => rows() }; }
export function App() { return <div />; }
export async function after() { const d = mount(() => { const s = useS(); log("r=" + s.read()); return null; }); d(); }
