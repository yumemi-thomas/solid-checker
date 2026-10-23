import { createSignal, onCleanup } from "solid-js";

// ADR 0365, the shapes that do not count: the scheduler is handed a named
// function (`later`), which may also be called elsewhere; the literal the
// scheduler runs only stores a closure that dispatches; and a `then` handler
// on a parameter typed `Promise<void>` (a subclass or a thenable has that type,
// ADR 0349). Each site may run on an owned stack: `focus()` fires the focus
// move, so the focus and change handlers are obligations; the click handlers
// keep their proof.
let saved: (() => void) | undefined;

export function schedule(target: HTMLElement) {
  function later() {
    target.focus();
  }
  queueMicrotask(later);
  queueMicrotask(() => {
    saved = () => target.focus();
  });
}

export function typedThen(target: HTMLElement, done: Promise<void>) {
  void done.then(() => target.focus());
}

export function runSaved() {
  saved?.();
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
