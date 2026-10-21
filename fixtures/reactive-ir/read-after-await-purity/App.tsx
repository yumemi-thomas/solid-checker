// SC1002 under ADR 0348: argument purity and the synchronous prefix. Each export
// is one review case; its id is the comment above it. Verdicts the checker keeps:
//  - a01-a14, a16-a20, r23-r27: a standard call before the await may read the
//    source, so the read after it is uncertifiable (U). a21 (`new Array(object)`: an element,
//    not a length), c01 (`Math.max(1)`) and d01-d03 stay violations (V).
//  - s01-s04, s07, s08, s11, r12, r17, u01, u04-u07, u09, u12: a prefix or try-head
//    read is a violation (V). s09, s10, u10, u11, r16, r18, r22, r28: U.
//  - u02, u03, u08, s05, s06, n01-n03, n05: nothing reported. n04: U. n06: V.
import { createMemo, createRoot, createSignal, createStore, getOwner, runWithOwner, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// a01-math-valueof
export function CaseA01MathValueof() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { valueOf: () => value() };
  const data = createMemo(async () => {
    tick();
    Math.max(o as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a02-map-iter
export function CaseA02MapIter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); } };
  const data = createMemo(async () => {
    tick();
    new Map(iter as Iterable<[number, number]>);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a03-set-iter
export function CaseA03SetIter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); yield 1; } };
  const data = createMemo(async () => {
    tick();
    new Set(iter);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a04-error-tostring
export function CaseA04ErrorTostring() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const m = { toString: () => { value(); return "x"; } };
  const data = createMemo(async () => {
    tick();
    new Error(m as unknown as string);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a05-promise-all-iter
export function CaseA05PromiseAllIter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); yield 1; } };
  const data = createMemo(async () => {
    tick();
    await Promise.all(iter);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a06-promise-resolve-getter
export function CaseA06PromiseResolveGetter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const t = { get then() { value(); return undefined; } };
  const data = createMemo(async () => {
    tick();
    await Promise.resolve(t as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a07-fetch-getter
export function CaseA07FetchGetter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const init = { get method() { value(); return "GET"; } };
  const data = createMemo(async () => {
    tick();
    await fetch("data:,x", init);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a08-date-valueof
export function CaseA08DateValueof() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { valueOf: () => value() };
  const data = createMemo(async () => {
    tick();
    new Date(o as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a09-parseint
export function CaseA09Parseint() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { toString: () => { value(); return "1"; } };
  const data = createMemo(async () => {
    tick();
    parseInt(o as unknown as string);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a10-settimeout-delay
export function CaseA10SettimeoutDelay() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { valueOf: () => { value(); return 1; } };
  const data = createMemo(async () => {
    tick();
    setTimeout(() => {}, o as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a11-encodeuri
export function CaseA11Encodeuri() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { toString: () => { value(); return "1"; } };
  const data = createMemo(async () => {
    tick();
    encodeURIComponent(o as unknown as string);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a12-regexp
export function CaseA12Regexp() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { toString: () => { value(); return "1"; } };
  const data = createMemo(async () => {
    tick();
    new RegExp(o as unknown as string);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a13-race-iter
export function CaseA13RaceIter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); yield 1; } };
  const data = createMemo(async () => {
    tick();
    await Promise.race(iter);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a14-math-spread
export function CaseA14MathSpread() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); yield 1; } };
  const data = createMemo(async () => {
    tick();
    Math.max(...iter);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a16-isnan
export function CaseA16Isnan() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { valueOf: () => { value(); return 1; } };
  const data = createMemo(async () => {
    tick();
    isNaN(o as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a17-map-array-proxy
export function CaseA17MapArrayProxy() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const arr = new Proxy([[1,1]] as [number,number][], { get(t, k, r) { value(); return Reflect.get(t, k, r); } });
  const data = createMemo(async () => {
    tick();
    new Map(arr);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a18-error-cause-getter
export function CaseA18ErrorCauseGetter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const opts = { get cause() { value(); return 1; } };
  const data = createMemo(async () => {
    tick();
    new Error("x", opts);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a19-promise-then-getter-thenable-handler
export function CaseA19PromiseThenGetterThenableHandler() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const h = { get then() { value(); return undefined; } };
  const data = createMemo(async () => {
    tick();
    await Promise.resolve(0).then(() => h as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a20-weakmap-iter
export function CaseA20WeakmapIter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); } };
  const data = createMemo(async () => {
    tick();
    new WeakMap(iter as Iterable<[object, number]>);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// a21-array-length
export function CaseA21ArrayLength() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = { valueOf: () => { value(); return 1; } };
  const data = createMemo(async () => {
    tick();
    new Array(o as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// c01-math-plain
export function CaseC01MathPlain() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    Math.max(1);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// c02-array-from
export function CaseC02ArrayFrom() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const iter = { *[Symbol.iterator]() { value(); yield 1; } };
  const data = createMemo(async () => {
    tick();
    Array.from(iter);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// d01-plain-const
export function CaseD01PlainConst() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    const x = value();
    return x;
  });
  return <div>{String(data())}</div>;
}

// d02-nothing
export function CaseD02Nothing() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// d03-log
export function CaseD03Log() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    console.log(value());
    return 0;
  });
  return <div>{String(data())}</div>;
}

// u01-untrack-early-return
export function CaseU01UntrackEarlyReturn() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const flag = tick() === 0 || true;
  async function load() { if (flag) return 0; await pause(); return value(); }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <div>{String(data())}</div>;
}

// u02-untrack-conditional-await
export function CaseU02UntrackConditionalAwait() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() { if (value() > 5) await pause(); return value(); }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <div>{String(data())}</div>;
}

// u03-untrack-read-before-await
export function CaseU03UntrackReadBeforeAwait() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() { const v = value(); await pause(); return v; }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <div>{String(data())}</div>;
}

// u04-null-async-noawait
export function CaseU04NullAsyncNoawait() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => value());
  });
  return <div>{String(data())}</div>;
}

// u05-null-async-early
export function CaseU05NullAsyncEarly() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const [f] = createSignal(true);
  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => { if (f()) return 0; await pause(); return value(); });
  });
  return <div>{String(data())}</div>;
}

// u06-null-async-awaited
export function CaseU06NullAsyncAwaited() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => { await pause(); return value(); });
  });
  return <div>{String(data())}</div>;
}

// u07-untrack-load-awaits-then-read
export function CaseU07UntrackLoadAwaitsThenRead() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() { await pause(); return value(); }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <div>{String(data())}</div>;
}

// u08-untrack-nonawaited
export function CaseU08UntrackNonawaited() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() { await pause(); return value(); }
  const data = createMemo(async () => {
    tick();
    untrack(load);
    return 0;
  });
  return <div>{String(data())}</div>;
}

// u09-untrack-load-read-before-await-then-read
export function CaseU09UntrackLoadReadBeforeAwaitThenRead() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() { value(); await pause(); return value(); }
  const data = createMemo(async () => {
    tick();
    return await untrack(() => load());
  });
  return <div>{String(data())}</div>;
}

// u10-untrack-load-returns-promise
export function CaseU10UntrackLoadReturnsPromise() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  function load() { return pause().then(() => value()); }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <div>{String(data())}</div>;
}

// u11-untrack-load-throws
export function CaseU11UntrackLoadThrows() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load(): Promise<number> { throw new Error("x"); }
  const data = createMemo(async () => {
    tick();
    try { return await untrack(load); } catch { return value(); }
  });
  return <div>{String(data())}</div>;
}

// u12-untrack-sync-then-await-after
export function CaseU12UntrackSyncThenAwaitAfter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load() { return 1; }
  const data = createMemo(async () => {
    tick();
    await untrack(load);
    return value();
  });
  return <div>{String(data())}</div>;
}

// s01-null-async-arrow
export function CaseS01NullAsyncArrow() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => value());
  });
  return <div>{String(data())}</div>;
}

// s02-async-iife
export function CaseS02AsyncIife() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return await (async () => value())();
  });
  return <div>{String(data())}</div>;
}

// s03-async-helper-noawait
export function CaseS03AsyncHelperNoawait() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function h() { return value(); }
  const data = createMemo(async () => {
    tick();
    await pause();
    return await h();
  });
  return <div>{String(data())}</div>;
}

// s04-null-named-async
export function CaseS04NullNamedAsync() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function h() { return value(); }
  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, h);
  });
  return <div>{String(data())}</div>;
}

// s05-untrack-after-await-sync
export function CaseS05UntrackAfterAwaitSync() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return untrack(() => value());
  });
  return <div>{String(data())}</div>;
}

// s06-untrack-after-await-async
export function CaseS06UntrackAfterAwaitAsync() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return await untrack(async () => value());
  });
  return <div>{String(data())}</div>;
}

// s07-try-catch-throw
export function CaseS07TryCatchThrow() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load(): Promise<number> { throw new Error("x"); }
  const data = createMemo(async () => {
    tick();
    try { return await load(); } catch { return value(); }
  });
  return <div>{String(data())}</div>;
}

// s08-try-catch-pause-throw
export function CaseS08TryCatchPauseThrow() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load(): Promise<number> { await pause(); throw new Error("x"); }
  const data = createMemo(async () => {
    tick();
    try { return await load(); } catch { return value(); }
  });
  return <div>{String(data())}</div>;
}

// s09-try-reject
export function CaseS09TryReject() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    try { await Promise.reject(1); } catch { }
    return value();
  });
  return <div>{String(data())}</div>;
}

// s10-untrack-throws-direct
export function CaseS10UntrackThrowsDirect() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  async function load(): Promise<number> { throw new Error("x"); }
  const data = createMemo(async () => {
    tick();
    try { await untrack(load); return 0; } catch { return value(); }
  });
  return <div>{String(data())}</div>;
}

// s11-null-owner-async-early-ret-flag
export function CaseS11NullOwnerAsyncEarlyRetFlag() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const flag = () => true;
  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => { if (flag()) return value(); await pause(); return 0; });
  });
  return <div>{String(data())}</div>;
}

// n01-null-sync-noawait
export function CaseN01NullSyncNoawait() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    return runWithOwner(null, () => value());
  });
  return <div>{String(data())}</div>;
}

// n02-null-async-noawait-nooutter
export function CaseN02NullAsyncNoawaitNooutter() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    return await runWithOwner(null, async () => value());
  });
  return <div>{String(data())}</div>;
}

// n03-null-async-await-in-inner-read-before
export function CaseN03NullAsyncAwaitInInnerReadBefore() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    return await runWithOwner(null, async () => { const v = value(); await pause(); return v; });
  });
  return <div>{String(data())}</div>;
}

// n04-untrack-async-null-owner-var
export function CaseN04UntrackAsyncNullOwnerVar() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const o = getOwner();
  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(o, async () => { await pause(); return value(); });
  });
  return <div>{String(data())}</div>;
}

// n05-null-async-conditional-await
export function CaseN05NullAsyncConditionalAwait() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    return await runWithOwner(null, async () => { if (value() > 5) await pause(); return value(); });
  });
  return <div>{String(data())}</div>;
}

// n06-null-async-await-then-read-store
export function CaseN06NullAsyncAwaitThenReadStore() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  const [s] = createStore({n: 1});
  const data = createMemo(async () => {
    tick();
    return await runWithOwner(null, async () => { await pause(); return s.n; });
  });
  return <div>{String(data())}</div>;
}

// r12-loader-after-await-async-foreach
export function CaseR12LoaderAfterAwaitAsyncForeach() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    [1].forEach(async () => { value(); });
    return 0;
  });
  return <div>{String(data())}</div>;
}

// r16-loader-catch-sync-negative
export function CaseR16LoaderCatchSyncNegative() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  function load(): never { throw new Error("sync"); }
  const data = createMemo(async () => {
    tick();
    try { await load(); } catch { return value(); }
  });
  return <div>{String(data())}</div>;
}

// r17-loader-after-await-callback-prefix
export function CaseR17LoaderAfterAwaitCallbackPrefix() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    await pause();
    return [1].map(async () => value())[0];
  });
  return <div>{String(data())}</div>;
}

// r18-loader-coercion-before
export function CaseR18LoaderCoercionBefore() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const data = createMemo(async () => {
    tick();
    const boxed = {valueOf() { value(); return 1; }};
    Math.max(+boxed);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r22-timer-callback-coercion
export function CaseR22TimerCallbackCoercion() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const callback = { toString() { value(); return "0"; } };
  const data = createMemo(async () => {
    tick();
    setTimeout(callback as unknown as TimerHandler, 1);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r23-proxy-constructor
export function CaseR23ProxyConstructor() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const Constructor = new Proxy(Map, {construct(target, args) { value(); return Reflect.construct(target, args); }});
  const data = createMemo(async () => {
    tick();
    new Constructor();
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r24-proxy-math
export function CaseR24ProxyMath() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const math = new Proxy(Math, {get(target, key, receiver) { value(); return Reflect.get(target, key, receiver); }});
  const data = createMemo(async () => {
    tick();
    math.max(1);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r25-shadow-math
export function CaseR25ShadowMath() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const Math = { max(n: number) { value(); return n; } };
  const data = createMemo(async () => {
    tick();
    Math.max(1);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r26-alias-math
export function CaseR26AliasMath() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const max = Math.max; const object = {valueOf() { value(); return 1; }};
  const data = createMemo(async () => {
    tick();
    max(object as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r27-delete-argument-proxy
export function CaseR27DeleteArgumentProxy() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);

  const proxy = new Proxy({x:1} as {x?:number}, {deleteProperty(t, k) { value(); return Reflect.deleteProperty(t,k); }});
  const data = createMemo(async () => {
    tick();
    Math.max(delete proxy.x as unknown as number);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// r28-catch-reassigned-sync
export function CaseR28CatchReassignedSync() {
  const [value, setValue] = createSignal(1);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  setTimeout(() => setValue(2), 160);
  let load = async (): Promise<number> => { await pause(); return 1; };
  load = function(): Promise<number> { throw new Error("sync"); };
  const data = createMemo(async () => {
    tick();
    try { await load(); } catch { return value(); }
  });
  return <div>{String(data())}</div>;
}
