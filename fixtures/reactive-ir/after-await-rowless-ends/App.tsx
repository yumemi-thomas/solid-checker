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

  // B5, positive: a callee that names no symbol, past the await.
  const fns = [n];
  const fifth = createMemo(async () => {
    await p;
    return fns[0]();
  });

  // B6-B8: a handler on the memo's flight the walk cannot place.
  const api = { fmt: (v: number) => v + n() };
  const viaMember = createMemo(() => p.then(api.fmt));
  const viaAsync = createMemo(() => p.then(async (v) => v + n()));
  const viaAny = createMemo(() => (p as any).then(() => n()) as Promise<number>);
  // B6, negative: no handler at all.
  const viaNull = createMemo(() => p.then(null));

  // B9 (ADR 0897): a composite callee in a late helper's prefix. Exact: every
  // operand is `n` and the callee total (V). Inexact: another operand, or `&&`
  // (U). Mixed with a tracked first operand (review D31): U, in the prefix and
  // in the member's own continuation.
  const [u] = createSignal(1);
  const plain = () => 0;
  let slot: typeof n | undefined;
  const lateComposite = async () => {
    (slot = n)();
    (flag ? n : n)();
    (flag ? n : plain)();
    (n && n)();
    await p;
    return 1;
  };
  const sixth = createMemo(async () => {
    await p;
    await lateComposite();
    return 1;
  });
  // B9, negative: called before the member's first await, its reads are dependencies.
  const earlyComposite = async () => {
    (flag ? n : n)();
    await p;
    return 1;
  };
  const seventh = createMemo(async () => {
    await earlyComposite();
    return 1;
  });
  const lateMixed = async () => {
    (flag ? n : u)();
    await p;
    return 1;
  };
  const eighth = createMemo(async () => {
    n();
    await p;
    await lateMixed();
    (flag ? n : u)();
    return 1;
  });

  // B10 (ADR 0898): a composite before the await proves tracking only when
  // exact. `n.length` is the arity (0): the call takes `plain`, so `n()` past
  // the await is a possible read (U), never a silent one. Exact: tracked, no row.
  const ninth = createMemo(async () => {
    (n.length > 5 ? n : plain)();
    await p;
    n();
    return 1;
  });
  const tenth = createMemo(async () => {
    (flag ? n : n)();
    await p;
    n();
    return 1;
  });
  // B10 (review D32): a composite whose first token is an accessor, past the
  // await, calls a helper that reads `u`: a row at the read, never silent.
  const readsU = () => u();
  const eleventh = createMemo(async () => {
    await p;
    (n.length > 5 ? readsU : readsU)();
    return 1;
  });

  createEffect(
    () => [first(), second(), third(), fourth(), query(), fifth(), viaMember(), viaAsync(), viaAny(), viaNull(), sixth(), seventh(), eighth(), ninth(), tenth(), eleventh()],
    () => {}
  );
  return <p />;
}
