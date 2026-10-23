import { createSignal, onCleanup } from "solid-js";

// The event is a parameter: its type is not read, so it may fire any type and
// every handler is an obligation.
function fire(target: EventTarget, event: Event) {
  target.dispatchEvent(event);
}

export function Unknown() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <button onClick={() => { setCount(count() + 1); }}>write</button>
      <button onClick={(event) => fire(event.currentTarget as EventTarget, new Event("x"))}>fire</button>
    </div>
  );
}
