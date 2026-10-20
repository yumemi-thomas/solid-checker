import { createSignal } from "solid-js";

export function App() {
  const [count] = createSignal(0);
  const cb = () => console.log(count());
  invokeIt(cb);
  return <button onClick={cb}>x</button>;
}
