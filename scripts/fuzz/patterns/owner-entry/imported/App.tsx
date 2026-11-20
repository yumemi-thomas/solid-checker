import { createEffect } from "solid-js";
import { useHookValue } from "./Hook";

// ADR 1142, a route the generators do not write: the read is in Hook.tsx, the effect and the carrying `forEach` are here (NO_OWNER_CONTEXT at the click).
export function App() {
  createEffect(useHookValue, () => {});
  return <button onClick={() => [useHookValue].forEach((g) => g())}>go</button>;
}

// The lane drives no events of its own (patterns.mjs ENTRY): the click is the program's.
export function after() {
  document.querySelector("button")?.click();
}
