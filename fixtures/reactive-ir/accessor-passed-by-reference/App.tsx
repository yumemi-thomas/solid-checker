import { createSignal } from "solid-js";
import { readNow } from "./helpers";

// Calls the parameter in its own body, during the call.
export function run(read: () => number) {
  return read();
}

// Calls the parameter from a timer, after the call returns.
export function later(read: () => number) {
  setTimeout(() => read(), 0);
}

// Keeps the parameter; something else may call it later.
const pending: (() => number)[] = [];
export function keep(read: () => number) {
  pending.push(read);
}

// Returns a closure that calls the parameter.
export function wrap(read: () => number) {
  return () => read();
}

// Calls the parameter from an async body, after its first await.
export async function settle(read: () => number) {
  await Promise.resolve();
  return read();
}

// Violation: `run` reads `count` during the call, in the component body.
export function ReadByHelper() {
  const [count] = createSignal(0);
  const value = run(count);
  return <p>{value}</p>;
}

// Violation: the same through a helper in another module.
export function ReadByImportedHelper() {
  const [count] = createSignal(0);
  const value = readNow(count);
  return <p>{value}</p>;
}

// Clean: the call is in a tracked JSX expression.
export function TrackedCall() {
  const [count] = createSignal(0);
  return <p>{run(count)}</p>;
}

// Clean: an event handler is not a tracked scope's body.
export function InHandler() {
  const [count] = createSignal(0);
  return <button onClick={() => run(count)}>go</button>;
}

// Clean: the timer reads `count` after the body.
export function ReadLater() {
  const [count] = createSignal(0);
  later(count);
  return <p>later</p>;
}

// Clean: `keep` only stores the accessor.
export function Stored() {
  const [count] = createSignal(0);
  keep(count);
  return <p>kept</p>;
}

// Clean: the read is in the closure `wrap` returns, which nothing calls here.
export function Wrapped() {
  const [count] = createSignal(0);
  const reader = wrap(count);
  return <button onClick={() => reader()}>read</button>;
}

// Clean: an async callee's read is not proven to run during the call.
export function Settled() {
  const [count] = createSignal(0);
  void settle(count);
  return <p>settled</p>;
}
