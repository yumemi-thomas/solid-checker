import { createMemo, createSignal, onCleanup } from "solid-js";
function tip(label: () => string, _witness: unknown) {
  return (el: HTMLDivElement) => { el.id = "w1"; void label; };
}
export function App() {
  const [n] = createSignal(1);
  return <div ref={tip(createMemo(() => `n${n()}`), onCleanup(() => undefined))}>w1</div>;
}
