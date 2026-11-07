import { createEffect, createMemo, createSignal } from "solid-js";
import { dynamic } from "@solidjs/web";

export function C1() {
  const m1 = createMemo(async () => {
    async function* gen() {
      yield 1;
      [1].forEach(() => {
        dynamic(() => Promise.resolve("p" as const), { static: true });
      });
    }
    for await (const _y of gen()) void _y;
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
