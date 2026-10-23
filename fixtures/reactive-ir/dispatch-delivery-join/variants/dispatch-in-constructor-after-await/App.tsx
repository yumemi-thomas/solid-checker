import { createSignal, onCleanup } from "solid-js";

// ADR 0365: a constructor of a class defined after an `await` runs when the
// class is constructed (here from the component body, under its owner), not
// on the continuation. The dispatch is no ownerless stack and fires the
// `click()` set: the click, focus and change handlers are obligations, as is
// the write.
declare function load(): Promise<void>;

export const holder: { C?: new () => object } = {};

export async function setup(target: HTMLElement) {
  await load();
  holder.C = class { constructor() { target.click(); } };
}

export function Handlers() {
  const [count, setCount] = createSignal(0);
  const C = holder.C;
  if (C) new C();
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onChange={() => { onCleanup(() => undefined); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
