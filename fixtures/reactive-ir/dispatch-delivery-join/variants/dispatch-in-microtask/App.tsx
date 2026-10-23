import { createSignal, onCleanup } from "solid-js";

// ADR 0366: a dispatch written directly in a callback literal a fresh-stack
// host scheduler runs from a task or microtask queue has no owner: the
// scheduler's callback starts on an empty stack (the dialect's
// `fresh_stack_callback_owner`, the same table the owner graph reads); a
// `then` handler on a promise proven native (`Promise.resolve(0)`, ADR 0349)
// runs from a promise job. No site fires anything: every handler keeps its proof.
export function schedule(target: HTMLElement) {
  queueMicrotask(() => target.focus());
  setTimeout(() => {
    target.click();
  }, 0);
  requestAnimationFrame(() => target.click());
  void Promise.resolve(0).then(() => target.click());
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
