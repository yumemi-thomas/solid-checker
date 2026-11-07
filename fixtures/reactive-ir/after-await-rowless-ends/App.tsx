import { createEffect, createMemo, createSignal, createStore } from "solid-js";

declare const flag: boolean;
declare const k: number;
declare const p: Promise<number>;
declare const q: Promise<{ v?: number }>;

// B2: a project wrapper hands its parameter to `createMemo` as the compute.
function useQuery<T>(fn: () => Promise<T>) {
  return createMemo(fn);
}
// Not a hand-off: the wrapper runs its parameter in place where it is called.
function now(f: () => unknown) {
  f();
}

export function Ends() {
  const [n] = createSignal(0);
  const [s] = createStore({ a: 0 });

  // B1, positive: a read after an await a branch may skip.
  createMemo(async () => {
    if (flag) await p;
    return n();
  });
  createMemo(async () => {
    flag && (await p);
    return n();
  });
  createMemo(async () => {
    switch (k) {
      case 1:
        await p;
    }
    return n();
  });
  createMemo(async () => {
    const { v = n() } = flag ? await q : {};
    return v;
  });
  // B1, positive: a store read in a closure run past the skippable await.
  createMemo(async () => {
    if (flag) await p;
    const read = () => s.a;
    read();
    return 1;
  });

  // B1, negative: the read comes before the await, or in the arm without it.
  createMemo(async () => {
    n();
    if (flag) await p;
    return 1;
  });
  createMemo(async () => {
    if (flag) await p;
    else n();
    return 1;
  });
  createMemo(async () => {
    const read = () => s.a;
    if (flag) await p;
    else read();
    return 1;
  });

  // B3: one helper shared by two computations; only the first read `n` before it.
  const shared = async () => {
    await p;
    n();
    return 1;
  };
  const first = createMemo(async () => {
    n();
    await shared();
    return 1;
  });
  const second = createMemo(async () => {
    await shared();
    return 1;
  });

  // B4: a helper called after the computation's first await reads nothing tracked.
  const late = async () => {
    n();
    await p;
    return 1;
  };
  const third = createMemo(async () => {
    await p;
    await late();
    return 1;
  });
  // B4, negative: called first, its read before its own await is a dependency.
  const early = async () => {
    n();
    await p;
    return 1;
  };
  const fourth = createMemo(async () => {
    await early();
    return 1;
  });

  // B2, positive: the wrapper's memo runs this compute.
  const query = useQuery(async () => {
    await p;
    return n();
  });
  // B2, negative: run in place in the component body, not as a compute.
  now(async () => {
    await p;
    return n();
  });

  createEffect(
    () => [first(), second(), third(), fourth(), query()],
    () => {}
  );
  return <p />;
}
