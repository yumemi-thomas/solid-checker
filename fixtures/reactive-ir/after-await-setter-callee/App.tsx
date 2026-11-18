import { action, createMemo, createSignal, createStore, refresh } from "solid-js";

// ADR 1097 (reopened narrowly): an argument-less call after an await whose callee is an exact same-file, unexported, never-written `const` holding a function
// literal (or an action's generator) whose body is nothing but exact setter calls with inert arguments runs no project code and reads no source.

export function Clean() {
  const [, setW] = createSignal(0);
  const wrapUse = () => {
    setW(7);
  };
  const wrapShort = () => setW(8);
  const run = action(function* () {
    setW(9);
  });
  const useIt = run;
  const callsAction = () => {
    run();
  };
  // Clean: each call below runs only exact setters.
  Promise.resolve().then(() => {
    wrapUse();
    wrapShort();
    useIt();
    callsAction();
    const local = () => {
      setW(10);
    };
    local();
  });
  return <p />;
}

const box = { v: 1 };
const n = 2;
function runIt(callback: () => void) {
  callback();
}
export const exportedWrap = () => {};

export function Refused() {
  const [count, setW] = createSignal(0);
  const [, setOwnEquals] = createSignal(0, { equals: (a, b) => a === b });
  const readsMember = () => {
    setW(box.v);
  };
  const otherIdentifier = () => {
    setW(n);
  };
  const otherCall = () => {
    setW(1);
    count();
  };
  const updater = () => {
    setW((previous) => previous + 1);
  };
  const iterates = () => {
    for (const value of [1]) setW(value);
  };
  const classInside = () => {
    class K {}
    void K;
    setW(1);
  };
  const ownEquals = () => {
    setOwnEquals(1);
  };
  const viaHelper = () => {
    runIt(() => setW(1));
  };
  const takesValue = (value: number) => {
    setW(value);
  };
  let reassigned = () => {
    setW(1);
  };
  reassigned = () => {
    count();
  };
  const yields = action(function* () {
    setW(1);
    yield;
  });
  // Uncertifiable: each call below has a callee outside the narrow form.
  Promise.resolve().then(() => {
    readsMember();
    otherIdentifier();
    otherCall();
    updater();
    iterates();
    classInside();
    ownEquals();
    viaHelper();
    takesValue(1);
    reassigned();
    yields();
    exportedWrap();
    (() => {
      setW(1);
    })();
  });
  return <p />;
}

// ADR 1097 widened (step 1, the ruling of 2026-11-19): transitive qualifying callees, an IIFE, exact `const` aliases of a setter, `refresh(accessor)` (ADR 1089 C)
// and a plain store setter with a literal-only updater (ADR 1089 D).
export function Widened() {
  const [, setW] = createSignal(0);
  const [store, setStore] = createStore({ v: 0 });
  void store;
  const source = createMemo(() => 1);
  const inner = () => {
    setW(1);
  };
  const outer = () => {
    inner();
    setW(2);
  };
  const set = setW;
  const viaAlias = () => {
    set(3);
  };
  const refreshes = () => {
    refresh(source);
  };
  const storeWrite = () => {
    setStore((draft) => {
      draft.v = 4;
    });
  };
  // Clean: each call below runs only proved calls.
  Promise.resolve().then(() => {
    outer();
    viaAlias();
    set(5);
    refreshes();
    storeWrite();
    (() => {
      inner();
      setW(6);
    })();
  });
  return <p />;
}

export function WidenedRefused() {
  const [count, setW] = createSignal(0);
  const reads = () => {
    count();
  };
  const callsReader = () => {
    reads();
  };
  const recursive = (): void => {
    recursive();
  };
  const d5 = () => setW(1);
  const d4 = () => d5();
  const d3 = () => d4();
  const d2 = () => d3();
  const d1 = () => d2();
  let setLet = setW;
  setLet = setW;
  const viaLet = () => {
    setLet(1);
  };
  const box = { get: () => 1 };
  const refreshesNonAccessor = () => {
    refresh(box.get);
  };
  // Uncertifiable: a transitive read, recursion, past the depth bound, a `let` alias, `refresh` of a member.
  Promise.resolve().then(() => {
    callsReader();
    recursive();
    d1();
    viaLet();
    refreshesNonAccessor();
  });
  return <p />;
}

// Misuse twin: the wrapper reads, after the memo's await, so its read is the memo's untracked read (Chrome: UNTRACKED_READ_AFTER_AWAIT).
export function Twin() {
  const [count] = createSignal(0);
  const wrapRead = () => {
    count();
  };
  const total = createMemo(async () => {
    await Promise.resolve();
    wrapRead();
    return 1;
  });
  return <p>{total()}</p>;
}
