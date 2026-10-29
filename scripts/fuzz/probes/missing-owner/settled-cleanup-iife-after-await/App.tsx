// ADR 0493 with ADR 0488 (from the misuse matrix): an onSettled returning a cleanup, in an IIFE after an await, has no owner (SETTLED_CLEANUP_UNOWNED).
import { createSignal, onSettled } from "solid-js";

export function C1() {
  const f1 = async () => {
    await Promise.resolve();
    (() => {
      onSettled(() => () => {});
    })();
  };
  f1();
  return (
    <div>
    </div>
  );
}
