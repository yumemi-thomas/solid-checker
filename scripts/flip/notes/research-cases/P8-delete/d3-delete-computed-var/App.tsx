import { createStore } from "solid-js";
const k = "a";
export function App() {
  const [st] = createStore<{ a?: number }>({});
  return <div><button onClick={() => { delete st[k]; }}>go</button></div>;
}
export async function after() { document.querySelector("button")!.click(); }
