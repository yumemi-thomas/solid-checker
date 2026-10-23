import { createSignal, onCleanup } from "solid-js";

// ADR 0365: a class body after an `await` is refused whole. This static block
// does run when the class is evaluated, on the continuation, so the refusal is
// a deliberate miss: the dispatch fires the `click()` set and the click, focus
// and change handlers are obligations, as is the write.
declare function load(): Promise<void>;

export const holder: { C?: object } = {};

export async function setup(target: HTMLElement) {
  await load();
  holder.C = class { static { target.click(); } };
}

export function Handlers() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onChange={() => { onCleanup(() => undefined); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
