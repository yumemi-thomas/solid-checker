import { createSignal, onCleanup } from "solid-js";

// The key is a union of two members' names: the call may be `focus()` or
// `click()`, so the `onFocus` handlers stay obligations. A union key type
// resolves to no target set, which is no evidence that no `lib.dom` member is
// called.
function press(target: HTMLInputElement, key: "click" | "focus") {
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
