import { createSignal, onCleanup } from "solid-js";

// `PointerEvent` is a `MouseEvent`: a synthetic `click` of it runs the same
// activation behavior (`input`, `change`).
function press(target: HTMLInputElement) {
  target.dispatchEvent(new PointerEvent("click"));
}

export function Reached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input type="checkbox" onChange={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onChange={() => { setCount(count() + 1); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as unknown as HTMLInputElement)}>press</button>
    </div>
  );
}
