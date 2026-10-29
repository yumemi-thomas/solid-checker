// ADR 0511: the imported memo taken out of an inline object literal; the first render's user effect reads it pending.
import { createEffect, createSignal } from "solid-js";
import { a1 } from "./helpers";

export function C1() {
  const { useIt } = { useIt: a1 };
  createEffect(() => {
    return 1;
  }, (v) => {
    useIt();
  });
  return (
    <div>
    </div>
  );
}
