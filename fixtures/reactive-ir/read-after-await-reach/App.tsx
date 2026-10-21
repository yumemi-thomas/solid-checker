import { createMemo, createRoot, createSignal, createStore, getOwner, runWithOwner, untrack } from "solid-js";

// SC1002 reaches the code an async computation's continuation runs, not only
// the awaiting function's own body. Every positive below is an
// `UNTRACKED_READ_AFTER_AWAIT` on `@solidjs/signals@2.0.0-rc.13`; the same
// shapes are runtime-oracle cases (`fixtures/runtime-oracle/after-await-*`).

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ---- A synchronous callback an exact standard-library call invokes, and an
// immediately invoked function expression, evaluated after the await.

export function MapCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].map(() => value())[0];
  });
  return <div>{data()}</div>;
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
  return <div>{data()}</div>;
}

export function ImmediatelyInvoked() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return (() => value())();
  });
  return <div>{data()}</div>;
}

export function NestedFilter() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].filter(() => [1].filter(() => value()).length)[0];
  });
  return <div>{data()}</div>;
}

export function MapInsideIife() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return (() => [1].map(() => value()))()[0];
  });
  return <div>{data()}</div>;
}

export function StandardFamily() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    const some = [1].some(() => value() > 0);
    const every = [1].every(() => value() > 0);
    const found = [1].find(() => value() > 0);
    const index = [1].findIndex(() => value() > 0);
    const flat = [1].flatMap(() => [value()]);
    const sum = [1].reduce((total) => total + value(), 0);
    return [some, every, found, index, flat, sum];
  });
  return <div>{String(data())}</div>;
}

// A `try` around the await: accessor-call dominance holds, the straight-line
// member-read proof does not, and the callback is still reached.
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
  return <div>{data()}</div>;
}

export function StoreReadInCallback() {
  const [state] = createStore({ count: 1 });
  const data = createMemo(async () => {
    await pause();
    return [1].map(() => state.count)[0];
  });
  return <div>{data()}</div>;
}

// ---- A `.then`/`.catch`/`.finally` callback on a proven native promise whose
// result is the computation's promise.

export function ThenCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <div>{data()}</div>;
}

export function ThenOnNewPromise() {
  const [value] = createSignal(1);
  const data = createMemo(() => new Promise<void>((done) => setTimeout(done, 20)).then(() => value()));
  return <div>{data()}</div>;
}

export function FinallyCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().finally(() => void value()));
  return <div>{data()}</div>;
}

export function ChainedThen() {
  const [value] = createSignal(1);
  const data = createMemo(() =>
    pause()
      .then(() => 1)
      .then(() => value()),
  );
  return <div>{data()}</div>;
}

export function AwaitedThen() {
  const [value] = createSignal(1);
  const data = createMemo(async () => await pause().then(() => value()));
  return <div>{data()}</div>;
}

export function ThenWithMapInside() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => [1].map(() => value())[0]));
  return <div>{data()}</div>;
}

// ---- An async project function the computation's promise awaits reads after
// its own first await.

export function AsyncHelperReturned() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(() => read());
  return <div>{data()}</div>;
}

export function AsyncHelperAwaited() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => await read());
  return <div>{data()}</div>;
}

export function AsyncHelperCallback() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return [1].map(() => value())[0];
  }
  const data = createMemo(async () => {
    const result = await read();
    return result;
  });
  return <div>{data()}</div>;
}

// A helper that calls a helper: the inner one's promise is awaited by the
// outer one, which the computation awaits.
export function AsyncHelperChain() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return value();
  }
  async function outer() {
    return (await inner()) + 1;
  }
  const data = createMemo(() => outer());
  return <div>{data()}</div>;
}

export function PromiseAllOfHelpers() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => (await Promise.all([read(), read()])).length);
  return <div>{data()}</div>;
}

export function PromiseAllOfAsyncCallbacks() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await Promise.all(
      [1].map(async () => {
        await pause();
        return value();
      }),
    );
  });
  return <div>{data().join()}</div>;
}

// ---- A synchronous project helper runs on the continuation's stack: a read
// it makes on every call is an untracked read after the await.

export function SyncHelper() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  const data = createMemo(async () => {
    await pause();
    return read();
  });
  return <div>{data()}</div>;
}

export function SyncHelperInCallback() {
  const [value] = createSignal(1);
  const read = () => value();
  const data = createMemo(async () => {
    await pause();
    return [1].map(() => read())[0];
  });
  return <div>{data()}</div>;
}

export function SyncHelperInThen() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  const data = createMemo(() => pause().then(() => read()));
  return <div>{data()}</div>;
}

// A helper the helper calls on every completion runs on the same stack.
export function SyncHelperThroughHelper() {
  const [value] = createSignal(1);
  function inner() {
    return value();
  }
  function outer() {
    return inner() + 1;
  }
  const data = createMemo(async () => {
    await pause();
    return outer();
  });
  return <div>{data()}</div>;
}

// A read on some calls only is the interprocedural obligation, not a proof.
export function SyncHelperConditional(props: { flag: boolean }) {
  const [value] = createSignal(1);
  function read(flag: boolean) {
    return flag ? value() : 0;
  }
  const data = createMemo(async () => {
    await pause();
    return read(props.flag);
  });
  return <div>{data()}</div>;
}

// ---- Negatives.

// The read runs in the tracking window, before the await.
export function MapBeforeAwait() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    const rows = [1].map(() => value());
    await pause();
    return rows;
  });
  return <div>{data()}</div>;
}

// A callback stored and never invoked here runs, if ever, elsewhere.
export function StoredCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    const later = () => value();
    return later;
  });
  return <div>{typeof data()}</div>;
}

// A deferred callback does not run on the continuation's stack.
export function DeferredCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    setTimeout(() => void value(), 0);
    return 1;
  });
  return <div>{data()}</div>;
}

// A project `map` is not `Array.prototype.map`: what it does with the
// callback is not the standard library's.
export function ProjectMap() {
  const [value] = createSignal(1);
  const box = {
    map(callback: () => number) {
      return () => callback();
    },
  };
  const data = createMemo(async () => {
    await pause();
    return box.map(() => value());
  });
  return <div>{typeof data()}</div>;
}

// A shadowed `Promise` is not the global one, so its `then` proves nothing.
export function ShadowedPromise() {
  const [value] = createSignal(1);
  const Promise = { resolve: () => ({ then: (callback: () => number) => callback() }) };
  const data = createMemo(() => Promise.resolve().then(() => value()));
  return <div>{data()}</div>;
}

// A thenable or an unknown receiver may call its handler inline.
declare const thenable: PromiseLike<void>;
export function ThenOnThenable() {
  const [value] = createSignal(1);
  const data = createMemo(() => thenable.then(() => value()));
  return <div>{data()}</div>;
}

export function ThenOnUnknown(props: { source: Promise<number> }) {
  const [value] = createSignal(1);
  const data = createMemo(() => props.source.then(() => value()));
  return <div>{data()}</div>;
}

// A `.then` whose result is dropped is not on the computation's flight.
export function ThenResultDropped() {
  const [value] = createSignal(1);
  const data = createMemo(() => {
    void pause().then(() => value());
    return 1;
  });
  return <div>{data()}</div>;
}

// Positive: an async computation that returns the promise adopts it, so the
// handler is still on the computation's flight (runtime-oracle
// `ThenReturnedFromAsync`).
export function ThenReturnedFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => pause().then(() => value()));
  return <div>{data()}</div>;
}

// Positives: the same promise reaches the computation's flight by a path that
// holds a reaction on it before the handler runs.
export function ThenReturnedFromSync() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(() => {
    tick();
    return Promise.resolve(1).then(() => value());
  });
  return <div>{data()}</div>;
}

export function ThenReturnedAwaitedFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return await Promise.resolve(1).then(() => value());
  });
  return <div>{data()}</div>;
}

export function ThenOnPendingAfterAwait() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return pause().then(() => value());
  });
  return <div>{data()}</div>;
}

export function ThenOnPendingAllFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => Promise.all([1]).then(() => value()));
  return <div>{data()}</div>;
}

// Uncertifiable: an `async` function returns a promise it did not await. A
// receiver that may already be settled queues the handler before the function
// adopts the promise (a resolve-thenable job), so no frame of the computation
// is below the handler and rc.13 raises nothing, even on a re-run.
export function ResolvedThenReturnedFromAsync() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    return Promise.resolve(1).then(() => value());
  });
  return <div>{data()}</div>;
}

export function ResolvedThenReturnedAfterAwait() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.resolve(1).then(() => value());
  });
  return <div>{data()}</div>;
}

async function one() {
  return 1;
}

export function AwaitlessHelperThenReturned() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return one().then(() => value());
  });
  return <div>{data()}</div>;
}

export function RejectedCatchReturnedFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.reject(new Error("no")).catch(() => value());
  });
  return <div>{data()}</div>;
}

export function ResolvedFinallyReturnedFromAsync() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.resolve(1).finally(() => void value());
  });
  return <div>{data()}</div>;
}

export function HelperReturnsResolvedThen() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return Promise.resolve(1).then(() => value());
  }
  const data = createMemo(() => inner());
  return <div>{data()}</div>;
}

export function AwaitedHelperReturnsResolvedThen() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return Promise.resolve(1).then(() => value());
  }
  const data = createMemo(async () => await inner());
  return <div>{data()}</div>;
}

export function ResolvedChainSecondHandler() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.resolve(1)
      .then(() => 1)
      .then(() => value());
  });
  return <div>{data()}</div>;
}

// Uncertifiable: a rejection handler runs only if the receiver rejects, and a
// fulfillment handler never runs on a receiver that is provably rejected.
export function RejectionHandlerOfThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => 1, () => value()));
  return <div>{data()}</div>;
}

export function CatchOfPending() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().catch(() => value()));
  return <div>{String(data())}</div>;
}

export function CatchOfResolved() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return Promise.resolve(1).catch(() => value());
  });
  return <div>{data()}</div>;
}

export function CatchCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.reject(new Error("no")).catch(() => value()));
  return <div>{data()}</div>;
}

export function ThenOnRejected() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.reject(new Error("no")).then(() => value()));
  return <div>{data()}</div>;
}

// Positive: `finally` runs on both paths.
export function FinallyOfRejected() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.reject(new Error("no")).finally(() => void value()));
  return <div>{data()}</div>;
}

// Uncertifiable: whether the callback runs depends on the receiver's contents
// or on the match.
export function SortComparator() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].sort(() => value());
  });
  return <div>{String(data())}</div>;
}

export function ReplaceReplacer() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return "abc".replace("z", () => String(value()));
  });
  return <div>{data()}</div>;
}

export function ReplaceAllReplacer() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return "abc".replaceAll("z", () => String(value()));
  });
  return <div>{data()}</div>;
}

// A helper called from an event handler is not in a computation.
export function HelperInEventHandler() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  return <div onClick={() => void read()} />;
}

// A helper whose promise the computation ignores.
export function HelperResultIgnored() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(() => {
    void read();
    return 1;
  });
  return <div>{data()}</div>;
}

// A helper that reads before its own first await.
export function HelperReadsFirst() {
  const [value] = createSignal(1);
  async function read() {
    const seen = value();
    await pause();
    return seen;
  }
  const data = createMemo(() => read());
  return <div>{data()}</div>;
}

// A source the computation already tracked before it suspended.
export function AlreadyTracked() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    value();
    await pause();
    return [1].map(() => value())[0];
  });
  return <div>{data()}</div>;
}

export function AlreadyTrackedThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => {
    value();
    return pause().then(() => value());
  });
  return <div>{data()}</div>;
}

// `untrack` opens no tracking scope: the read is deliberate.
export function UntrackedCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return untrack(() => [1].map(() => value())[0]);
  });
  return <div>{data()}</div>;
}

export function UntrackedHelper() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return untrack(() => value());
  }
  const data = createMemo(() => read());
  return <div>{data()}</div>;
}

// A plain async function no computation runs.
export function PlainAsync() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return [1].map(() => value())[0];
  }
  void read();
  return <div />;
}

// A synchronous helper called before the await reads inside the window.
export function SyncHelperBeforeAwait() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  const data = createMemo(async () => {
    const seen = read();
    await pause();
    return seen;
  });
  return <div>{data()}</div>;
}

// A helper whose read is in a closure it returns does not read during the call.
export function SyncHelperReturnsClosure() {
  const [value] = createSignal(1);
  function make() {
    return () => value();
  }
  const data = createMemo(async () => {
    await pause();
    return make();
  });
  return <div>{typeof data()}</div>;
}

// A helper called from an event handler.
export function SyncHelperInHandler() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  return <div onClick={() => void read()} />;
}

// ---- An async helper the computation returns without `await` is adopted one
// job after the call returns. A first await on a settled value resumes the
// helper before then, with no frame of the computation below it, and the
// runtime raises nothing: uncertifiable. A first await on a proven pending
// promise, or an awaited call, is on the flight.

export function AdoptedHelperAwaitZero() {
  const [value] = createSignal(1);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    return load();
  });
  return <div>{data()}</div>;
}

export function AdoptedHelperAwaitSettledInAll() {
  const [value] = createSignal(1);
  async function load() {
    await Promise.resolve();
    return value();
  }
  const data = createMemo(async () => Promise.all([load()]));
  return <div>{String(data())}</div>;
}

export function AdoptedHelperBehindAdoptedHelper() {
  const [value] = createSignal(1);
  async function inner() {
    await 0;
    return value();
  }
  async function outer() {
    return inner();
  }
  const data = createMemo(async () => await outer());
  return <div>{data()}</div>;
}

export function AdoptedHelperAfterAwait() {
  const [value] = createSignal(1);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    await pause();
    return load();
  });
  return <div>{data()}</div>;
}

// Positives.
export function AdoptedHelperAwaitsTimer() {
  const [value] = createSignal(1);
  async function load() {
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    return load();
  });
  return <div>{data()}</div>;
}

export function AwaitedHelperAwaitZero() {
  const [value] = createSignal(1);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => {
    return await load();
  });
  return <div>{data()}</div>;
}

export function AwaitedAllOfHelperAwaitZero() {
  const [value] = createSignal(1);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(async () => (await Promise.all([load()]))[0]);
  return <div>{data()}</div>;
}

export function SyncComputationReturnsHelperAwaitZero() {
  const [value] = createSignal(1);
  async function load() {
    await 0;
    return value();
  }
  const data = createMemo(() => load());
  return <div>{data()}</div>;
}

// ---- A fulfillment handler counts only on a receiver proven to fulfill. A
// timer executor fulfills through the parameter the timer passes.

export function TimerRejectReturned() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return new Promise<number>((_, reject) => setTimeout(reject, 10)).then(() => value());
  });
  return <div>{data()}</div>;
}

export function TimerRejectAwaited() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await new Promise<number>((_, reject) => setTimeout(reject, 10)).then(() => value());
  });
  return <div>{data()}</div>;
}

export function AllOfRejected() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return await Promise.all([Promise.reject(new Error("x"))]).then(() => value());
  });
  return <div>{data()}</div>;
}

export function RaceOfRejected() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await Promise.race([Promise.reject(new Error("x"))]).then(() => value());
  });
  return <div>{data()}</div>;
}

export function AnyOfRejected() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await Promise.any([Promise.reject(new Error("x"))]).then(() => value());
  });
  return <div>{data()}</div>;
}

export function FinallyOfRejectedThen() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await Promise.reject(new Error("x"))
      .finally(() => {})
      .then(() => value());
  });
  return <div>{data()}</div>;
}

export function ThenOfRejectedThen() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await Promise.reject<number>(new Error("x"))
      .then((x) => x)
      .then(() => value());
  });
  return <div>{data()}</div>;
}

// Unknown settlement: the executor decides, so the handler is an obligation.
export function UnknownExecutorThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => new Promise<void>((done) => queueMicrotask(done)).then(() => value()));
  return <div>{data()}</div>;
}

// Positives: a rejection recovered by `catch` fulfills; `allSettled` and a
// literal `Promise.all` fulfill; a timer passing the resolve parameter.
export function RecoveredThen() {
  const [value] = createSignal(1);
  const data = createMemo(() =>
    Promise.reject(new Error("x"))
      .catch(() => 0)
      .then(() => value()),
  );
  return <div>{data()}</div>;
}

export function AllSettledThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.allSettled([Promise.reject(new Error("x"))]).then(() => value()));
  return <div>{String(data())}</div>;
}

export function TimerResolveWithTwoParameters() {
  const [value] = createSignal(1);
  const data = createMemo(() => new Promise<void>((done, _fail) => setTimeout(done, 10)).then(() => value()));
  return <div>{data()}</div>;
}

// ---- A reducer without an initial value is never called for fewer than two
// elements.

export function ReduceWithoutInitial() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].reduce((a, b) => a + b + value());
  });
  return <div>{data()}</div>;
}

export function ReduceRightWithoutInitial() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return [1].reduceRight((a, b) => a + b + value());
  });
  return <div>{data()}</div>;
}

// ---- A store key read on every run before the first suspension is a
// dependency already (ADR 0195, `el._deps` in `checkPostAwaitRead`). Only the
// same key, or a key on the way to one read before, is exempt.

export function StoreTrackedBefore() {
  const [store] = createStore({ a: 1, b: 2 });
  const data = createMemo(async () => {
    store.a;
    await pause();
    return store.a;
  });
  return <div>{data()}</div>;
}

export function StoreTrackedDeeperBefore() {
  const [store] = createStore({ a: { x: 1, y: 2 }, b: 2 });
  const data = createMemo(async () => {
    store.a.x;
    await pause();
    return store.a;
  });
  return <div>{String(data())}</div>;
}

export function StoreOtherKeyAfter() {
  const [store] = createStore({ a: 1, b: 2 });
  const data = createMemo(async () => {
    store.a;
    await pause();
    return store.b;
  });
  return <div>{data()}</div>;
}

export function StoreDeeperKeyAfter() {
  const [store] = createStore({ a: { x: 1, y: 2 }, b: 2 });
  const data = createMemo(async () => {
    store.a.x;
    await pause();
    return store.a.y;
  });
  return <div>{data()}</div>;
}

export function StoreReadBeforeConditionalAwait(props: { flag: boolean }) {
  const [store] = createStore({ a: 1 });
  const data = createMemo(async () => {
    if (props.flag) store.a;
    await pause();
    return store.a;
  });
  return <div>{data()}</div>;
}

// ---- Round 5: what the pre-await segment and the preceding chain prove.
// Each group is a runtime-oracle case (`after-await-settlement`,
// `after-await-tracked`, `after-await-disposed`, `after-await-owner`). The
// uncertifiable components are shapes where the runtime stays silent (or
// warns for a reason the analysis cannot prove); the *Control components are
// the proven violations next to them.

// ---- after-await-settlement

export function ThenRejects() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(0).then(() => Promise.reject("expected")).then(() => value()).catch(() => 0);
  });
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
}

// ---- after-await-tracked

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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
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
  return <div>{String(data())}</div>;
}

// ---- after-await-disposed

export function DisposedByTimer() {
  const [value] = createSignal(1);
  createRoot((dispose) => {
    const data = createMemo(async () => {
      await pause();
      console.log("disposed read", value());
      return 0;
    });
    createMemo(() => data());
    setTimeout(dispose, 5);
  });
  return <span>disposed</span>;
}

export function DisposedInsideComputation() {
  const [value] = createSignal(1);
  createRoot((dispose) => {
    const data = createMemo(async () => {
      await pause();
      dispose();
      console.log("disposed read", value());
      return 0;
    });
    createMemo(() => data());
  });
  return <span>disposed</span>;
}

export function RootControl() {
  const [value] = createSignal(1);
  createRoot(() => {
    const data = createMemo(async () => {
      await pause();
      console.log("live read", value());
      return 0;
    });
    createMemo(() => data());
  });
  return <span>live</span>;
}

// ---- after-await-owner

export function UntrackedAsyncHelper() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  async function load() {
    value();
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await untrack(() => load());
  });
  return <div>{String(data())}</div>;
}

export function TrackedHelperControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  async function load() {
    value();
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await load();
  });
  return <div>{String(data())}</div>;
}

export function NullOwnerCallback() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    await pause();
    return (() => runWithOwner(null, () => value()))();
  });
  return <div>{String(data())}</div>;
}

export function CapturedOwnerControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    const owner = getOwner();
    await pause();
    return runWithOwner(owner, () => value());
  });
  return <div>{String(data())}</div>;
}

export function ForAwait() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {}
    return value();
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitReadBeforeControl() {
  const [value, setValue] = createSignal(1);
  setTimeout(() => setValue(2), 160);
  const [tick, setTick] = createSignal(0);
  setTimeout(() => setTick(1), 80);
  const data = createMemo(async () => {
    tick();
    value();
    for await (const _ of [1]) {}
    return value();
  });
  return <div>{String(data())}</div>;
}

// ---- Round 6: what a call before the first await may have tracked. A callee
// that is not an exactly resolved project function with a visible body (a
// package, a global, a standard-library call outside the audited inert rows)
// may invoke a callback registered earlier or read a source a package store
// holds: the read after the await is uncertifiable, never a violation. Only
// the audited inert rows (`Math.*`, timers, `Promise` statics) leave it one.

declare function register(callback: () => unknown): void;
declare function invoke(): void;
declare function holdInStore(source: object): void;
declare function readHeld(): void;

export function PackageRegisteredCallback() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  register(() => value());
  const data = createMemo(async () => {
    tick();
    invoke();
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

export function PackageStoreHolds() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  holdInStore({ value });
  const data = createMemo(async () => {
    tick();
    readHeld();
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

export function GlobalEventDispatch() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  window.addEventListener("review-track", () => value());
  const data = createMemo(async () => {
    tick();
    window.dispatchEvent(new Event("review-track"));
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

export function StandardFunctionCall() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const captured = () => value();
  const data = createMemo(async () => {
    tick();
    captured.call(undefined);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

export function InertStandardCallsControl() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    Math.max(tick(), 1);
    setTimeout(() => {}, 0);
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// ---- A disposal the analysis follows through a named root callback.

export function DisposedNamedRoot() {
  const [value] = createSignal(1);
  function start(dispose: () => void) {
    const data = createMemo(async () => {
      await pause();
      return value();
    });
    createMemo(() => data());
    setTimeout(dispose, 5);
  }
  createRoot(start);
  return <span>disposed</span>;
}

export function NamedRootWithoutDisposeControl() {
  const [value] = createSignal(1);
  function start(dispose: () => void) {
    const data = createMemo(async () => {
      await pause();
      return value();
    });
    createMemo(() => data());
  }
  createRoot(start);
  return <span>kept</span>;
}

declare const unseenRoot: (dispose: () => void) => void;

export function UnresolvedRootCallback() {
  const [value] = createSignal(1);
  createRoot(unseenRoot);
  const data = createMemo(async () => {
    await pause();
    return value();
  });
  return <div>{String(data())}</div>;
}

// ---- `undefined`, `NaN` and `Infinity` are the globals, not a spelling: a
// binding of that name is a value that may be a rejection.

export function ShadowedUndefinedHandler() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
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
  return <div>{String(data())}</div>;
}

export function ShadowedUndefinedResolve() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    const undefined = Promise.reject("expected");
    return await Promise.resolve(undefined)
      .then(() => value())
      .catch(() => 0);
  });
  return <div>{String(data())}</div>;
}

export function GlobalUndefinedResolveControl() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    return await Promise.resolve(undefined).then(() => value());
  });
  return <div>{String(data())}</div>;
}

export function ShadowedOwnerUndefined() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    const undefined = null;
    await pause();
    return runWithOwner(undefined, () => value());
  });
  return <div>{String(data())}</div>;
}

// ---- A handler that calls a function that never returns throws.

export function HandlerCallsNever() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
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
  return <div>{String(data())}</div>;
}

export function HandlerCallsVoidControl() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
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
  return <div>{String(data())}</div>;
}

// ---- `for await` suspends before its body and after the loop: a source read
// in either is an obligation, however it is read.

export function ForAwaitStore() {
  const [tick] = createSignal(0);
  const [store] = createStore({ n: 1 });
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
    }
    return store.n;
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitCallback() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
    }
    return [1].map(() => value())[0];
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitHelper() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  function read() {
    return value();
  }
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
    }
    return read();
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitBody() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    for await (const _ of [1]) {
      return value();
    }
    return 0;
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitBodyStore() {
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    const [store] = createStore({ n: 1 });
    for await (const _ of [1]) {
      return store.n;
    }
    return 0;
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitBodyReadBeforeControl() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    value();
    for await (const _ of [1]) {
      return value();
    }
    return 0;
  });
  return <div>{String(data())}</div>;
}

export function ForAwaitIterableReadControl() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    for await (const _ of [value()]) {
    }
    return 0;
  });
  return <div>{String(data())}</div>;
}

// ---- `untrack(fn)` and `runWithOwner(owner, fn)` with the function passed by
// reference or written async.

export function UntrackDirectHelper() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  async function load() {
    await pause();
    return value();
  }
  const data = createMemo(async () => {
    tick();
    return await untrack(load);
  });
  return <div>{String(data())}</div>;
}

declare const unseenLoader: () => Promise<number>;

export function UntrackUnresolvedFunction() {
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    return await untrack(unseenLoader);
  });
  return <div>{String(data())}</div>;
}

export function NullOwnerAsyncCallback() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    await pause();
    return await runWithOwner(null, async () => {
      await pause();
      return value();
    });
  });
  return <div>{String(data())}</div>;
}

export function CapturedOwnerAsyncCallback() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const data = createMemo(async () => {
    tick();
    const owner = getOwner();
    return await runWithOwner(owner, async () => {
      await pause();
      return value();
    });
  });
  return <div>{String(data())}</div>;
}
