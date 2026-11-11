import { createEffect, createMemo, createSignal } from "solid-js";
import { arrowCreates, createsMemo, onlyFromHandler } from "./helper";
import { viaLib } from "./lib";

// ADR 0963 (batch 10b, group C): an exported function is entered by calls outside the project as well, so no run of it is every
// run, but the calls the walk sees are proven runs in the effect phase: a helper an apply calls creates a primitive with no owner
// (Chrome, rc.21: PRIMITIVE_IN_EFFECT_CALLBACK). The walk answered None for an exported function, so the helper was certified
// clean for the one reason that it is exported; its creation is an uncertifiable row of SC6002, as any run the walk proves with
// others it does not (ADR 0516).

export function InApply() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    createsMemo();
    arrowCreates();
    viaLib();
  });
  return <div />;
}

// The same helper, not exported: every run is on an apply's stack, a violation as before.
function localCreates() {
  createMemo(() => 4);
}
export function LocalTwin() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    localCreates();
  });
  return <div />;
}

// Silent: an exported helper only an event handler calls has no run in the effect phase.
export function FromHandler() {
  return <button onClick={() => onlyFromHandler()}>go</button>;
}
