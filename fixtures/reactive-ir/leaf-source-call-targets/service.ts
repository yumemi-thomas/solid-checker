import { createMemo, createSignal, onCleanup } from "solid-js";

export function makeService() {
  return {
    cleanLeaf() { void 0; },
    async idleLeaf() { await Promise.resolve(); },
    forbiddenLeaf() { onCleanup(() => {}); },
    nestedLeaf() { createMemo(() => 1); },
    conditionalLeaf(flag: boolean) { if (flag) onCleanup(() => {}); },
    defaultLeaf(value = onCleanup(() => {})) { void value; },
  };
}

export function makeReceiverService() {
  return { receiverLeaf() { return this; } };
}

export function makeArrowService() {
  return { arrowLeaf: () => {} };
}

export function makeMutableService() {
  return { mutableLeaf() {} };
}

export function makeEscapedService() {
  return { escapedLeaf() {} };
}

export function makeLeakyService() {
  return { untouchedLeaf() {}, leak: function () { return this; } };
}

export function makeSelection() {
  const [selected, setSelected] = createSignal(0);
  return { selected, select(next: number) { setSelected(next); } };
}
