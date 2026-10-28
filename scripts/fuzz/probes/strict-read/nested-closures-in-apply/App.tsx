// ADR 0489 (from the misuse matrix): a read in a component closure called only from a closure in an effect apply is untracked (STRICT_READ_UNTRACKED).
import { createEffect, createSignal } from "solid-js";

export function C1() {
  const [r1, setR1] = createSignal(3);
  const wrapUse = () => { r1(); };
  createEffect(() => {
    return 1;
  }, (v) => {
    const f1 = () => {
      wrapUse();
    };
    f1();
  });
  return (
    <div>
    </div>
  );
}
