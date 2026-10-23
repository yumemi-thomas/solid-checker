import { createSignal, onCleanup } from "solid-js";

// A `MouseEvent` of type `click` is an activation event (DOM Standard
// `isActivationEvent`): dispatching it runs the checkbox's activation behavior,
// `input` and `change`, so the `onChange` handlers stay obligations. A scroll
// handler is never reached.
function press(target: HTMLInputElement) {
  target.dispatchEvent(new MouseEvent("click", { bubbles: true }));
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
