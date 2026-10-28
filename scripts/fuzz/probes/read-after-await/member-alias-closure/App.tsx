// ADR 0487 (from the misuse matrix): a hook member read in a closure after the memo's await is untracked (UNTRACKED_READ_AFTER_AWAIT).
import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const [r1, setR1] = createSignal(3);
  const useThing = () => ({ useIt: r1 });
  const thing = useThing();
  const m1 = createMemo(async () => {
    await Promise.resolve();
    const f1 = () => {
      thing.useIt();
    };
    f1();
    return 1;
  });
  createEffect(() => {
    return m1();
  }, (v) => {});
  return (
    <div>
    </div>
  );
}
