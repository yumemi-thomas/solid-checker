import { createMemo, createSignal, createStore, createRoot, runWithOwner, getOwner, untrack, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function ThenRejects() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0).then(() => Promise.reject("expected")).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function FinallyRejects() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0).finally(() => Promise.reject("expected")).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function CatchRejects() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.reject("first").catch(() => Promise.reject("expected")).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ThenThrows() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0).then(() => { throw new Error("expected"); }).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function AllSettledNever() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.allSettled([new Promise<void>(() => {})]).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function TimerResolvesWithRejection() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const rejected = Promise.reject("expected");
  void rejected.catch(() => 0);
  const data = createMemo(async () => {
    tick();
    return await new Promise<number>((done) => setTimeout(done, 20, rejected)).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function TimerResolvesWithNever() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const never = new Promise<number>(() => {});
  const data = createMemo(async () => {
    tick();
    return await new Promise<number>((done) => setTimeout(done, 20, never)).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function FinallyOnNever() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    await new Promise<void>(() => {}).finally(() => { value(); });
    return 0;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ThenReturnsNever() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0).then(() => new Promise<void>(() => {})).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ThenPlainControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await pause().then(() => 0).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function TimerPlainValueControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await new Promise<number>((done) => setTimeout(done, 20, 5)).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function FinallyControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    await pause().finally(() => { value(); });
    return 0;
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function AllSettledControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.allSettled([pause()]).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ThenReturnsFulfillingControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await pause().then(() => Promise.resolve(0)).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}
