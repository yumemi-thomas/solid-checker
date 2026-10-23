import { createSignal, onCleanup } from "solid-js";

// ADR 0365: a dispatch after an `await` that dominates it in its own async
// function runs from a promise continuation, with no owner (ADR 0176), so the
// handlers it runs have none either, as when the host runs them. The site fires
// nothing: every handler keeps its proof (the `onCleanup` calls are violations,
// the write is legal).
declare function load(): Promise<void>;

export async function clickLater(target: HTMLElement) {
  await load();
  target.click();
}

export async function focusLater(target: HTMLElement) {
  try {
    await load();
    target.focus();
  } finally {
    void 0;
  }
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
