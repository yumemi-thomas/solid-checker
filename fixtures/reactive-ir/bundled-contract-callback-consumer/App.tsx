import { createEffect, createMemo, createSignal, flatten } from "solid-js";
import { applyRef } from "@solidjs/web";

const [count] = createSignal(0);
const doubled = createMemo(() => count() * 2);
declare const element: Element;
const readDoubled = () => doubled();

// This ambient-only fixture has no exact installed artifact and therefore
// cannot bind the receipt-issued first-party bundle. The nested accessor read
// is still proven locally; the package call itself receives no name-only claim.
export function Untracked() {
  const value = flatten(() => doubled());
  return <div>{value}</div>;
}

// The same ambient export in compiler-tracked JSX remains the clean control.
export function Tracked() {
  return <div>{flatten(() => doubled())}</div>;
}

// `applyRef` is also ambient-only here. Package spelling cannot substitute for
// exact artifact identity, so no first-party claim is applied.
export function WebUntracked() {
  applyRef(readDoubled, element);
  return <div />;
}

// The corresponding compiler-tracked expression stays clean.
export function WebTracked() {
  return <div>{(applyRef(() => void doubled(), element), "")}</div>;
}

// The dialect-shadowed control. `createEffect` is in the Solid 2.0 primitive
// table, so `native_vocabulary_outranks_contract` never creates a contract
// binding for it and no bundled row can change what it reports.
export function Watcher() {
  createEffect(
    () => doubled(),
    value => {
      void value;
    }
  );
  return <div />;
}
