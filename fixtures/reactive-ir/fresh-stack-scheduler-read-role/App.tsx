import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const list = [1, 2];

// Positive: a read in the component body runs inside the strict-read window.
export function Body() {
  console.log(count());
  return <div />;
}

// Negatives: a callback handed to a reviewed fresh-stack host scheduler runs
// from a task or microtask queue, after the component body returned.
export function Timer() {
  setTimeout(() => console.log(count()), 0);
  return <div />;
}

export function Microtask() {
  queueMicrotask(() => console.log(count()));
  return <div />;
}

export function Settled() {
  Promise.resolve(1).then(() => console.log(count()));
  return <div />;
}

export function Frame() {
  requestAnimationFrame(() => console.log(count()));
  return <div />;
}

// A standard-library callback run inline inside the timer callback runs
// wherever that callback runs.
export function TimerForEach() {
  setTimeout(() => list.forEach(() => console.log(count())), 0);
  return <div />;
}

// A read through a local function called from the timer callback.
export function TimerHelper() {
  setTimeout(() => {
    const read = () => count();
    console.log(read());
  }, 0);
  return <div />;
}

// A read used to render once, from a timer callback: not a strict-read issue.
export function Deferred() {
  const [label, setLabel] = createSignal("");
  setTimeout(() => setLabel(String(count())), 0);
  return <div>{label()}</div>;
}

// Also silent, and not claimed: a source that never settled throws a plain
// NotReadyError here, as it does in a listener, but not the
// PENDING_ASYNC_UNTRACKED_READ the body read raises (see README.md).
export function TimerAsync() {
  const user = createMemo(async () => 1);
  setTimeout(() => console.log(user()), 0);
  return <div />;
}

// Positive: the same async read in the body.
export function BodyAsync() {
  const user = createMemo(async () => 1);
  console.log(user());
  return <div />;
}

// Uncertifiable: each can run on its invoker's stack, in or after the window
declare const thenable: PromiseLike<number>;
declare const target: HTMLElement;
export function Listener() {
  target.addEventListener("click", () => console.log(count()));
  return <div />;
}

export function Thenable() {
  thenable.then(() => console.log(count()));
  return <div />;
}

export function Bound() {
  const log = (read: () => number) => console.log(read());
  log.bind(null, () => count());
  return <div />;
}

// Retained: a local function spelled like a scheduler is not the
// standard-library declaration, and runs its callback in the body.
export function Shadowed() {
  const setTimeout = (fn: () => void, _delay: number) => fn();
  setTimeout(() => console.log(count()), 0);
  return <div />;
}

// Uncertifiable: the scheduler receives what `wrap` returns, and `wrap`'s
// body does not call the arrow during the call, so nothing places it.
function wrap(fn: () => void): () => void {
  return fn;
}
export function Wrapped() {
  setTimeout(
    wrap(() => console.log(count())),
    0,
  );
  return <div />;
}
