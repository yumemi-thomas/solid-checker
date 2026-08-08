// A comparator the runtime reaches through a primitive's *options object*
// rather than through a positional callback slot. `createMemo(fn, value,
// { equals })` invokes `equals` on every recompute, so the comparator is live
// code -- but the dialect's callback table models positional arguments only and
// has nothing to say about `{ equals }`.
//
// Reachability must therefore keep following *every* argument of a matched
// primitive call. Narrowed to the arguments carrying a callback-execution fact,
// this comparator becomes unreachable: its read of `tolerance` and its write
// through `setTolerance` both disappear from the IR.
//
// The memo is module-scoped and not exported on purpose. An export declaration
// makes every function inside it a reachability root, and a call nested inside
// another primitive's callback argument inherits that argument's edge -- either
// shape would reach the comparator without the options-object edge and so would
// prove nothing.
import { createMemo, createSignal } from "solid-js";

const [tolerance, setTolerance] = createSignal(1);
const [input] = createSignal(0);

const rounded = createMemo(() => input(), undefined, {
  equals: (previous: number, next: number) => {
    setTolerance(next - previous);
    return next - previous < tolerance();
  },
});

export function Widget() {
  return rounded();
}
