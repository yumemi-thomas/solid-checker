import { createSignal, onSettled } from "solid-js";

export function C1() {
  return (
    <div>
      <button data-act="b1" onClick={() => {
        onSettled.call(null, () => () => {});
      }}>go</button>
    </div>
  );
}
