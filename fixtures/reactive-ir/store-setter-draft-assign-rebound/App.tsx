import { createSignal, createStore } from "solid-js";

// ADR 1067 negative: a `let` setter the program rebinds may be any function by the time it is called.
export function useForm(next: () => { name: string }, other: (fn: (draft: { name: string }) => void) => void) {
  let [values, setValues] = createStore({ name: "" });
  setValues = other;
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
