import { Loading, createEffect, createMemo, untrack } from "solid-js";

// ADR 1096: a read of a pending async memo inside exact in-place layers (a named closure called once, an IIFE, a helper that calls its callback)
// that a memo's compute runs is the memo's tracked read: it suspends the memo, as the same read written directly in the compute does.

function runNow(callback: () => unknown) {
  callback();
  return 1;
}

// Clean: a named closure called once in the compute.
export function MemoClosure() {
  const source = createMemo(async () => 1);
  const view = createMemo(() => {
    const read = () => {
      source();
    };
    read();
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Clean: an IIFE in the compute.
export function MemoIife() {
  const source = createMemo(async () => 1);
  const view = createMemo(() => {
    (() => {
      source();
    })();
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Clean: a helper's callback it calls in its own body.
export function MemoHelper() {
  const source = createMemo(async () => 1);
  const view = createMemo(() => {
    runNow(() => {
      source();
    });
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Clean: a named closure called once inside the helper's callback.
export function MemoClosureHelper() {
  const source = createMemo(async () => 1);
  const view = createMemo(() => {
    runNow(() => {
      const read = () => {
        source();
      };
      read();
    });
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Not ended (twin): the closure runs in an effect's apply, which is untracked: the layers end at no computation's read.
export function ApplyClosure() {
  const source = createMemo(async () => 1);
  createEffect(
    () => 1,
    () => {
      const read = () => {
        source();
      };
      read();
    },
  );
  return <p>static</p>;
}

// Not ended (twin): `untrack` is a primitive's callback, which the climb does not cross: the read is untracked there.
export function MemoUntrackClosure() {
  const source = createMemo(async () => 1);
  const view = createMemo(() => {
    untrack(() => {
      const read = () => {
        source();
      };
      read();
    });
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}

// Not ended (twin): the closure is also kept, so a later call may run it outside any computation.
export function MemoClosureKept() {
  const source = createMemo(async () => 1);
  let kept: (() => void) | undefined;
  const view = createMemo(() => {
    const read = () => {
      source();
    };
    read();
    kept = read;
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
      <button onClick={() => kept?.()}>run</button>
    </Loading>
  );
}

// Not ended (fail closed): the named closure is handed to the helper as a value, a reference the climb does not follow.
export function MemoClosureHanded() {
  const source = createMemo(async () => 1);
  const view = createMemo(() => {
    const read = () => {
      source();
    };
    runNow(read);
    return 1;
  });
  return (
    <Loading fallback={0}>
      <p>{view()}</p>
    </Loading>
  );
}
