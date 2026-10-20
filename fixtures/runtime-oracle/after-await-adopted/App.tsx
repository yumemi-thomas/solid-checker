import { createMemo, createSignal, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

async function one() {
  return 1;
}

// An `async` function that returns a promise chain it did not await adopts the
// promise in a later job. A handler on an already-settled receiver is queued
// before that job, so no frame of the computation is below it and the runtime
// raises nothing, on the first run or after a re-run. The checker keeps these
// uncertifiable (ADR 0340); every case in this group but the last four is
// quiet at runtime.

export function ResolvedThenReturned() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(async () => {
    tick();
    return Promise.resolve(1).then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ResolvedThenReturnedAfterAwait() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.resolve(1).then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AwaitlessHelperThenReturned() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return one().then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function RejectedCatchReturned() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.reject(new Error("no")).catch(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ResolvedFinallyReturned() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.resolve(1).finally(() => void value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function HelperReturnsResolvedThen() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return Promise.resolve(1).then(() => value());
  }
  const data = createMemo(() => inner());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// A rejection handler on a pending receiver that never rejects, and a
// fulfillment handler on a receiver that is already rejected, never run.
export function CatchOfPending() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().catch(() => value()));
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function RejectionHandlerOfThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => 1, () => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// The runtime does warn for these: the call is awaited, the receiver is
// pending past the adoption job, or the computation is synchronous.
export function ResolvedThenAwaited() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return await Promise.resolve(1).then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function PendingThenReturned() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return pause().then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function SyncResolvedThenReturned() {
  const [value] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 50);
  const data = createMemo(() => {
    tick();
    return Promise.resolve(1).then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}
