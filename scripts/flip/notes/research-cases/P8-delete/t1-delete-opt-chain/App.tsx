import { createStore } from "solid-js";
export function App() {
  const [st] = createStore<{ o?: { a?: number } }>({ o: {} });
  return <button onClick={() => { delete st.o?.a; }} />;
}
export async function after() { document.querySelector("button")!.click(); }
