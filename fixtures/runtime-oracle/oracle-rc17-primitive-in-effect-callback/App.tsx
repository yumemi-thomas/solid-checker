import { Errored, createDeferred, createEffect, createMemo, createRoot, createSignal, getOwner, runWithOwner } from "solid-js";

// rc.17 dev throws PRIMITIVE_IN_EFFECT_CALLBACK when a root, memo or deferred
// memo is created with no owner on the stack of a createEffect apply
// (`assertNotInEffectCallback`, `@solidjs/signals` `dist/dev-shared.js:3947-3957`).
// Positives are the calls SC4005 claims; the control creates the memo under
// `runWithOwner(owner, …)` with the component's owner, which the runtime allows
// and the checker leaves alone. Each case sits under an `Errored` boundary so
// the thrown error is routed there instead of halting reactivity for the
// cases after it.

function Guard(props: { children: any }) {
  return <Errored fallback={<span>failed</span>}>{props.children}</Errored>;
}

function RootCase() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    createRoot(() => {});
  });
  return <span>root</span>;
}
export function RootInApply() {
  return (
    <Guard>
      <RootCase />
    </Guard>
  );
}

function MemoCase() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    createMemo(() => value() + 1);
  });
  return <span>memo</span>;
}
export function MemoInApply() {
  return (
    <Guard>
      <MemoCase />
    </Guard>
  );
}

function DeferredCase() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    createDeferred(() => value() + 1);
  });
  return <span>deferred</span>;
}
export function DeferredInApply() {
  return (
    <Guard>
      <DeferredCase />
    </Guard>
  );
}

function makeMemo(source: () => number) {
  return createMemo(() => source() + 1);
}
function HelperCase() {
  const [value] = createSignal(1);
  createEffect(value, () => {
    makeMemo(value);
  });
  return <span>helper</span>;
}
export function MemoThroughHelper() {
  return (
    <Guard>
      <HelperCase />
    </Guard>
  );
}

function ControlCase() {
  const [value] = createSignal(1);
  const owner = getOwner();
  createEffect(value, () => {
    runWithOwner(owner, () => createMemo(() => value() + 1));
  });
  return <span>control</span>;
}
export function ControlRunWithOwner() {
  return (
    <Guard>
      <ControlCase />
    </Guard>
  );
}
