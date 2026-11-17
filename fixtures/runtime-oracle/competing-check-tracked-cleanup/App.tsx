import { createSignal, createTrackedEffect, onCleanup } from "solid-js";

export function C1() {
  const g1 = () => {
    onCleanup(() => {});
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
