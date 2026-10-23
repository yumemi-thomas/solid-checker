import { createSignal, onCleanup } from "solid-js";

// A call through a computed key that cannot name a member fires nothing: an
// index typed `number` (a function array, an `any` receiver), a well-known
// symbol, and a callee that resolves to a typed signature. No handler is
// withheld, so each keeps its ownerless proof.
function runAll(fns: Array<() => void>, loose: any, table: Record<string, () => number>, name: string) {
  for (let i = 0; i < fns.length; i++) fns[i]();
  for (let i = 0; i < 2; i++) loose[i]();
  loose[Symbol.asyncIterator]();
  return table[name]();
}

export function Benign() {
  const [count, setCount] = createSignal(0);
  runAll([], {}, {}, "a");
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <div onScroll={() => { onCleanup(() => undefined); }}>scroll</div>
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
