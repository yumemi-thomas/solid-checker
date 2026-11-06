import { createStore } from "solid-js";
export function App() {
  const [st] = createStore<{ a?: number }>({});
  return <div><button onClick={() => { delete st["a"]; }}>go</button></div>;
}
export async function after() { document.querySelector("button")!.click(); }
