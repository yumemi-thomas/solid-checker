import { createSignal, onCleanup } from "solid-js";

// ADR 0365: the producer lists a call written in a getter defined after an
// `await` among the calls after it, but the getter runs when it is read: here
// from the component body, synchronously, under its owner. The dispatch is no
// ownerless stack and fires the `click()` set: the click, focus and change
// handlers are obligations, as is the write.
declare function load(): Promise<void>;

export const holder: { o?: { readonly go: number } } = {};

export async function setup(target: HTMLElement) {
  await load();
  holder.o = { get go() { target.click(); return 1; } };
}

export function Handlers() {
  const [count, setCount] = createSignal(0);
  // Runs the getter, and its `click()`, on this component's owned stack.
  void holder.o?.go;
  return (
    <div>
      <button onClick={() => { onCleanup(() => undefined); }}>click</button>
      <input onFocus={() => { onCleanup(() => undefined); }} />
      <input onChange={() => { onCleanup(() => undefined); }} />
      <button onClick={() => { setCount(count() + 1); }}>write</button>
    </div>
  );
}
