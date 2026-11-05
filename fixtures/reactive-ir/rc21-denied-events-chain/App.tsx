import { createSignal } from "solid-js";

export function App() {
  const [count, setCount] = createSignal(0);
  let other!: HTMLButtonElement;
  return (
    <div>
      <button onBlur={() => other.click()}>a</button>
      <button ref={other} onClick={() => setCount(count() + 1)}>b</button>
    </div>
  );
}
