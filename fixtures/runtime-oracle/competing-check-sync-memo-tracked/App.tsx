import { createMemo, createSignal, createTrackedEffect } from "solid-js";

export function C1() {
  createTrackedEffect(() => {
    createMemo(async () => 1, { sync: true });
  });
  return (
    <div>
    </div>
  );
}
