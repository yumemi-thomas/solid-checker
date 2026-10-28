import { createEffect, createMemo, createSignal, flush } from "solid-js";

// ADR 0484: the effect-phase walk (SC2008, SC6002) crosses a closure written in
// an exported component and called only in place, and a project helper that
// calls its callback in its own body.

function runNow(cb: () => unknown) {
  cb();
  return 1;
}

const kept: Array<() => unknown> = [];
function keep(cb: () => unknown) {
  kept.push(cb);
  return 1;
}

// SC2008 violations: the flush runs on the apply's stack whenever it runs.
export function FlushInClosure() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      const reveal = () => {
        flush();
      };
      reveal();
    },
  );
  return null;
}

export function FlushThroughHelper() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      runNow(() => {
        flush();
      });
    },
  );
  return null;
}

// SC6002 through a layer: a violation (ADR 0495): `runNow` calls its callback
// before anything else, so no earlier call can lower the marker.
export function MemoThroughHelper() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      runNow(() => createMemo(() => 1));
    },
  );
  return null;
}

// Silent: a helper that keeps its callback may run it anywhere.
export function FlushKept() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      keep(() => flush());
    },
  );
  return null;
}

function logThenRun(cb: () => unknown) {
  console.log("run");
  cb();
}

// SC6002 uncertifiable (ADR 0495): the helper calls something before its
// callback, which may run an effect synchronously and lower the marker.
export function MemoAfterHelperCall() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      logThenRun(() => createMemo(() => 1));
    },
  );
  return null;
}

// SC6002 uncertifiable (ADR 0495): a call runs between the closure's
// declaration and its call.
export function MemoAfterEarlierCall() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      const make = () => {
        createMemo(() => 1);
      };
      console.log("before");
      make();
    },
  );
  return null;
}

declare function tick(): number;

function runWith(_n: number, cb: () => unknown) {
  cb();
}
function runPassing(cb: (n: number) => unknown) {
  cb(tick());
}
function runDefault(cb: () => unknown, _x: number = tick()) {
  cb();
}

// SC6002 uncertifiable (ADR 0495 review): the helper call's own argument
// runs first.
export function MemoAfterHelperArgument() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      runWith(tick(), () => createMemo(() => 1));
    },
  );
  return null;
}

// SC6002 uncertifiable: the helper's call of its parameter evaluates an
// argument first.
export function MemoAfterParameterArgument() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      runPassing(() => createMemo(() => 1));
    },
  );
  return null;
}

// SC6002 uncertifiable: a parameter default of the helper, and of the
// callback, runs first.
export function MemoAfterDefaults() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      runDefault(() => createMemo(() => 1));
      runNow((_x: number = tick()) => createMemo(() => 2));
    },
  );
  return null;
}

// SC6002 uncertifiable: a getter read before the helper call runs a call
// with no call written at the read.
export function MemoAfterGetter() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      const probe = {
        get v() {
          return tick();
        },
      };
      void probe.v;
      runNow(() => createMemo(() => 1));
    },
  );
  return null;
}
