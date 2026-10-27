// Clean. Mirrors probus-hk src/lib/tanstack/virtual.ts:65-69,94-102: a store write wrapped in
// runWithOwner(null, ...), called from a render effect's apply and from a handler.
import { createRenderEffect, createSignal, createStore, reconcile, runWithOwner } from "solid-js";

export function Virtual() {
  const [items, setItems] = createStore({ list: [] as { index: number }[] });
  const [total, setTotal] = createSignal(0, { ownedWrite: true });
  let calls = 0;
  const publish = () =>
    runWithOwner(null, () => {
      setItems(reconcile({ list: [{ index: calls }] }, "index"));
      setTotal(++calls);
    });
  createRenderEffect(
    () => undefined,
    () => publish(),
  );
  return (
    <button type="button" onClick={() => publish()}>
      {items.list.length}:{total()}
    </button>
  );
}
