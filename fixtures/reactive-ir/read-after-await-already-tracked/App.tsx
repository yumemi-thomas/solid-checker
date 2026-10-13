import { createMemo, createSignal } from "solid-js";

declare function load(): Promise<number>;
declare const maybe: { use(value: number): void } | undefined;
declare const flag: boolean;

export function Reads() {
  const [count] = createSignal(1);
  const [other] = createSignal(2);

  // Negative: `count` is read before the first await on every run, so it is a
  // dependency; reading it again after the await loses nothing.
  createMemo(async () => {
    count();
    await load();
    return count();
  });

  // Negative: the earlier read is a plain statement-level initializer.
  createMemo(async () => {
    const before = count();
    await load();
    return before + count();
  });

  // Positive: the earlier read is conditional, so a run may track nothing.
  createMemo(async () => {
    if (flag) count();
    await load();
    return count();
  });

  // Positive: a conditional await comes first, so the earlier read may run
  // after a suspension.
  createMemo(async () => {
    if (flag) await load();
    count();
    await load();
    return count();
  });

  // Positive: a different accessor was read before the await.
  createMemo(async () => {
    other();
    await load();
    return count();
  });

  // Positive: the earlier read sits in an optional chain, evaluated only when
  // `maybe` is not nullish.
  createMemo(async () => {
    maybe?.use(count());
    await load();
    return count();
  });

  // Positive: the earlier read is in a nested function, not this one's flow.
  createMemo(async () => {
    const read = () => count();
    void read;
    await load();
    return count();
  });

  return <div />;
}
