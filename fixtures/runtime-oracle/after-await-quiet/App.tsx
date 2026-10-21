import { createMemo, createSignal, createStore, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Shapes the rc.13 runtime raises nothing for although a read follows an await (round-2 review of ADR 0340): a helper returned without await whose first await is on a settled value; a fulfillment handler on a receiver that rejects; a reducer without an initial value over one element; a store key already read before the first suspension. Each re-runs on a timer. SC1002 keeps them uncertifiable or silent.

// r01
export function TimerRejectAdopted() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return new Promise<number>((_, reject) => setTimeout(reject, 10)).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r02
export function TimerRejectHeld() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return await new Promise<number>((_, reject) => setTimeout(reject, 10)).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r03
export function AllRejectHeld() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    await pause();
    return await Promise.all([Promise.reject(new Error("x"))]).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r04
export function RaceRejectAdopted() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return Promise.race([Promise.reject(new Error("x"))]).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r05
export function AnyRejectHeld() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return await Promise.any([Promise.reject(new Error("x"))]).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r06
export function RejectFinallyThen() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return await Promise.reject(new Error("x")).finally(() => {}).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r07
export function RejectThenThen() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return await Promise.reject<number>(new Error("x")).then((x) => x).then(() => value()).catch(() => 0);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r09
export function ReduceOneNoInit() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    await pause();
    return [1].reduce((a, b) => a + b + value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r10
export function ReduceRightOneNoInit() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    await pause();
    return [1].reduceRight((a, b) => a + b + value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r20
export function StoreTrackedBefore() {
  const [store] = createStore({ a: 1 });
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    store.a;
    await pause();
    return store.a;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r40
export function AsyncHelperAwaitZeroAdopted() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return load();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r41
export function AsyncHelperResolvedAllAdopted() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await Promise.resolve();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return Promise.all([load()]);
  });
  return <Loading fallback="loading"><span>{data()?.[0]}</span></Loading>;
}

// r43
export function NestedAdoptedHelper() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function inner() {
    await 0;
    return value();
  }
  async function outer() {
    return inner();
  }
  const data = createMemo(async () => {
    tick();
    return await outer();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// r45
export function ReturnedAwaitZeroHelperAfterAwait() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    tick();
    await pause();
    return load();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}
