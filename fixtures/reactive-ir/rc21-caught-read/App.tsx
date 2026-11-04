import { createMemo, createSignal, createStore, latest } from "solid-js";

// The compute runs where it is created, the memo pending: it catches and completes.
export function Caught() {
  const memo = createMemo(async () => 1);
  const safe = createMemo(() => {
    try {
      return memo();
    } catch {
      return 0;
    }
  });
  return <p>{safe()}</p>;
}

// A rethrow lets the suspension go on.
export function Rethrown() {
  const memo = createMemo(async () => 1);
  const safe = createMemo(() => {
    try {
      return memo();
    } catch (error) {
      throw error;
    }
  });
  return <p>{safe()}</p>;
}

// A memo may throw its error: open. A signal cannot.
export function Open() {
  const [count] = createSignal(1);
  const inner = createMemo(() => count());
  const one = createMemo(() => {
    try {
      return inner();
    } catch {
      return 0;
    }
  });
  const two = createMemo(() => {
    try {
      return count();
    } catch {
      return 0;
    }
  });
  return <p>{[one(), two()].length}</p>;
}

// An uncaught throw leaves the pass: nothing reports, before or after the `try`.
export function UncaughtToo() {
  const memo = createMemo(async () => 1);
  const before = createMemo(() => {
    memo();
    try {
      memo();
    } catch {}
    return 1;
  });
  const after = createMemo(() => {
    try {
      memo();
    } catch {}
    memo();
    return 1;
  });
  return <p>{[before(), after()].length}</p>;
}

// A store's Get and a probe read too.
export function Store() {
  const [store] = createStore(async () => ({ a: 1 }), { a: 0 });
  const memo = createMemo(async () => 1);
  const one = createMemo(() => {
    try {
      return store.a;
    } catch {
      return 0;
    }
  });
  const two = createMemo(() => {
    try {
      return Object.keys(store).length;
    } catch {
      return 0;
    }
  });
  const three = createMemo(() => {
    try {
      return latest(memo);
    } catch {
      return 0;
    }
  });
  return <p>{[one(), two(), three()].length}</p>;
}

// A setter performs no read, whatever may run it.
export function save(setError: (value: string) => void) {
  const [, setLocal] = createSignal("");
  try {
    setLocal("x");
    setError("y");
  } catch {
    return 0;
  }
  return 1;
}

// The untracked read reports before it throws, so a `catch` keeps SC5001.
export function CaughtInBody() {
  const memo = createMemo(async () => 1);
  try {
    memo();
  } catch {
    return <p>{0}</p>;
  }
  return <p>{1}</p>;
}
