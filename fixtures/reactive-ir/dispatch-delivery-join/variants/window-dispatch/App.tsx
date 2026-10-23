import { createSignal, onCleanup } from "solid-js";

// A window dispatch runs only the listeners registered on `window` (its event
// path is `[window]`); a compiler-emitted element handler is a listener on the
// element, or a property the delegated walk finds from `e.target` upwards, so
// none is reached. Every element handler here keeps its proof.
export function WindowDispatch() {
  const [count, setCount] = createSignal(0);
  window.dispatchEvent(new CustomEvent("app:ping", { detail: 1 }));
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>cleanup</button>
      <button onClick={() => { setCount(count() + 1); }}>write</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
    </div>
  );
}
