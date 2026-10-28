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

// SC6002 through a layer: uncertifiable (an earlier call may lower the marker).
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
