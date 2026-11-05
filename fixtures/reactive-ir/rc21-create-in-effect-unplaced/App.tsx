import { createEffect, createMemo, createRoot, createSignal, getOwner, runWithOwner } from "solid-js";

// Each layer below runs where no call edge places it, and an effect's apply
// calls it with no owner (Chrome, rc.21: PRIMITIVE_IN_EFFECT_CALLBACK). Each
// is an SC6002 obligation, blocker `function-run-unplaced`.

const viaObject = { go() { createMemo(() => 1); } };

class Viewer {
  run() { createMemo(() => 1); }
}
const viewer = new Viewer();

const viaGetter = { get value() { createMemo(() => 1); return 1; } };

// `createRoot` is itself a creating call (an obligation here); the memo in it
// has the root's owner, and the one in `runWithOwner` the owner it sets:
// silent.
let saved: ReturnType<typeof getOwner> = null;
const owned = { go() { runWithOwner(saved, () => { createMemo(() => 1); }); } };
const rooted = { go() { createRoot(() => { createMemo(() => 1); }); } };

export function Effects() {
  const [count] = createSignal(0);
  saved = getOwner();
  createEffect(count, () => {
    viaObject.go();
    viewer.run();
    void viaGetter.value;
    owned.go();
    rooted.go();
  });
  // The proven position stays a violation.
  createEffect(count, () => { createMemo(() => 1); });
  return <i />;
}
