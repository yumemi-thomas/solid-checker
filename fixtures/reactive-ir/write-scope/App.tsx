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
  untrack(() => setCount(6));
  onSettled(() => setCount(7));
  createTrackedEffect(() => { setCount(8); save(); });
  return <button onClick={() => { setCount(previous => previous + 1); save(); }}>{count()}{aliased()}{namespaced()}{optimistic()}{optimisticState.value}</button>;
}
