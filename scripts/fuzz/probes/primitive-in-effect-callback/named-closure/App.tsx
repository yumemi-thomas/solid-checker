// ADR 0495 (from the misuse matrix): a memo created in a closure an effect apply calls first, with no earlier call, throws PRIMITIVE_IN_EFFECT_CALLBACK.
import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  createEffect(() => {
    return 1;
  }, (v) => {
    const f1 = () => {
      createMemo(() => 1);
    };
    f1();
  });
  return (
    <div>
    </div>
  );
}
