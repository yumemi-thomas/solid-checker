import { createMemo, createSignal, createStore, createRoot, runWithOwner, getOwner, untrack, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function UntrackedAsyncHelper() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  async function load() {
    value();
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await untrack(() => load());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function TrackedHelperControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  async function load() {
    value();
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await load();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function NullOwnerCallback() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    await pause();
    return (() => runWithOwner(null, () => value()))();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function CapturedOwnerControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    const owner = getOwner();
    await pause();
    return runWithOwner(owner, () => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwait() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {}
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwaitReadBeforeControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    value();
    for await (const _ of [1]) {}
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}
