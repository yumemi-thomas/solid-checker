import { createEffect, createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);

// Batch 11 (the `then` arm of the SC1002 walk; fuzz cells `then-unplaced-chain*`). A memo's compute returns a promise chain over a receiver no fact proves
// native (a call's result: here an identity function's), and a handler of a `then` reads a source. The handler may run after the flight settles, which
// Chrome (rc.21) warns about (UNTRACKED_READ_AFTER_AWAIT at the read), or inline, which tracks the read. The arm returned with no row and without walking the
// receiver's own chain (the `then-withheld-chain` program is masked: its computed write withholds `then`, and the walk rows a withheld call). Every handler
// is a proof obligation, an uncertifiable row of SC1002 (`HandlerMayNotRun`), never a violation: nothing here proves the receiver native or the handler run.

const settle = () => new Promise<number>((done) => setTimeout(() => done(1), 1));
const same = (x: Promise<number>) => x;
// The handler handed by name, after one earlier link.
export function Named() {
  const f = () => {
    count();
    return 1;
  };
  const m = createMemo(() => same(settle()).then(() => 1).then(f));
  createEffect(() => m(), () => {});
  return <div />;
}

// An arrow at the call, and the handler written in place.
export function Arrow() {
  const f = () => {
    count();
    return 1;
  };
  const m = createMemo(() => same(settle()).then(() => 1).then(() => f()));
  createEffect(() => m(), () => {});
  return <div />;
}

export function InPlace() {
  const m = createMemo(() =>
    same(settle()).then(() => {
      count();
      return 1;
    }),
  );
  createEffect(() => m(), () => {});
  return <div />;
}

// A member of a closed object: the walk does not resolve the slot, so the call is an obligation.
export function Member() {
  const o = {
    f: () => {
      count();
      return 1;
    },
  };
  const m = createMemo(() => same(settle()).then(() => 1).then(o.f));
  createEffect(() => m(), () => {});
  return <div />;
}

// Three links, one link.
export function Depth3() {
  const f = () => {
    count();
    return 1;
  };
  const m = createMemo(() => same(settle()).then(() => 1).then(() => 2).then(f));
  createEffect(() => m(), () => {});
  return <div />;
}

export function Identity() {
  const f = () => {
    count();
    return 1;
  };
  const m = createMemo(() => same(settle()).then(f));
  createEffect(() => m(), () => {});
  return <div />;
}

// The receiver a promise literal is a native promise (proven): the handler is an obligation as before.
export function Native() {
  const f = () => {
    count();
    return 1;
  };
  const m = createMemo(() => settle().then(() => 1).then(f));
  createEffect(() => m(), () => {});
  return <div />;
}

// Silent: the handlers read nothing.
export function Plain() {
  const m = createMemo(() => same(settle()).then(() => 1).then(() => 2));
  createEffect(() => m(), () => {});
  return <div />;
}
