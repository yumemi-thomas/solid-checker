import { createEffect, createMemo, createRoot, createSignal, Loading, Loading as Await, onCleanup, onSettled } from "solid-js";

createEffect(() => 1, () => {});
onCleanup(() => {});
export const orphan = <Loading fallback={<div />}>content</Loading>;
export const orphanAlias = <Await fallback={<div />}>content</Await>;
onSettled(() => () => {});

createRoot(() => {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
  onSettled(() => () => {});
  return <Loading fallback={<div />}>owned</Loading>;
});

function rootHelper() {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
}
createRoot(() => rootHelper());

function directRootHelper() {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
}
createRoot(directRootHelper);

createSignal(createEffect(() => 1, () => {}));

createMemo(() => {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
});

function installOrphans() {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
  return <Loading fallback={<div />}>orphan helper</Loading>;
}
installOrphans();

function unusedOwnerOperations() {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
  return <Loading fallback={<div />}>unused</Loading>;
}

export function installMaybe() {
  createEffect(() => 1, () => {});
}

function ownedHelper() {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
  return <Loading fallback={<div />}>owned helper</Loading>;
}

function eventHelper() {
  createEffect(() => 1, () => {});
}

export function App() {
  ownedHelper();
  createEffect(() => 1, () => {});
  onCleanup(() => {});
  onSettled(() => {
    onSettled(() => () => {});
  });
  createEffect(() => 1, () => {
    createEffect(() => 1, () => {});
  });
  return <Loading fallback={<div />}><button onClick={() => createEffect(() => 1, () => {})}>content</button><button onClick={eventHelper}>named</button></Loading>;
}
