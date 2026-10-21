import { createMemo, createSignal, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Object.defineProperty on Promise withdraws the Promise premise.
Object.defineProperty(Promise, "all", { value: Promise.all, configurable: true });

export function TripwireShape() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <div>{String(data())}</div>;
}
