import { createMemo, createRoot, createSignal, runWithOwner, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// A named root callback that mentions its dispose parameter: the root may be
// disposed before the continuation resumes, and the runtime stays silent.
export function DisposedNamedRoot() {
  const [value] = createSignal(1);
  function start(dispose: () => void) {
    const data = createMemo(async () => {
      await pause();
      console.log("continued read", value());
      return 0;
    });
    createMemo(() => data());
    setTimeout(dispose, 5);
  }
  createRoot(start);
  return <span>disposed</span>;
}

export function DisposedNullOwner() {
  const [value] = createSignal(1);
  function start(dispose: () => void) {
    const data = createMemo(async () => {
      await pause();
      console.log("continued read", runWithOwner(null, () => value()));
      return 0;
    });
    createMemo(() => data());
    setTimeout(dispose, 5);
  }
  createRoot(start);
  return <span>disposed</span>;
}

// `undefined` shadowed by a rejected promise is a value, not the global.
export function ShadowedUndefinedHandler() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0)
      .then(() => {
        const undefined = Promise.reject("expected");
        return undefined;
      })
      .then(() => value())
      .catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ShadowedUndefinedResolve() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    const undefined = Promise.reject("expected");
    return await Promise.resolve(undefined)
      .then(() => value())
      .catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ShadowedUndefinedTimer() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    const undefined = new Promise<number>(() => {});
    return await new Promise<number>((done) => setTimeout(done, 20, undefined)).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ShadowedOwnerUndefined() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    const undefined = null;
    await pause();
    return runWithOwner(undefined, () => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

// A handler that calls a function that never returns throws, so the chain
// rejects and the fulfillment handler below it never runs.
export function HandlerCallsNever() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  function fail(): never {
    throw new Error("expected");
  }
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0)
      .then(() => {
        fail();
        return 0;
      })
      .then(() => value())
      .catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

// Controls: the global `undefined` fulfills, so the handler runs and warns.
export function GlobalUndefinedResolveControl() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(undefined).then(() => value());
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function HandlerCallsVoidControl() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  function log() {}
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0)
      .then(() => {
        log();
        return 0;
      })
      .then(() => value())
      .catch(() => 0);
  });
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function NamedRootWithoutDisposeControl() {
  const [value] = createSignal(1);
  function start(_dispose: () => void) {
    const data = createMemo(async () => {
      await pause();
      return value();
    });
    createMemo(() => data());
  }
  createRoot(start);
  return <span>kept</span>;
}
