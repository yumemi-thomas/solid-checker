import { action, createMemo, createSignal } from "solid-js";

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
