import { createEffect, createMemo, createSignal } from "solid-js";
import { dynamic } from "@solidjs/web";

export function C1() {
  const m1 = createMemo(async () => {
    function* gen() {
      yield 1;
      [1].forEach(() => {
        dynamic(() => Promise.resolve("p" as const), { static: true });
      });
    }
    for (const _y of gen()) await Promise.resolve();
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
