import { createSignal, getOwner, onCleanup, runWithOwner } from "solid-js";

// ADR 0366: only a dispatch written directly in the scheduler's callback (or
// directly after the await) counts. Here the callback restores an owner with
// `runWithOwner` and dispatches inside it: the handlers may run under that
// owner. `focus()` fires the focus move, so the focus and change handlers are
// obligations; the click handlers keep their proof.
export function Restored(props: { target: HTMLElement }) {
  const owner = getOwner();
  setTimeout(() => {
    runWithOwner(owner, () => props.target.focus());
  }, 0);
  return <p>restored</p>;
}

export async function restoredAfterAwait(target: HTMLElement, done: Promise<void>) {
  const owner = getOwner();
  await done;
  runWithOwner(owner, () => target.focus());
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
