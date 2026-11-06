// P13: a source no binding names has no reference to start from, so the walk starts at its call.
import { createMemo, createSignal, createStore } from "solid-js";

export function Created() {
  // Open: a carrier, a thrown value, a whole tuple.
  void Object.values({ a: createMemo(() => 1) });
  void Object.values([createSignal(1)[0]]);
  void JSON.stringify(createStore({ a: 1 })[0]);
  try { throw createMemo(() => 1); } catch {}
  const pair = createSignal(1);
  try { throw pair[0]; } catch {}
  const useO = () => ({ a: createMemo(() => 1) });
  void Object.values(useO());

  // Followed: a discarded source, and one read where it was made.
  createSignal(1);
  return <p>{createMemo(() => 1)()}</p>;
}
