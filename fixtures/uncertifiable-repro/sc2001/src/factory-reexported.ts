// app-game solid-dnd createDragOverlay.ts:71 (re-exported by the package's index.ts): the factory is exported again by a barrel
// (`export { createDragOverlay } from ...`), a reference that is no call, so the walk cannot account for the factory's invocations.
// The barrel is factory-barrel.ts.
import { createSignal } from "solid-js";

export function createOverlay() {
  const [offset, setOffset] = createSignal(0);
  return {
    offset,
    reset: () => {
      setOffset(0);
    },
  };
}
