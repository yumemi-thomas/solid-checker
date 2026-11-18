import { createSignal, createStore } from "solid-js";

// ADR 1067: the store-merge idiom. rc.21's createStore setter calls its updater with the store's own proxy, whose traps write
// a backing the store made: Object.assign onto it writes no standard root.
export function useForm(next: () => { name: string }) {
  let seed: object;
  seed = new Proxy({}, {
    get: (target, key, receiver) => typeof key === "symbol"
      ? { px: seed, _fam: null, u: null }
      : Reflect.get(target, key, receiver),
    set: (_target, key, value) => Reflect.set(Number, key, value)
  });
  const [values, setValues] = createStore(seed);
  const sync = () => setValues((draft) => {
    Object.assign(draft, next());
  });
  return [values, sync] as const;
}

// A standard root goes on as a value.
const box = (value: unknown, key: string) => {
  const held: Record<string, unknown> = {};
  held[key] = value;
  return held;
};

export function C1() {
  const [r1] = createSignal(3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
