import { createSignal, createEffect, onCleanup, runWithOwner, getOwner, createRoot } from "solid-js";

function owned(cb: () => void) {
  const owner = getOwner();
  return { onClick: () => runWithOwner(owner, cb) };
}

export function ViaRunWithOwner() {
  const [count] = createSignal(0);
  const h = owned(() => {
    createEffect(() => count(), () => undefined);
    onCleanup(() => undefined);
  });
  return <button {...h}>a</button>;
}

function rooted(cb: () => void) {
  return { onClick: () => createRoot(() => cb()) };
}

export function ViaRoot() {
  const [count] = createSignal(0);
  const h = rooted(() => {
    createEffect(() => count(), () => undefined);
    onCleanup(() => undefined);
  });
  return <button {...h}>b</button>;
}

function rootedDirect(cb: () => void) {
  return { onClick: () => createRoot(cb) };
}

export function ViaRootDirect() {
  const [count] = createSignal(0);
  const h = rootedDirect(() => {
    onCleanup(() => undefined);
  });
  return <button {...h}>c</button>;
}
