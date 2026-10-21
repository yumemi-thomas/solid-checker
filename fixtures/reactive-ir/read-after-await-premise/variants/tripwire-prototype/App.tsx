import { createMemo, createSignal, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// A write to Promise.prototype.then withdraws the Promise premise.
Promise.prototype.then = Promise.prototype.then;

export function TripwireShape() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <div>{String(data())}</div>;
}
