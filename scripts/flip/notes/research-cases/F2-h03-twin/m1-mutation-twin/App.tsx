import { createSignal, createEffect, getOwner } from "solid-js";
const [c] = createSignal(1);
const misuseEffect = (tag: string) => { (window as any).__log?.(tag, String(getOwner())); createEffect(() => c(), () => {}); };
export function App() {
  let el!: HTMLDivElement;
  queueMicrotask(() => new MutationObserver(() => misuseEffect("mo")).observe(el, { childList: true }));
  return <div class="t" ref={el}>m</div>;
}
export async function after() {
  document.querySelector(".t")!.append("x"); await new Promise((r) => setTimeout(r, 20));
}
