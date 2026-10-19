import { createSignal } from "solid-js";
const [count, setCount] = createSignal(0);
function Row(item: string) {
  const prefix = count();
  return <li>{prefix}:{item}</li>;
}
export function App() {
  const [items] = createSignal(["a", "b"]);
  return <ul><button onClick={() => setCount(c => c + 1)}>inc</button>{items().map(Row)}</ul>;
}
