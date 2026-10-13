import { createLoop } from "dep-loop";

const frame = () => {};

// `host-dead-return`'s proven `Loop`, in a project that also holds a wildcard
// member write (`Object.assign` with an open source). It cannot reach the
// fresh tuple `createLoop` returns, so `start()` is reported.
export function Loop() {
  const [running, start, stop] = createLoop(frame);
  start();
  return <button onClick={stop}>{String(running())}</button>;
}

export function assignAll<T extends object>(target: T, source: object): T {
  return Object.assign(target, source);
}
