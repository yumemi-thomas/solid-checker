import { createSignal, createStore } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const holder = { c: createSignal(1)[0] };
export function App() {
  const [st] = createStore({ items: [] as unknown[] });
  const list = st.items;
  list.push(holder.c);
  log(st.items.length);
  return <p>{st.items.length}</p>;
}
