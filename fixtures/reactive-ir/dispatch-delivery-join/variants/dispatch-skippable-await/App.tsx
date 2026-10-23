import { createSignal, onCleanup } from "solid-js";

// ADR 0366 with ADR 0357: an `await` that may be skipped does not dominate the
// dispatch, which may run synchronously in its caller's (owned) stack. It fires
// the `click()` set: the click and change handlers are obligations; the focus
// handler (`click()` also fires the focus move) is one as well.
declare function load(): Promise<void>;

export async function clickMaybeLater(target: HTMLElement, wait: boolean) {
  if (wait) await load();
  target.click();
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
