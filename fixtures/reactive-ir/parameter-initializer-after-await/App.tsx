import { createEffect, createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const pause = () => Promise.resolve();

// ADR 0961 (PD, batch-10 addendum 12 group D): a call after the await of an async tracked computation runs the parameter
// initializers of the function it calls (or of the constructor it constructs) when it omits the argument, on the
// continuation, with no observer. The walk reads an accessor call in a default; an IIFE, a closure, a destructuring default
// and a constructor's it did not, so Chrome (rc.21) warned UNTRACKED_READ_AFTER_AWAIT where the checker was silent. The call
// is an uncertifiable row of SC1002 (`parameter-initializer-unplaced`), never a violation and never silence.
function iifeDefault(x: unknown = (() => count())()) {
  return x;
}
function closureDefault(f: () => unknown = () => count()) {
  return f();
}
function destructuredDefault({ a = count() }: { a?: unknown } = {}) {
  return a;
}
class Box {
  constructor(_x: unknown = count()) {}
}

export function Iife() {
  const m = createMemo(async () => {
    await pause();
    iifeDefault();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
export function Closure() {
  const m = createMemo(async () => {
    await pause();
    closureDefault();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
export function Destructured() {
  const m = createMemo(async () => {
    await pause();
    destructuredDefault();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
export function Constructed() {
  const m = createMemo(async () => {
    await pause();
    new Box();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
// `undefined` is no argument: the default runs.
export function UndefinedArgument() {
  const m = createMemo(async () => {
    await pause();
    iifeDefault(undefined);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}

// Silent: the argument is passed, so no default runs.
export function Passed() {
  const m = createMemo(async () => {
    await pause();
    iifeDefault(1);
    closureDefault(() => 1);
    new Box(1);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
// Silent: the call is before the await (a read there is a dependency).
export function BeforeAwait() {
  const m = createMemo(async () => {
    iifeDefault();
    await pause();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
// Silent: defaults that read nothing.
function constantDefault(x: unknown = 5) {
  return x;
}
function emptyIife(x: unknown = (() => 1)()) {
  return x;
}
export function NothingToRead() {
  const m = createMemo(async () => {
    await pause();
    constantDefault();
    emptyIife();
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div />;
}
