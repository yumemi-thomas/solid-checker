import { createMemo, createSignal, createStore, createRoot, runWithOwner, getOwner, untrack, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function HelperReadTracked() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  function read() { return value(); }
  const data = createMemo(async () => {
    tick();
    read();
    await pause();
    return [0].map(() => value())[0];
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function GetterReadTracked() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const obj = { get n() { return value(); } };
  const data = createMemo(async () => {
    tick();
    obj.n;
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function CallbackReadTracked() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    [0].forEach(() => value());
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function StoreSharedRoots() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const item = { n: 1 };
  const [store, setStore] = createStore({ a: item, b: item });
  setTimeout(() => setStore((draft) => { draft.a.n = 2; }), 160);
  const data = createMemo(async () => {
    tick();
    store.a.n;
    store.b;
    await pause();
    return store.b.n;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function StoreAliasTracked() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const [store] = createStore({ a: 1 });
  const alias = store;
  const data = createMemo(async () => {
    tick();
    store.a;
    await pause();
    return alias.a;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function HelperUntouchedControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  function read() { return 0; }
  const data = createMemo(async () => {
    tick();
    await pause();
    return [0].map(() => value())[0];
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function GetterUntouchedControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const obj = { get n() { return value(); } };
  const data = createMemo(async () => {
    tick();
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function StoreOtherKeyControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const item = { n: 1 };
  const [store, setStore] = createStore({ a: item, b: item });
  setTimeout(() => setStore((draft) => { draft.a.n = 2; }), 160);
  const data = createMemo(async () => {
    tick();
    store.b;
    await pause();
    return store.b.n;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function OtherSignalHelperControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const [second] = createSignal(0);
  function other() { return second(); }
  const data = createMemo(async () => {
    tick();
    other();
    await pause();
    return value();
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}
