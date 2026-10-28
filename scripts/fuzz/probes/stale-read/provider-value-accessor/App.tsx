// The control: the provider hands the accessor and the child calls it in JSX, which tracks it. Fresh: `#w` shows "2".
import { createContext, createSignal, useContext, type Accessor } from "solid-js";

const Count = createContext<Accessor<number>>(() => 0);

function Child() {
  const value = useContext(Count);
  return <span id="w">{value()}</span>;
}

export default function App() {
  const [count, setCount] = createSignal(1);
  return (
    <div>
      <button id="b" onClick={() => setCount(2)}>count</button>
      <Count value={count}><Child /></Count>
    </div>
  );
}
