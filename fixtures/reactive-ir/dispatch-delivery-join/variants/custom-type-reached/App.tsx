import { createSignal, onCleanup } from "solid-js";

// The same `CustomEvent("sync")` dispatched on a parameter, with an element
// that has an `onSync` handler: that handler stays an obligation (its type is
// fired), every other handler keeps its proof.
function fire(target: EventTarget) {
  target.dispatchEvent(new CustomEvent("sync", { bubbles: true }));
}

export function Sync() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <section onSync={() => { onCleanup(() => undefined); }} />
      <section onSync={() => { setCount(count() + 1); }} />
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <button onClick={() => { setCount(count() + 1); }}>write</button>
      <button onClick={(event) => fire(event.currentTarget as EventTarget)}>fire</button>
    </div>
  );
}
