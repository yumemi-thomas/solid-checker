import { createSignal } from "solid-js";
export function CallbackTdz() {
  const [count] = createSignal(0);
  function enter() { timer.invoke(() => { void count(); }); }
  try { enter(); } catch {}
  const timer = { invoke(fn: () => void) { fn(); } };
  return <p>no callback runs</p>;
}
