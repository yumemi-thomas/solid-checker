import { createEffect, createMemo, createSignal } from "solid-js";
import { arrowCreates, createsMemo, onlyFromHandler } from "./helper";
import { viaLib } from "./lib";
import { selfConst, selfFunction } from "./App";

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

// ADR 0963 (a call the call graph withheld): a helper reached through a self-import of this module, the way another module's
// import would reach it. The call graph keeps no edge to a `const` arrow here (ADR 0339: through a cycle the call may run before the
// binding's declarator), so the walk found no caller and the helper was certified clean; the call is followed as a run the walk does
// not prove, an uncertifiable row. A hoisted function has its edge, and it is published by a specifier too (ADR 0970, R1 of the re-review of #221: `module_exports` took
// no specifier for an export, so it was a violation, where an exported helper is an obligation, ADR 0516): an uncertifiable row, as `createsMemo`'s.
const selfConstLocal = () => {
  createMemo(() => 5);
};
function selfFunctionLocal() {
  createMemo(() => 6);
}
export { selfConstLocal as selfConst, selfFunctionLocal as selfFunction };

export function ThroughSelfImport() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    selfConst();
  });
  return <div />;
}
export function ThroughSelfImportFunction() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    selfFunction();
  });
  return <div />;
}
