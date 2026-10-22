import { createMemo, createSignal, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function ThenCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ThenOnNewPromise() {
  const [value] = createSignal(1);
  const data = createMemo(() => new Promise<void>((done) => setTimeout(done, 20)).then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function FinallyCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().finally(() => void value()));
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function CatchCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.reject(new Error("no")).catch(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ChainedThen() {
  const [value] = createSignal(1);
  const data = createMemo(() =>
    pause()
      .then(() => 1)
      .then(() => value()),
  );
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AwaitedThen() {
  const [value] = createSignal(1);
  const data = createMemo(async () => await pause().then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ThenWithMapInside() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => [1].map(() => value())[0]));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ThenReturnedFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => pause().then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ThenOnAllReturnedFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => Promise.all([1]).then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// The handler runs after the awaiting function returned, from a microtask: the
// returned promise carries it, so the filter inside it reads past the await.
// The receiver is pending (a timer), so the memo has adopted the returned
// promise before the handler runs and the async stack reaches the computation.
// Over an already-resolved receiver the handler is queued before the adoption
// job, no frame of the computation is below it, and `attributedFlight` finds
// none: the runtime is silent on every run (ADR 0340, SC1002 stays
// uncertifiable there; the shapes are in `read-after-await-reach`).
export function ThenAfterAwaitFilter() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return pause().then(() => [1, 2].filter(() => value() > 0));
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}
