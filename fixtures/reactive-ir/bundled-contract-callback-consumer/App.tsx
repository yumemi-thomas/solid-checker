import { createEffect, createMemo, createSignal, flatten } from "solid-js";
import { applyRef } from "@solidjs/web";

const [count] = createSignal(0);
const doubled = createMemo(() => count() * 2);
declare const element: Element;
const readDoubled = () => doubled();

// `flatten` is not in the Solid 2.0 dialect's primitive table, so nothing
// shadows the bundled contract here and its callback row is what answers.
// `callbacks[0]=inline` propagates the read inside the callback to this call
// site, which is a component body and tracks nothing.
export function Untracked() {
  const value = flatten(() => doubled());
  return <div>{value}</div>;
}

// The same export called from compiler-tracked JSX: the inline attribution
// lands the read in a tracked position, so nothing is reported for the call.
export function Tracked() {
  return <div>{flatten(() => doubled())}</div>;
}

// `applyRef` is not a dialect primitive. Its bundled @solidjs/web row must
// therefore propagate this callback read to the untracked component call.
export function WebUntracked() {
  applyRef(readDoubled, element);
  return <div />;
}

// The same contract row lands in compiler-tracked JSX and stays clean.
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
