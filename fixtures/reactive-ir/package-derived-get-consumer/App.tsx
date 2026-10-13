import { createSignal } from "solid-js";
import { readResult, unknownResult, callTrackedResult, getTrackedResult, escapeResult } from "reactive-package";

declare const unknownValue: unknown;
declare const value: unknown;
declare const unknownObject: object;
declare const key: string;

export function PlainReads() {
  const [count] = createSignal(0);
  readResult(() => ({ value: count() }));
  readResult(() => { return { value: count() }; });
  readResult(() => ({ value: unknownValue }));
  readResult(() => ({ value }));
  readResult(() => ({ other: unknownValue }));
  readResult(() => ({ value: () => unknownValue }));
  readResult(() => ({ value: { get nested() { return unknownValue; } } }));
  readResult((() => ({ value: count() })) satisfies (() => object));
  return <p>{count()}</p>;
}

export function ProducerViolation() {
  const [count, setCount] = createSignal(0);
  readResult(() => { setCount(1); return { value: count() }; });
  return <p>{count()}</p>;
}

export function RefusedShapes() {
  readResult(() => ({ get value() { return unknownValue; } }));
  readResult(() => ({ set value(next: unknown) {} }));
  readResult(() => ({ value() { return unknownValue; } }));
  readResult(() => ({ ...unknownObject }));
  readResult(() => ({ [key]: unknownValue }));
  readResult(() => ({ __proto__: unknownObject, value: unknownValue }));
  readResult(() => [unknownValue]);
  readResult(() => new Proxy({}, {}));
  readResult(() => unknownObject);
  readResult(() => unknownValue as { value: number });
  readResult(() => ({}));
  readResult(async () => ({ value: unknownValue }));
  const producer = () => ({ value: unknownValue });
  readResult(producer);
  return <div />;
}

export function OpenOrFurtherUses() {
  unknownResult(() => ({ value: unknownValue }));
  callTrackedResult(() => ({ value: () => unknownValue }));
  getTrackedResult(() => ({ value: unknownValue }));
  escapeResult(() => ({ value: unknownValue }));
  return <div />;
}
