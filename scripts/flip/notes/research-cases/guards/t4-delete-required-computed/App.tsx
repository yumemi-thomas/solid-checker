import { createStore } from "solid-js";
export function App() {
  const [st] = createStore<{ a: number }>({ a: 1 });
  return <button onClick={() => { delete st["a"]; }} />;
}
export async function after() { document.querySelector("button")!.click(); }
