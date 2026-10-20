import { createMemo, createSignal, createStore, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Controls for after-await-quiet: the runtime does warn when the helper is awaited, awaited inside Promise.all, called by a synchronous computation, or awaits a timer.

// r42
export function AsyncHelperTimerAdoptedControl() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return load();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r44
export function AwaitedAllAwaitZeroControl() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return (await Promise.all([load()]))[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r47
export function ReturnedAwaitZeroHelperControl() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await load();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r50
export function SyncComputeReturnsAwaitZeroHelper() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(() => {
    tick();
    return load();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}
