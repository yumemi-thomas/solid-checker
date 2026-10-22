import { createMemo, createSignal, untrack } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// A class extending Promise withdraws the Promise premise for the program.
class Tracked extends Promise<number> {}
void Tracked;

export function TripwireShape() {
  const [value] = createSignal(1);
  const data = createMemo(() => pause().then(() => value()));
  return <div>{String(data())}</div>;
}
