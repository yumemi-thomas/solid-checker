import { createSignal, onCleanup } from "solid-js";

// `dispatchEvent` of an inline `CustomEvent` with a literal type fires only that
// type, on any receiver: a parameter names no element, but no element here has
// an `onSync` handler, so every handler keeps its proof.
function fire(target: EventTarget) {
  target.dispatchEvent(new CustomEvent("sync"));
}

export function Sync() {
  const [count, setCount] = createSignal(0);
  fire(window);
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
