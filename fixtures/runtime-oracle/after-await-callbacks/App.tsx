import { createMemo, createSignal, createStore, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function MapCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].map(() => value())[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ForEachCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    let seen = 0;
    [1].forEach(() => {
      seen = value();
    });
    return seen;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ImmediatelyInvoked() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return (() => value())();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function NestedFilter() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].filter(() => [1].filter(() => value()).length)[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function MapInsideIife() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return (() => [1].map(() => value()))()[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// One signal per method: the runtime reports each source once per computation.
export function StandardFamily() {
  const [a] = createSignal(1);
  const [b] = createSignal(1);
  const [c] = createSignal(1);
  const [d] = createSignal(1);
  const [e] = createSignal(1);
  const [f] = createSignal(1);
  const [g] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    const some = [1].some(() => a() > 0);
    const every = [1].every(() => b() > 0);
    const found = [1].find(() => c() > 0);
    const index = [1].findIndex(() => d() > 0);
    const flat = [1].flatMap(() => [e()]);
    const sum = [1].reduce((total) => total + f(), 0);
    const sorted = [2, 1].sort(() => g());
    return String([some, every, found, index, flat, sum, sorted]);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function TryWrappedAwait() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    try {
      await pause();
      return [1].map(() => value())[0];
    } catch {
      return 0;
    }
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function StoreReadInCallback() {
  const [state] = createStore({ count: 1 });
  const data = createMemo(async () => {
    await pause();
    return [1].map(() => state.count)[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}
