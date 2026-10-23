import { createSignal, onCleanup } from "solid-js";

// A `method=dialog` form closes its dialog on submission, which fires
// `beforetoggle` before it queues `toggle` and `close` (HTML Standard, form
// submission algorithm): the `onBeforeToggle` handlers stay obligations.
function press(target: HTMLFormElement) {
  target.requestSubmit();
}

export function Reached() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <input type="checkbox" onBeforeToggle={() => { onCleanup(() => undefined); }} />
      <input type="checkbox" onBeforeToggle={() => { setCount(count() + 1); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <div onScroll={() => { setCount(count() + 1); }}>scroll write</div>
      <button onClick={(event) => press(event.currentTarget as unknown as HTMLFormElement)}>press</button>
    </div>
  );
}
