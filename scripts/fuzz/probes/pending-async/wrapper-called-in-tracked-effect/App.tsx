// ADR 0490 (from the misuse matrix): a read in a wrapper called in createTrackedEffect is in a forbidden scope (PENDING_ASYNC_FORBIDDEN_SCOPE).
import { createMemo, createSignal, createTrackedEffect } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const wrapUse = () => { a1(); };
  createTrackedEffect(() => {
    wrapUse();
  });
  return (
    <div>
    </div>
  );
}
