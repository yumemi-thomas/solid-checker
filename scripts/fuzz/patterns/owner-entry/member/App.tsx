import { createContext, createEffect, useContext } from "solid-js";

// ADR 1142, a route the generators do not write: a member of an object the click's handler calls (NO_OWNER_CONTEXT).
const Ctx = createContext<number>();

export function App() {
  const f1 = () => {
    useContext(Ctx);
  };
  createEffect(f1, () => {});
  const box = { run: f1 };
  return <button onClick={() => box.run()}>go</button>;
}

// The lane drives no events of its own (patterns.mjs ENTRY): the click is the program's.
export function after() {
  document.querySelector("button")?.click();
}
