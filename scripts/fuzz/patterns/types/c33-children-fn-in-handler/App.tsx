import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, getOwner, runWithOwner, createRoot, untrack, Show, For, Errored, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
import type { JSX } from "solid-js";
function Wrap(props: { children: () => JSX.Element }) {
  let b!: HTMLButtonElement;
  (globalThis as any).__b = () => b;
  return <button ref={b} onClick={() => { props.children(); }} />;
}
export function App() {
  return <Wrap>{() => { __log("child fn owner", own()); onCleanup(() => __log("x")); setW(1); return <i />; }}</Wrap>;
}
export async function after() { (globalThis as any).__b().click(); }
