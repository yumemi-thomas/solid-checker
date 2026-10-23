import { createSignal, onCleanup } from "solid-js";

// `target[name]()` on an `any` receiver names no member: it may be any
// dispatching member, so it fires any type. (A literal key spelled like a
// member is that member's set.)
function call(target: any, name: string) {
  target[name]();
}

export function Computed() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <button onClick={() => { setCount(count() + 1); }}>write</button>
      <button onClick={(event) => call(event.currentTarget, "focus")}>call</button>
    </div>
  );
}
