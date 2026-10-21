import { createMemo, createSignal, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 0349: the Promise and array-iteration premises hold here, so a handler
// on a native receiver in a synchronous memo is a violation (V).
// A visible write to an unrelated object does not withdraw them.
const holder = { value: 1 };
holder.value = 2;

// V: a handler on a native `pause()` promise.
export function NativeThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <div>{String(data())}</div>;
}

// V: a handler on a `new Promise` the executor resolves with a timer.
export function NativeNewPromise() {
  const [value] = createSignal(1);
  const data = createMemo(() => new Promise<void>((done) => setTimeout(done, 20)).then(() => value()));
  return <div>{String(data())}</div>;
}

// V: a chain of handlers, each on a native promise.
export function NativeChain() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => 1).then(() => value()));
  return <div>{String(data())}</div>;
}

// V: `finally` on a native promise.
export function NativeFinally() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().finally(() => void value()));
  return <div>{String(data())}</div>;
}

// V: `Promise.resolve` of a primitive fulfills.
export function NativeResolvePrimitive() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.resolve(1).then(() => value()));
  return <div>{String(data())}</div>;
}

// V: `Promise.all` over an array literal of primitives fulfills (ADR 0349).
export function NativeAllPrimitives() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.all([1]).then(() => value()));
  return <div>{String(data())}</div>;
}

// V: `Promise.allSettled` over a rejected native element fulfills.
export function NativeAllSettledRejected() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.allSettled([Promise.reject(new Error("x"))]).then(() => value()));
  return <div>{String(data())}</div>;
}

// V: `Promise.all` of async helpers the computation awaits: each helper's read
// after its own await is in the flight.
export function NativeAllOfHelpers() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => (await Promise.all([read(), read()])).length);
  return <div>{String(data())}</div>;
}

// V: `Math.random()` before the await is a primitive call result (ADR 0349).
export function NativeMathRandomBranch() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    if (Math.random() > 0.5) await pause();
    else await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// V: the await-boundary coercion of an awaited value (the operand suspends).
export function NativeAwaitBoundaryCoercion() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return value();
  }
  async function outer() {
    return (await inner()) + 1;
  }
  const data = createMemo(() => outer());
  return <div>{String(data())}</div>;
}

// U: a `fetch` receiver is native, but its outcome is unknown: a fulfillment
// handler on it is an obligation.
export function NativeFetchOutcome() {
  const [value] = createSignal(1);
  const data = createMemo(() => fetch("/api").then(() => value()));
  return <div>{String(data())}</div>;
}

// Quiet: a typed parameter is not a native promise, so nothing is claimed.
export function TypedParameterReceiver(p: Promise<number>) {
  const [value] = createSignal(1);
  const data = createMemo(() => p.then(() => value()));
  return <div>{String(data())}</div>;
}
