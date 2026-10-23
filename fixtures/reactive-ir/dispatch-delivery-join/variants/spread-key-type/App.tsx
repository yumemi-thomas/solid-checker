import { createSignal, onCleanup } from "solid-js";

// A handler a spread holder supplies is typed by its key (ADR 0358): `onFocus`
// is reached by `focus()`, `onScroll` is not; a holder whose key cannot be read
// (a computed key) leaves the whole spread unresolved and every handler of that
// element an obligation.
const handlers = {
  onFocus: () => { onCleanup(() => undefined); },
  onScroll: () => { onCleanup(() => undefined); },
};

export function Spread() {
  const [count, setCount] = createSignal(0);
  const loose = (<input />) as any;
  loose.focus();
  return (
    <div>
      <input {...handlers} />
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
