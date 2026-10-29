// ADR 0493 (from the misuse matrix): an effect in an IIFE after an await runs with no owner (NO_OWNER_EFFECT).
import { createEffect, createSignal } from "solid-js";

export function C1() {
  const f1 = async () => {
    await Promise.resolve();
    (() => {
      createEffect(() => 1, () => {});
    })();
  };
  f1();
  return (
    <div>
    </div>
  );
}
