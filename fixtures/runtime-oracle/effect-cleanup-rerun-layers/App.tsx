import { createEffect, createSignal } from "solid-js";

// ADR 1195: a read or a write in function layers run in place (an IIFE, a closure called once, a project helper's callback) inside the
// cleanup an inline apply returns runs where the cleanup does: on the rerun a reached click makes (ADR 1183) and at disposal.

function runNow(cb: () => void) {
  cb();
}

function writeBoth(set: (value: number) => void) {
  set(5);
}

// Violations: the read runs in the apply's strict-read window on the rerun (rc.21 STRICT_READ_UNTRACKED).
export function ReadInClosure() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      const f = () => {
        r();
      };
      f();
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

export function ReadInIife() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      (() => {
        r();
      })();
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

export function ReadInHelperCallback() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      runNow(() => {
        r();
      });
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

// Clean: a write in the cleanup is legal on every run of it (ADR 1037), so it is in a layer run in place there.
export function WriteInClosure() {
  const [, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      const f = () => {
        setW(7);
      };
      f();
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

export function WriteInIife() {
  const [, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      (() => {
        setW(7);
      })();
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

export function WriteInHelperCallback() {
  const [, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      runNow(() => {
        setW(7);
      });
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

// Uncertifiable as before: not a layer run in place, or not only there.
export function ReadInForEach() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      [1].forEach(() => {
        r();
      });
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

export function ReadInStoredClosure() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  let kept: (() => void) | undefined;
  createEffect(
    () => t(),
    () => () => {
      kept = () => {
        r();
      };
    },
  );
  void kept;
  return <button onClick={() => setT(2)}>x</button>;
}

export function ReadAfterAwaitInAsyncIife() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  createEffect(
    () => t(),
    () => () => {
      void (async () => {
        await Promise.resolve();
        r();
      })();
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}

// The helper writes from the component body too: its write is not only the cleanup's.
export function WriteInSharedHelper() {
  const [, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  writeBoth(setW);
  createEffect(
    () => t(),
    () => () => {
      writeBoth(setW);
    },
  );
  return <button onClick={() => setT(2)}>x</button>;
}
