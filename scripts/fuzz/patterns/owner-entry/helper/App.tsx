import { createContext, createEffect, useContext } from "solid-js";

// ADR 1142, a route the generators do not write: a project helper calls the function the handler hands it, with no owner (NO_OWNER_CONTEXT).
const Ctx = createContext<number>();

function run(callback: () => void) {
  callback();
}
export function App() {
  const f1 = () => {
    useContext(Ctx);
  };
  createEffect(f1, () => {});
  return <button onClick={() => run(f1)}>go</button>;
}

// The lane drives no events of its own (patterns.mjs ENTRY): the click is the program's.
export function after() {
  document.querySelector("button")?.click();
}
