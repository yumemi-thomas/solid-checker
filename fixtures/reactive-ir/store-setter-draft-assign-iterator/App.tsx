import { createMemo, createStore } from "solid-js";

// A replaced iterator can put an arbitrary function in the setter slot.
Reflect.set(Array.prototype, Symbol.iterator, function* () {
  yield undefined;
  yield (fn: (draft: object) => void) => fn(Math);
});

// ADR 1067: the store-merge idiom. rc.21's createStore setter calls its updater with the store's own proxy, whose traps write
// a backing the store made: Object.assign onto it writes no standard root.
export function useForm(next: () => { name: string }) {
  const [values, setValues] = createStore({ name: "" });
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
  const r1 = createMemo(() => 3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
