import { onCleanup, runWithOwner, getOwner, createRoot, untrack, onSettled } from "solid-js";
function handlers(cb: () => void) { return {onClick: cb}; }
export function App() {
  const o = getOwner();
  const h = handlers(() => { onCleanup(() => undefined); });
  runWithOwner(o, h.onClick);
  return <button {...h}>mix</button>;
}
function outer(cb: () => void) {
  return createRoot(() => ({onClick: cb}));
}
export function ClosureRoot() {
  const h = outer(() => {onCleanup(() => undefined);});
  return <button {...h}>unowned despite closure</button>;
}
