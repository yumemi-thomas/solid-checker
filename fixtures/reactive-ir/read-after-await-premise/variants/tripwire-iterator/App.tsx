import { createMemo, createSignal, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// A write to Array.prototype[Symbol.iterator] withdraws the array-iteration premise: the aggregate below is then an obligation.
Object.defineProperty(Array.prototype, Symbol.iterator, { value: Array.prototype[Symbol.iterator], configurable: true });

export function TripwireAggregate() {
  const [value] = createSignal(1);
  const data = createMemo(() => Promise.all([1]).then(() => value()));
  return <div>{String(data())}</div>;
}
