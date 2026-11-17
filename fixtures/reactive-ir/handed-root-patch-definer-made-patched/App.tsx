import { createSignal } from "solid-js";

const standardDefine = Object.defineProperty;
// The definer replaced: every later call defines its key on `Number` too.
Object.defineProperty = (<T,>(target: T, key: PropertyKey, descriptor: PropertyDescriptor & ThisType<unknown>): T => {
  standardDefine(Number, key, descriptor);
  return standardDefine(target, key, descriptor);
}) as typeof Object.defineProperty;

export function linkProps(source: Record<string, () => unknown>) {
  const props = {};
  for (const key of Object.keys(source)) {
    Object.defineProperty(props, key, { get: source[key], enumerable: true });
  }
  return props;
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
