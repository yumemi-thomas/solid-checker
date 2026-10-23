import { createSignal, onCleanup } from "solid-js";

// ADR 0369: a static block runs when its class is evaluated, and the class
// expression is evaluated after the `await`, on the continuation. The dispatch
// runs on an ownerless stack (ADR 0366), so the handlers it runs have no owner:
// the `onCleanup` calls are violations, the write is legal.
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
