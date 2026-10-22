import { createRoot, createSignal } from "solid-js";

declare function load(): Promise<number>;
declare const flag: boolean;

// ---- Past an await that dominates it, a write runs on a later task with no
// owner: legal, no finding.

export function AfterAwaitInTry() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    try {
      const value = await load();
      setX(value);
    } catch {
      // A synchronous throw of `load()` reaches this unsuspended, in the
      // body's window: uncertifiable.
      setX(-1);
    } finally {
      setX(0);
    }
  };
  void run();
  return <div>{x()}</div>;
}

export function AfterAwaitInBranch() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    if (flag) {
      await load();
      setX(1);
    }
  };
  void run();
  return <div>{x()}</div>;
}

// ---- Not dominated: an await under a conditional expression may be skipped.

export function ConditionalAwait() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    const value = flag ? await load() : 0;
    setX(value);
  };
  void run();
  return <div>{x()}</div>;
}

// Control: before the await the write runs in the body's window (SC2001).
export function BeforeAwait() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    try {
      setX(2);
      await load();
    } catch {
      // nothing
    }
  };
  void run();
  return <div>{x()}</div>;
}

// An instance field written past the await runs when the class is
// constructed, which may be under an owner: not dominated (review round 1).
export function FieldAfterAwait() {
  const [x, setX] = createSignal(0);
  const factory = async () => {
    await load();
    return class {
      field = setX(1);
    };
  };
  void factory();
  return <div>{x()}</div>;
}

// JSX written past the await runs in a render effect the compiler creates,
// under an owner: not dominated (review round 2).
export function JsxAfterAwait() {
  const [x, setX] = createSignal(0);
  const factory = async () => {
    await load();
    return <div>{setX(5)}</div>;
  };
  void factory();
  return <div>{x()}</div>;
}

// A read past a dominating await is past the strict-read window: silent, as
// the runtime is (an SC1001 violation before facts schema 65).
export function ReadAfterAwaitInRoot() {
  const [x] = createSignal(0);
  createRoot(async () => {
    try {
      await load();
      console.log(x());
    } catch {
      // nothing
    }
  });
  return <div />;
}

// The root is the ambient owner only before the await: the write after it is
// legal (a violation before), whether the body is inline or passed by name.
export function WriteAfterAwaitInRoot() {
  const [, setX] = createSignal(0);
  createRoot(async () => {
    await load();
    setX(7);
  });
  async function init() {
    await load();
    setX(8);
  }
  createRoot(init);
  return <div />;
}

// An async generator suspends again at each `yield`, and its caller may
// resume it under an owner: no generator is past its awaits (review round 3).
export function AsyncGenerator() {
  const [x, setX] = createSignal(0);
  async function* steps() {
    try {
      await load();
      yield 1;
      setX(6);
    } finally {
      // nothing
    }
  }
  void steps().next();
  return <div>{x()}</div>;
}

// The same without `try`: an await at the generator's top level is no proof
// either (before step 4 this was silent).
export function AsyncGeneratorTopLevel() {
  const [x, setX] = createSignal(0);
  async function* steps() {
    await load();
    yield 1;
    setX(7);
  }
  void steps().next();
  return <div>{x()}</div>;
}

// ---- A `try` that starts by awaiting a call of an async function: calling
// one never throws synchronously, so `catch` and `finally` run only after
// the await suspended (no finding).

async function fetchCount(): Promise<number> {
  return load();
}
async function fetchOther(base: number): Promise<number> {
  return base + (await load());
}

export function CatchAfterAsyncCall() {
  const [x, setX] = createSignal(0);
  const run = async () => {
    try {
      const value = await fetchCount();
      setX(value);
    } catch {
      setX(-1);
    } finally {
      setX(0);
    }
  };
  const other = async (base: number) => {
    try {
      await fetchOther(base);
    } catch {
      setX(-2);
    }
  };
  void run();
  void other(1);
  return <div>{x()}</div>;
}

// Not proven: an argument that may throw, a `let`-bound function that may be
// reassigned, a member call (`api.load` may throw while evaluated), a local
// `const` argument (it may be in its temporal dead zone), a bare awaited
// value (awaiting it reads its `constructor`, which may throw), `NaN` (an
// identifier a binding may shadow), an expression that is no literal node.
declare function compute(): number;
declare const api: { load(): Promise<number> };
export function CatchAfterUnprovenHead() {
  const [x, setX] = createSignal(0);
  let reassigned = async () => 1;
  const run = async () => {
    try {
      await fetchOther(compute());
    } catch {
      setX(-3);
    }
    try {
      await reassigned();
    } catch {
      setX(-4);
    }
    try {
      await api.load();
    } catch {
      setX(-5);
    }
    const local = 1;
    try {
      await fetchOther(local);
    } catch {
      setX(-6);
    }
    const pending = load();
    try {
      await pending;
    } catch {
      setX(-7);
    }
    try {
      await fetchOther(NaN);
    } catch {
      setX(-8);
    }
    try {
      await fetchOther(1 + compute());
    } catch {
      setX(-9);
    }
  };
  reassigned = async () => 2;
  void run();
  return <div>{x()}</div>;
}
