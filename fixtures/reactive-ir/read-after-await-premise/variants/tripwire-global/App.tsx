import { createMemo, createSignal, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// A write to the global Promise withdraws the Promise premise.
globalThis.Promise = Promise;

export function TripwireShape() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <div>{String(data())}</div>;
}
