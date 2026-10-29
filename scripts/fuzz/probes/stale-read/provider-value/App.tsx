// ADR 0443 (2): the provider reads `value` once and stores a snapshot; the child's useContext never sees the write. Stale: `#w` keeps "1" (fresh would be "2").
import { createContext, createSignal, useContext } from "solid-js";

const Count = createContext<number>(0);

function Child() {
  const value = useContext(Count);
  return <span id="w">{value}</span>;
}

export default function App() {
  const [count, setCount] = createSignal(1);
  return (
    <div>
      <button id="b" onClick={() => setCount(2)}>count</button>
      <Count value={count()}><Child /></Count>
    </div>
  );
}
