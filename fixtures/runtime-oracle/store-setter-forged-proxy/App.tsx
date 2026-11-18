import { createSignal, createStore } from "solid-js";

// rc.21's wrap accepts this forged private-target protocol. A genuine setter
// can therefore hand its updater a user proxy that forwards to a standard root.
let seed: object;
seed = new Proxy({}, {
  get: (target, key, receiver) => typeof key === "symbol"
    ? { px: seed, _fam: null, u: null }
    : Reflect.get(target, key, receiver),
  set: (_target, key, value) => Reflect.set(Number, key, value)
});
const [, setValue] = createStore(seed);
setValue(draft => {
  Object.assign(draft, { isFinite: (fn: unknown) => {
    if (typeof fn === "function") fn();
    return false;
  } });
});

export function C1() {
  const [r1] = createSignal(3);
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{sf1()}</p>;
}
