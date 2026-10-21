import { createMemo, createSignal, createStore, getOwner, runWithOwner, untrack, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// `for await` suspends before its body and after the loop: reads in either are
// obligations, never a silent clean.
export function ForAwaitStore() {
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const [store, setStore] = createStore({ n: 1 });
  setTimeout(() => setStore((draft) => { draft.n = 2; }), 160);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
    }
    return store.n;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwaitCallback() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
    }
    return [1].map(() => value())[0];
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwaitHelper() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  function read() {
    return value();
  }
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
    }
    return read();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwaitBody() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
      return value();
    }
    return 0;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwaitBodyStore() {
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    const [store] = createStore({ n: 1 });
    for await (const _ of [1]) {
      return store.n;
    }
    return 0;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ForAwaitBodyReadBeforeControl() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    value();
    for await (const _ of [1]) {
      return value();
    }
    return 0;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

// `untrack(load)` and `runWithOwner(null, async ...)` hand the computation the
// promise of a function written by reference or async.
export function UntrackDirectHelper() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() {
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function NullOwnerAsyncCallback() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => {
      await pause();
      return value();
    });
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function CapturedOwnerAsyncCallback() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    const owner = getOwner();
    return await runWithOwner(owner, async () => {
      await pause();
      return value();
    });
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}
