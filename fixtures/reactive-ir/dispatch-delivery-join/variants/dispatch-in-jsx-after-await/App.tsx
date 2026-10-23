import { createSignal, onCleanup } from "solid-js";

// ADR 0365: a call in a JSX expression after an `await` runs where the compiled
// template evaluates it, which may be a render effect's run (owned), not the
// continuation. The dispatch is no ownerless stack and fires the `click()`
// set: the click, focus and change handlers are obligations, as is the write.
declare function load(): Promise<void>;

export async function view(target: HTMLElement) {
  await load();
  return <div>{(target.click(), "x")}</div>;
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
