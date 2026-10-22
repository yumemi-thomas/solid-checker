import { createSignal, Show } from "solid-js";
const [count, setCount] = createSignal(0);
function Row(item?: unknown) {
  const prefix = count();
  return <li>{prefix}:{String(item)}</li>;
}
export function App() {
  const [items] = createSignal(["a", "b"]);
  return (
    <ul>
      <button onClick={() => setCount(c => c + 1)}>inc</button>
      <Show when={false}><Row /></Show>
      {items().map(Row)}
    </ul>
  );
}
