import {
  createEffect,
  createMemo,
  createSignal,
  flush,
  onSettled,
} from "solid-js";

const [count, setCount] = createSignal(0);
createEffect(() => count(), value => setCount(value));
onSettled(() => setCount(1));
flush(() => setCount(2));
const flushWrite = () => setCount(3);
flush(flushWrite);

createMemo(() => {
  flush(() => setCount(4));
  flush(flushWrite);
});

const [owned, setOwned] = createSignal(0, { ownedWrite: true });
createMemo(() => setOwned(owned() + 1));

function localCreateSignal<T>(value: T) {
  return [() => value, (_next: T) => {}] as const;
}
const [local, setLocal] = localCreateSignal(0);
createMemo(() => setLocal(local() + 1));
