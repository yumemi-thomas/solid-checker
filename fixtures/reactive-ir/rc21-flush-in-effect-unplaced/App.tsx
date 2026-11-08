import { createEffect, createSignal, flush } from "solid-js";

// Each layer below runs where no call edge places it, and an effect's apply
// calls it: it runs inside the flush draining effects (Chrome, rc.21:
// FLUSH_IN_EFFECT_CALLBACK). Each is an SC2008 obligation, blocker
// `function-run-unplaced`.

const viaObject = { go() { flush(); } };

class Viewer {
  go() { flush(); }
}
const viewer = new Viewer();

const viaMap = new Map([["go", () => { flush(); }]]);

const wrapUse = () => { flush(); };
const viaSlot = { go: () => { wrapUse(); } };

export function Effects() {
  const [count] = createSignal(0);
  createEffect(count, () => {
    viaObject.go();
    viewer.go();
    viaMap.get("go")!();
    viaSlot.go();
  });
  // The bundle's `effect` is the apply.
  createEffect(count, {
    effect: () => { flush(); },
    error: () => {},
  });
  // The proven position stays a violation.
  createEffect(count, () => { flush(); });
  return <i />;
}

// Not reached by anything: silent.
function neverCalled() {
  const unused = { go() { flush(); } };
  return unused;
}
