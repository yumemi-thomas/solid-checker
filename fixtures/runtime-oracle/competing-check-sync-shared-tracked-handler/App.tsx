import { createMemo, createSignal, createTrackedEffect } from "solid-js";

export function C1() {
  const g1 = () => {
    createMemo(async () => 1, { sync: true });
  };
  createTrackedEffect(() => {
    g1();
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        g1();
      }}>go</button>
    </div>
  );
}
