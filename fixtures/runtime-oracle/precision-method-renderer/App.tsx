import { createSignal } from "solid-js";
const [count, setCount] = createSignal(0);
const renderers = {
  badge() {
    const value = count();
    return <b>{value}</b>;
  },
};
export function App() {
  return <p><button onClick={() => setCount(c => c + 1)}>inc</button>{renderers.badge()}</p>;
}
