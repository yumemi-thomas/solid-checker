import { createSignal, onCleanup } from "solid-js";

const [flag, setFlag] = createSignal(false);
const pause = () => Promise.resolve();
const makeHandle = (): number => 1;
function track(handle: unknown) {
  void handle;
}

// ADR 0984. A `let` is an inert argument when every reference of it is a proven write (a plain `=` of a call of the unpatched `setTimeout` or `setInterval`, or `undefined`) or the sole
// argument of a `clearTimeout` or `clearInterval`, in a browser (a handle is a number there; in Node it is an object). The program writes no built-in and names the browser runtime
// target (`.solid-checker/runtime.json`).

// Cleared: the reproduction case `clean-copied-timer`'s shape: the handle is cleared in a promise handler and in a cleanup.
export function Cleared() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => clearTimeout(timer));
  void pause().then(() => {
    clearTimeout(timer);
    timer = setTimeout(() => setFlag(false), 1500);
  });
  return <p>x</p>;
}

// Cleared: after the function's own suspension, an interval.
export async function ClearedInterval() {
  let ticker: ReturnType<typeof setInterval> | undefined;
  await pause();
  ticker = setInterval(() => setFlag(true), 1000);
  clearInterval(ticker);
}

// Kept: a write by a call no fact resolves.
export function UnknownWrite() {
  let handle: number | undefined;
  handle = makeHandle();
  void pause().then(() => {
    clearTimeout(handle);
  });
  return <p>x</p>;
}

// Kept: the handle is copied into an object (a second name of the value, read by a reference that is no clear).
export function Copied() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  timer = setTimeout(() => setFlag(false), 10);
  const holder = { timer };
  void holder;
  void pause().then(() => {
    clearTimeout(timer);
  });
  return <p>x</p>;
}

// Kept: the handle is handed to a function before the clear.
export function Handed() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  timer = setTimeout(() => setFlag(false), 10);
  track(timer);
  void pause().then(() => {
    clearTimeout(timer);
  });
  return <p>x</p>;
}

// Kept: a compound write and a destructured write.
export function Compound() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  [timer] = [setTimeout(() => setFlag(false), 10)];
  void pause().then(() => {
    clearTimeout(timer);
  });
  return <p>x</p>;
}

// Kept: the handle is written by a nested function through a name the call does not resolve as a standard timer (`window.setTimeout` names a member a project may replace).
export function Member() {
  let timer: number | undefined;
  timer = window.setTimeout(() => setFlag(false), 10);
  void pause().then(() => {
    clearTimeout(timer);
  });
  return <p>x</p>;
}

// Kept: a `const` copy of a `let` handle.
export function ConstCopy() {
  let timer: ReturnType<typeof setTimeout> | undefined;
  timer = setTimeout(() => setFlag(false), 10);
  const copy = timer;
  void pause().then(() => {
    clearTimeout(copy);
  });
  return <p>x</p>;
}
