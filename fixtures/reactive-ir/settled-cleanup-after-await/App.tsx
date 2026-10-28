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
