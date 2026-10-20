import { createSignal, createRoot, runWithOwner, getOwner, onCleanup } from "solid-js";
function root(cb: () => void) { return createRoot(cb); }
function handlers(cb: () => void) { return {onClick: () => root(cb)}; }
export function App() {
  const h = handlers(() => { onCleanup(() => undefined); });
  return <button {...h}>root</button>;
}
function owner(cb: () => void) {
  const o = getOwner();
  const run = runWithOwner;
  return {onClick: () => run(o, cb)};
}
export function Named() {
  const h = owner(() => { onCleanup(() => undefined); });
  return <button {...h}>owner</button>;
}
