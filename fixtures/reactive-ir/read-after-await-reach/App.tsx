import { createMemo, createSignal, createStore, untrack } from "solid-js";

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
    const sorted = [2, 1].sort(() => value());
    return [some, every, found, index, flat, sum, sorted];
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

export function CatchCallback() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.reject(new Error("no")).catch(() => value()));
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
