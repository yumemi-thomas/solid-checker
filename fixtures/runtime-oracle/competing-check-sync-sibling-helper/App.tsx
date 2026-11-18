import { createMemo, createSignal, createTrackedEffect } from "solid-js";

export function C1() {
  const f1 = async () => {};
  const g1 = () => {
    createMemo(async () => 1, { sync: true });
    f1();
  };
  createTrackedEffect(() => {
    g1();
  });
  return (
    <div>
    </div>
  );
}
