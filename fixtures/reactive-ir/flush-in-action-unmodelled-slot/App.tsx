import * as S from "solid-js";
import { action, createRoot, flush, isPending, latest, runWithOwner, untrack } from "solid-js";

// A callback of a primitive that runs it in place, with a parameter the primitive never passes or async, inside an action's
// step: the slot climb crosses it as an obligation (ADR 0959), so the flush is SC2006's uncertifiable row, never silence.
const paramUntrack = action(function* () {
  untrack((p0?: number) => {
    flush();
  });
  yield Promise.resolve();
});
const asyncUntrack = action(function* () {
  void untrack(async () => {
    flush();
  });
  yield Promise.resolve();
});
const asyncLatest = action(function* () {
  void latest(async () => {
    flush();
    return 1;
  });
  yield Promise.resolve();
});
// The plain callback: the walk's own proof (a violation).
const plainUntrack = action(function* () {
  untrack(() => {
    flush();
  });
  yield Promise.resolve();
});
// A parameter callback that flushes nothing: silent.
const quiet = action(function* () {
  untrack((p0?: number) => (p0 ?? 1) + 1);
  yield Promise.resolve();
});
// The namespace spelling, and the layers with their own climb path: `runWithOwner(null, …)` and `createRoot` (its parameter is the dispose).
const namespaceUntrack = action(function* () {
  S.untrack((p0?: number) => {
    flush();
  });
  yield Promise.resolve();
});
const ownerNullAsync = action(function* () {
  void runWithOwner(null, async () => {
    flush();
  });
  yield Promise.resolve();
});
const rootDispose = action(function* () {
  createRoot((dispose: () => void) => {
    flush();
    dispose();
  });
  yield Promise.resolve();
});
// Not in an action: the flush is no SC2006 site.
export function Handler() {
  return (
    <button
      onClick={() => {
        untrack((p0?: number) => {
          flush();
        });
        void paramUntrack();
        void asyncUntrack();
        void asyncLatest();
        void plainUntrack();
        void quiet();
        void namespaceUntrack();
        void ownerNullAsync();
        void rootDispose();
        void isPending(() => 1);
      }}
    >
      go
    </button>
  );
}
