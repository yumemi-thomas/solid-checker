import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, getOwner, runWithOwner, createRoot, untrack, Show, For, Errored, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
function Thrower(): any { throw new Error("boom"); }
export function App() {
  return <Errored fallback={(err, reset) => { const v = c(); __log("fallback ran", v, "owner", own()); return <p>{v}</p>; }}><Thrower /></Errored>;
}
export async function after() { setC(2); }
