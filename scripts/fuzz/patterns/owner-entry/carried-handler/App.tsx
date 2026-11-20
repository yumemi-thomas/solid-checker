import { createContext, createEffect, useContext } from "solid-js";

// ADR 1142 (the fuzz context `barm-carried-handler`, mx-census CONTEXT_NOT_FOUND, ADR 1124 round 2): the effect's compute runs `f1` under the effect's owner (ContextNotFoundError, logged), and the click's
// `forEach` carries `f1` on and calls it from the handler with no owner: NO_OWNER_CONTEXT at the same read (Chrome rc.21). main certified the second code (SC4002's violation stood alone at the read).
const Ctx = createContext<number>();

export function App() {
  const f1 = () => {
    useContext(Ctx);
  };
  createEffect(f1, () => {});
  return <button onClick={() => [f1].forEach((g) => g())}>go</button>;
}

// The lane drives no events of its own (patterns.mjs ENTRY): the click is the program's.
export function after() {
  document.querySelector("button")?.click();
}
