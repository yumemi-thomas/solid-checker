import { createSignal, onCleanup } from "solid-js";

// A literal key spelled like a member is that member's set, on an `any`
// receiver or a typed one: `["focus"]()` fires the focus move, so a scroll or
// click handler keeps its proof.
export function Literal() {
  const [count, setCount] = createSignal(0);
  const loose = (<input />) as any;
  loose["focus"]();
  return (
    <div>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onFocus={() => { setCount(count() + 1); }} />
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
    </div>
  );
}
