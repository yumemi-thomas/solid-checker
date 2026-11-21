import { affects, createMemo } from "solid-js";
import { $TARGET } from "@solidjs/signals";

// The accessor's own prototype is swapped for an object whose `$TARGET` is a getter: the read `affects` makes first runs it.
const hooks = {
  get [$TARGET]() {
    return undefined;
  },
};

export function Swapped() {
  const m = createMemo(() => 1);
  Object.setPrototypeOf(m, hooks);
  return <button onClick={() => { affects(m); }}>go</button>;
}
