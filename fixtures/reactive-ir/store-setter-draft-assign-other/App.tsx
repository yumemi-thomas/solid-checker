import { createSignal, createStore } from "solid-js";

// ADR 1067 negative: slot 0 of the tuple (the store) is no setter: a function held in it and called with a callback is not
// read as the setter.
export function useForm(next: () => { name: string }) {
  const [holder] = createStore({ run: (fn: (draft: object) => void) => fn(Math) });
  const run = holder.run;
  const sync = () => run((draft) => {
    Object.assign(draft, next());
  });
  return sync;
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
