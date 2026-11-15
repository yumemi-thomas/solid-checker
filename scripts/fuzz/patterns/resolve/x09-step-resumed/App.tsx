import { action, createEffect, createMemo, createSignal, onCleanup, onSettled, resolve } from "solid-js";
const [n, setN] = createSignal(1);
const p = () => n();
const act = action(function* () { yield Promise.resolve(); void resolve(p); });
export function App() {
  return <button onClick={() => { void act(); }}>go</button>;
}
