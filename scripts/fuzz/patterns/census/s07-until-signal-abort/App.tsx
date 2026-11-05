import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack } from "solid-js";
import { Dynamic, clientOnly } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log?.(...a);
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
const [xs, setXs] = createSignal([{ id: 1 }, { id: 2 }]);
const misuseEffect = (tag: string) => { log(tag, own()); createEffect(() => c(), () => {}); };
export function App() {
  const ac = new AbortController();
  const m = createMemo(async () => { try { await (await import("solid-js")).until(() => c() > 5, { signal: ac.signal }); } catch (e) { log("until", "caught " + (e as Error).name); } return c(); });
  (window as any).__ac = ac;
  return <Show when={m()}>{(v) => <i>{v()}</i>}</Show>;
}
export async function after() {
  (window as any).__ac.abort(); await new Promise((r) => setTimeout(r, 20));
}