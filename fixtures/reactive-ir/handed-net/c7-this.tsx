import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const [r1, setR1] = createSignal(3);
  const f1 = () => {
    {
      class HK {
        m?: () => void;
        constructor() { this.m = () => {
        r1();
      }; }
      }
      new HK().m?.();
    }
    return 1;
  };
  const m1 = createMemo(() => {
    return new Promise<number>((done) => setTimeout(() => done(1), 1)).then(f1);
  });
  createEffect(() => {
    return m1();
  }, (v) => {});
  return (
    <div>
    </div>
  );
}

export { C1 as App };
