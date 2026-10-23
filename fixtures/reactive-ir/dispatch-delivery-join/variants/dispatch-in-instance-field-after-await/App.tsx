import { createSignal, onCleanup } from "solid-js";

// ADR 0369: an instance field initializer runs at `new` (here nowhere the
// analysis sees), not where the class is evaluated after the `await`. The
// dispatch is no ownerless stack and fires the `click()` set: the click, focus
// and change handlers are obligations, as is the write.
declare function load(): Promise<void>;

export const holder: { C?: new () => object } = {};

export async function setup(target: HTMLElement) {
  await load();
  holder.C = class { ready = (target.click(), true); };
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
