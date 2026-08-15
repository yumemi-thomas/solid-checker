import * as Solid from "solid-js";
import { action, createEffect, createMemo, createOptimistic, createOptimisticStore, createSignal, createSignal as makeSignal, createStore, createTrackedEffect, onSettled, untrack } from "solid-js";

const [shared, setShared] = createSignal(0);
const save = action(function* () {});

function runSave() {
  save();
}

function writeShared() {
  setShared(1);
}

function unusedWrite() {
  setShared(2);
}

createMemo(() => setShared(3));
createMemo(() => runSave());
setShared(4);

export function Counter() {
  const [count, setCount] = createSignal(0);
  const [aliased, setAliased] = makeSignal(0);
  const [namespaced, setNamespaced] = Solid.createSignal(0);
  const [state, setState] = createStore({ value: 0 });
  const [optimistic, setOptimistic] = createOptimistic(0);
  const [optimisticState, setOptimisticState] = createOptimisticStore({ value: 0 });
  const [internal, setInternal] = createSignal(0, { ownedWrite: true });
  setCount(1);
  save();
  runSave();
  setAliased(1);
  setNamespaced(1);
  setState(draft => { draft.value = 1; });
  setOptimistic(1);
  setOptimisticState(draft => { draft.value = 1; });
  setInternal(1);
  createMemo(() => setCount(2));
	createMemo(() => setAliased(2));
	Solid.createMemo(() => setNamespaced(2));
	writeShared();
  createEffect(
    () => setCount(3),
    () => { setCount(4); save(); },
  );
  const update = action(() => setCount(5));
  // untrack clears tracking but keeps the component body's owner, so the
  // write still throws REACTIVE_WRITE_IN_OWNED_SCOPE at runtime.
  untrack(() => setCount(6));
  onSettled(() => setCount(7));
  // Children-forbidden leaf scopes are legal write/action regions: the rc.0
  // guard exempts them, so no finding here.
  createTrackedEffect(() => { setCount(8); save(); });
  // untrack in a memo keeps the memo's owner: still a violation.
  createMemo(() => untrack(() => setCount(9)));
  // untrack inside a leaf scope inherits the leaf's write legality.
  createTrackedEffect(() => { untrack(() => setCount(10)); });
  // untrack in an event handler stays legal: no owner is live there.
  return <button onClick={() => { setCount(previous => previous + 1); save(); untrack(() => setCount(11)); }}>{count()}{aliased()}{namespaced()}{optimistic()}{optimisticState.value}</button>;
}
