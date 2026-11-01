import { createMemo, createSignal } from "solid-js";
const [count] = createSignal(0);
// ADR 0516: stepping a generator's iterator in place runs its body on the
// continuation, which no call edge follows: uncertifiable.
export const stepped = createMemo(async () => {
  await Promise.resolve();
  (function* () {
    count();
    yield 1;
  })().next();
  return 1;
});
// A discarded iterator runs none of the body: silent.
export const discarded = createMemo(async () => {
  await Promise.resolve();
  (function* () {
    count();
  })();
  return 1;
});
