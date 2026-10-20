import { createLoop } from "dep-loop";

const frame = () => {};

// `host-dead-return`'s proven `Loop`, in a project that also writes through
// `Array.prototype` reflectively. Destructuring runs
// `Array.prototype[Symbol.iterator]`, so the tuple call is uncertifiable.
export function Loop() {
  const [running, start, stop] = createLoop(frame);
  start();
  return <button onClick={stop}>{String(running())}</button>;
}

export function patchArrays(iterator: () => Iterator<unknown>) {
  Object.assign(Array.prototype, { [Symbol.iterator]: iterator });
}
