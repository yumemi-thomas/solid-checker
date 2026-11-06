import { createStore } from "solid-js";

// Uncertifiable: a direct `eval` anywhere in the file may reach the setter.
export function DirectEval() {
  const [store, setStore] = createStore({ a: { v: 0 } });
  eval("setStore((s) => { s.a = new Date(); })");
  store.a.v = 1;
}
