import { affects, createMemo } from "solid-js";
import { $TARGET } from "@solidjs/signals";

// A hook at the key `affects` reads first, on the prototype every accessor reads it through: it runs on the stack of each `affects(accessor)`
// call, with the accessor as its receiver. The row at the accessor stays.
Object.defineProperty(Function.prototype, $TARGET, {
  get() {
    return undefined;
  },
  configurable: true,
});

export function Hooked() {
  const m = createMemo(() => 1);
  return <button onClick={() => { affects(m); }}>go</button>;
}
