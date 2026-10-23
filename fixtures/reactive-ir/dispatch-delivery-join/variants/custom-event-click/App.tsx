import { createSignal, onCleanup } from "solid-js";

// Contrast: exactly `Event` or `CustomEvent` is no `MouseEvent`, so a `click`
// of it is no activation event. It fires `{click}` alone: the `onChange`
// handlers keep their proof (a violation, and a quiet write).
function press(target: HTMLInputElement) {
  target.dispatchEvent(new CustomEvent("click"));
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
