import { createStore } from "solid-js";
export function App() {
  const [st] = createStore<{ o: Record<string, number> }>({ o: { a: 1 } });
  return <button onClick={() => { delete st.o["a"]; }} />;
}
export async function after() { document.querySelector("button")!.click(); }
