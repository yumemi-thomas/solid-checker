import { createMemo, untrack } from "solid-js";

declare const cache: Map<string, number>;
declare function load(key: string): Promise<number>;
declare function promised(): Promise<number>;
declare const thenable: PromiseLike<number>;

// The compute may answer on its call: nothing proves a pending first read.
export function MaybeSync(props: { id: string }) {
  const cached = createMemo(() => cache.get(props.id) ?? load(props.id));
  const custom = createMemo(() => thenable);
  const one = cached();
  const two = untrack(() => custom());
  return <p>{[one, two].length}</p>;
}

// A native promise, or an `async` function, is pending on the call's stack.
export function AlwaysAFlight() {
  const direct = createMemo(() => promised());
  const literal = createMemo(async () => 1);
  const one = direct();
  const two = literal();
  return <p>{[one, two].length}</p>;
}
