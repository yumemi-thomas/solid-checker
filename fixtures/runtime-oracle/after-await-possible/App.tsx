import { createMemo, createSignal, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// A call before the first await that is not an exactly resolved project
// function may run a callback captured earlier: here the listener reads
// `value`, so the memo tracks it and the read after the await is silent at
// runtime. The checker's finding is uncertifiable (no runtime diagnostic).
export function GlobalEventDispatch() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  window.addEventListener("review-track", () => value());
  const data = createMemo(async () => {
    tick();
    window.dispatchEvent(new Event("review-track"));
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function FunctionCallInvokes() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const captured = () => value();
  const data = createMemo(async () => {
    tick();
    captured.call(undefined);
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

// Control: only audited inert calls run before the await, so the read after it
// is a violation and the runtime warns.
export function InertCallsControl() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    Math.max(tick(), 1);
    setTimeout(() => {}, 0);
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}
