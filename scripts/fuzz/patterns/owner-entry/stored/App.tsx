import { createContext, createEffect, useContext } from "solid-js";

// ADR 1142, a route the generators do not write: the effect's function is also stored in a variable the click's handler calls (NO_OWNER_CONTEXT at the click; the owner graph holds no edge for a stored call).
const Ctx = createContext<number>();

let kept: (() => void) | undefined;
export function App() {
  const f1 = () => {
    useContext(Ctx);
  };
  createEffect(f1, () => {});
  kept = f1;
  return <button onClick={() => kept?.()}>go</button>;
}

// The lane drives no events of its own (patterns.mjs ENTRY): the click is the program's.
export function after() {
  document.querySelector("button")?.click();
}
