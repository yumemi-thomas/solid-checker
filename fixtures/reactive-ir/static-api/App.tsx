import { affects, createEffect, createMemo, createOptimistic, createOptimisticStore, createProjection, createSignal, createStore, refresh } from "solid-js";

createEffect(() => 1);
createEffect(() => 1, undefined);

createMemo(async () => 1, { sync: true });
createSignal(async () => 1, { sync: true });
createStore(async () => ({ value: 1 }), { value: 0 }, { sync: true });
createProjection(async () => ({ value: 1 }), { value: 0 }, { sync: true });
createOptimistic(async () => 1, { sync: true });
createOptimisticStore(async () => ({ value: 1 }), { value: 0 }, { sync: true });

createMemo(() => 1, { sync: true });
createEffect(() => 1, () => {});

const target = createMemo(() => 1);
const [signal] = createSignal(0);
const [store] = createStore({ value: 1 });
refresh(target);
refresh(() => target());
refresh({});
affects(signal, "value");
affects(store, "value", "extra");

export function App() {
  refresh(target);
  return <div />;
}
