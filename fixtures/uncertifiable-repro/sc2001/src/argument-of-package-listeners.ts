// app-game card-stack createHorizontalRail.ts:94 (createPointerListeners from @solid-primitives/pointer): handlers in an object
// handed to a package function whose behaviour is not analyzed from source here.
import { createPointerListeners } from "@solid-primitives/pointer";
import { createSignal } from "solid-js";

export function createRail(target: HTMLElement) {
  const [dragging, setDragging] = createSignal(false);
  createPointerListeners({
    target,
    onDown() {
      setDragging(true);
    },
    onUp() {
      setDragging(false);
    },
  });
  return dragging;
}
