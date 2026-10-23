import { createSignal, onCleanup } from "solid-js";

// A generic key bounded by two members' names: the same call, the `onFocus`
// handlers stay obligations.
function press<K extends "click" | "focus">(target: HTMLInputElement, key: K) {
  target[key]();
}

export function Reached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input type="checkbox" onFocus={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onFocus={() => { setCount(count() + 1); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as unknown as HTMLInputElement, "focus")}>press</button>
    </div>
  );
}
