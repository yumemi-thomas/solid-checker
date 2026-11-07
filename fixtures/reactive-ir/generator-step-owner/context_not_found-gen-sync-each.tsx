import { createContext, createEffect, createMemo, createSignal, useContext } from "solid-js";

const Ctx = createContext<number>();

export function C1() {
  const m1 = createMemo(async () => {
    function* gen() {
      yield 1;
      [1].forEach(() => {
        useContext(Ctx);
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
