import { createSignal, createEffect, createMemo, onCleanup, getOwner, createRoot, children, mapArray, repeat, Show, For, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const [list, setList] = createSignal([1, 2]);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
const btn = (b: HTMLButtonElement) => { (globalThis as any).__b = () => b; };
const click = async () => { (globalThis as any).__b().click(); };
function Wrap(props: { children: JSX.Element }) {
  const resolved = children(() => props.children);
  return <div>{resolved()}</div>;
}
export function App() { return <Wrap>{c()}</Wrap>; }
