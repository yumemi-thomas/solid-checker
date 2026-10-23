import { createSignal, onCleanup } from "solid-js";

// `focus()` on an `any` receiver resolves to no declaration: spelled as the
// member, it fires the member's set (the focus move), so a click or scroll
// handler keeps its proof.
export function Spelled() {
  const [count, setCount] = createSignal(0);
  const loose = (<input />) as any;
  loose.focus();
  return (
    <div>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onBlur={() => { setCount(count() + 1); }} />
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
    </div>
  );
}
