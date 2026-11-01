import { onSettled } from "solid-js";

// ADR 0488: `onSettled` called after an await of its async function runs on a
// continuation with no owner, so a callback that returns a cleanup throws
// SETTLED_CLEANUP_UNOWNED (rc.17), as it does in any unowned scope.

const pause = () => Promise.resolve();

// Violation.
export function AfterAwait() {
  const run = async () => {
    await pause();
    onSettled(() => () => {});
  };
  void run();
  return null;
}

// Not proven (uncertifiable, unchanged): before the await the call runs under
// whatever owns the closure's call, which is not placed here.
export function BeforeAwait() {
  const run = async () => {
    onSettled(() => () => {});
    await pause();
  };
  void run();
  return null;
}

// Clean: a callback that returns no cleanup has nothing to register.
export function NoCleanupAfterAwait() {
  const run = async () => {
    await pause();
    onSettled(() => {});
  };
  void run();
  return null;
}

// Uncertifiable (ADR 0516): a later iteration of a loop that awaits runs the
// call after the await, with no owner; the first runs it before.
export function LoopAwait() {
  const run = async () => {
    for (let i = 0; i < 2; i += 1) {
      onSettled(() => () => {});
      await pause();
    }
  };
  void run();
  return null;
}

// Uncertifiable (ADR 0516): after an await that may be skipped.
export function SkippableAwait(props: { wait: boolean }) {
  const run = async () => {
    if (props.wait) await pause();
    onSettled(() => () => {});
  };
  void run();
  return null;
}

// Clean: the loop's callback returns no cleanup.
export function NoCleanupLoop() {
  const run = async () => {
    for (let i = 0; i < 2; i += 1) {
      onSettled(() => {});
      await pause();
    }
  };
  void run();
  return null;
}

// Uncertifiable (ADR 0516, the fuzzer's shape): the registration runs only on
// a later iteration, after the loop's await; silent before.
export function LoopAwaitLater() {
  const run = async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        onSettled(() => () => {});
      }
      await pause();
    }
  };
  run();
  return <div />;
}
