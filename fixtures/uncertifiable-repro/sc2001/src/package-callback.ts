// app-game card-stack createFreeDrag.ts:36-41,66-68: a function that also runs from another member is handed to onCleanup.
import { createSignal, onCleanup } from "solid-js";

export function createDrag(onFinish: () => void) {
  const [dragging, setDragging] = createSignal(false);
  function reset() {
    setDragging(false);
  }
  onCleanup(reset);
  return {
    dragging,
    finish() {
      onFinish();
      reset();
    },
  };
}
