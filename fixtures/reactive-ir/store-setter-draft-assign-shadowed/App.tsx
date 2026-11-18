import { createSignal } from "solid-js";

// ADR 1067 negative: a local createStore of the same shape hands its updater whatever it likes (here the global Math).
function createStore<T extends object>(value: T): [T, (fn: (draft: T) => void) => void] {
  return [value, (fn) => fn(Math as unknown as T)];
}
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
  const [r1] = createSignal(3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
